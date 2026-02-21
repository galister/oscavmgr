use std::{
    ops::Add,
    sync::Arc,
    time::{Duration, Instant},
};

use colored::{Color, Colorize};
use glam::{vec3, Affine3A, EulerRot, Quat};
use mint::{Quaternion, Vector3};
use once_cell::sync::Lazy;
use openxr as xr;
use strum::EnumCount;

use crate::core::{AppState, INSTRUCTIONS_END, INSTRUCTIONS_START, TRACK_ON};

use super::{
    htc::{htc_to_unified, HtcFacialData},
    unified::{UnifiedExpressions, UnifiedShapeAccessors, UnifiedTrackingData},
    FaceReceiver,
};

static STA_GAZE: Lazy<Arc<str>> = Lazy::new(|| format!("{}", "GAZE".color(Color::Green)).into());
static STA_GAZE_OFF: Lazy<Arc<str>> = Lazy::new(|| format!("{}", "GAZE".color(Color::Red)).into());
static STA_FACE: Lazy<Arc<str>> = Lazy::new(|| format!("{}", "FACE".color(Color::Green)).into());
static STA_FACE_OFF: Lazy<Arc<str>> = Lazy::new(|| format!("{}", "FACE".color(Color::Red)).into());

pub struct OpenXrReceiver {
    state: Option<XrState>,
    last_attempt: Instant,
}

impl OpenXrReceiver {
    pub fn new() -> Self {
        Self {
            state: None,
            last_attempt: Instant::now(),
        }
    }

    fn try_init(&mut self) {
        self.state = XrState::new().map_err(|e| log::error!("XR: {}", e)).ok();
        self.last_attempt = Instant::now();
    }
}

impl FaceReceiver for OpenXrReceiver {
    fn start_loop(&mut self) {
        log::info!("{}", *INSTRUCTIONS_START);
        log::info!("");
        log::info!("Using OpenXR (WiVRn/Monado) to provide face data.");
        log::info!(
            "It's normal to see {} if the HMD is not yet connected.",
            "errors".color(Color::Red)
        );
        log::info!("");
        log::info!("Status bar tickers:");
        log::info!("• {} → face data is being received", *STA_FACE);
        log::info!("• {} → eye data is being received", *STA_GAZE);
        log::info!("• {} → head & wrist data is being received", *TRACK_ON);
        log::info!("");
        log::info!("{}", *INSTRUCTIONS_END);
        self.try_init();
    }

    fn receive(&mut self, data: &mut UnifiedTrackingData, app: &mut AppState) {
        let Some(state) = self.state.as_mut() else {
            if self.last_attempt.add(Duration::from_secs(15)) < Instant::now() {
                self.try_init();
            }
            app.status.add_item(STA_GAZE_OFF.clone());
            app.status.add_item(STA_FACE_OFF.clone());
            return;
        };

        if let Err(e) = state.receive(data, app) {
            log::error!("XR: {}", e);
            self.state = None;
        }
    }
}

pub(super) struct XrState {
    instance: xr::Instance,
    system: xr::SystemId,
    session: xr::Session<xr::Headless>,
    frame_waiter: xr::FrameWaiter,
    frame_stream: xr::FrameStream<xr::Headless>,
    stage_space: xr::Space,
    view_space: xr::Space,
    eye_space: xr::Space,
    aim_spaces: [xr::Space; 2],
    actions: xr::ActionSet,
    eye_action: xr::Action<xr::Posef>,
    aim_actions: [xr::Action<xr::Posef>; 2],
    events: xr::EventDataBuffer,
    session_running: bool,

    face_tracker_fb: Option<xr::FaceTrackerFB>,
    eye_tracker_htc: Option<xr::FacialTrackerHTC>,
    lip_tracker_htc: Option<xr::FacialTrackerHTC>,

    eyes_closed_frames: u32,
}

