use std::{sync::Arc, time::Instant};

use colored::{Color, Colorize};
use glam::{vec3, Affine3A, EulerRot, Quat};
use mint::{Quaternion, Vector3};
use once_cell::sync::Lazy;
use openxr::{
    self as xr,
    raw::FaceTracking2FB,
    sys::{
        Bool32, FaceExpressionInfo2FB, FaceExpressionWeights2FB, FaceTracker2FB,
        FaceTrackerCreateInfo2FB,
    },
    FaceExpressionSet2FB, FaceTrackingDataSource2FB, SpaceLocation, Version,
};

use crate::core::AppState;

use super::unified::UnifiedTrackingData;

static STA_GAZE: Lazy<Arc<str>> = Lazy::new(|| format!("{}", "GAZE".color(Color::Green)).into());
static STA_GAZE_OFF: Lazy<Arc<str>> = Lazy::new(|| format!("{}", "GAZE".color(Color::Red)).into());
static STA_FACE: Lazy<Arc<str>> = Lazy::new(|| format!("{}", "FACE".color(Color::Green)).into());
static STA_FACE_OFF: Lazy<Arc<str>> = Lazy::new(|| format!("{}", "FACE".color(Color::Red)).into());

pub struct OpenXrReceiver {
    instance: xr::Instance,
    system: xr::SystemId,
    session: xr::Session<xr::Headless>,
    frame_waiter: xr::FrameWaiter,
    frame_stream: xr::FrameStream<xr::Headless>,
    face_tracker: Option<MyFaceTracker>,
    stage_space: xr::Space,
    view_space: xr::Space,
    eye_space: xr::Space,
    aim_spaces: [xr::Space; 2],
    actions: xr::ActionSet,
    eye_action: xr::Action<xr::Posef>,
    aim_actions: [xr::Action<xr::Posef>; 2],
    events: xr::EventDataBuffer,
    session_running: bool,
}

impl OpenXrReceiver {
    pub fn new() -> anyhow::Result<Self> {
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

        let eye_space =
            eye_action.create_space(session.clone(), xr::Path::NULL, xr::Posef::IDENTITY)?;

        let aim_spaces = [
            aim_actions[0].create_space(session.clone(), xr::Path::NULL, xr::Posef::IDENTITY)?,
            aim_actions[0].create_space(session.clone(), xr::Path::NULL, xr::Posef::IDENTITY)?,
        ];

        let face_tracker = MyFaceTracker::new(&session).ok();

        Ok(Self {
            instance,
            system,
            session,
            frame_waiter,
            frame_stream,
            face_tracker,
            stage_space,
            view_space,
            eye_space,
            aim_spaces,
            actions,
            eye_action,
            aim_actions,
            events: xr::EventDataBuffer::new(),
            session_running: false,
        })
    }

    pub fn receive(
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
                        log::warn!("XrSession started.")
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

        let now = self.instance.now()?;

        self.session.sync_actions(&[(&self.actions).into()])?;

        let hmd_loc = self.view_space.locate(&self.stage_space, now)?;
        if hmd_loc
            .location_flags
            .contains(xr::SpaceLocationFlags::POSITION_VALID)
        {
            state.tracking.head = to_affine(&hmd_loc);
            state.tracking.last_received = Instant::now();
        }

        let aim_loc = self.aim_spaces[0].locate(&self.stage_space, now)?;
        state.tracking.left_hand = to_affine(&aim_loc);
        let aim_loc = self.aim_spaces[1].locate(&self.stage_space, now)?;
        state.tracking.right_hand = to_affine(&aim_loc);

        let eye_loc = self.eye_space.locate(&self.view_space, now)?;
        if eye_loc
            .location_flags
            .contains(xr::SpaceLocationFlags::ORIENTATION_VALID)
        {
            let (y, x, z) = to_quat(eye_loc.pose.orientation).to_euler(EulerRot::YXZ);
            data.eyes[0] = Some(vec3(x, y, z));
            data.eyes[1] = data.eyes[0];
            state.status.add_item(STA_GAZE.clone());
        } else {
            state.status.add_item(STA_GAZE_OFF.clone());
        }

        if let Some(face_tracker) = self.face_tracker.as_ref() {
            let mut weights = [0f32; 70];
            let mut confidences = [0f32; 2];

            let is_valid =
                face_tracker.get_face_expression_weights(now, &mut weights, &mut confidences)?;

            if is_valid {
                if let Some(shapes) = super::face2_fb::face2_fb_to_unified(&weights) {
                    data.shapes = shapes;
                }
                state.status.add_item(STA_FACE.clone());
            } else {
                state.status.add_item(STA_FACE_OFF.clone());
            }
        };

        Ok(())
    }
}

