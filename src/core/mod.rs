use glam::{Mat4, Quat, Vec3};
use log::info;
use rosc::{OscBundle, OscPacket, OscType};
use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    sync::{mpsc, Arc},
    thread,
    time::Duration,
};

use self::{
    bundle::AvatarBundle, ext_autopilot::autopilot_step, ext_gogo::ExtGogo,
    ext_oscjson::ExtOscJson, ext_storage::ExtStorage, ext_tracking::ExtTracking,
};

mod bundle;
mod ext_autopilot;
mod ext_gogo;
mod ext_oscjson;
mod ext_storage;
mod ext_tracking;
mod folders;

pub const PARAM_PREFIX: &str = "/avatar/parameters/";
const AVATAR_PREFIX: &str = "/avatar/change";
const TRACK_PREFIX: &str = "/tracking/vrsystem";
const INPUT_PREFIX: &str = "/input/";

pub type AvatarParameters = HashMap<Arc<str>, OscType>;

pub struct AvatarOsc {
    osc_port: u16,
    upstream: UdpSocket,
    ext_oscjson: ExtOscJson,
    ext_storage: ExtStorage,
    ext_gogo: ExtGogo,
    ext_tracking: ExtTracking,
}

pub struct Tracking {
    pub head: Mat4,
    pub left_hand: Mat4,
    pub right_hand: Mat4,
}

impl AvatarOsc {
    pub fn new(osc_port: u16, vrc_port: u16) -> AvatarOsc {
        let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);

        let upstream = UdpSocket::bind("0.0.0.0:0").expect("bind upstream socket");
        upstream
            .connect(SocketAddr::new(ip, vrc_port))
            .expect("upstream connect");

        let ext_storage = ExtStorage::new();
        let ext_gogo = ExtGogo::new();
        let ext_facetrack = ExtTracking::new();
        let ext_oscjson = ExtOscJson::new();

        AvatarOsc {
            osc_port,
            upstream,
            ext_oscjson,
            ext_storage,
            ext_gogo,
            ext_tracking: ext_facetrack,
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

        let (drive_sender, receiver) = mpsc::channel();

        thread::spawn(move || {
            let mut running = true;

            loop {
                receiver.try_iter().last().map(|x| {
                    info!("Self-drive: {}", x);
                    running = x;
                });
                if running {
                    let _ = lo.send(&[0u8; 1]);
                    thread::sleep(Duration::from_millis(11));
                } else {
                    thread::sleep(Duration::from_secs(1));
                }
            }
        });

        info!(
            "Listening for OSC messages on {}",
            listener.local_addr().unwrap()
        );

        let mut parameters: AvatarParameters = AvatarParameters::new();
        let mut tracking: Tracking = Tracking {
            head: Mat4::IDENTITY,
            left_hand: Mat4::IDENTITY,
            right_hand: Mat4::IDENTITY,
        };

        let mut buf = [0u8; rosc::decoder::MTU];
        loop {
            if let Ok((size, addr)) = listener.recv_from(&mut buf) {
                if addr == lo_addr {
                    self.process(&parameters, &drive_sender);
                    continue;
                }

                if let Ok((_, OscPacket::Message(packet))) = rosc::decoder::decode_udp(&buf[..size])
                {
                    if packet.addr.starts_with(PARAM_PREFIX) {
                        let name: Arc<str> = packet.addr[PARAM_PREFIX.len()..].into();
                        if &*name == "VSync" {
                            self.process(&parameters, &drive_sender);
                        } else if let Some(arg) = packet.args.into_iter().next() {
                            self.ext_storage.notify(&name, &arg);
                            self.ext_gogo.notify(&name, &arg);
                            parameters.insert(name, arg);
                        }
                    } else if packet.addr.starts_with(TRACK_PREFIX) {
                        if let [OscType::Float(x), OscType::Float(y), OscType::Float(z), OscType::Float(ex), OscType::Float(ey), OscType::Float(ez)] =
                            packet.args[..]
                        {
                            let transform = Mat4::from_rotation_translation(
                                Quat::from_euler(glam::EulerRot::ZXY, ex, ey, ez),
                                Vec3::new(x, y, z),
                            );

                            if packet.addr[TRACK_PREFIX.len()..].starts_with("head") {
                                tracking.head = transform;
                            } else if packet.addr[TRACK_PREFIX.len()..].starts_with("leftwrist") {
                                tracking.left_hand = transform;
                            } else if packet.addr[TRACK_PREFIX.len()..].starts_with("rightwrist") {
                                tracking.right_hand = transform;
                            }
                        }
                    } else if packet.addr.starts_with(AVATAR_PREFIX) {
                        if let [OscType::String(avatar)] = &packet.args[..] {
                            self.avatar(avatar, &drive_sender);
                        }
                    }
                }
            };
        }
    }

    fn avatar(&mut self, avatar: &str, drive_sender: &mpsc::Sender<bool>) {
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

        let _ = drive_sender.send(!osc_root_node.is_some_and(|n| n.has_vsync()));
    }

    fn process(&mut self, parameters: &AvatarParameters, drive_sender: &mpsc::Sender<bool>) {
        let mut bundle = OscBundle::new_bundle();

        if self.ext_oscjson.step() {
            self.avatar("<unknown>", drive_sender);
        }
        self.ext_storage.step(&mut bundle);
        self.ext_tracking.step(parameters, &mut bundle);
        self.ext_gogo.step(parameters, &mut bundle);
        autopilot_step(parameters, &self.ext_tracking, &mut bundle);

        if let Some(packet) = bundle.content.first() {
            match packet {
                OscPacket::Message(..) => {
                    rosc::encoder::encode(&packet)
                        .ok()
                        .and_then(|buf| self.send_upstream(&buf).ok());
                    bundle.content.remove(0);
                }
                _ => {}
            }
        }

        for bundle in bundle.content.chunks(30).map(|chunk| {
            let mut bundle = OscBundle::new_bundle();
            bundle.content.extend_from_slice(chunk);
            bundle
        }) {
            bundle
                .serialize()
                .and_then(|buf| self.send_upstream(&buf).ok());
        }
    }
}
