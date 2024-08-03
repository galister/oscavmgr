use glam::{Quat, Vec3};
use rosc::{OscBundle, OscType};
use strum::{EnumCount, EnumIter, EnumString, IntoStaticStr};

use crate::core::{bundle::AvatarBundle, ext_oscjson::MysteryParam, AppState};

#[derive(Debug, Default, Clone)]
pub struct Posef {
    pub orientation: Quat,
    pub position: Vec3,
}

#[derive(Debug, Default, Clone)]
pub struct UnifiedEyeData {
    pub left: Option<Posef>,
    pub right: Option<Posef>,
}

pub type UnifiedShapes = [f32; NUM_SHAPES];

pub trait UnifiedShapeAccessors {
    fn getu(&self, exp: UnifiedExpressions) -> f32;
    fn getc(&self, exp: CombinedExpression) -> f32;
    fn setu(&mut self, exp: UnifiedExpressions, value: f32);
    fn setc(&mut self, exp: CombinedExpression, value: f32);
}

impl UnifiedShapeAccessors for UnifiedShapes {
    #[inline(always)]
    fn getu(&self, exp: UnifiedExpressions) -> f32 {
        self[exp as usize]
    }

    #[inline(always)]
    fn getc(&self, exp: CombinedExpression) -> f32 {
        self[exp as usize]
    }

    #[inline(always)]
    fn setu(&mut self, exp: UnifiedExpressions, value: f32) {
        self[exp as usize] = value;
    }

    #[inline(always)]
    fn setc(&mut self, exp: CombinedExpression, value: f32) {
        self[exp as usize] = value;
    }
}

pub type UnifiedExpressionShape = f32;

#[derive(Debug, Clone)]
pub struct UnifiedTrackingData {
    pub eyes: [Option<Vec3>; 2],
    pub shapes: [UnifiedExpressionShape; NUM_SHAPES],
    old_shapes: Option<[UnifiedExpressionShape; NUM_SHAPES]>,
    expression_tracking: bool,
    lip_tracking: bool,
}

impl Default for UnifiedTrackingData {
    fn default() -> Self {
        Self {
            eyes: [None, None],
            shapes: [0.0; NUM_SHAPES],
            old_shapes: None,
            expression_tracking: false,
            lip_tracking: false,
        }
    }
}

impl UnifiedTrackingData {
    #[inline(always)]
    pub fn getu(&self, exp: UnifiedExpressions) -> f32 {
        self.shapes[exp as usize]
    }

    #[inline(always)]
    pub fn getc(&self, exp: CombinedExpression) -> f32 {
        self.shapes[exp as usize]
    }

    #[inline(always)]
    pub fn setu(&mut self, exp: UnifiedExpressions, value: f32) {
        self.shapes[exp as usize] = value;
    }

    #[inline(always)]
    pub fn setc(&mut self, exp: CombinedExpression, value: f32) {
        self.shapes[exp as usize] = value;
    }

