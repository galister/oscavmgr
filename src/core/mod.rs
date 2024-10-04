use colored::{Color, Colorize};
use glam::{Affine3A, Quat, Vec3};
use indicatif::MultiProgress;
use log::info;
use once_cell::sync::Lazy;
use rosc::{OscBundle, OscPacket, OscType};
use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

use crate::Args;

use self::bundle::AvatarBundle;

mod bundle;
mod ext_autopilot;
mod ext_gogo;
mod ext_oscjson;
mod ext_storage;
mod ext_tracking;
mod folders;
mod watchdog;

pub mod status;

pub const PARAM_PREFIX: &str = "/avatar/parameters/";
const AVATAR_PREFIX: &str = "/avatar/change";
const TRACK_PREFIX: &str = "/tracking/trackers/";
const INPUT_PREFIX: &str = "/input/";

pub type AvatarParameters = HashMap<Arc<str>, OscType>;

pub struct AppState {
    pub tracking: OscTrack,
    pub params: AvatarParameters,
    pub status: status::StatusBar,
    pub self_drive: Arc<AtomicBool>,
    pub delta_t: f32,
}

pub struct AvatarOsc {
    osc_port: u16,
    upstream: UdpSocket,
    ext_autopilot: ext_autopilot::ExtAutoPilot,
    ext_oscjson: ext_oscjson::ExtOscJson,
    ext_storage: ext_storage::ExtStorage,
    ext_gogo: ext_gogo::ExtGogo,
    ext_tracking: ext_tracking::ExtTracking,
    multi: MultiProgress,
}

pub struct OscTrack {
    pub head: Affine3A,
    pub left_hand: Affine3A,
    pub right_hand: Affine3A,
    pub last_received: Instant,
}

impl AvatarOsc {
    pub fn new(args: Args, multi: MultiProgress) -> AvatarOsc {
        let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);

        let upstream = UdpSocket::bind("0.0.0.0:0").expect("bind upstream socket");
        upstream
            .connect(SocketAddr::new(ip, args.vrc_port))
            .expect("upstream connect");

        let ext_autopilot = ext_autopilot::ExtAutoPilot::new();
        let ext_storage = ext_storage::ExtStorage::new();
        let ext_gogo = ext_gogo::ExtGogo::new();
        let ext_tracking = ext_tracking::ExtTracking::new(args.face);
        let ext_oscjson = ext_oscjson::ExtOscJson::new();