impl XrState {
    fn new() -> anyhow::Result<Self> {
        let (instance, system) = xr_init()?;

        let actions = instance.create_action_set("oscavmgr", "OscAvMgr", 0)?;

        let eye_action = actions.create_action("eye_gaze", "Eye Gaze", &[])?;
        let aim_actions = [
            actions.create_action("left_aim", "Left Aim", &[])?,
            actions.create_action("right_aim", "Right Aim", &[])?,
        ];

        let (session, frame_waiter, frame_stream) =
            unsafe { instance.create_session(system, &xr::headless::SessionCreateInfo {})? };

        instance.suggest_interaction_profile_bindings(
            instance.string_to_path("/interaction_profiles/khr/simple_controller")?,
            &[
                xr::Binding::new(
                    &aim_actions[0],
                    instance.string_to_path("/user/hand/left/input/aim/pose")?,
                ),
                xr::Binding::new(
                    &aim_actions[1],
                    instance.string_to_path("/user/hand/right/input/aim/pose")?,
                ),
            ],
        )?;

        instance.suggest_interaction_profile_bindings(
            instance.string_to_path("/interaction_profiles/ext/eye_gaze_interaction")?,
            &[xr::Binding::new(
                &eye_action,
                instance.string_to_path("/user/eyes_ext/input/gaze_ext/pose")?,
            )],
        )?;

        session.attach_action_sets(&[&actions])?;

        let stage_space =
            session.create_reference_space(xr::ReferenceSpaceType::STAGE, xr::Posef::IDENTITY)?;

        let view_space =
            session.create_reference_space(xr::ReferenceSpaceType::VIEW, xr::Posef::IDENTITY)?;

        let eye_space = eye_action.create_space(&session, xr::Path::NULL, xr::Posef::IDENTITY)?;

        let aim_spaces = [
            aim_actions[0].create_space(&session, xr::Path::NULL, xr::Posef::IDENTITY)?,
            aim_actions[1].create_space(&session, xr::Path::NULL, xr::Posef::IDENTITY)?,
        ];

        let mut me = Self {
            instance,
            system,
            session,
            frame_waiter,
            frame_stream,
            face_tracker_fb: None,
            eye_tracker_htc: None,
            lip_tracker_htc: None,
            stage_space,
            view_space,
            eye_space,
            aim_spaces,
            actions,
            eye_action,
            aim_actions,
            events: xr::EventDataBuffer::new(),
            session_running: false,
            eyes_closed_frames: 0,
        };

        if let Ok(fb_supported) = me
            .instance
            .supports_face_tracking_fb(me.system)
            .inspect_err(|e| log::info!("FB_face_tracking2: {e}"))
        {
            if fb_supported.supports_visual_face_tracking {
                me.face_tracker_fb = me
                    .session
                    .create_face_tracker_fb(
                        xr::FaceExpressionSet2FB::DEFAULT,
                        &[xr::FaceTrackingDataSource2FB::VISUAL],
                    )
                    .inspect(|_| log::info!("Using FB_face_tracking2 for face."))
                    .inspect_err(|e| log::info!("FB_face_tracking2: {e}"))
                    .ok();
            } else {
                log::info!("FB_face_tracking2: visual face tracking not supported");
            }
        }

        if let Ok(htc_supported) = me
            .instance
            .supports_facial_tracking_htc(me.system)
            .inspect_err(|e| log::info!("HTC_facial_tracking: {e}"))
        {
            if htc_supported.support_eye_facial_tracking {
                me.eye_tracker_htc = me
                    .session
                    .create_facial_tracker_htc(xr::FacialTrackingTypeHTC::EYE_DEFAULT)
                    .inspect(|_| log::info!("Using HTC_facial_tracking for eyes."))
                    .inspect_err(|e| log::info!("HTC_facial_tracking (eye): {e}"))
                    .ok();
            } else {
                log::info!("HTC_facial_tracking (eye): not supported");
            }

            if htc_supported.support_lip_facial_tracking {
                me.lip_tracker_htc = me
                    .session
                    .create_facial_tracker_htc(xr::FacialTrackingTypeHTC::LIP_DEFAULT)
                    .inspect(|_| log::info!("Using HTC_facial_tracking for lip."))
                    .inspect_err(|e| log::info!("HTC_facial_tracking (lip): {e}"))
                    .ok();
            } else {
                log::info!("HTC_facial_tracking (lip): not supported");
            }
        }

        Ok(me)
    }

