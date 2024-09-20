use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    sync::{
        mpsc::{sync_channel, Receiver, SyncSender},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

use colored::{Color, Colorize};
use once_cell::sync::Lazy;
use rosc::{OscPacket, OscType};

use crate::core::{
    ext_tracking::unified::UnifiedExpressions, AppState, INSTRUCTIONS_END, INSTRUCTIONS_START,
    TRACK_ON,
};

use super::{unified::UnifiedTrackingData, FaceReceiver};

static STA_BABL1: Lazy<Arc<str>> = Lazy::new(|| format!("{}", "BABBLE".color(Color::Green)).into());
static STA_BABL0: Lazy<Arc<str>> = Lazy::new(|| format!("{}", "BABBLE".color(Color::Red)).into());
static STA_ETVR1: Lazy<Arc<str>> = Lazy::new(|| format!("{}", "ETVR".color(Color::Green)).into());
static STA_ETVR0: Lazy<Arc<str>> = Lazy::new(|| format!("{}", "ETVR".color(Color::Red)).into());

pub(super) struct BabbleEtvrReceiver {
    listen_port: u16,
    sender: SyncSender<Box<BabbleEtvrEvent>>,
    receiver: Receiver<Box<BabbleEtvrEvent>>,
    last_received_babble: Instant,
    last_received_etvr: Instant,
}

impl BabbleEtvrReceiver {
    pub fn new(listen_port: u16) -> Self {
        let (sender, receiver) = sync_channel(128);
        Self {
            listen_port,
            sender,
            receiver,
            last_received_babble: Instant::now(),
            last_received_etvr: Instant::now(),
        }
    }
}

impl FaceReceiver for BabbleEtvrReceiver {
    fn start_loop(&mut self) {
        let sender = self.sender.clone();
        let listen_port = self.listen_port;

        let babble_recv_port = listen_port + 10;
        let babble_http_port = babble_recv_port + 1;
        let etvr_recv_port = babble_recv_port + 10;
        let etvr_http_port = etvr_recv_port + 1;

        log::info!("{}", *INSTRUCTIONS_START);
        log::info!("");
        log::info!("Selected ProjectBabble + EyeTrackVR to provide face data.");
        log::info!("(You don't have to have both!)");
        log::info!("");
        log::info!("For Babble:");
        log::info!(
            "• Set {} to {}",
            "Port".color(Color::BrightYellow),
            format!("{}", listen_port).color(Color::Cyan),
        );
        log::info!(
            "• Set {} to {}",
            "Receiver Port".color(Color::BrightYellow),
            format!("{}", babble_recv_port).color(Color::Cyan),
        );
        log::info!(
            "• Start: {}",
            format!(
                "./VrcAdvert babble {} {}",
                babble_http_port, babble_recv_port
            )
            .on_color(Color::White)
            .color(Color::Black)
        );
        log::info!("");
        log::info!("For ETVR:");
        log::info!(
            "• Set {} to {}",
            "OSC Port".color(Color::BrightYellow),
            format!("{}", listen_port).color(Color::Cyan),
        );
        log::info!(
            "• Set {} to {}",
            "OSC Receiver Port".color(Color::BrightYellow),
            format!("{}", etvr_recv_port).color(Color::Cyan),
        );
        log::info!(
            "• Start: {}",
            format!("./VrcAdvert etvr {} {}", etvr_http_port, etvr_recv_port)
                .on_color(Color::White)
                .color(Color::Black)
        );
        log::info!("");
        log::info!("Status bar tickers:");
        log::info!("• {} → mouth data is being received", *STA_BABL1);
        log::info!("• {} → eye data is being received", *STA_ETVR1);
        log::info!(
            "• {} → head & wrist data is being received (for AutoPilot)",
            *TRACK_ON
        );
        log::info!("");
        log::info!("To use AutoPilot:");
        log::info!("• Run OscAvMgr's VrcAdvert with --tracking");
        log::info!("• In VRChat Settings/Tracking & IK: enable sending of Head and Wrist data");
        log::info!("");
        log::info!("{}", *INSTRUCTIONS_END);

        thread::spawn(move || babble_loop(listen_port, sender));
    }

    fn receive(&mut self, data: &mut UnifiedTrackingData, state: &mut AppState) {
        for event in self.receiver.try_iter() {
            data.shapes[event.expression as usize] = event.value;

            if (event.expression as usize) < (UnifiedExpressions::BrowPinchRight as usize) {
                self.last_received_etvr = Instant::now();
            } else {
                self.last_received_babble = Instant::now();
            }
        }

        if self.last_received_babble.elapsed() < Duration::from_secs(1) {
            state.status.add_item(STA_BABL1.clone());
        } else {
            state.status.add_item(STA_BABL0.clone());
        }

        if self.last_received_etvr.elapsed() < Duration::from_secs(1) {
            state.status.add_item(STA_ETVR1.clone());
        } else {
            state.status.add_item(STA_ETVR0.clone());
        }
    }
}

fn babble_loop(listen_port: u16, mut sender: SyncSender<Box<BabbleEtvrEvent>>) {
    loop {
        if let Some(()) = receive_babble_osc(listen_port, &mut sender) {
            break;
        } else {
            thread::sleep(Duration::from_millis(5000));
        }
    }
}

fn receive_babble_osc(
    listen_port: u16,
    sender: &mut SyncSender<Box<BabbleEtvrEvent>>,
) -> Option<()> {
    let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let listener = UdpSocket::bind(SocketAddr::new(ip, listen_port)).expect("bind listener socket");
    let mut buf = [0u8; rosc::decoder::MTU];
    loop {
        if let Ok((size, _addr)) = listener.recv_from(&mut buf) {
            if let Ok((_, OscPacket::Message(packet))) = rosc::decoder::decode_udp(&buf[..size]) {
                if packet.args.is_empty() {
                    log::warn!("Babble/ETVR OSC Message has no args?");
                } else if let OscType::Float(x) = packet.args[0] {
                    if let Some(expv) = ADDR_TO_UNIFIED.get(packet.addr.as_str()).cloned() {
                        for exp in expv.iter() {
                            let event = Box::new(BabbleEtvrEvent::new(*exp, x));
                            if let Err(e) = sender.try_send(event) {
                                log::warn!("Failed to send Babble/ETVR message: {}", e);
                            }
                        }
                    }
                } else {
                    log::warn!("Babble/ETVR OSC: Unsupported arg {:?}", packet.args[0]);
                }
            }
        }
    }
}

struct BabbleEtvrEvent {
    pub expression: UnifiedExpressions,
    pub value: f32,
}

impl BabbleEtvrEvent {
    pub fn new(expression: UnifiedExpressions, value: f32) -> Self {
        Self { expression, value }
    }
}

#[rustfmt::skip]
static ADDR_TO_UNIFIED: Lazy<HashMap<&'static str, Vec<UnifiedExpressions>>> = Lazy::new(|| {
    [
        // ProjectBabble
        ("/cheekPuffLeft", vec![UnifiedExpressions::CheekPuffLeft]),
        ("/cheekPuffRight", vec![UnifiedExpressions::CheekPuffRight]),
        ("/cheekSuckLeft", vec![UnifiedExpressions::CheekSuckLeft]),
        ("/cheekSuckRight", vec![UnifiedExpressions::CheekSuckRight]),
        ("/jawOpen", vec![UnifiedExpressions::JawOpen]),
        ("/jawForward", vec![UnifiedExpressions::JawForward]),
        ("/jawLeft", vec![UnifiedExpressions::JawLeft]),
        ("/jawRight", vec![UnifiedExpressions::JawRight]),
        ("/noseSneerLeft", vec![UnifiedExpressions::NoseSneerLeft]),
        ("/noseSneerRight", vec![UnifiedExpressions::NoseSneerRight]),
        ("/mouthFunnel", vec![UnifiedExpressions::LipFunnelUpperRight, UnifiedExpressions::LipFunnelUpperLeft, UnifiedExpressions::LipFunnelLowerRight, UnifiedExpressions::LipFunnelLowerLeft]),
        ("/mouthPucker", vec![UnifiedExpressions::LipPuckerUpperRight, UnifiedExpressions::LipPuckerUpperLeft, UnifiedExpressions::LipPuckerLowerRight, UnifiedExpressions::LipPuckerLowerLeft]),
        ("/mouthLeft", vec![UnifiedExpressions::MouthPressLeft]),
        ("/mouthRight", vec![UnifiedExpressions::MouthPressRight]),
        ("/mouthRollUpper", vec![UnifiedExpressions::LipSuckUpperLeft, UnifiedExpressions::LipSuckUpperRight]),
        ("/mouthRollLower", vec![UnifiedExpressions::LipSuckLowerLeft, UnifiedExpressions::LipSuckLowerRight]),
        ("/mouthShrugUpper", vec![UnifiedExpressions::MouthRaiserUpper]),
        ("/mouthShrugLower", vec![UnifiedExpressions::MouthRaiserLower]),
        ("/mouthClose", vec![UnifiedExpressions::MouthClosed]),
        ("/mouthSmileLeft", vec![UnifiedExpressions::MouthCornerPullLeft, UnifiedExpressions::MouthCornerSlantLeft]),
        ("/mouthSmileRight", vec![UnifiedExpressions::MouthCornerPullRight, UnifiedExpressions::MouthCornerSlantRight]),
        ("/mouthFrownLeft", vec![UnifiedExpressions::MouthFrownLeft, UnifiedExpressions::MouthStretchLeft]),
        ("/mouthFrownRight", vec![UnifiedExpressions::MouthFrownRight, UnifiedExpressions::MouthStretchRight]),
        ("/mouthDimpleLeft", vec![UnifiedExpressions::MouthDimpleLeft]),
        ("/mouthDimpleRight", vec![UnifiedExpressions::MouthDimpleRight]),
        ("/mouthUpperUpLeft", vec![UnifiedExpressions::MouthUpperUpLeft]),
        ("/mouthUpperUpRight", vec![UnifiedExpressions::MouthUpperUpRight]),
        ("/mouthLowerDownLeft", vec![UnifiedExpressions::MouthLowerDownLeft]),
        ("/mouthLowerDownRight", vec![UnifiedExpressions::MouthLowerDownRight]),
        ("/mouthStretchLeft", vec![UnifiedExpressions::MouthStretchLeft]),
        ("/mouthStretchRight", vec![UnifiedExpressions::MouthStretchRight]),
        ("/tongueOut", vec![UnifiedExpressions::TongueOut]),
        ("/tongueUp", vec![UnifiedExpressions::TongueUp]),
        ("/tongueDown", vec![UnifiedExpressions::TongueDown]),
        ("/tongueLeft", vec![UnifiedExpressions::TongueLeft]),
        ("/tongueRight", vec![UnifiedExpressions::TongueRight]),
        ("/tongueRoll", vec![UnifiedExpressions::TongueRoll]),
        ("/tongueBendDown", vec![UnifiedExpressions::TongueBendDown]),
        ("/tongueCurlUp", vec![UnifiedExpressions::TongueCurlUp]),
        ("/tongueSquish", vec![UnifiedExpressions::TongueSquish]),
        ("/tongueFlat", vec![UnifiedExpressions::TongueFlat]),
        ("/tongueTwistLeft", vec![UnifiedExpressions::TongueTwistLeft]),
        ("/tongueTwistRight", vec![UnifiedExpressions::TongueTwistRight]),
        ("/mouthPressLeft", vec![UnifiedExpressions::MouthPressLeft]),
        ("/mouthPressRight", vec![UnifiedExpressions::MouthPressRight]),

        // ETVR
        ("/avatar/parameters/LeftEyeX", vec![UnifiedExpressions::EyeLeftX]),
        ("/avatar/parameters/RightEyeX", vec![UnifiedExpressions::EyeRightX]),
        ("/avatar/parameters/EyesY", vec![UnifiedExpressions::EyeY]),
        ("/avatar/parameters/LeftEyeLid", vec![UnifiedExpressions::EyeClosedLeft]),
        ("/avatar/parameters/RightEyeLid", vec![UnifiedExpressions::EyeClosedRight]),

        ("/avatar/parameters/v2/EyeLeftX", vec![UnifiedExpressions::EyeLeftX]),
        ("/avatar/parameters/v2/EyeRightX", vec![UnifiedExpressions::EyeRightX]),
        ("/avatar/parameters/v2/EyeLeftY", vec![UnifiedExpressions::EyeY]),
        ("/avatar/parameters/v2/EyeLidLeft", vec![UnifiedExpressions::EyeClosedLeft]),
        ("/avatar/parameters/v2/EyeLidRight", vec![UnifiedExpressions::EyeClosedRight]),
    ]
    .into_iter()
    .collect()
});