    pub fn calc_combined(&mut self, state: &mut AppState) {
        let left_eye_openness =
            (1. - self.getu(UnifiedExpressions::EyeClosedLeft) * 1.5).clamp(0., 1.);
        self.setc(
            CombinedExpression::EyeLidLeft,
            left_eye_openness * 0.75
                + self.getu(UnifiedExpressions::EyeWideLeft) * left_eye_openness * 0.25,
        );

        let right_eye_openness =
            (1. - self.getu(UnifiedExpressions::EyeClosedRight) * 1.5).clamp(0., 1.);
        self.setc(
            CombinedExpression::EyeLidRight,
            right_eye_openness * 0.75
                + self.getu(UnifiedExpressions::EyeWideRight) * right_eye_openness * 0.25,
        );

        self.setc(
            CombinedExpression::EyeLid,
            (self.getc(CombinedExpression::EyeLidLeft)
                + self.getc(CombinedExpression::EyeLidRight))
                * 0.5,
        );

        self.setc(
            CombinedExpression::EyeSquint,
            (self.getu(UnifiedExpressions::EyeSquintLeft)
                + self.getu(UnifiedExpressions::EyeSquintRight))
                * 0.5,
        );

        let brow_down_left = self.getu(UnifiedExpressions::BrowLowererLeft) * 0.75
            + self.getu(UnifiedExpressions::BrowPinchLeft) * 0.25;
        let brow_down_right = self.getu(UnifiedExpressions::BrowLowererRight) * 0.75
            + self.getu(UnifiedExpressions::BrowPinchRight) * 0.25;

        self.setc(CombinedExpression::BrowDownLeft, brow_down_left);
        self.setc(CombinedExpression::BrowDownRight, brow_down_right);

        let brow_outer_up = (self.getu(UnifiedExpressions::BrowOuterUpLeft)
            + self.getu(UnifiedExpressions::BrowOuterUpRight))
            * 0.5;
        self.setc(CombinedExpression::BrowOuterUp, brow_outer_up);

        let brow_inner_up = (self.getu(UnifiedExpressions::BrowInnerUpLeft)
            + self.getu(UnifiedExpressions::BrowInnerUpRight))
            * 0.5;
        self.setc(CombinedExpression::BrowInnerUp, brow_inner_up);

        self.setc(
            CombinedExpression::BrowUp,
            (brow_outer_up + brow_inner_up) * 0.5,
        );

        let brow_exp_left = (self.getu(UnifiedExpressions::BrowInnerUpLeft) * 0.5
            + self.getu(UnifiedExpressions::BrowOuterUpLeft) * 0.5)
            - brow_down_left;
        let brow_exp_right = (self.getu(UnifiedExpressions::BrowInnerUpRight) * 0.5
            + self.getu(UnifiedExpressions::BrowOuterUpRight) * 0.5)
            - brow_down_right;

        self.setc(CombinedExpression::BrowExpressionLeft, brow_exp_left);
        self.setc(CombinedExpression::BrowExpressionRight, brow_exp_right);
        self.setc(
            CombinedExpression::BrowExpression,
            (brow_exp_left + brow_exp_right) * 0.5,
        );

        let mouth_smile_left = self.getu(UnifiedExpressions::MouthCornerPullLeft) * 0.75
            + self.getu(UnifiedExpressions::MouthCornerSlantLeft) * 0.25;
        let mouth_smile_right = self.getu(UnifiedExpressions::MouthCornerPullRight) * 0.75
            + self.getu(UnifiedExpressions::MouthCornerSlantRight) * 0.25;

        let mouth_sad_left = self.getu(UnifiedExpressions::MouthFrownLeft) * 0.75
            + self.getu(UnifiedExpressions::MouthStretchLeft) * 0.25;
        let mouth_sad_right = self.getu(UnifiedExpressions::MouthFrownRight) * 0.75
            + self.getu(UnifiedExpressions::MouthStretchRight) * 0.25;

        self.setc(CombinedExpression::MouthSmileLeft, mouth_smile_left);
        self.setc(CombinedExpression::MouthSmileRight, mouth_smile_right);
        self.setc(CombinedExpression::MouthSadLeft, mouth_sad_left);
        self.setc(CombinedExpression::MouthSadRight, mouth_sad_right);

        self.setc(
            CombinedExpression::MouthUpperX,
            self.getu(UnifiedExpressions::MouthUpperRight)
                - self.getu(UnifiedExpressions::MouthUpperLeft),
        );

        self.setc(
            CombinedExpression::MouthLowerX,
            self.getu(UnifiedExpressions::MouthLowerRight)
                - self.getu(UnifiedExpressions::MouthLowerLeft),
        );

        self.setc(
            CombinedExpression::SmileSadLeft,
            mouth_smile_left - mouth_sad_left,
        );
        self.setc(
            CombinedExpression::SmileSadRight,
            mouth_smile_right - mouth_sad_right,
        );
        self.setc(
            CombinedExpression::SmileSad,
            (mouth_smile_left - mouth_sad_left + mouth_smile_right - mouth_sad_right) * 0.5,
        );
        self.setc(
            CombinedExpression::SmileFrownLeft,
            mouth_smile_left - self.getu(UnifiedExpressions::MouthFrownLeft),
        );
        self.setc(
            CombinedExpression::SmileFrownRight,
            mouth_smile_right - self.getu(UnifiedExpressions::MouthFrownRight),
        );
        self.setc(
            CombinedExpression::SmileFrown,
            (mouth_smile_left - self.getu(UnifiedExpressions::MouthFrownLeft) + mouth_smile_right
                - self.getu(UnifiedExpressions::MouthFrownRight))
                * 0.5,
        );
        self.setc(
            CombinedExpression::CheekPuffSuckLeft,
            self.getu(UnifiedExpressions::CheekPuffLeft)
                - self.getu(UnifiedExpressions::CheekSuckLeft),
        );
        self.setc(
            CombinedExpression::CheekPuffSuckRight,
            self.getu(UnifiedExpressions::CheekPuffRight)
                - self.getu(UnifiedExpressions::CheekSuckRight),
        );
        self.setc(
            CombinedExpression::CheekPuffSuck,
            (self.getu(UnifiedExpressions::CheekPuffLeft)
                + self.getu(UnifiedExpressions::CheekPuffRight)
                - self.getu(UnifiedExpressions::CheekSuckLeft)
                - self.getu(UnifiedExpressions::CheekSuckRight))
                * 0.5,
        );

        self.setc(
            CombinedExpression::CheekSquint,
            (self.getu(UnifiedExpressions::CheekSquintLeft)
                + self.getu(UnifiedExpressions::CheekSquintRight))
                * 0.5,
        );

        self.setc(
            CombinedExpression::LipSuckUpper,
            (self.getu(UnifiedExpressions::LipSuckUpperLeft)
                + self.getu(UnifiedExpressions::LipSuckUpperRight))
                * 0.5,
        );
        self.setc(
            CombinedExpression::LipSuckLower,
            (self.getu(UnifiedExpressions::LipSuckLowerLeft)
                + self.getu(UnifiedExpressions::LipSuckLowerRight))
                * 0.5,
        );
        self.setc(
            CombinedExpression::LipSuck,
            (self.getu(UnifiedExpressions::LipSuckLowerLeft)
                + self.getu(UnifiedExpressions::LipSuckLowerRight)
                + self.getu(UnifiedExpressions::LipSuckUpperLeft)
                + self.getu(UnifiedExpressions::LipSuckUpperRight))
                * 0.25,
        );
        self.setc(
            CombinedExpression::MouthStretchTightenLeft,
            self.getu(UnifiedExpressions::MouthStretchLeft)
                - self.getu(UnifiedExpressions::MouthTightenerLeft),
        );

        self.setc(
            CombinedExpression::MouthStretchTightenRight,
            self.getu(UnifiedExpressions::MouthStretchRight)
                - self.getu(UnifiedExpressions::MouthTightenerRight),
        );

        self.setc(
            CombinedExpression::MouthStretch,
            (self.getu(UnifiedExpressions::MouthStretchLeft)
                + self.getu(UnifiedExpressions::MouthStretchRight))
                * 0.5,
        );

        self.setc(
            CombinedExpression::MouthTightener,
            (self.getu(UnifiedExpressions::MouthTightenerLeft)
                + self.getu(UnifiedExpressions::MouthTightenerRight))
                * 0.5,
        );

        self.setc(
            CombinedExpression::MouthDimple,
            (self.getu(UnifiedExpressions::MouthDimpleLeft)
                + self.getu(UnifiedExpressions::MouthDimpleRight))
                * 0.5,
        );

        let mouth_upper_up = (self.getu(UnifiedExpressions::MouthUpperUpLeft)
            + self.getu(UnifiedExpressions::MouthUpperUpRight))
            * 0.5;
        let mouth_lower_down = (self.getu(UnifiedExpressions::MouthLowerDownLeft)
            + self.getu(UnifiedExpressions::MouthLowerDownRight))
            * 0.5;

        self.setc(CombinedExpression::MouthUpperUp, mouth_upper_up);
        self.setc(CombinedExpression::MouthLowerDown, mouth_lower_down);
        self.setc(
            CombinedExpression::MouthOpen,
            (mouth_upper_up + mouth_lower_down) * 0.5,
        );
        self.setc(
            CombinedExpression::MouthX,
            (self.getu(UnifiedExpressions::MouthUpperRight)
                + self.getu(UnifiedExpressions::MouthLowerRight)
                - self.getu(UnifiedExpressions::MouthUpperLeft)
                - self.getu(UnifiedExpressions::MouthLowerLeft))
                * 0.5,
        );
        self.setc(
            CombinedExpression::JawX,
            self.getu(UnifiedExpressions::JawRight) - self.getu(UnifiedExpressions::JawLeft),
        );
        self.setc(
            CombinedExpression::JawZ,
            self.getu(UnifiedExpressions::JawForward) - self.getu(UnifiedExpressions::JawBackward),
        );
        let lip_pucker_left = (self.getu(UnifiedExpressions::LipPuckerLowerLeft)
            + self.getu(UnifiedExpressions::LipPuckerUpperLeft))
            * 0.5;
        let lip_pucker_right = (self.getu(UnifiedExpressions::LipPuckerLowerRight)
            + self.getu(UnifiedExpressions::LipPuckerUpperRight))
            * 0.5;
        self.setc(
            CombinedExpression::LipPucker,
            (lip_pucker_left + lip_pucker_right) * 0.5,
        );
        let lip_funnel_upper = (self.getu(UnifiedExpressions::LipFunnelUpperLeft)
            + self.getu(UnifiedExpressions::LipFunnelUpperRight))
            * 0.5;
        let lip_funnel_lower = (self.getu(UnifiedExpressions::LipFunnelLowerLeft)
            + self.getu(UnifiedExpressions::LipFunnelLowerRight))
            * 0.5;

        self.setc(CombinedExpression::LipFunnelUpper, lip_funnel_upper);
        self.setc(CombinedExpression::LipFunnelLower, lip_funnel_lower);
        self.setc(
            CombinedExpression::LipFunnel,
            (lip_funnel_upper + lip_funnel_lower) * 0.5,
        );

        self.setc(
            CombinedExpression::MouthPress,
            (self.getu(UnifiedExpressions::MouthPressLeft)
                + self.getu(UnifiedExpressions::MouthPressRight))
                * 0.5,
        );

        self.setc(
            CombinedExpression::NoseSneer,
            (self.getu(UnifiedExpressions::NoseSneerLeft)
                + self.getu(UnifiedExpressions::NoseSneerRight))
                * 0.5,
        );

        self.setc(
            CombinedExpression::EarLeft,
            (self.getu(UnifiedExpressions::BrowInnerUpLeft)
                + self.getu(UnifiedExpressions::EyeWideLeft)
                - self.getu(UnifiedExpressions::EyeSquintLeft)
                - self.getu(UnifiedExpressions::BrowPinchLeft))
            .clamp(-1.0, 1.0),
        );

        self.setc(
            CombinedExpression::EarRight,
            (self.getu(UnifiedExpressions::BrowInnerUpLeft)
                + self.getu(UnifiedExpressions::EyeWideRight)
                - self.getu(UnifiedExpressions::EyeSquintRight)
                - self.getu(UnifiedExpressions::BrowPinchRight))
            .clamp(-1.0, 1.0),
        );

        // Custom stuff
        let blush_face = match state.params.get("BlushFace") {
            Some(OscType::Float(f)) => *f > 0.1,
            _ => false,
        };
        let blush_nade = match state.params.get("BlushNade") {
            Some(OscType::Float(f)) => *f > 0.1,
            _ => false,
        };
        let blush_eye = self.eyes[0].map(|e| e.y).unwrap_or(0.0) > 0.25;

        let rate = if blush_face || blush_nade || blush_eye {
            0.10
        } else {
            -0.05
        };

        let old_blush = self.getc(CombinedExpression::Blush);
        let new_blush = (old_blush + rate * state.delta_t).clamp(0.0, 1.0);
        self.setc(CombinedExpression::Blush, new_blush);
    }

