use std::{
    sync::{
        mpsc::{Receiver, SyncSender},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

use alvr_common::{
    glam::{EulerRot, Quat},
    DeviceMotion, Pose, HAND_LEFT_PATH, HAND_RIGHT_PATH, HEAD_PATH,
};
use anyhow::bail;
use colored::{Color, Colorize};
use glam::Vec3;
use once_cell::sync::Lazy;
use strum::EnumCount;
use sysinfo::ProcessesToUpdate;
use websocket_lite::{ClientBuilder, Message, Opcode};

use crate::core::{
    ext_tracking::face2_fb::face2_fb_to_unified, AppState, INSTRUCTIONS_END, INSTRUCTIONS_START,
    TRACK_ON,
};

use super::{
    unified::{UnifiedExpressions, UnifiedTrackingData, NUM_SHAPES},
    FaceReceiver,
};

static STA_ON: Lazy<Arc<str>> = Lazy::new(|| format!("{}", "ALVR".color(Color::Green)).into());
static STA_OFF: Lazy<Arc<str>> = Lazy::new(|| format!("{}", "ALVR".color(Color::Red)).into());

#[derive(Default)]
struct AlvrTrackingData {
    eye: [Option<Vec3>; 2],
    head: Option<Pose>,
    hands: [Option<Pose>; 2],
    shapes: Option<[f32; NUM_SHAPES]>,
}

impl AlvrTrackingData {}

pub(super) struct AlvrReceiver {
    sender: SyncSender<Box<AlvrTrackingData>>,
    receiver: Receiver<Box<AlvrTrackingData>>,
    last_received: Instant,
}

impl AlvrReceiver {
    pub fn new() -> Self {
        let (sender, receiver) = std::sync::mpsc::sync_channel(8);
        Self {
            sender,
            receiver,
            last_received: Instant::now(),
        }
    }
}

impl FaceReceiver for AlvrReceiver {
    fn start_loop(&mut self) {
        log::info!("{}", *INSTRUCTIONS_START);
        log::info!("");
        log::info!("Selected ALVR to provide face data.");
        log::info!(
            "ALVR version {} is supported. Other versions may not work!",
            (*alvr_common::ALVR_VERSION)
                .to_string()
                .color(Color::BrightYellow)
        );
        log::info!("");
        log::info!("Required ALVR settings:");
        log::info!(
            "• Extra tab: enable {}",
            "Log Tracking".color(Color::BrightYellow)
        );
        log::info!(
            "• Presets tab: set {} based on your HMD:",
            "Eye and Face tracking".color(Color::BrightYellow)
        );
        log::info!("  ‣ Quest Pro → VRCFaceTracking");
        log::info!("  ‣ Other Eye-tracked HMD → VRChat Eye OSC");
        log::info!("");
        log::info!("Status bar tickers:");
        log::info!("• {} → face and/or eye data is being received", *STA_ON);
        log::info!(
            "• {} → head & wrist data is being received (for AutoPilot)",
            *TRACK_ON
        );
        log::info!("");
        log::info!("{}", *INSTRUCTIONS_END);
        let sender = self.sender.clone();
        thread::spawn(move || {
            alvr_receive(sender);
        });
    }

    fn receive(&mut self, data: &mut UnifiedTrackingData, state: &mut AppState) {
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

            if let Some(head) = new_data.head {
                state.tracking.head =
                    glam::Affine3A::from_rotation_translation(head.orientation, head.position);
                state.tracking.last_received = Instant::now();
            }

            if let Some(left_hand) = new_data.hands[0] {
                state.tracking.left_hand = glam::Affine3A::from_rotation_translation(
                    left_hand.orientation,
                    left_hand.position,
                );
            }

            if let Some(right_hand) = new_data.hands[1] {
                state.tracking.right_hand = glam::Affine3A::from_rotation_translation(
                    right_hand.orientation,
                    right_hand.position,
                );
            }
        }

        if self.last_received.elapsed() < Duration::from_secs(1) {
            state.status.add_item(STA_ON.clone());
        } else {
            state.status.add_item(STA_OFF.clone());
        }
    }
}

#[inline(always)]
// Takes ALVR-specific Quat
fn quat_to_euler(q: Quat) -> Vec3 {
    let (y, x, z) = q.to_euler(EulerRot::YXZ);
    Vec3 { x, y, z }
}

const VR_PROCESSES: [&str; 6] = [
    "vrdashboard",
    "vrcompositor",
    "vrserver",
    "vrmonitor",
    "vrwebhelper",
    "vrstartup",
];

fn alvr_receive(mut sender: SyncSender<Box<AlvrTrackingData>>) {
    let mut system = sysinfo::System::new();
    loop {
        match receive_until_err(&mut sender, &mut system) {
            Ok(_) => {
                thread::sleep(Duration::from_millis(20000));
            }
            Err(e) => {
                log::warn!("WebSocket error: {}", e);
                thread::sleep(Duration::from_millis(5000));
            }
        }
    }
}

fn receive_until_err(
    sender: &mut SyncSender<Box<AlvrTrackingData>>,
    system: &mut sysinfo::System,
) -> anyhow::Result<()> {
    const WS_URL: &str = "ws://127.0.0.1:8082/api/events";
    let mut builder = ClientBuilder::new(WS_URL)?;
    builder.add_header("X-ALVR".to_string(), "true".to_string());
    let Ok(mut ws) = builder.connect_insecure() else {}
        return Ok(()); // long retry
    };

    while let Ok(Some(message)) = ws.receive() {
        match message.opcode() {
            Opcode::Close => {
                let _ = ws.send(Message::close(None));
                bail!("connection closed");
            }
            Opcode::Ping => {
                let _ = ws.send(Message::pong(message.into_data()));
            }
            Opcode::Text => {
                let Some(text) = message.as_text() else {
                    log::warn!("websocket: no content");
                    continue;
                };
                match serde_json::from_str::<alvr_events::Event>(text) {
                    Ok(msg) => {
                        match msg.event_type {
                            alvr_events::EventType::ServerRequestsSelfRestart => {
                                log::warn!("ALVR: Server requested self restart");
                                // kill steamvr processes and let auto-restart handle it
                                system.refresh_processes(ProcessesToUpdate::All, true);
                                system.processes().values().for_each(|p| {
                                    for vrp in VR_PROCESSES.iter() {
                                        if p.name().to_string_lossy().contains(vrp) {
                                            p.kill();
                                        }
                                    }
                                });
                                return Ok(()); // long retry
                            }
                            alvr_events::EventType::Tracking(tracking) => {
                                let mut data = AlvrTrackingData::default();
                                load_gazes(&tracking.eye_gazes, &mut data);
                                load_devices(
                                    &tracking.device_motions,
                                    &tracking.hand_skeletons,
                                    &mut data,
                                );
                                if let Some(face_fb) = tracking.fb_face_expression {
                                    data.shapes = face2_fb_to_unified(&face_fb);
                                }
                                if let Err(e) = sender.try_send(Box::new(data)) {
                                    log::debug!("Failed to send tracking message: {}", e);
                                }
                            }
                            _ => {}
                        }
                    }
                    Err(e) => {
                        log::warn!("Failed to parse tracking message: {}", e);
                    }
                }
            }
            _ => {}
        }
    }

    bail!("connection lost");
}

