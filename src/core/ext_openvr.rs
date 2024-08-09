use std::{
    env,
    f32::consts::PI,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use super::{bundle::AvatarBundle, AppState};
use colored::{Color, Colorize};
use glam::{vec3, Affine3A, Quat, Vec3A};
use once_cell::sync::Lazy;
use ovr_overlay::{
    sys::{
        ETrackedDeviceClass, ETrackingResult, ETrackingUniverseOrigin, EVRApplicationType,
        EVREventType, HmdMatrix34_t,
    },
    system::SystemManager,
    Context, TrackedDeviceIndex,
};
use rosc::{OscBundle, OscType};

pub struct ExtOpenVr {
    devices: TrackedDevices,
    context: Option<Context>,
    next_device_update: Instant,
    next_init_attempt: Instant,
    head: Option<usize>,
    head_y: f32,
    floor_y: f32,
    frames: u32,
}

static OPENVR_ON: Lazy<Arc<str>> = Lazy::new(|| format!("{}", "OPENVR".color(Color::Green)).into());
static OPENVR_OFF: Lazy<Arc<str>> = Lazy::new(|| format!("{}", "OPENVR".color(Color::Red)).into());

const FEET_Y: f32 = 0.10;

macro_rules! env_parse {
    ($x:expr) => {
        env::var($x)
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or_default()
    };
}

static HEAD_OFFSET: Lazy<Affine3A> = Lazy::new(|| {
    let rotation = Quat::from_rotation_y(env_parse!("HEAD_YAW"))
        * Quat::from_rotation_x(env_parse!("HEAD_PITCH"))
        * Quat::from_rotation_z(env_parse!("HEAD_ROLL"));

    let translation = vec3(
        env_parse!("HEAD_X"),
        env_parse!("HEAD_Y"),
        env_parse!("HEAD_Z"),
    );

    Affine3A::from_rotation_translation(rotation, translation)
});

static TRACKER_ADJUST: Lazy<Affine3A> = Lazy::new(|| Affine3A::from_rotation_x(PI * 0.5));

static DEVICE_COUNTER: AtomicU32 = AtomicU32::new(1);

type TrackedDevices = [TrackedDevice; 32];

#[derive(Default)]
struct TrackedDevice {
    pub index: u32,
    serial: String,
    active: bool,
    pos: Vec3A,
}

impl ExtOpenVr {
    pub fn new() -> Self {
        Self {
            next_device_update: Instant::now(),
            next_init_attempt: Instant::now(),
            context: None,
            devices: Default::default(),
            head: None,
            head_y: f32::MIN,
            floor_y: f32::MAX,
            frames: 0,
        }
    }

    pub fn step(&mut self, state: &mut AppState, bundle: &mut OscBundle) {
        if self.context.is_none() {
            if self.next_init_attempt > Instant::now() {
                state.status.add_item(OPENVR_OFF.clone());
                return;
            }

            let app_type = EVRApplicationType::VRApplication_Background;
            let Ok(context) = ovr_overlay::Context::init(app_type) else {
                self.next_init_attempt = Instant::now() + Duration::from_secs(15);
                log::debug!("OpenVR: server unavailable");
                state.status.add_item(OPENVR_OFF.clone());
                return;
            };
            self.context = Some(context);
        }

        let context = self.context.as_mut().unwrap();
        let mut system_mgr = context.system_mngr();

        while let Some(event) = system_mgr.poll_next_event() {
            match event.event_type {
                EVREventType::VREvent_Quit => {
                    log::warn!("OpenVR: shutting down");
                    unsafe { context.shutdown() };
                    self.context = None;
                    self.next_init_attempt = Instant::now() + Duration::from_secs(15);
                    state.status.add_item(OPENVR_OFF.clone());
                    return;
                }
                EVREventType::VREvent_TrackedDeviceActivated
                | EVREventType::VREvent_TrackedDeviceDeactivated
                | EVREventType::VREvent_TrackedDeviceUpdated => {
                    self.next_device_update = Instant::now();
                }
                _ => {}
            }
        }
        state.status.add_item(OPENVR_ON.clone());

        if self.next_device_update <= Instant::now() {
            log::debug!("OpenVR: TrackedDevice update");
            update_devices(&mut system_mgr, &mut self.devices);
            self.next_device_update = Instant::now() + Duration::from_secs(30);
        }

        let device_tracking = system_mgr.get_device_to_absolute_tracking_pose(
            ETrackingUniverseOrigin::TrackingUniverseStanding,
            state.status.last_frame_time,
        );
        for (idx, device) in self.devices.iter_mut().enumerate() {
            if !device.active {
                continue;
            }

            let tracking = device_tracking.get(idx).unwrap();

            if !tracking.bPoseIsValid
                || !tracking.bDeviceIsConnected
                || !matches!(
                    tracking.eTrackingResult,
                    ETrackingResult::TrackingResult_Running_OK
                )
            {
                continue;
            }

            let mut affine = tracking.mDeviceToAbsoluteTracking.to_affine() * *TRACKER_ADJUST;

            if self.frames < 90 {
                self.floor_y = self.floor_y.min(affine.translation.y - FEET_Y);

                if affine.translation.y > self.floor_y + 1.6 && self.head_y < affine.translation.y {
                    self.head_y = affine.translation.y;
                    self.head = Some(idx);
                }

                continue;
            }

            let (addr_pos, addr_rot) = if self.head.is_some_and(|head| head == idx) {
                affine *= *HEAD_OFFSET;
                (
                    "/tracking/trackers/head/position".into(),
                    "/tracking/trackers/head/rotation".into(),
                )
            } else {
                (
                    format!("/tracking/trackers/{}/position", device.index),
                    format!("/tracking/trackers/{}/rotation", device.index),
                )
            };

            let p = affine.translation;
            let quat = Quat::from_affine3(&affine);
            let (ry, rx, rz) = quat.to_euler(glam::EulerRot::YXZ);
            let deg = vec3(rx.to_degrees(), ry.to_degrees(), rz.to_degrees());

            bundle.send_tracking(
                &addr_pos,
                vec![
                    OscType::Float(p.x),
                    OscType::Float(p.y - self.floor_y),
                    OscType::Float(p.z),
                ],
            );

            bundle.send_tracking(
                &addr_rot,
                vec![
                    OscType::Float(deg.x),
                    OscType::Float(deg.y),
                    OscType::Float(deg.z),
                ],
            );
        }
        self.frames += 1;
        if self.frames == 90 {
            let head_str = self
                .head
                .and_then(|idx| self.devices.get(idx))
                .map(|dev| dev.serial.clone());

            log::info!(
                "OpenVR: Calibration complete.\n  head: {:?}\n  floor_y: {:.2}",
                head_str,
                self.floor_y
            );
        }
    }
}

fn update_devices(system: &mut SystemManager, devices: &mut TrackedDevices) {
    for (idx, device) in devices.iter_mut().enumerate() {
        let dev_idx = TrackedDeviceIndex::new(idx as _).unwrap(); // safe
        if !system.is_tracked_device_connected(dev_idx) {
            device.active = false;
            continue;
        }
        if let Ok(serial) = system.get_tracked_device_property::<String>(
            dev_idx,
            ovr_overlay::sys::ETrackedDeviceProperty::Prop_SerialNumber_String,
        ) {
            device.serial = serial;
        }

        let class = system.get_tracked_device_class(dev_idx);
        match class {
            ETrackedDeviceClass::TrackedDeviceClass_HMD
            | ETrackedDeviceClass::TrackedDeviceClass_TrackingReference
            | ETrackedDeviceClass::TrackedDeviceClass_Controller => {
                device.active = false;
                log::debug!("OpenVR: Not a tracker: {}", &device.serial);
                continue;
            }
            ETrackedDeviceClass::TrackedDeviceClass_GenericTracker => {}
            _ => {
                device.active = false;
                log::debug!("OpenVR: Invalid device: {}", &device.serial);
                continue;
            }
        }

        if !device.active {
            log::info!("OpenVR: New tracker: {}", &device.serial);
            device.index = DEVICE_COUNTER.fetch_add(1, Ordering::Relaxed);
            device.active = true;
        }

        if let Ok(soc) = system.get_tracked_device_property::<f32>(
            dev_idx,
            ovr_overlay::sys::ETrackedDeviceProperty::Prop_DeviceBatteryPercentage_Float,
        ) {
            log::info!("OpenVR: {} is at {}%", device.serial, (soc * 100.0) as i32)
        }
    }
}

pub trait Affine3AConvert {
    fn to_affine(&self) -> Affine3A;
}

impl Affine3AConvert for HmdMatrix34_t {
    fn to_affine(&self) -> Affine3A {
        Affine3A::from_cols_array_2d(&[
            [self.m[0][0], self.m[1][0], -self.m[2][0]],
            [self.m[0][1], self.m[1][1], -self.m[2][1]],
            [-self.m[0][2], -self.m[1][2], self.m[2][2]],
            [self.m[0][3], self.m[1][3], -self.m[2][3]],
        ])
    }
}