    fn dirty_shapes(&self) -> Vec<usize> {
        let mut dirty = Vec::new();

        if let Some(old_shapes) = self.old_shapes.as_ref() {
            for (i, item) in old_shapes.iter().enumerate().take(NUM_SHAPES) {
                if (self.shapes[i] - item).abs() > 0.01 {
                    dirty.push(i);
                }
            }
        } else {
            dirty.extend(0..NUM_SHAPES);
        }
        dirty
    }

    pub fn apply_to_bundle(
        &mut self,
        params: &mut [Option<MysteryParam>; NUM_SHAPES],
        bundle: &mut OscBundle,
    ) {
        if !self.expression_tracking {
            bundle.send_parameter("ExpressionTrackingActive", OscType::Bool(true));
            self.expression_tracking = true;
        }
        if !self.lip_tracking {
            bundle.send_parameter("LipTrackingActive", OscType::Bool(true));
            self.lip_tracking = true;
        }
        //bundle.send_parameter("EyeTrackingActive", OscType::Bool(true));

        for (idx, shape) in self.shapes.iter().enumerate() {
            if let Some(param) = &mut params[idx] {
                param.send(*shape, bundle);
            }
        }
        self.old_shapes = Some(self.shapes);

        if let Some(left_euler) = self.eyes[0] {
            if params[CombinedExpression::EyeLidLeft as usize].is_none() {
                // in case avatar doesn't support separate eye closed
                bundle.send_tracking(
                    "/tracking/eye/EyesClosedAmount",
                    vec![OscType::Float(self.getu(UnifiedExpressions::EyeClosedLeft))],
                );
            }
            let right_euler = self.eyes[1].unwrap_or(left_euler);

            bundle.send_tracking(
                "/tracking/eye/LeftRightPitchYaw",
                vec![
                    OscType::Float(-left_euler.x.to_degrees()),
                    OscType::Float(-left_euler.y.to_degrees()),
                    OscType::Float(-right_euler.x.to_degrees()),
                    OscType::Float(-right_euler.y.to_degrees()),
                ],
            );
        }
    }
}

