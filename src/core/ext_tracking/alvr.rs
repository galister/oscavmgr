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
use websocket_lite::{ClientBuilder, Message, Opcode};

use crate::core::AppState;

use super::unified::{UnifiedExpressions, UnifiedTrackingData, NUM_SHAPES};

static STA_ON: Lazy<Arc<str>> = Lazy::new(|| format!("{}", "ALVR".color(Color::Green)).into());
static STA_OFF: Lazy<Arc<str>> = Lazy::new(|| format!("{}", "ALVR".color(Color::Red)).into());

#[derive(Default)]
struct AlvrTrackingData {
    eye: [Option<Vec3>; 2],
    head: Option<Pose>,
    hands: [Option<Pose>; 2],
    shapes: Option<[f32; NUM_SHAPES]>,
}

impl AlvrTrackingData {
    #[inline(always)]
    pub fn setu(&mut self, exp: UnifiedExpressions, value: f32) {
        unsafe {
            self.shapes.as_mut().unwrap_unchecked()[exp as usize] = value;
        }
    }
}

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

    pub fn start_loop(&self) {
        let sender = self.sender.clone();
        thread::spawn(move || {
            alvr_receive(sender);
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

            if let Some(head) = new_data.head {
                let rot: glam::Quat = unsafe { std::mem::transmute(head.orientation) };
                let pos: glam::Vec3 = unsafe { std::mem::transmute(head.position) };
                state.tracking.head = glam::Affine3A::from_rotation_translation(rot, pos);
                state.tracking.last_received = Instant::now();
            }

            if let Some(left_hand) = new_data.hands[0] {
                let rot: glam::Quat = unsafe { std::mem::transmute(left_hand.orientation) };
                let pos: glam::Vec3 = unsafe { std::mem::transmute(left_hand.position) };
                state.tracking.left_hand = glam::Affine3A::from_rotation_translation(rot, pos);
            }

            if let Some(right_hand) = new_data.hands[1] {
                let rot: glam::Quat = unsafe { std::mem::transmute(right_hand.orientation) };
                let pos: glam::Vec3 = unsafe { std::mem::transmute(right_hand.position) };
                state.tracking.right_hand = glam::Affine3A::from_rotation_translation(rot, pos);
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
fn quat_to_euler(q: Quat) -> Vec3 {
    let (x, y, z) = q.to_euler(EulerRot::ZXY);
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
    let Ok(mut ws) = ClientBuilder::new(WS_URL)?.connect_insecure() else {
        bail!("failed to connect");
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
                                log::warn!("ALVR: Server requested data restart");
                                // kill steamvr processes and let auto-restart handle it
                                system.refresh_processes();
                                system.processes().values().for_each(|p| {
                                    for vrp in VR_PROCESSES.iter() {
                                        if p.name().contains(vrp) {
                                            p.kill();
                                        }
                                    }
                                });
                                return Ok(());
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
                                    load_face(&face_fb, &mut data);
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
    hand_skeletons: &[Option<[Pose; 31]>; 2],
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

fn load_face(face_fb: &[f32], data: &mut AlvrTrackingData) {
    if face_fb.len() < FaceFb::Max as usize {
        log::warn!(
            "Face tracking data is too short: {} < {}",
            face_fb.len(),
            FaceFb::Max as usize
        );
        return;
    }

    // this must be initialized because setu is unchecked
    data.shapes = Some([0.0; NUM_SHAPES]);

    let getf = |index| face_fb[index as usize];
    let getf2 = |index| face_fb[index as usize];

    data.setu(UnifiedExpressions::EyeClosedLeft, getf(FaceFb::EyesClosedL));
    data.setu(
        UnifiedExpressions::EyeClosedRight,
        getf(FaceFb::EyesClosedR),
    );

    data.setu(
        UnifiedExpressions::EyeSquintRight,
        getf(FaceFb::LidTightenerR) - getf(FaceFb::EyesClosedR),
    );
    data.setu(
        UnifiedExpressions::EyeSquintLeft,
        getf(FaceFb::LidTightenerL) - getf(FaceFb::EyesClosedL),
    );
    data.setu(
        UnifiedExpressions::EyeWideRight,
        getf(FaceFb::UpperLidRaiserR),
    );
    data.setu(
        UnifiedExpressions::EyeWideLeft,
        getf(FaceFb::UpperLidRaiserL),
    );

    data.setu(
        UnifiedExpressions::BrowPinchRight,
        getf(FaceFb::BrowLowererR),
    );
    data.setu(
        UnifiedExpressions::BrowPinchLeft,
        getf(FaceFb::BrowLowererL),
    );
    data.setu(
        UnifiedExpressions::BrowLowererRight,
        getf(FaceFb::BrowLowererR),
    );
    data.setu(
        UnifiedExpressions::BrowLowererLeft,
        getf(FaceFb::BrowLowererL),
    );
    data.setu(
        UnifiedExpressions::BrowInnerUpRight,
        getf(FaceFb::InnerBrowRaiserR),
    );
    data.setu(
        UnifiedExpressions::BrowInnerUpLeft,
        getf(FaceFb::InnerBrowRaiserL),
    );
    data.setu(
        UnifiedExpressions::BrowOuterUpRight,
        getf(FaceFb::OuterBrowRaiserR),
    );
    data.setu(
        UnifiedExpressions::BrowOuterUpLeft,
        getf(FaceFb::OuterBrowRaiserL),
    );

    data.setu(
        UnifiedExpressions::CheekSquintRight,
        getf(FaceFb::CheekRaiserR),
    );
    data.setu(
        UnifiedExpressions::CheekSquintLeft,
        getf(FaceFb::CheekRaiserL),
    );
    data.setu(UnifiedExpressions::CheekPuffRight, getf(FaceFb::CheekPuffR));
    data.setu(UnifiedExpressions::CheekPuffLeft, getf(FaceFb::CheekPuffL));
    data.setu(UnifiedExpressions::CheekSuckRight, getf(FaceFb::CheekSuckR));
    data.setu(UnifiedExpressions::CheekSuckLeft, getf(FaceFb::CheekSuckL));

    data.setu(UnifiedExpressions::JawOpen, getf(FaceFb::JawDrop));
    data.setu(UnifiedExpressions::JawRight, getf(FaceFb::JawSidewaysRight));
    data.setu(UnifiedExpressions::JawLeft, getf(FaceFb::JawSidewaysLeft));
    data.setu(UnifiedExpressions::JawForward, getf(FaceFb::JawThrust));
    data.setu(UnifiedExpressions::MouthClosed, getf(FaceFb::LipsToward));

    data.setu(
        UnifiedExpressions::LipSuckUpperRight,
        (1.0 - getf(FaceFb::UpperLipRaiserR).powf(0.1666)).min(getf(FaceFb::LipSuckRT)),
    );
    data.setu(
        UnifiedExpressions::LipSuckUpperLeft,
        (1.0 - getf(FaceFb::UpperLipRaiserL).powf(0.1666)).min(getf(FaceFb::LipSuckLT)),
    );

    data.setu(
        UnifiedExpressions::LipSuckLowerRight,
        getf(FaceFb::LipSuckRB),
    );
    data.setu(
        UnifiedExpressions::LipSuckLowerLeft,
        getf(FaceFb::LipSuckLB),
    );
    data.setu(
        UnifiedExpressions::LipFunnelUpperRight,
        getf(FaceFb::LipFunnelerRT),
    );
    data.setu(
        UnifiedExpressions::LipFunnelUpperLeft,
        getf(FaceFb::LipFunnelerLT),
    );
    data.setu(
        UnifiedExpressions::LipFunnelLowerRight,
        getf(FaceFb::LipFunnelerRB),
    );
    data.setu(
        UnifiedExpressions::LipFunnelLowerLeft,
        getf(FaceFb::LipFunnelerLB),
    );
    data.setu(
        UnifiedExpressions::LipPuckerUpperRight,
        getf(FaceFb::LipPuckerR),
    );
    data.setu(
        UnifiedExpressions::LipPuckerUpperLeft,
        getf(FaceFb::LipPuckerL),
    );
    data.setu(
        UnifiedExpressions::LipPuckerLowerRight,
        getf(FaceFb::LipPuckerR),
    );
    data.setu(
        UnifiedExpressions::LipPuckerLowerLeft,
        getf(FaceFb::LipPuckerL),
    );

    data.setu(
        UnifiedExpressions::NoseSneerRight,
        getf(FaceFb::NoseWrinklerR),
    );
    data.setu(
        UnifiedExpressions::NoseSneerLeft,
        getf(FaceFb::NoseWrinklerL),
    );

    data.setu(
        UnifiedExpressions::MouthLowerDownRight,
        getf(FaceFb::LowerLipDepressorR),
    );
    data.setu(
        UnifiedExpressions::MouthLowerDownLeft,
        getf(FaceFb::LowerLipDepressorL),
    );

    let mouth_upper_up_right = getf(FaceFb::UpperLipRaiserR);
    let mouth_upper_up_left = getf(FaceFb::UpperLipRaiserL);

    data.setu(UnifiedExpressions::MouthUpperUpRight, mouth_upper_up_right);
    data.setu(UnifiedExpressions::MouthUpperUpLeft, mouth_upper_up_left);
    data.setu(
        UnifiedExpressions::MouthUpperDeepenRight,
        mouth_upper_up_right,
    );
    data.setu(
        UnifiedExpressions::MouthUpperDeepenLeft,
        mouth_upper_up_left,
    );

    data.setu(
        UnifiedExpressions::MouthUpperRight,
        getf(FaceFb::MouthRight),
    );
    data.setu(UnifiedExpressions::MouthUpperLeft, getf(FaceFb::MouthLeft));
    data.setu(
        UnifiedExpressions::MouthLowerRight,
        getf(FaceFb::MouthRight),
    );
    data.setu(UnifiedExpressions::MouthLowerLeft, getf(FaceFb::MouthLeft));

    data.setu(
        UnifiedExpressions::MouthCornerPullRight,
        getf(FaceFb::LipCornerPullerR),
    );
    data.setu(
        UnifiedExpressions::MouthCornerPullLeft,
        getf(FaceFb::LipCornerPullerL),
    );
    data.setu(
        UnifiedExpressions::MouthCornerSlantRight,
        getf(FaceFb::LipCornerPullerR),
    );
    data.setu(
        UnifiedExpressions::MouthCornerSlantLeft,
        getf(FaceFb::LipCornerPullerL),
    );

    data.setu(
        UnifiedExpressions::MouthFrownRight,
        getf(FaceFb::LipCornerDepressorR),
    );
    data.setu(
        UnifiedExpressions::MouthFrownLeft,
        getf(FaceFb::LipCornerDepressorL),
    );
    data.setu(
        UnifiedExpressions::MouthStretchRight,
        getf(FaceFb::LipStretcherR),
    );
    data.setu(
        UnifiedExpressions::MouthStretchLeft,
        getf(FaceFb::LipStretcherL),
    );

    data.setu(
        UnifiedExpressions::MouthDimpleLeft,
        (getf(FaceFb::DimplerL) * 2.0).min(1.0),
    );
    data.setu(
        UnifiedExpressions::MouthDimpleRight,
        (getf(FaceFb::DimplerR) * 2.0).min(1.0),
    );

    data.setu(
        UnifiedExpressions::MouthRaiserUpper,
        getf(FaceFb::ChinRaiserT),
    );
    data.setu(
        UnifiedExpressions::MouthRaiserLower,
        getf(FaceFb::ChinRaiserB),
    );
    data.setu(
        UnifiedExpressions::MouthPressRight,
        getf(FaceFb::LipPressorR),
    );
    data.setu(
        UnifiedExpressions::MouthPressLeft,
        getf(FaceFb::LipPressorL),
    );
    data.setu(
        UnifiedExpressions::MouthTightenerRight,
        getf(FaceFb::LipTightenerR),
    );
    data.setu(
        UnifiedExpressions::MouthTightenerLeft,
        getf(FaceFb::LipTightenerL),
    );

    if face_fb.len() >= Face2Fb::Max as usize {
        data.setu(UnifiedExpressions::TongueOut, getf2(Face2Fb::TongueOut));
        data.setu(
            UnifiedExpressions::TongueCurlUp,
            getf2(Face2Fb::TongueTipAlveolar),
        );
    }
}

#[allow(non_snake_case, unused)]
#[repr(usize)]
pub enum FaceFb {
    BrowLowererL,
    BrowLowererR,
    CheekPuffL,
    CheekPuffR,
    CheekRaiserL,
    CheekRaiserR,
    CheekSuckL,
    CheekSuckR,
    ChinRaiserB,
    ChinRaiserT,
    DimplerL,
    DimplerR,
    EyesClosedL,
    EyesClosedR,
    EyesLookDownL,
    EyesLookDownR,
    EyesLookLeftL,
    EyesLookLeftR,
    EyesLookRightL,
    EyesLookRightR,
    EyesLookUpL,
    EyesLookUpR,
    InnerBrowRaiserL,
    InnerBrowRaiserR,
    JawDrop,
    JawSidewaysLeft,
    JawSidewaysRight,
    JawThrust,
    LidTightenerL,
    LidTightenerR,
    LipCornerDepressorL,
    LipCornerDepressorR,
    LipCornerPullerL,
    LipCornerPullerR,
    LipFunnelerLB,
    LipFunnelerLT,
    LipFunnelerRB,
    LipFunnelerRT,
    LipPressorL,
    LipPressorR,
    LipPuckerL,
    LipPuckerR,
    LipStretcherL,
    LipStretcherR,
    LipSuckLB,
    LipSuckLT,
    LipSuckRB,
    LipSuckRT,
    LipTightenerL,
    LipTightenerR,
    LipsToward,
    LowerLipDepressorL,
    LowerLipDepressorR,
    MouthLeft,
    MouthRight,
    NoseWrinklerL,
    NoseWrinklerR,
    OuterBrowRaiserL,
    OuterBrowRaiserR,
    UpperLidRaiserL,
    UpperLidRaiserR,
    UpperLipRaiserL,
    UpperLipRaiserR,
    Max,
}

#[allow(non_snake_case, unused)]
#[repr(usize)]
pub enum Face2Fb {
    TongueTipInterdental = 63,
    TongueTipAlveolar,
    TongueFrontDorsalPalate,
    TongueMidDorsalPalate,
    TongueBackDorsalPalate,
    TongueOut,
    TongueRetreat,
    Max,
}