    fn receive(
        &mut self,
        data: &mut UnifiedTrackingData,
        state: &mut AppState,
    ) -> anyhow::Result<()> {
        while let Some(event) = self.instance.poll_event(&mut self.events)? {
            use xr::Event::*;
            match event {
                SessionStateChanged(e) => match e.state() {
                    xr::SessionState::READY => {
                        self.session
                            .begin(xr::ViewConfigurationType::PRIMARY_STEREO)?;
                        self.session_running = true;
                        log::info!("XrSession started.")
                    }
                    xr::SessionState::STOPPING => {
                        self.session.end()?;
                        self.session_running = false;
                        log::warn!("XrSession stopped.")
                    }
                    xr::SessionState::EXITING | xr::SessionState::LOSS_PENDING => {
                        anyhow::bail!("XR session exiting");
                    }
                    _ => {}
                },
                InstanceLossPending(_) => {
                    anyhow::bail!("XR instance loss pending");
                }
                EventsLost(e) => {
                    log::warn!("lost {} events", e.lost_event_count());
                }
                _ => {}
            }
        }

        if !self.session_running {
            return Ok(());
        }

        let next_frame = xr::Time::from_nanos(
            self.instance.now()?.as_nanos()
                + (state.status.last_frame_time.max(0.03334) * 1_000_000_000f32) as i64,
        );

        self.session.sync_actions(&[(&self.actions).into()])?;

        let hmd_loc = self.view_space.locate(&self.stage_space, next_frame)?;
        if hmd_loc
            .location_flags
            .contains(xr::SpaceLocationFlags::POSITION_VALID)
        {
            state.tracking.head = to_affine(&hmd_loc);
            state.tracking.last_received = Instant::now();
        } else {
            // HMD is sleeping, close the avatar's eyes (unless face tracker can provide valid data)
            data.shapes.setu(UnifiedExpressions::EyeClosedLeft, 1.0);
            data.shapes.setu(UnifiedExpressions::EyeClosedRight, 1.0);
        }

        let aim_loc = self.aim_spaces[0].locate(&self.stage_space, next_frame)?;
        state.tracking.left_hand = to_affine(&aim_loc);
        let aim_loc = self.aim_spaces[1].locate(&self.stage_space, next_frame)?;
        state.tracking.right_hand = to_affine(&aim_loc);

        let eye_loc = self.eye_space.locate(&self.view_space, next_frame)?;
        if eye_loc.location_flags.contains(
            xr::SpaceLocationFlags::ORIENTATION_VALID | xr::SpaceLocationFlags::ORIENTATION_TRACKED,
        ) {
            let now_q = to_quat(eye_loc.pose.orientation);
            let (y, x, z) = now_q.to_euler(EulerRot::YXZ);

            let mut eye_closed = ((x.to_degrees() + 5.0) / -55.0).max(0.0);

            if let Some(last) = data.eyes[0] {
                let last_q = Quat::from_euler(EulerRot::YXZ, last.y, last.x, last.z);

                if last_q.angle_between(now_q).to_degrees() > 10.0 {
                    self.eyes_closed_frames = 5;
                }
            }

            if self.eyes_closed_frames > 0 {
                self.eyes_closed_frames -= 1;
                eye_closed = 1.0;
            }

            data.shapes
                .setu(UnifiedExpressions::EyeClosedLeft, eye_closed);
            data.shapes
                .setu(UnifiedExpressions::EyeClosedRight, eye_closed);

            data.eyes[0] = Some(vec3(x, y, z));
            data.eyes[1] = data.eyes[0];
            state.status.add_item(STA_GAZE.clone());
        } else {
            state.status.add_item(STA_GAZE_OFF.clone());
        }

        if let Some(face_tracker) = self.face_tracker_fb.as_ref() {
            let expression = face_tracker.get_expression_weights(next_frame)?;

            if let Some(expression) = expression {
                if let Some(shapes) = super::face2_fb::face2_fb_to_unified(&expression.weights) {
                    data.shapes[..=UnifiedExpressions::COUNT]
                        .copy_from_slice(&shapes[..=UnifiedExpressions::COUNT]);
                }
                state.status.add_item(STA_FACE.clone());
            } else {
                state.status.add_item(STA_FACE_OFF.clone());
            }
        };

        {
            let eye = self
                .eye_tracker_htc
                .as_ref()
                .and_then(|t| t.get_expressions(next_frame).ok())
                .flatten();
            let lip = self
                .lip_tracker_htc
                .as_ref()
                .and_then(|t| t.get_expressions(next_frame).ok())
                .flatten();

            if eye.is_some() || lip.is_some() {
                let shapes = htc_to_unified(&HtcFacialData {
                    eye: eye.map(|e| e.expressions.try_into().unwrap()),
                    lip: lip.map(|l| l.expressions.try_into().unwrap()),
                });
                data.shapes[..=UnifiedExpressions::COUNT]
                    .copy_from_slice(&shapes[..=UnifiedExpressions::COUNT]);
                state.status.add_item(STA_FACE.clone());
            } else {
                state.status.add_item(STA_FACE_OFF.clone());
            }
        }

        Ok(())
    }
}