pub const NUM_SHAPES: usize = UnifiedExpressions::COUNT + CombinedExpression::COUNT;

#[allow(unused)]
#[repr(usize)]
#[derive(Debug, Clone, Copy, EnumIter, EnumCount, EnumString, IntoStaticStr)]
pub enum UnifiedExpressions {
    EyeLeftX,
    EyeRightX,
    EyeY,

    // 'Biometrically' accurate data that is included with UnifiedEye
    EyeClosedRight, // Closes the right eyelid. Basis on the overall constriction of the palpebral part of orbicularis oculi.
    EyeClosedLeft, // Closes the left eyelid. Basis on the overall constriction of the palpebral part of orbicularis oculi.
    //EyeDilationRight, // Dilates the right eye's pupil
    //EyeDilationLeft, // Dilates the left eye's pupil
    //EyeConstrictRight, // Constricts the right eye's pupil
    //EyeConstrictLeft, // Constricts the left eye's pupil
    EyeSquintRight, // Squeezes the right eye socket muscles, causing the lower eyelid to constrict a little bit as well. Basis on the mostly lower constriction of the inner parts of the orbicularis oculi and the stressing of the muscle group as the eyelid is closed.
    EyeSquintLeft, // Squeezes the left eye socket muscles, causing the lower eyelid to constrict a little bit as well. Basis on the mostly lower constriction of the inner parts of the orbicularis oculi and the stressing of the muscle group as the eyelid is closed.
    EyeWideRight, // Right eyelid widens beyond the eyelid's relaxed position. Basis on the action of the levator palpebrae superioris.
    EyeWideLeft, // Left eyelid widens beyond the eyelid's relaxed position. Basis on the action of the levator palpebrae superioris.

