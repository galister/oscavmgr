use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    sync::{
        mpsc::{Receiver, SyncSender},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

use colored::{Color, Colorize};
use glam::{EulerRot, Quat, Vec3};
use once_cell::sync::Lazy;
use strum::EnumCount;

use crate::core::AppState;

use super::{
    face2_fb::face2_fb_to_unified,
    unified::{UnifiedExpressions, UnifiedTrackingData, NUM_SHAPES},
};

static STA_ON: Lazy<Arc<str>> = Lazy::new(|| format!("{}", "WIVRN".color(Color::Green)).into());
static STA_OFF: Lazy<Arc<str>> = Lazy::new(|| format!("{}", "WIVRN".color(Color::Red)).into());

struct WivrnPayload {
    eyes: [f32; 8],
    face_fb: [f32; 70], //Face2Fb::Max
}

#[derive(Default)]
struct WivrnTrackingData {
    eye: [Option<Vec3>; 2],
    shapes: Option<[f32; NUM_SHAPES]>,
}

pub(super) struct WivrnReceiver {
    sender: SyncSender<Box<WivrnTrackingData>>,
    receiver: Receiver<Box<WivrnTrackingData>>,
    last_received: Instant,
}

impl WivrnReceiver {
    pub fn new() -> Self {
        let (sender, receiver) = std::sync::mpsc::sync_channel(8);
        Self {
            sender,
            receiver,
            last_received: Instant::now(),
        }
    }

    pub fn start_loop(&self) {
        let sender = self.sender.clone();
        thread::spawn(move || {
            wivrn_receive(sender);
        });
    }

    pub fn receive(&mut self, data: &mut UnifiedTrackingData, state: &mut AppState) {
        for new_data in self.receiver.try_iter() {
            if let Some(new_left) = new_data.eye[0] {
                data.eyes[0] = Some(new_left);
            }
            if let Some(new_right) = new_data.eye[1] {
                data.eyes[1] = Some(new_right);
            }
            if let Some(new_shapes) = new_data.shapes {
                data.shapes[..=UnifiedExpressions::COUNT]
                    .copy_from_slice(&new_shapes[..=UnifiedExpressions::COUNT]);
                self.last_received = Instant::now();
            }
        }

        if self.last_received.elapsed() < Duration::from_secs(1) {
            state.status.add_item(STA_ON.clone());
        } else {
            state.status.add_item(STA_OFF.clone());
        }
    }
}

fn wivrn_receive(sender: SyncSender<Box<WivrnTrackingData>>) {
    let ip = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
    let listener = UdpSocket::bind(SocketAddr::new(ip, 9009)).expect("bind listener socket");
    let mut buf = [0u8; 1000];

    loop {
        let Ok((size, _)) = listener.recv_from(&mut buf) else {
            thread::sleep(Duration::from_millis(1000));
            continue;
        };

        if size != 312 {
            log::warn!("Invalid WIVRN message size: {}", size);
            continue;
        }

        unsafe {
            let payload = buf.as_ptr() as *const WivrnPayload;
            let shapes = face2_fb_to_unified(&(*payload).face_fb);
            let data = WivrnTrackingData {
                eye: [
                    Some(quat_to_euler(Quat::from_slice(&(*payload).eyes[0..4]))),
                    Some(quat_to_euler(Quat::from_slice(&(*payload).eyes[4..8]))),
                ],
                shapes,
            };

            if let Err(e) = sender.try_send(Box::new(data)) {
                log::debug!("Failed to send tracking message: {}", e);
            }
        }
    }
}

#[inline(always)]
fn quat_to_euler(q: Quat) -> Vec3 {
    let (x, y, z) = q.to_euler(EulerRot::ZXY);
    Vec3 { x, y, z }
}