fn xr_init() -> anyhow::Result<(xr::Instance, xr::SystemId)> {
    let entry = xr::Entry::linked(&())?;

    let Ok(available_extensions) = entry.enumerate_extensions() else {
        anyhow::bail!("Failed to enumerate OpenXR extensions.");
    };

    anyhow::ensure!(
        available_extensions.mnd_headless,
        "Missing MND_headless extension."
    );

    let mut enabled_extensions = xr::ExtensionSet::default();
    enabled_extensions.mnd_headless = true;
    enabled_extensions.khr_convert_timespec_time = true;

    if available_extensions.ext_eye_gaze_interaction {
        enabled_extensions.ext_eye_gaze_interaction = true;
    } else {
        log::warn!("Missing EXT_eye_gaze_interaction extension. Is Monado/WiVRn up to date?");
    }

    if available_extensions.fb_face_tracking2 {
        enabled_extensions.fb_face_tracking2 = true;
    }

    if available_extensions.htc_facial_tracking {
        enabled_extensions.htc_facial_tracking = true;
    }

    let Ok(instance) = entry.create_instance(
        &xr::ApplicationInfo {
            api_version: xr::Version::new(1, 0, 0),
            application_name: "oscavmgr",
            application_version: 0,
            engine_name: "oscavmgr",
            engine_version: 0,
        },
        &enabled_extensions,
        &[],
        &(),
    ) else {
        anyhow::bail!("Failed to create OpenXR instance.");
    };

    let Ok(instance_props) = instance.properties() else {
        anyhow::bail!("Failed to query OpenXR instance properties.");
    };
    log::info!(
        "Using OpenXR runtime: {} {}",
        instance_props.runtime_name,
        instance_props.runtime_version
    );

    let Ok(system) = instance.system(xr::FormFactor::HEAD_MOUNTED_DISPLAY) else {
        anyhow::bail!("Failed to access OpenXR HMD system.");
    };

    Ok((instance, system))
}

fn to_quat(p: xr::Quaternionf) -> Quat {
    let q: Quaternion<f32> = p.into();
    q.into()
}

fn to_affine(loc: &xr::SpaceLocation) -> Affine3A {
    let t: Vector3<f32> = loc.pose.position.into();
    Affine3A::from_rotation_translation(to_quat(loc.pose.orientation), t.into())
}