    BrowPinchRight, // Inner right eyebrow pulls diagnally inwards and downwards slightly. Basis on the constriction of the corrugator supercilii muscle.
    BrowPinchLeft, // Inner left eyebrow pulls diagnally inwards and downwards slightly. Basis on the constriction of the corrugator supercilii muscle.
    BrowLowererRight, // Outer right eyebrow pulls downward. Basis on depressor supercilii, procerus, and partially the upper orbicularis oculi muscles action of lowering the eyebrow.
    BrowLowererLeft, // Outer Left eyebrow pulls downward. Basis on depressor supercilii, procerus, and partially the upper orbicularis oculi muscles action of lowering the eyebrow.
    BrowInnerUpRight, // Inner right eyebrow pulls upward. Basis on the inner grouping action of the frontal belly of the occipitofrontalis.
    BrowInnerUpLeft, // Inner left eyebrow pulls upward. Basis on the inner grouping action of the frontal belly of the occipitofrontalis.
    BrowOuterUpRight, // Outer right eyebrow pulls upward. Basis on the outer grouping action of the frontal belly of the occipitofrontalis.
    BrowOuterUpLeft, // Outer left eyebrow pulls upward. Basis on the outer grouping action of the frontal belly of the occipitofrontalis.

    NasalDilationRight, // Right side nose's canal dilates. Basis on the alar nasalis muscle.
    NasalDilationLeft,  // Left side nose's canal dilates. Basis on the alar nasalis muscle.
    NasalConstrictRight, // Right side nose's canal constricts. Basis on the transverse nasalis muscle.
    NasalConstrictLeft, // Left side nose's canal constricts. Basis on the transverse nasalis muscle.

