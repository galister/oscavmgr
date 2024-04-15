use std::{
    collections::HashMap,
    f32::consts::PI,
    ops::Range,
    sync::{
        atomic::{AtomicBool, AtomicI32},
        Arc,
    },
};

use glam::Vec3;
use log::info;
use rosc::{OscBundle, OscType};

use crate::core::ext_tracking::unified::UnifiedExpressions;

use super::{bundle::AvatarBundle, ext_tracking::ExtTracking, AvatarParameters};

const CONTACT_RADIUS: f32 = 3.;
const DIST_MULTIPLIER: f32 = 25.;

fn contact_to_dist(d: &f32) -> f32 {
    (1. - d) * CONTACT_RADIUS
}

const P1: Vec3 = Vec3::new(1., 0., 0.);
const P2: Vec3 = Vec3::new(0., 1., 0.);
const P3: Vec3 = Vec3::new(0., 0., 1.);

const MOVE_THRESHOLD_METERS: f32 = 0.1;
const RUN_THRESHOLD_METERS: f32 = 1.0;
const ROTATE_THRESHOLD_RAD: f32 = PI / 4.; // 45deg
const ROTATE_START_THRESHOLD_RAD: f32 = PI * 2.; // never

fn trilaterate(r1: f32, r2: f32, r3: f32, r4: f32) -> Vec3 {
    let p2_neg_p1 = P2 - P1;
    let p3_neg_p1 = P3 - P1;

    let e_x = p2_neg_p1.normalize();
    let i = e_x.dot(p3_neg_p1);

    let e_y = (p3_neg_p1 - i * e_x).normalize();
    let e_z = e_x.cross(e_y);
    let d = p2_neg_p1.length();
    let j = e_y.dot(p3_neg_p1);

    let r1_sq = r1 * r1;

    let x = (r1_sq - r2 * r2 + d * d) / (2. * d);
    let y = ((r1_sq - r3 * r3 + i * i + j * j) / (2. * j)) - (i / j * x);

    let z1 = (r1_sq - x * x - y * y).sqrt();
    let z2 = -1. * z1;

    let ans1 = P1 + x * e_x + y * e_y + z1 * e_z;
    let ans2 = P1 + x * e_x + y * e_y + z2 * e_z;

    if ans1.length() - r4 < ans2.length() - r4 {
        ans1
    } else {
        ans2
    }
}

fn vec3_to_target(parameters: &HashMap<Arc<str>, OscType>) -> Option<Vec3> {
    let par1 = parameters.get("Seeker_P0")?;
    let par2 = parameters.get("Seeker_P1")?;
    let par3 = parameters.get("Seeker_P2")?;
    let par4 = parameters.get("Seeker_P3")?;

    match (par1, par2, par3, par4) {
        (OscType::Float(c1), OscType::Float(c2), OscType::Float(c3), OscType::Float(c4)) => {
            let r1 = contact_to_dist(c1);
            let r2 = contact_to_dist(c2);
            let r3 = contact_to_dist(c3);
            let r4 = contact_to_dist(c4);
            Some(trilaterate(r1, r2, r3, r4) * DIST_MULTIPLIER)
        }
        _ => None,
    }
}

