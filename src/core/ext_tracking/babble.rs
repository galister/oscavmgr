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

use crate::core::{ext_tracking::unified::UnifiedExpressions, status::StatusBar};

use super::unified::UnifiedTrackingData;

static STA_ON: Lazy<Arc<str>> = Lazy::new(|| format!("{}", "BABBLE".color(Color::Green)).into());
static STA_OFF: Lazy<Arc<str>> = Lazy::new(|| format!("{}", "BABBLE".color(Color::Red)).into());

pub(super) struct BabbleReceiver {
    sender: SyncSender<Box<BabbleEvent>>,
    receiver: Receiver<Box<BabbleEvent>>,
    last_received: Instant,
}

impl BabbleReceiver {
    pub fn new() -> Self {
        let (sender, receiver) = sync_channel(128);
        Self {
            sender,
            receiver,
            last_received: Instant::now(),
        }
    }

    pub fn start_loop(&self) {
        let sender = self.sender.clone();
        thread::spawn(move || babble_loop(sender));
    }

    pub fn receive(&mut self, data: &mut UnifiedTrackingData, status: &mut StatusBar) {
        for event in self.receiver.try_iter() {
            data.shapes[event.expression as usize] = event.value;
            self.last_received = Instant::now();
        }

        if self.last_received.elapsed() < Duration::from_secs(1) {
            status.add_item(STA_ON.clone());
        } else {
            status.add_item(STA_OFF.clone());
        }
    }
}

fn babble_loop(mut sender: SyncSender<Box<BabbleEvent>>) {
    loop {
        if let Some(()) = receive_babble_osc(&mut sender) {
            break;
        } else {
            thread::sleep(Duration::from_millis(5000));
        }
    }
}

fn receive_babble_osc(sender: &mut SyncSender<Box<BabbleEvent>>) -> Option<()> {
    let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let listener = UdpSocket::bind(SocketAddr::new(ip, 8888)).expect("bind listener socket"); // yaay, more magic numbers! (ProjectBabble default OSC port)
    let mut buf = [0u8; rosc::decoder::MTU];
    loop {
        if let Ok((size, _addr)) = listener.recv_from(&mut buf) {
            if let Ok((_, OscPacket::Message(packet))) = rosc::decoder::decode_udp(&buf[..size]) {
                if !packet.args.is_empty() {
                    log::warn!("Babble OSC Message has no args?");
                } else if let OscType::Float(x) = packet.args[0] {
                    if let Some(index) = ADDR_TO_UNIFIED.get(packet.addr.as_str()).cloned() {
                        // I have no idea if this is a good way to do it or not, probably not
                        let mut event = Box::new(BabbleEvent::new());
                        event.expression = index;
                        event.value = x;
                        if let Err(e) = sender.try_send(event) {
                            log::warn!("Failed to send babble message: {}", e);
                        }
                    }
                } else {
                    log::warn!("Babble OSC: Unsupported arg {:?}", packet.args[0]);
                }
            }
        }
    }
}

struct BabbleEvent {
    pub expression: UnifiedExpressions,
    pub value: f32,
}

impl BabbleEvent {
    pub fn new() -> Self {
        Self {
            expression: UnifiedExpressions::EyeSquintLeft,
            value: 0.0,
        }
    }
}

static ADDR_TO_UNIFIED: Lazy<HashMap<&'static str, UnifiedExpressions>> = Lazy::new(|| {
    [
        ("/cheekPuffLeft", UnifiedExpressions::CheekPuffLeft),
        ("/cheekPuffRight", UnifiedExpressions::CheekPuffRight),
        ("/cheekSuckLeft", UnifiedExpressions::CheekSuckLeft),
        ("/cheekSuckRight", UnifiedExpressions::CheekSuckRight),
        ("/jawOpen", UnifiedExpressions::JawOpen),
        ("/jawForward", UnifiedExpressions::JawForward),
        ("/jawLeft", UnifiedExpressions::JawLeft),
        ("/jawRight", UnifiedExpressions::JawRight),
        ("/noseSneerLeft", UnifiedExpressions::NoseSneerLeft),
        ("/noseSneerRight", UnifiedExpressions::NoseSneerRight),
        ("/mouthFunnel", UnifiedExpressions::LipFunnelLowerLeft),
        ("/mouthPucker", UnifiedExpressions::LipPuckerLowerLeft),
        ("/mouthLeft", UnifiedExpressions::MouthPressLeft),
        ("/mouthRight", UnifiedExpressions::MouthPressRight),
        ("/mouthRollUpper", UnifiedExpressions::LipSuckUpperLeft),
        ("/mouthRollLower", UnifiedExpressions::LipSuckLowerLeft),
        ("/mouthShrugUpper", UnifiedExpressions::MouthRaiserUpper),
        ("/mouthShrugLower", UnifiedExpressions::MouthRaiserLower),
        ("/mouthClose", UnifiedExpressions::MouthClosed),
        ("/mouthSmileLeft", UnifiedExpressions::MouthCornerPullLeft),
        ("/mouthSmileRight", UnifiedExpressions::MouthCornerPullRight),
        ("/mouthFrownLeft", UnifiedExpressions::MouthFrownLeft),
        ("/mouthFrownRight", UnifiedExpressions::MouthFrownRight),
        ("/mouthDimpleLeft", UnifiedExpressions::MouthDimpleLeft),
        ("/mouthDimpleRight", UnifiedExpressions::MouthDimpleRight),
        ("/mouthUpperUpLeft", UnifiedExpressions::MouthUpperUpLeft),
        ("/mouthUpperUpRight", UnifiedExpressions::MouthUpperUpRight),
        (
            "/mouthLowerDownLeft",
            UnifiedExpressions::MouthLowerDownLeft,
        ),
        (
            "/mouthLowerDownRight",
            UnifiedExpressions::MouthLowerDownRight,
        ),
        ("/mouthStretchLeft", UnifiedExpressions::MouthStretchLeft),
        ("/mouthStretchRight", UnifiedExpressions::MouthStretchRight),
        ("/tongueOut", UnifiedExpressions::TongueOut),
        ("/tongueUp", UnifiedExpressions::TongueUp),
        ("/tongueDown", UnifiedExpressions::TongueDown),
        ("/tongueLeft", UnifiedExpressions::TongueLeft),
        ("/tongueRight", UnifiedExpressions::TongueRight),
        ("/tongueRoll", UnifiedExpressions::TongueRoll),
        ("/tongueBendDown", UnifiedExpressions::TongueBendDown),
        ("/tongueCurlUp", UnifiedExpressions::TongueCurlUp),
        ("/tongueSquish", UnifiedExpressions::TongueSquish),
        ("/tongueFlat", UnifiedExpressions::TongueFlat),
        ("/tongueTwistLeft", UnifiedExpressions::TongueTwistLeft),
        ("/tongueTwistRight", UnifiedExpressions::TongueTwistRight),
        ("/mouthPressLeft", UnifiedExpressions::MouthPressLeft),
        ("/mouthPressRight", UnifiedExpressions::MouthPressRight),
    ]
    .into_iter()
    .collect()
});