    CheekSquintRight, // Raises the right side cheek. Basis on the main action of the lower outer part of the orbicularis oculi.
    CheekSquintLeft, // Raises the left side cheek. Basis on the main action of the lower outer part of the orbicularis oculi.
    CheekPuffRight, // Puffs the right side cheek. Basis on the cheeks' ability to stretch orbitally.
    CheekPuffLeft,  // Puffs the left side cheek. Basis on the cheeks' ability to stretch orbitally.
    CheekSuckRight, // Sucks in the right side cheek. Basis on the cheeks' ability to stretch inwards from suction.
    CheekSuckLeft, // Sucks in the left side cheek. Basis on the cheeks' ability to stretch inwards from suction.

    JawOpen, // Opens the jawbone. Basis of the general action of the jaw opening by the masseter and temporalis muscle grouping.
    JawRight, // Pushes the jawbone right. Basis on medial pterygoid and lateral pterygoid's general action of shifting the jaw sideways.
    JawLeft, // Pushes the jawbone left. Basis on medial pterygoid and lateral pterygoid's general action of shifting the jaw sideways.
    JawForward, // Pushes the jawbone forward. Basis on the lateral pterygoid's ability to shift the jaw forward.
    JawBackward, // Pulls the jawbone backwards slightly. Based on the retraction of the temporalis muscle.
    JawClench, // Specific jaw muscles that forces the jaw closed. Causes the masseter muscle (visible close to the back of the jawline) to visibly flex.
    JawMandibleRaise, // Raises mandible (jawbone).

    MouthClosed, // Closes the mouth relative to JawOpen. Basis on the complex tightening action of the orbicularis oris muscle.

    // 'Lip Push/Pull' group
    LipSuckUpperRight, // Upper right part of the lip gets tucked inside the mouth. No direct muscle basis as this action is caused from many indirect movements of tucking the lips.
    LipSuckUpperLeft, // Upper left part of the lip gets tucked inside the mouth. No direct muscle basis as this action is caused from many indirect movements of tucking the lips.
    LipSuckLowerRight, // Lower right part of the lip gets tucked inside the mouth. No direct muscle basis as this action is caused from many indirect movements of tucking the lips.
    LipSuckLowerLeft, // Lower left part of the lip gets tucked inside the mouth. No direct muscle basis as this action is caused from many indirect movements of tucking the lips.

    LipSuckCornerRight, // The right corners of the lips fold inward and into the mouth. Basis on the lips ability to stretch inwards from suction.
    LipSuckCornerLeft, // The left corners of the lips fold inward and into the mouth. Basis on the lips ability to stretch inwards from suction.

