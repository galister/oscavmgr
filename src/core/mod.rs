use glam::{Mat4, Quat, Vec3};
use log::{info, debug};
use rosc::{OscBundle, OscPacket, OscType};
use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    sync::Arc,
};

use self::{ext_autopilot::autopilot_step, ext_storage::ExtStorage, ext_gogo::ExtGogo, bundle::AvatarBundle};

mod bundle;
mod ext_autopilot;
mod ext_gogo;
mod ext_storage;

const AVATAR_PREFIX: &str = "/avatar/change";
const TRACK_PREFIX: &str = "/tracking/vrsystem";
const PARAM_PREFIX: &str = "/avatar/parameters/";
const INPUT_PREFIX: &str = "/input/";

pub type AvatarParameters = HashMap<Arc<str>, OscType>;

pub struct AvatarOsc {
    upstream: UdpSocket,
    listener: UdpSocket,
    ext_storage: ExtStorage,
    ext_gogo: ExtGogo,
}

pub struct Tracking {
    pub head: Mat4,
    pub left_hand: Mat4,
    pub right_hand: Mat4,
}

impl AvatarOsc {
    pub fn new(osc_port: u16, vrc_port: u16) -> AvatarOsc {
        let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);

        let listener =
            UdpSocket::bind(SocketAddr::new(ip, osc_port)).expect("bind listener socket");
        listener.connect("0.0.0.0:0").expect("listener connect");

        let upstream = UdpSocket::bind("0.0.0.0:0").expect("bind upstream socket");
        upstream
            .connect(SocketAddr::new(ip, vrc_port))
            .expect("upstream connect");

        let ext_storage = ExtStorage::new();
        let ext_gogo = ExtGogo::new();

        AvatarOsc {
            upstream,
            listener,
            ext_storage,
            ext_gogo,
        }
    }

    pub fn send_upstream(&self, buf: &[u8]) -> std::io::Result<usize> {
        self.upstream.send(buf)
    }

    pub fn handle_messages(&mut self) {
        info!(
            "Listening for OSC messages on {}",
            self.listener.local_addr().unwrap()
        );

        let mut parameters: AvatarParameters = AvatarParameters::new();
        let mut tracking: Tracking = Tracking {
            head: Mat4::IDENTITY,
            left_hand: Mat4::IDENTITY,
            right_hand: Mat4::IDENTITY,
        };

        let mut buf = [0u8; rosc::decoder::MTU];
        loop {
            if let Ok(size) = self.listener.recv(&mut buf) {
                if let Ok((_, OscPacket::Message(packet))) = rosc::decoder::decode_udp(&buf[..size])
                {
                    if packet.addr.starts_with(PARAM_PREFIX) {
                        let name: Arc<str> = packet.addr[PARAM_PREFIX.len()..].into();
                        if &*name == "VSync" {
                            self.process(&parameters, &tracking);
                        } else if let Some(arg) = packet.args.into_iter().next() {
                            self.ext_storage.notify(&name, &arg);
                            self.ext_gogo.notify(&name, &arg);
                            debug!("{} => {:?}", name, arg);
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
                            info!("Avatar changed: {:?}", avatar);
                            let mut bundle = OscBundle::new_bundle();
                            self.ext_gogo.avatar(&mut bundle);
                            bundle.serialize().and_then(|buf| self.send_upstream(&buf).ok());
                        }
                    }
                }
            };
        }
    }

    fn process(&mut self, parameters: &AvatarParameters, tracking: &Tracking) {
        let mut bundle = OscBundle::new_bundle();

        self.ext_storage.step(&mut bundle);
        autopilot_step(parameters, tracking, &mut bundle);

        bundle.serialize().and_then(|buf| self.send_upstream(&buf).ok());
    }
}