fn load_devices(
    device_motions: &[(String, DeviceMotion)],
    hand_skeletons: &[Option<[Pose; 26]>; 2],
    data: &mut AlvrTrackingData,
) {
    if let Some(left_hand) = hand_skeletons[0] {
        data.hands[0] = Some(left_hand[0]);
    }
    if let Some(right_hand) = hand_skeletons[0] {
        data.hands[1] = Some(right_hand[0]);
    }

    let mut remain = 3;
    for (name, motion) in device_motions {
        if remain == 0 {
            break;
        }
        match name.as_str() {
            HEAD_PATH => {
                data.head = Some(motion.pose);
                remain -= 1;
            }
            HAND_LEFT_PATH => {
                if data.hands[0].is_none() {
                    data.hands[0] = Some(motion.pose);
                }
                remain -= 1;
            }
            HAND_RIGHT_PATH => {
                if data.hands[1].is_none() {
                    data.hands[1] = Some(motion.pose);
                }
                remain -= 1;
            }
            _ => {}
        }
    }
}

fn load_gazes(gazes: &[Option<Pose>; 2], data: &mut AlvrTrackingData) {
    if let Some(gaze) = gazes[0] {
        data.eye[0] = Some(quat_to_euler(gaze.orientation));
    }
    if let Some(gaze) = gazes[1] {
        data.eye[1] = Some(quat_to_euler(gaze.orientation));
    }
}