    LipFunnelUpperRight, // Upper right part of the lip pushes outward into a funnel shape. Basis on the orbicularis oris orbital muscle around the mouth pushing outwards and puckering.
    LipFunnelUpperLeft, // Upper left part of the lip pushes outward into a funnel shape. Basis on the orbicularis oris orbital muscle around the mouth pushing outwards and puckering.
    LipFunnelLowerRight, // Lower right part of the lip pushes outward into a funnel shape. Basis on the orbicularis oris orbital muscle around the mouth pushing outwards and puckering.
    LipFunnelLowerLeft, // Lower left part of the lip pushes outward into a funnel shape. Basis on the orbicularis oris orbital muscle around the mouth pushing outwards and puckering.

    LipPuckerUpperRight, // Upper right part of the lip pinches inward and pushes outward. Basis on complex action of the orbicularis-oris orbital muscle around the lips.
    LipPuckerUpperLeft, // Upper left part of the lip pinches inward and pushes outward. Basis on complex action of the orbicularis-oris orbital muscle around the lips.
    LipPuckerLowerRight, // Lower right part of the lip pinches inward and pushes outward. Basis on complex action of the orbicularis-oris orbital muscle around the lips.
    LipPuckerLowerLeft, // Lower left part of the lip pinches inward and pushes outward. Basis on complex action of the orbicularis-oris orbital muscle around the lips.

    // 'Upper lip raiser' group
    MouthUpperUpRight, // Upper right part of the lip is pulled upward. Basis on the levator labii superioris muscle.
    MouthUpperUpLeft, // Upper left part of the lip is pulled upward. Basis on the levator labii superioris muscle.
    MouthUpperDeepenRight, // Upper outer right part of the lip is pulled upward, backward, and rightward. Basis on the zygomaticus minor muscle.
    MouthUpperDeepenLeft, // Upper outer left part of the lip is pulled upward, backward, and rightward. Basis on the zygomaticus minor muscle.
    NoseSneerRight, // The right side face pulls upward into a sneer and raises the inner part of the lips at extreme ranges. Based on levator labii superioris alaeque nasi muscle.
    NoseSneerLeft, // The right side face pulls upward into a sneer and raises the inner part of the lips slightly at extreme ranges. Based on levator labii superioris alaeque nasi muscle.

    // 'Lower lip depressor' group
    MouthLowerDownRight, // Lower right part of the lip is pulled downward. Basis on the depressor labii inferioris muscle.
    MouthLowerDownLeft, // Lower left part of the lip is pulled downward. Basis on the depressor labii inferioris muscle.

    // 'Mouth Direction' group
    MouthUpperRight, // Moves upper lip right. Basis on the general horizontal movement action of the upper orbicularis oris orbital, levator anguli oris, and buccinator muscle grouping.
    MouthUpperLeft, // Moves upper lip left. Basis on the general horizontal movement action of the upper orbicularis oris orbital, levator anguli oris, and buccinator muscle grouping.
    MouthLowerRight, // Moves lower lip right. Basis on the general horizontal movement action of the lower orbicularis oris orbital, risorius, depressor labii inferioris, and buccinator muscle grouping.
    MouthLowerLeft, // Moves lower lip left. Basis on the general horizontal movement action of the lower orbicularis oris orbital, risorius, depressor labii inferioris, and buccinator muscle grouping.

    // 'Smile' group
    MouthCornerPullRight, // Right side of the lip is pulled diagnally upwards and rightwards significantly. Basis on the action of the levator anguli oris muscle.
    MouthCornerPullLeft, // :eft side of the lip is pulled diagnally upwards and leftwards significantly. Basis on the action of the levator anguli oris muscle.
    MouthCornerSlantRight, // Right corner of the lip is pulled upward slightly. Basis on the action of the levator anguli oris muscle.
    MouthCornerSlantLeft, // Left corner of the lip is pulled upward slightly. Basis on the action of the levator anguli oris muscle.

    // 'Sad' group
    MouthFrownRight, // Right corner of the lip is pushed downward. Basis on the action of the depressor anguli oris muscle. Directly opposes the levator muscles.
    MouthFrownLeft, // Left corner of the lip is pushed downward. Basis on the action of the depressor anguli oris muscle. Directly opposes the levator muscles.
    MouthStretchRight, // Stretches the right side lips together horizontally and thins them vertically slightly. Basis on the risorius muscle.
    MouthStretchLeft, // Stretches the left side lips together horizontally and thins them vertically slightly. Basis on the risorius muscle.