        AvatarOsc {
            osc_port: args.osc_port,
            upstream,
            ext_autopilot,
            ext_oscjson,
            ext_storage,
            ext_gogo,
            ext_tracking,
            multi,
        }
    }

    pub fn send_upstream(&self, buf: &[u8]) -> std::io::Result<usize> {
        self.upstream.send(buf)
    }

    pub fn handle_messages(&mut self) {
        let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
        let listener =
            UdpSocket::bind(SocketAddr::new(ip, self.osc_port)).expect("bind listener socket");

        let lo = UdpSocket::bind("0.0.0.0:0").expect("bind self socket");
        lo.connect(SocketAddr::new(ip, self.osc_port)).unwrap();
        let lo_addr = lo.local_addr().unwrap();

        let mut state = AppState {
            status: status::StatusBar::new(&self.multi),
            params: AvatarParameters::new(),
            tracking: OscTrack {
                head: Affine3A::IDENTITY,
                left_hand: Affine3A::IDENTITY,
                right_hand: Affine3A::IDENTITY,
                last_received: Instant::now(),
            },
            self_drive: Arc::new(AtomicBool::new(true)),
            delta_t: 0.011f32,
        };

        let watchdog = watchdog::Watchdog::new(state.self_drive.clone());
        watchdog.run();
        thread::spawn({
            let drive = state.self_drive.clone();
            move || loop {
                if drive.load(Ordering::Relaxed) {
                    let _ = lo.send(&[0u8; 1]);
                    thread::sleep(Duration::from_millis(11));
                } else {
                    thread::sleep(Duration::from_millis(200));
                }
            }
        });

        info!(
            "Listening for OSC messages on {}",
            listener.local_addr().unwrap()
        );

        let mut last_frame = Instant::now();
        let mut buf = [0u8; rosc::decoder::MTU];
        loop {
            if let Ok((size, addr)) = listener.recv_from(&mut buf) {
                if addr == lo_addr {
                    self.process(&mut state);
                    watchdog.update();
                    state.delta_t = last_frame.elapsed().as_secs_f32();
                    last_frame = Instant::now();
                    continue;
                }

                if let Ok((_, OscPacket::Message(packet))) = rosc::decoder::decode_udp(&buf[..size])
                {
                    state.status.trip_recv_counter();
                    if packet.addr.starts_with(PARAM_PREFIX) {
                        let name: Arc<str> = packet.addr[PARAM_PREFIX.len()..].into();
                        if &*name == "VSync" {
                            state.self_drive.store(false, Ordering::Relaxed);
                            self.process(&mut state);
                            state.delta_t = last_frame.elapsed().as_secs_f32();
                            last_frame = Instant::now();
                            watchdog.update();
                        } else if let Some(arg) = packet.args.into_iter().next() {
                            self.ext_storage.notify(&name, &arg);
                            self.ext_gogo.notify(&name, &arg);
                            state.params.insert(name, arg);
                        }
                    } else if packet.addr.starts_with(TRACK_PREFIX) {
                        if let [OscType::Float(x), OscType::Float(y), OscType::Float(z), OscType::Float(ex), OscType::Float(ey), OscType::Float(ez)] =
                            packet.args[..]
                        {
                            let transform = Affine3A::from_rotation_translation(
                                Quat::from_euler(glam::EulerRot::ZXY, ex, ey, ez),
                                Vec3::new(x, y, z),
                            );

                            if packet.addr[TRACK_PREFIX.len()..].starts_with("head") {
                                state.tracking.last_received = Instant::now();
                                state.tracking.head = transform;
                            } else if packet.addr[TRACK_PREFIX.len()..].starts_with("leftwrist") {
                                state.tracking.left_hand = transform;
                            } else if packet.addr[TRACK_PREFIX.len()..].starts_with("rightwrist") {
                                state.tracking.right_hand = transform;
                            }
                        }
                    } else if packet.addr.starts_with(AVATAR_PREFIX) {
                        if let [OscType::String(avatar)] = &packet.args[..] {
                            self.avatar(avatar, &mut state);
                        }
                    } else {
                        log::info!("Received data: {:?}", packet);
                    }
                }
            };
        }
    }

    fn avatar(&mut self, avatar: &str, state: &mut AppState) {
        info!("Avatar changed: {}", avatar);
        let osc_root_node = self.ext_oscjson.avatar();
        if let Some(osc_root_node) = osc_root_node.as_ref() {
            self.ext_tracking.osc_json(osc_root_node);
        }

        let mut bundle = OscBundle::new_bundle();
        self.ext_gogo.avatar(&mut bundle);
        bundle
            .serialize()
            .and_then(|buf| self.send_upstream(&buf).ok());

        state.self_drive.store(
            !osc_root_node.is_some_and(|n| {
                let has_vsync = n.has_vsync();

                let vsync_name = "VSync".color(Color::BrightYellow);

                if !has_vsync {
                    log::warn!(
                        "This avatar does not have a {} parameter, falling back to {} mode.",
                        vsync_name,
                        *DRIVE_ON,
                    );
                    log::warn!(
                        "The {} parameter helps OscAvMgr keep in sync with your avatar's animator.",
                        vsync_name
                    );
                    log::warn!(
                        "Consider implementing a {} parameter using either:",
                        vsync_name
                    );
                    log::warn!("- a bool param that flips every animator frame.");
                    log::warn!("- a float param that randomizes each animator frame.");
                }
                has_vsync
            }),
            Ordering::Relaxed,
        );
    }

    fn process(&mut self, state: &mut AppState) {
        let mut bundle = OscBundle::new_bundle();

        state
            .status
            .add_item(match state.self_drive.load(Ordering::Relaxed) {
                true => DRIVE_ON.clone(),
                false => DRIVE_OFF.clone(),
            });

        state.status.add_item(
            match state.tracking.last_received.elapsed() < Duration::from_secs(1) {
                true => TRACK_ON.clone(),
                false => TRACK_OFF.clone(),
            },
        );

        if self.ext_oscjson.step() {
            self.avatar("default", state);
        }
        self.ext_storage.step(&mut bundle);
        self.ext_tracking.step(state, &mut bundle);
        self.ext_gogo.step(&state.params, &mut bundle);
        self.ext_autopilot
            .step(state, &self.ext_tracking, &mut bundle);

        if let Some(packet) = bundle.content.first() {
            if let OscPacket::Message(..) = packet {
                rosc::encoder::encode(packet)
                    .ok()
                    .and_then(|buf| self.send_upstream(&buf).ok());
                bundle.content.remove(0);
            }
        }

        state.status.trip_fps_counter();
        state.status.set_sent_count(bundle.content.len() as _);
        state.status.recv_summary();

        for bundle in bundle.content.chunks(30).map(|chunk| {
            let mut bundle = OscBundle::new_bundle();
            bundle.content.extend_from_slice(chunk);
            bundle
        }) {
            bundle
                .serialize()
                .and_then(|buf| self.send_upstream(&buf).ok());
        }

        state.status.display();
    }
}

static DRIVE_ON: Lazy<Arc<str>> = Lazy::new(|| format!("{}", "DRIVE".color(Color::Blue)).into());
static DRIVE_OFF: Lazy<Arc<str>> = Lazy::new(|| format!("{}", "VSYNC".color(Color::Green)).into());

pub static TRACK_ON: Lazy<Arc<str>> =
    Lazy::new(|| format!("{}", "TRACK".color(Color::Green)).into());
pub static TRACK_OFF: Lazy<Arc<str>> =
    Lazy::new(|| format!("{}", "TRACK".color(Color::Red)).into());

pub static INSTRUCTIONS_START: Lazy<Arc<str>> = Lazy::new(|| {
    format!(
        "{}{}{}",
        "==".color(Color::BrightBlue),
        "Instructions".color(Color::BrightYellow),
        "================================".color(Color::BrightBlue)
    )
    .into()
});

pub static INSTRUCTIONS_END: Lazy<Arc<str>> = Lazy::new(|| {
    format!(
        "{}{}{}",
        "================================".color(Color::BrightBlue),
        "Instructions".color(Color::BrightYellow),
        "==".color(Color::BrightBlue)
    )
    .into()
});