static VOICE: AtomicBool = AtomicBool::new(false);
static VOICE_LOCK: AtomicBool = AtomicBool::new(false);
static JUMPED: AtomicBool = AtomicBool::new(false);
static COUNTDOWN: AtomicI32 = AtomicI32::new(0);
fn avatar_flight(parameters: &AvatarParameters, tracking: &ExtTracking, bundle: &mut OscBundle) {
    const FLIGHT_INTS: Range<i32> = 120..125;

    let (Some(hmd), [Some(left), Some(right)]) = (
        &tracking.data.hmd,
        &tracking.data.hands,
    ) else {
        return;
    };

    if let Some(OscType::Int(emote)) = parameters.get("VRCEmote") {
        if FLIGHT_INTS.contains(emote)
            && left.position.y > hmd.position.y
            && right.position.y > hmd.position.y
        {
            let jumped = JUMPED.load(std::sync::atomic::Ordering::Relaxed);
            let countdown = COUNTDOWN.load(std::sync::atomic::Ordering::Relaxed);

            if !jumped && countdown <= 0 {
                let diff = (left.position.y + left.position.y) * 0.5 + 0.1 - hmd.position.y;
                let diff = diff.clamp(0., 0.3);

                bundle.send_input_button("Jump", true);
                info!("Jumping with diff {}", diff);

                JUMPED.store(true, std::sync::atomic::Ordering::Relaxed);
                COUNTDOWN.store(
                    (30. - 100. * diff) as i32,
                    std::sync::atomic::Ordering::Relaxed,
                );
            } else {
                bundle.send_input_button("Jump", false);
                COUNTDOWN.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                JUMPED.store(false, std::sync::atomic::Ordering::Relaxed);
            }
        } else if JUMPED.load(std::sync::atomic::Ordering::Relaxed) {
            bundle.send_input_button("Jump", false);
            COUNTDOWN.store(0, std::sync::atomic::Ordering::Relaxed);
            JUMPED.store(false, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

pub fn autopilot_step(
    parameters: &AvatarParameters,
    tracking: &ExtTracking,
    bundle: &mut OscBundle,
) {
    static FOLLOW_BEFORE: AtomicBool = AtomicBool::new(false);

    avatar_flight(parameters, tracking, bundle);

    let mut follow = false;
    let mut follow_distance = MOVE_THRESHOLD_METERS;
    let mut allow_rotate = false;

    if let Some(OscType::Bool(true)) = parameters.get("Seeker_IsGrabbed") {
        follow = true;
    } else if let Some(OscType::Bool(true)) = parameters.get("Tracker1_Enable") {
        follow = true;
        allow_rotate = true;
        follow_distance = RUN_THRESHOLD_METERS;
    }

    let mut look_horizontal = 0.;
    let mut vertical = 0.;
    let mut horizontal = 0.;

    if follow {
        if let Some(tgt) = vec3_to_target(parameters) {
            let dist_horizontal = (tgt.x * tgt.x + tgt.z * tgt.z).sqrt();
            let mut theta = (tgt.x / tgt.z).atan();

            if tgt.z < 0. {
                theta = if theta < 0. { PI + theta } else { -PI + theta };
            }

            let abs_theta = theta.abs();

            if dist_horizontal > follow_distance {
                let mult = (dist_horizontal / RUN_THRESHOLD_METERS).clamp(0., 1.);

                vertical = tgt.z / dist_horizontal * mult;
                horizontal = tgt.x / dist_horizontal * mult;
                if allow_rotate {
                    look_horizontal = theta.signum() * (abs_theta / (PI / 2.)).clamp(0., 1.);
                }
                FOLLOW_BEFORE.store(true, std::sync::atomic::Ordering::Relaxed);
            } else if allow_rotate && abs_theta > ROTATE_START_THRESHOLD_RAD {
                look_horizontal = theta.signum() * (abs_theta / (PI / 2.)).clamp(0., 1.);
            }
        }
    } else if let [Some(left), Some(right)] = &tracking.data.hands {
        let palm_left = left.orientation * Vec3::X;
        let palm_right = right.orientation * Vec3::NEG_X;

        let to_left = (left.position - right.position).normalize();
        let to_right = (right.position - left.position).normalize();

        let dot_left = palm_left.dot(to_right);
        let dot_right = palm_right.dot(to_left);

        if (left.position - right.position).length() < 0.2 && dot_left < -0.8 && dot_right < -0.8 {
            if let Some(eye) = tracking.data.eyes[0] {
                let deg_x = eye.x.atan().to_degrees();
                if !(-10. ..=20.).contains(&deg_x) {
                    look_horizontal = (deg_x * 0.02).min(1.);
                }

                let deg_y = eye.y.atan().to_degrees();
                if deg_y > 15. && !JUMPED.load(std::sync::atomic::Ordering::Relaxed) {
                    bundle.send_input_button("Jump", true);
                    JUMPED.store(true, std::sync::atomic::Ordering::Relaxed);
                } else if JUMPED.load(std::sync::atomic::Ordering::Relaxed) {
                    bundle.send_input_button("Jump", false);
                    JUMPED.store(false, std::sync::atomic::Ordering::Relaxed);
                }
            }

            let puff = tracking.data.getu(UnifiedExpressions::CheekPuffLeft)
                + tracking.data.getu(UnifiedExpressions::CheekPuffRight);

            let suck = tracking.data.getu(UnifiedExpressions::CheekSuckLeft)
                + tracking.data.getu(UnifiedExpressions::CheekSuckRight);

            if puff > 0.5 {
                vertical = (puff * 0.6).min(1.0);
            } else if suck > 0.5 {
                vertical = -(suck * 0.6).min(1.0);
            }

            let brows = tracking.data.getu(UnifiedExpressions::BrowInnerUpLeft)
                + tracking.data.getu(UnifiedExpressions::BrowInnerUpRight)
                + tracking.data.getu(UnifiedExpressions::BrowOuterUpLeft)
                + tracking.data.getu(UnifiedExpressions::BrowOuterUpRight);

            if brows < 2.0 {
                VOICE_LOCK.store(false, std::sync::atomic::Ordering::Relaxed);
            }

            if brows > 3.0 && !VOICE.load(std::sync::atomic::Ordering::Relaxed) {
                bundle.send_input_button("Voice", true);
                VOICE.store(true, std::sync::atomic::Ordering::Relaxed);
                VOICE_LOCK.store(true, std::sync::atomic::Ordering::Relaxed);
            } else if VOICE.load(std::sync::atomic::Ordering::Relaxed)
                && !VOICE_LOCK.load(std::sync::atomic::Ordering::Relaxed)
            {
                bundle.send_input_button("Voice", false);
                VOICE.store(false, std::sync::atomic::Ordering::Relaxed);
            }
        }
    }

    bundle.send_input_axis("LookHorizontal", look_horizontal);
    bundle.send_input_axis("Vertical", vertical);
    bundle.send_input_axis("Horizontal", horizontal);
}