    MouthDimpleRight, // Right corner of the lip is pushed backwards into the face, creating a dimple. Basis on buccinator muscle structure.
    MouthDimpleLeft, // Left corner of the lip is pushed backwards into the face, creating a dimple. Basis on buccinator muscle structure.

    MouthRaiserUpper, // Raises the upper part of the mouth in response to MouthRaiserLower. No muscular basis.
    MouthRaiserLower, // Raises the lower part of the mouth. Based on the complex lower pushing action of the mentalis muscle.
    MouthPressRight, // Squeezes the right side lips together vertically and flattens them. Basis on the complex tightening action of the orbicularis oris muscle.
    MouthPressLeft, // Squeezes the left side lips together vertically and flattens them. Basis on the complex tightening action of the orbicularis oris muscle.
    MouthTightenerRight, // Squeezes the right side lips together horizontally and thickens them vertically slightly. Basis on the complex tightening action of the orbicularis oris muscle.
    MouthTightenerLeft, // Squeezes the right side lips together horizontally and thickens them vertically slightly. Basis on the complex tightening action of the orbicularis oris muscle.

    TongueOut, // Combined LongStep1 and LongStep2 into one shape, as it can be emulated in-animation

    // Based on SRanipal tracking standard's tongue tracking.
    TongueUp,    // Tongue points in an upward direction.
    TongueDown,  // Tongue points in an downward direction.
    TongueRight, // Tongue points in an rightward direction.
    TongueLeft,  // Tongue points in an leftward direction.

    // Based on https://www.naun.org/main/NAUN/computers/2018/a042007-060.pdf
    TongueRoll,     // Rolls up the sides of the tongue into a 'hotdog bun' shape.
    TongueBendDown, // Pushes tip of the tongue below the rest of the tongue in an arch.
    TongueCurlUp,   // Pushes tip of the tongue above the rest of the tongue in an arch.
    TongueSquish,   // Tongue becomes thinner width-wise and slightly thicker height-wise.
    TongueFlat,     // Tongue becomes thicker width-wise and slightly thinner height-wise.

    TongueTwistRight, // Tongue tip rotates clockwise from POV with the rest of the tongue following gradually.
    TongueTwistLeft, // Tongue tip rotates counter-clockwise from POV with the rest of the tongue following gradually.
}

#[allow(unused)]
#[repr(usize)]
#[derive(Debug, Clone, Copy, EnumIter, EnumCount, EnumString, IntoStaticStr)]
pub enum CombinedExpression {
    EyeLidLeft = UnifiedExpressions::COUNT,
    EyeLidRight,
    EyeLid,
    EyeSquint,
    JawX,
    JawZ,
    BrowDownLeft,
    BrowDownRight,
    BrowOuterUp,
    BrowInnerUp,
    BrowUp,
    BrowExpressionLeft,
    BrowExpressionRight,
    BrowExpression,
    MouthX,
    MouthUpperX,
    MouthLowerX,
    MouthUpperUp,
    MouthLowerDown,
    MouthOpen,
    MouthSmileLeft,
    MouthSmileRight,
    MouthSadLeft,
    MouthSadRight,
    MouthStretchTightenLeft,
    MouthStretchTightenRight,
    MouthStretch,
    MouthTightener,
    MouthDimple,
    MouthPress,
    SmileFrownLeft,
    SmileFrownRight,
    SmileFrown,
    SmileSadLeft,
    SmileSadRight,
    SmileSad,
    LipSuckUpper,
    LipSuckLower,
    LipSuck,
    LipFunnelUpper,
    LipFunnelLower,
    LipFunnel,
    LipPuckerUpper,
    LipPuckerLower,
    LipPucker,
    NoseSneer,
    CheekPuffSuckLeft,
    CheekPuffSuckRight,
    CheekPuffSuck,
    CheekSquint,

    // Non-standard
    EarLeft,
    EarRight,
    Blush,
}