fn xr_init() -> anyhow::Result<(xr::Instance, xr::SystemId)> {
    let entry = xr::Entry::linked();

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
        log::warn!("Missing EXT_eye_gaze_interaction extension.");
    }

    if available_extensions.fb_face_tracking2 {
        enabled_extensions.fb_face_tracking2 = true;
    } else {
        log::warn!("Missing FB_face_tracking2 extension.");
    }

    let Ok(instance) = entry.create_instance(
        &xr::ApplicationInfo {
            api_version: Version::new(1, 0, 0),
            application_name: "wlx-overlay-s",
            application_version: 0,
            engine_name: "wlx-overlay-s",
            engine_version: 0,
        },
        &enabled_extensions,
        &[],
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

struct MyFaceTracker {
    api: FaceTracking2FB,
    tracker: FaceTracker2FB,
}

impl MyFaceTracker {
    pub fn new<G>(session: &xr::Session<G>) -> anyhow::Result<Self> {
        let api = unsafe {
            FaceTracking2FB::load(session.instance().entry(), session.instance().as_raw())?
        };

        let mut data_source = FaceTrackingDataSource2FB::VISUAL;

        let info = FaceTrackerCreateInfo2FB {
            ty: xr::StructureType::FACE_TRACKER_CREATE_INFO2_FB,
            next: std::ptr::null(),
            face_expression_set: FaceExpressionSet2FB::DEFAULT,
            requested_data_source_count: 1,
            requested_data_sources: &mut data_source,
        };

        let mut tracker = FaceTracker2FB::default();

        let res = unsafe { (api.create_face_tracker2)(session.as_raw(), &info, &mut tracker) };
        if res.into_raw() != 0 {
            anyhow::bail!("Failed to create face tracker: {:?}", res);
        }

        Ok(Self { api, tracker })
    }

    pub fn get_face_expression_weights(
        &self,
        time: xr::Time,
        weights: &mut [f32],
        confidences: &mut [f32],
    ) -> anyhow::Result<bool> {
        let mut expressions = FaceExpressionWeights2FB {
            ty: xr::StructureType::FACE_EXPRESSION_WEIGHTS2_FB,
            next: std::ptr::null_mut(),
            weight_count: weights.len() as _,
            weights: weights.as_mut_ptr(),
            confidence_count: confidences.len() as _,
            confidences: confidences.as_mut_ptr(),
            is_eye_following_blendshapes_valid: Bool32::from_raw(0),
            is_valid: Bool32::from_raw(0),
            data_source: FaceTrackingDataSource2FB::VISUAL,
            time,
        };

        let info = FaceExpressionInfo2FB {
            ty: xr::StructureType::FACE_EXPRESSION_INFO2_FB,
            next: std::ptr::null(),
            time,
        };

        let res = unsafe {
            (self.api.get_face_expression_weights2)(self.tracker, &info, &mut expressions)
        };
        if res.into_raw() != 0 {
            anyhow::bail!("Failed to get expression weights");
        }

        Ok(expressions.is_valid.into_raw() != 0)
    }
}

impl Drop for MyFaceTracker {
    fn drop(&mut self) {
        unsafe {
            (self.api.destroy_face_tracker2)(self.tracker);
        }
    }
}

fn to_quat(p: xr::Quaternionf) -> Quat {
    let q: Quaternion<f32> = p.into();
    q.into()
}

fn to_affine(loc: &SpaceLocation) -> Affine3A {
    let t: Vector3<f32> = loc.pose.position.into();
    Affine3A::from_rotation_translation(to_quat(loc.pose.orientation), t.into())
}
