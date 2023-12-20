use std::io::{Cursor, Read};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::str::FromStr;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::Arc;
use std::time::Duration;
use std::{array, slice, thread};

use super::bundle::AvatarBundle;
use super::ext_oscjson::{MysteryParam, OscJsonNode};
use super::AvatarParameters;
use glam::{Quat, Vec2, Vec3};
use log::{debug, info, warn};
use once_cell::sync::Lazy;
use regex::Regex;
use rosc::{OscBundle, OscType};
use strum::{EnumCount, EnumIter, EnumString, IntoStaticStr};

enum TrackingMessage {
    FaceFb([f32; 63]),
    PoseHead(Pose),
    PoseConL(Pose),
    PoseConR(Pose),
    HandSkeL([Pose; 26]),
    HandSkeR([Pose; 26]),
    EyesQuat([Quat; 2]),
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Pose {
    pub orientation: Quat, // NB: default Quat is identity
    pub position: Vec3,
}

impl Pose {
    pub fn from_arr(arr: &[f32]) -> Self {
        Self {
            orientation: Quat::from_xyzw(arr[0], arr[1], arr[2], arr[3]),
            position: Vec3::new(arr[4], arr[5], arr[6]),
        }
    }
}

#[allow(unused)]
#[repr(usize)]
#[derive(Debug, Clone, Copy, EnumIter, EnumCount, EnumString, IntoStaticStr)]
pub enum UnifiedExpressions {
    // These are currently unused for expressions and used in the UnifiedEye structure.
    // EyeLookOutRight,
    // EyeLookInRight,
    // EyeLookUpRight,
    // EyeLookDownRight,
    // EyeLookOutLeft,
    // EyeLookInLeft,
    // EyeLookUpLeft,
    // EyeLookDownLeft,

    // 'Biometrically' accurate data that is included with UnifiedEye
    //EyeClosedRight, // Closes the right eyelid. Basis on the overall constriction of the palpebral part of orbicularis oculi.
    //EyeClosedLeft, // Closes the left eyelid. Basis on the overall constriction of the palpebral part of orbicularis oculi.
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

                        /*
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

                        SoftPalateClose, // Visibly lowers the back of the throat (soft palate) inside the mouth to close off the throat.
                        ThroatSwallow, // Visibly causes the Adam's apple to pull upward into the throat as if swallowing.

                        NeckFlexRight, // Flexes the Right side of the neck and face (causes the right corner of the face to stretch towards.)
                        NeckFlexLeft, // Flexes the left side of the neck and face (causes the left corner of the face to stretch towards.)
                        */
}

#[allow(unused)]
#[repr(usize)]
#[derive(Debug, Clone, Copy, EnumIter, EnumCount, EnumString, IntoStaticStr)]
pub enum CombinedExpression {
    EyeLidLeft,
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
    MouthStretch,   //TODO verify
    MouthTightener, //TODO verify
    MouthDimple,    // TODO verify
    MouthPress,     // TODO verify
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
}
const NUM_SHAPES: usize = UnifiedExpressions::COUNT + CombinedExpression::COUNT;

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
}

#[derive(Debug, Default, Clone)]
pub struct UnifiedEyeDataSingle {
    pub gaze: Vec2,
    pub pupil_diameter_mm: f32,
    pub openness: f32,
}

#[derive(Debug, Default, Clone)]
pub struct UnifiedEyeData {
    pub left: UnifiedEyeDataSingle,
    pub right: UnifiedEyeDataSingle,
    pub combined: UnifiedEyeDataSingle,
    pub max_dilation: f32,
    pub min_dilation: f32,
}

pub type UnifiedExpressionShape = f32;

#[derive(Debug, Clone)]
pub struct UnifiedTrackingData {
    pub eye: UnifiedEyeData,
    pub shapes: [UnifiedExpressionShape; NUM_SHAPES],
}

impl UnifiedTrackingData {
    pub fn new() -> Self {
        Self {
            eye: UnifiedEyeData::default(),
            shapes: [0.0; NUM_SHAPES],
        }
    }

    pub fn load_face(&mut self, face_fb: &[f32; 63]) {
        self.eye.left.openness = 1.
            - (face_fb[FaceFb::EyesClosedL as usize]) //+ face_fb[FaceFb::EyesClosedL as usize] * face_fb[FaceFb::LidTightenerL as usize])
                .clamp(0.0, 1.0);
        self.eye.right.openness = 1.
            - (face_fb[FaceFb::EyesClosedR as usize]) //+ face_fb[FaceFb::EyesClosedR as usize] * face_fb[FaceFb::LidTightenerR as usize])
                .clamp(0.0, 1.0);

        self.eye.left.gaze.x = (face_fb[FaceFb::EyesLookRightL as usize]
            - face_fb[FaceFb::EyesLookLeftL as usize])
            * 0.5;
        self.eye.left.gaze.y =
            (face_fb[FaceFb::EyesLookUpL as usize] - face_fb[FaceFb::EyesLookDownL as usize]) * 0.5;
        self.eye.right.gaze.x = (face_fb[FaceFb::EyesLookRightR as usize]
            - face_fb[FaceFb::EyesLookLeftR as usize])
            * 0.5;
        self.eye.right.gaze.y =
            (face_fb[FaceFb::EyesLookUpR as usize] - face_fb[FaceFb::EyesLookDownR as usize]) * 0.5;
        self.eye.combined.gaze = (self.eye.left.gaze + self.eye.right.gaze) * 0.5;

        self.eye.left.pupil_diameter_mm = 0.5;
        self.eye.right.pupil_diameter_mm = 0.5;

        self.shapes[UnifiedExpressions::EyeSquintRight as usize] =
            face_fb[FaceFb::LidTightenerR as usize] - face_fb[FaceFb::EyesClosedR as usize];
        self.shapes[UnifiedExpressions::EyeSquintLeft as usize] =
            face_fb[FaceFb::LidTightenerL as usize] - face_fb[FaceFb::EyesClosedL as usize];
        self.shapes[UnifiedExpressions::EyeWideRight as usize] =
            face_fb[FaceFb::UpperLidRaiserR as usize];
        self.shapes[UnifiedExpressions::EyeWideLeft as usize] =
            face_fb[FaceFb::UpperLidRaiserL as usize];

        self.shapes[UnifiedExpressions::BrowPinchRight as usize] =
            face_fb[FaceFb::BrowLowererR as usize];
        self.shapes[UnifiedExpressions::BrowPinchLeft as usize] =
            face_fb[FaceFb::BrowLowererL as usize];
        self.shapes[UnifiedExpressions::BrowLowererRight as usize] =
            face_fb[FaceFb::BrowLowererR as usize];
        self.shapes[UnifiedExpressions::BrowLowererLeft as usize] =
            face_fb[FaceFb::BrowLowererL as usize];
        self.shapes[UnifiedExpressions::BrowInnerUpRight as usize] =
            face_fb[FaceFb::InnerBrowRaiserR as usize];
        self.shapes[UnifiedExpressions::BrowInnerUpLeft as usize] =
            face_fb[FaceFb::InnerBrowRaiserL as usize];
        self.shapes[UnifiedExpressions::BrowOuterUpRight as usize] =
            face_fb[FaceFb::OuterBrowRaiserR as usize];
        self.shapes[UnifiedExpressions::BrowOuterUpLeft as usize] =
            face_fb[FaceFb::OuterBrowRaiserL as usize];

        self.shapes[UnifiedExpressions::CheekSquintRight as usize] =
            face_fb[FaceFb::CheekRaiserR as usize];
        self.shapes[UnifiedExpressions::CheekSquintLeft as usize] =
            face_fb[FaceFb::CheekRaiserL as usize];
        self.shapes[UnifiedExpressions::CheekPuffRight as usize] =
            face_fb[FaceFb::CheekPuffR as usize];
        self.shapes[UnifiedExpressions::CheekPuffLeft as usize] =
            face_fb[FaceFb::CheekPuffL as usize];
        self.shapes[UnifiedExpressions::CheekSuckRight as usize] =
            face_fb[FaceFb::CheekSuckR as usize];
        self.shapes[UnifiedExpressions::CheekSuckLeft as usize] =
            face_fb[FaceFb::CheekSuckL as usize];

        self.shapes[UnifiedExpressions::JawOpen as usize] = face_fb[FaceFb::JawDrop as usize];
        self.shapes[UnifiedExpressions::JawRight as usize] =
            face_fb[FaceFb::JawSidewaysRight as usize];
        self.shapes[UnifiedExpressions::JawLeft as usize] =
            face_fb[FaceFb::JawSidewaysLeft as usize];
        self.shapes[UnifiedExpressions::JawForward as usize] = face_fb[FaceFb::JawThrust as usize];
        self.shapes[UnifiedExpressions::MouthClosed as usize] =
            face_fb[FaceFb::LipsToward as usize];

        self.shapes[UnifiedExpressions::LipSuckUpperRight as usize] = (1.0
            - face_fb[FaceFb::UpperLipRaiserR as usize].powf(0.1666))
        .min(face_fb[FaceFb::LipSuckRT as usize]);
        self.shapes[UnifiedExpressions::LipSuckUpperLeft as usize] = (1.0
            - face_fb[FaceFb::UpperLipRaiserL as usize].powf(0.1666))
        .min(face_fb[FaceFb::LipSuckLT as usize]);

        self.shapes[UnifiedExpressions::LipSuckLowerRight as usize] =
            face_fb[FaceFb::LipSuckRB as usize];
        self.shapes[UnifiedExpressions::LipSuckLowerLeft as usize] =
            face_fb[FaceFb::LipSuckLB as usize];
        self.shapes[UnifiedExpressions::LipFunnelUpperRight as usize] =
            face_fb[FaceFb::LipFunnelerRT as usize];
        self.shapes[UnifiedExpressions::LipFunnelUpperLeft as usize] =
            face_fb[FaceFb::LipFunnelerLT as usize];
        self.shapes[UnifiedExpressions::LipFunnelLowerRight as usize] =
            face_fb[FaceFb::LipFunnelerRB as usize];
        self.shapes[UnifiedExpressions::LipFunnelLowerLeft as usize] =
            face_fb[FaceFb::LipFunnelerLB as usize];
        self.shapes[UnifiedExpressions::LipPuckerUpperRight as usize] =
            face_fb[FaceFb::LipPuckerR as usize];
        self.shapes[UnifiedExpressions::LipPuckerUpperLeft as usize] =
            face_fb[FaceFb::LipPuckerL as usize];
        self.shapes[UnifiedExpressions::LipPuckerLowerRight as usize] =
            face_fb[FaceFb::LipPuckerR as usize];
        self.shapes[UnifiedExpressions::LipPuckerLowerLeft as usize] =
            face_fb[FaceFb::LipPuckerL as usize];

        self.shapes[UnifiedExpressions::NoseSneerRight as usize] =
            face_fb[FaceFb::NoseWrinklerR as usize];
        self.shapes[UnifiedExpressions::NoseSneerLeft as usize] =
            face_fb[FaceFb::NoseWrinklerL as usize];

        self.shapes[UnifiedExpressions::MouthLowerDownRight as usize] =
            face_fb[FaceFb::LowerLipDepressorR as usize];
        self.shapes[UnifiedExpressions::MouthLowerDownLeft as usize] =
            face_fb[FaceFb::LowerLipDepressorL as usize];

        let mouth_upper_up_right = face_fb[FaceFb::UpperLipRaiserR as usize];
        let mouth_upper_up_left = face_fb[FaceFb::UpperLipRaiserL as usize];

        self.shapes[UnifiedExpressions::MouthUpperUpRight as usize] = mouth_upper_up_right;
        self.shapes[UnifiedExpressions::MouthUpperUpLeft as usize] = mouth_upper_up_left;
        self.shapes[UnifiedExpressions::MouthUpperDeepenRight as usize] = mouth_upper_up_right;
        self.shapes[UnifiedExpressions::MouthUpperDeepenLeft as usize] = mouth_upper_up_left;

        self.shapes[UnifiedExpressions::MouthUpperRight as usize] =
            face_fb[FaceFb::MouthRight as usize];
        self.shapes[UnifiedExpressions::MouthUpperLeft as usize] =
            face_fb[FaceFb::MouthLeft as usize];
        self.shapes[UnifiedExpressions::MouthLowerRight as usize] =
            face_fb[FaceFb::MouthRight as usize];
        self.shapes[UnifiedExpressions::MouthLowerLeft as usize] =
            face_fb[FaceFb::MouthLeft as usize];

        self.shapes[UnifiedExpressions::MouthCornerPullRight as usize] =
            face_fb[FaceFb::LipCornerPullerR as usize];
        self.shapes[UnifiedExpressions::MouthCornerPullLeft as usize] =
            face_fb[FaceFb::LipCornerPullerL as usize];
        self.shapes[UnifiedExpressions::MouthCornerSlantRight as usize] =
            face_fb[FaceFb::LipCornerPullerR as usize];
        self.shapes[UnifiedExpressions::MouthCornerSlantLeft as usize] =
            face_fb[FaceFb::LipCornerPullerL as usize];

        self.shapes[UnifiedExpressions::MouthFrownRight as usize] =
            face_fb[FaceFb::LipCornerDepressorR as usize];
        self.shapes[UnifiedExpressions::MouthFrownLeft as usize] =
            face_fb[FaceFb::LipCornerDepressorL as usize];
        self.shapes[UnifiedExpressions::MouthStretchRight as usize] =
            face_fb[FaceFb::LipStretcherR as usize];
        self.shapes[UnifiedExpressions::MouthStretchLeft as usize] =
            face_fb[FaceFb::LipStretcherL as usize];

        self.shapes[UnifiedExpressions::MouthDimpleLeft as usize] =
            (face_fb[FaceFb::DimplerL as usize] * 2.0).min(1.0);
        self.shapes[UnifiedExpressions::MouthDimpleRight as usize] =
            (face_fb[FaceFb::DimplerR as usize] * 2.0).min(1.0);

        self.shapes[UnifiedExpressions::MouthRaiserUpper as usize] =
            face_fb[FaceFb::ChinRaiserT as usize];
        self.shapes[UnifiedExpressions::MouthRaiserLower as usize] =
            face_fb[FaceFb::ChinRaiserB as usize];
        self.shapes[UnifiedExpressions::MouthPressRight as usize] =
            face_fb[FaceFb::LipPressorR as usize];
        self.shapes[UnifiedExpressions::MouthPressLeft as usize] =
            face_fb[FaceFb::LipPressorL as usize];
        self.shapes[UnifiedExpressions::MouthTightenerRight as usize] =
            face_fb[FaceFb::LipTightenerR as usize];
        self.shapes[UnifiedExpressions::MouthTightenerLeft as usize] =
            face_fb[FaceFb::LipTightenerL as usize];

        // Combined
        let z = UnifiedExpressions::COUNT;
        self.shapes[z + CombinedExpression::EyeLidLeft as usize] = self.eye.left.openness * 0.75
            + self.shapes[UnifiedExpressions::EyeWideLeft as usize] * 0.25;

        self.shapes[z + CombinedExpression::EyeLidRight as usize] = self.eye.right.openness * 0.75
            + self.shapes[UnifiedExpressions::EyeWideRight as usize] * 0.25;

        self.shapes[z + CombinedExpression::EyeLid as usize] = (self.shapes
            [z + CombinedExpression::EyeLidLeft as usize]
            + self.shapes[z + CombinedExpression::EyeLidRight as usize])
            * 0.5;

        self.shapes[z + CombinedExpression::EyeSquint as usize] = (self.shapes
            [UnifiedExpressions::EyeSquintLeft as usize]
            + self.shapes[UnifiedExpressions::EyeSquintRight as usize])
            * 0.5;

        let brow_down_left = self.shapes[UnifiedExpressions::BrowLowererLeft as usize] * 0.75
            + self.shapes[UnifiedExpressions::BrowPinchLeft as usize] * 0.25;
        let brow_down_right = self.shapes[UnifiedExpressions::BrowLowererRight as usize] * 0.75
            + self.shapes[UnifiedExpressions::BrowPinchRight as usize] * 0.25;

        self.shapes[z + CombinedExpression::BrowDownLeft as usize] = brow_down_left;
        self.shapes[z + CombinedExpression::BrowDownRight as usize] = brow_down_right;

        let brow_outer_up = (self.shapes[UnifiedExpressions::BrowOuterUpLeft as usize]
            + self.shapes[UnifiedExpressions::BrowOuterUpRight as usize])
            * 0.5;
        self.shapes[z + CombinedExpression::BrowOuterUp as usize] = brow_outer_up;

        let brow_inner_up = (self.shapes[UnifiedExpressions::BrowInnerUpLeft as usize]
            + self.shapes[UnifiedExpressions::BrowInnerUpRight as usize])
            * 0.5;
        self.shapes[z + CombinedExpression::BrowInnerUp as usize] = brow_inner_up;

        self.shapes[z + CombinedExpression::BrowUp as usize] =
            (brow_outer_up + brow_inner_up) * 0.5;

        let brow_exp_left = (self.shapes[UnifiedExpressions::BrowInnerUpLeft as usize] * 0.5
            + self.shapes[UnifiedExpressions::BrowOuterUpLeft as usize] * 0.5)
            - brow_down_left;
        let brow_exp_right = (self.shapes[UnifiedExpressions::BrowInnerUpRight as usize] * 0.5
            + self.shapes[UnifiedExpressions::BrowOuterUpRight as usize] * 0.5)
            - brow_down_right;

        self.shapes[z + CombinedExpression::BrowExpressionLeft as usize] = brow_exp_left;
        self.shapes[z + CombinedExpression::BrowExpressionRight as usize] = brow_exp_right;
        self.shapes[z + CombinedExpression::BrowExpression as usize] =
            (brow_exp_left + brow_exp_right) * 0.5;

        let mouth_smile_left = self.shapes[UnifiedExpressions::MouthCornerPullLeft as usize] * 0.75
            + self.shapes[UnifiedExpressions::MouthCornerSlantLeft as usize] * 0.25;
        let mouth_smile_right = self.shapes[UnifiedExpressions::MouthCornerPullRight as usize]
            * 0.75
            + self.shapes[UnifiedExpressions::MouthCornerSlantRight as usize] * 0.25;

        let mouth_sad_left = self.shapes[UnifiedExpressions::MouthFrownLeft as usize] * 0.75
            + self.shapes[UnifiedExpressions::MouthStretchLeft as usize] * 0.25;
        let mouth_sad_right = self.shapes[UnifiedExpressions::MouthFrownRight as usize] * 0.75
            + self.shapes[UnifiedExpressions::MouthStretchRight as usize] * 0.25;

        self.shapes[z + CombinedExpression::MouthSmileLeft as usize] = mouth_smile_left;
        self.shapes[z + CombinedExpression::MouthSmileRight as usize] = mouth_smile_right;
        self.shapes[z + CombinedExpression::MouthSadLeft as usize] = mouth_sad_left;
        self.shapes[z + CombinedExpression::MouthSadRight as usize] = mouth_sad_right;

        self.shapes[z + CombinedExpression::MouthUpperX as usize] = self.shapes
            [UnifiedExpressions::MouthUpperRight as usize]
            - self.shapes[UnifiedExpressions::MouthUpperLeft as usize];

        self.shapes[z + CombinedExpression::MouthLowerX as usize] = self.shapes
            [UnifiedExpressions::MouthLowerRight as usize]
            - self.shapes[UnifiedExpressions::MouthLowerLeft as usize];

        self.shapes[z + CombinedExpression::SmileSadLeft as usize] =
            mouth_smile_left - mouth_sad_left;
        self.shapes[z + CombinedExpression::SmileSadRight as usize] =
            mouth_smile_right - mouth_sad_right;
        self.shapes[z + CombinedExpression::SmileSad as usize] =
            (mouth_smile_left - mouth_sad_left + mouth_smile_right - mouth_sad_right) * 0.5;
        self.shapes[z + CombinedExpression::SmileFrownLeft as usize] =
            mouth_smile_left - self.shapes[UnifiedExpressions::MouthFrownLeft as usize];
        self.shapes[z + CombinedExpression::SmileFrownRight as usize] =
            mouth_smile_right - self.shapes[UnifiedExpressions::MouthFrownRight as usize];
        self.shapes[z + CombinedExpression::SmileFrown as usize] = (mouth_smile_left
            - self.shapes[UnifiedExpressions::MouthFrownLeft as usize]
            + mouth_smile_right
            - self.shapes[UnifiedExpressions::MouthFrownRight as usize])
            * 0.5;
        self.shapes[z + CombinedExpression::CheekPuffSuckLeft as usize] = self.shapes
            [UnifiedExpressions::CheekPuffLeft as usize]
            - self.shapes[UnifiedExpressions::CheekSuckLeft as usize];
        self.shapes[z + CombinedExpression::CheekPuffSuckRight as usize] = self.shapes
            [UnifiedExpressions::CheekPuffRight as usize]
            - self.shapes[UnifiedExpressions::CheekSuckRight as usize];
        self.shapes[z + CombinedExpression::CheekPuffSuck as usize] = (self.shapes
            [UnifiedExpressions::CheekPuffLeft as usize]
            + self.shapes[UnifiedExpressions::CheekPuffRight as usize]
            - self.shapes[UnifiedExpressions::CheekSuckLeft as usize]
            - self.shapes[UnifiedExpressions::CheekSuckRight as usize])
            * 0.5;

        self.shapes[z + CombinedExpression::CheekSquint as usize] = (self.shapes
            [UnifiedExpressions::CheekSquintLeft as usize]
            + self.shapes[UnifiedExpressions::CheekSquintRight as usize])
            * 0.5;

        self.shapes[z + CombinedExpression::LipSuckUpper as usize] = (self.shapes
            [UnifiedExpressions::LipSuckUpperLeft as usize]
            + self.shapes[UnifiedExpressions::LipSuckUpperRight as usize])
            * 0.5;
        self.shapes[z + CombinedExpression::LipSuckLower as usize] = (self.shapes
            [UnifiedExpressions::LipSuckLowerLeft as usize]
            + self.shapes[UnifiedExpressions::LipSuckLowerRight as usize])
            * 0.5;
        self.shapes[z + CombinedExpression::LipSuck as usize] = (self.shapes
            [UnifiedExpressions::LipSuckLowerLeft as usize]
            + self.shapes[UnifiedExpressions::LipSuckLowerRight as usize]
            + self.shapes[UnifiedExpressions::LipSuckUpperLeft as usize]
            + self.shapes[UnifiedExpressions::LipSuckUpperRight as usize])
            * 0.25;
        self.shapes[z + CombinedExpression::MouthStretchTightenLeft as usize] = self.shapes
            [UnifiedExpressions::MouthStretchLeft as usize]
            - self.shapes[UnifiedExpressions::MouthTightenerLeft as usize];

        self.shapes[z + CombinedExpression::MouthStretchTightenRight as usize] = self.shapes
            [UnifiedExpressions::MouthStretchRight as usize]
            - self.shapes[UnifiedExpressions::MouthTightenerRight as usize];

        self.shapes[z + CombinedExpression::MouthStretch as usize] = (self.shapes
            [UnifiedExpressions::MouthStretchLeft as usize]
            + self.shapes[UnifiedExpressions::MouthStretchRight as usize])
            * 0.5;

        self.shapes[z + CombinedExpression::MouthTightener as usize] = (self.shapes
            [UnifiedExpressions::MouthTightenerLeft as usize]
            + self.shapes[UnifiedExpressions::MouthTightenerRight as usize])
            * 0.5;

        self.shapes[z + CombinedExpression::MouthDimple as usize] = (self.shapes
            [UnifiedExpressions::MouthDimpleLeft as usize]
            + self.shapes[UnifiedExpressions::MouthDimpleRight as usize])
            * 0.5;

        let mouth_upper_up = (self.shapes[UnifiedExpressions::MouthUpperUpLeft as usize]
            + self.shapes[UnifiedExpressions::MouthUpperUpRight as usize])
            * 0.5;
        let mouth_lower_down = (self.shapes[UnifiedExpressions::MouthLowerDownLeft as usize]
            + self.shapes[UnifiedExpressions::MouthLowerDownRight as usize])
            * 0.5;

        self.shapes[z + CombinedExpression::MouthUpperUp as usize] = mouth_upper_up;
        self.shapes[z + CombinedExpression::MouthLowerDown as usize] = mouth_lower_down;
        self.shapes[z + CombinedExpression::MouthOpen as usize] =
            (mouth_upper_up + mouth_lower_down) * 0.5;
        self.shapes[z + CombinedExpression::MouthX as usize] = (self.shapes
            [UnifiedExpressions::MouthUpperRight as usize]
            + self.shapes[UnifiedExpressions::MouthLowerRight as usize]
            - self.shapes[UnifiedExpressions::MouthUpperLeft as usize]
            - self.shapes[UnifiedExpressions::MouthLowerLeft as usize])
            * 0.5;
        self.shapes[z + CombinedExpression::JawX as usize] = self.shapes
            [UnifiedExpressions::JawRight as usize]
            - self.shapes[UnifiedExpressions::JawLeft as usize];
        self.shapes[z + CombinedExpression::JawZ as usize] = self.shapes
            [UnifiedExpressions::JawForward as usize]
            - self.shapes[UnifiedExpressions::JawBackward as usize];
        let lip_pucker_left = (self.shapes[UnifiedExpressions::LipPuckerLowerLeft as usize]
            + self.shapes[UnifiedExpressions::LipPuckerUpperLeft as usize])
            * 0.5;
        let lip_pucker_right = (self.shapes[UnifiedExpressions::LipPuckerLowerRight as usize]
            + self.shapes[UnifiedExpressions::LipPuckerUpperRight as usize])
            * 0.5;
        self.shapes[z + CombinedExpression::LipPucker as usize] =
            (lip_pucker_left + lip_pucker_right) * 0.5;
        let lip_funnel_upper = (self.shapes[UnifiedExpressions::LipFunnelUpperLeft as usize]
            + self.shapes[UnifiedExpressions::LipFunnelUpperRight as usize])
            * 0.5;
        let lip_funnel_lower = (self.shapes[UnifiedExpressions::LipFunnelLowerLeft as usize]
            + self.shapes[UnifiedExpressions::LipFunnelLowerRight as usize])
            * 0.5;

        self.shapes[z + CombinedExpression::LipFunnelUpper as usize] = lip_funnel_upper;
        self.shapes[z + CombinedExpression::LipFunnelLower as usize] = lip_funnel_lower;
        self.shapes[z + CombinedExpression::LipFunnel as usize] =
            (lip_funnel_upper + lip_funnel_lower) * 0.5;

        self.shapes[z + CombinedExpression::MouthPress as usize] = (self.shapes
            [UnifiedExpressions::MouthPressLeft as usize]
            + self.shapes[UnifiedExpressions::MouthPressRight as usize])
            * 0.5;

        self.shapes[z + CombinedExpression::NoseSneer as usize] = (self.shapes
            [UnifiedExpressions::NoseSneerLeft as usize]
            + self.shapes[UnifiedExpressions::NoseSneerRight as usize])
            * 0.5;

        self.shapes[z + CombinedExpression::EarLeft as usize] = (self.shapes
            [UnifiedExpressions::BrowInnerUpLeft as usize]
            + self.shapes[UnifiedExpressions::EyeWideLeft as usize]
            - self.shapes[UnifiedExpressions::EyeSquintLeft as usize]
            - self.shapes[UnifiedExpressions::BrowPinchLeft as usize])
            .clamp(-1.0, 1.0);

        self.shapes[z + CombinedExpression::EarRight as usize] = (self.shapes
            [UnifiedExpressions::BrowInnerUpLeft as usize]
            + self.shapes[UnifiedExpressions::EyeWideRight as usize]
            - self.shapes[UnifiedExpressions::EyeSquintRight as usize]
            - self.shapes[UnifiedExpressions::BrowPinchRight as usize])
            .clamp(-1.0, 1.0);
    }

    pub fn apply_to_bundle(
        &mut self,
        params: &[Option<MysteryParam>; NUM_SHAPES],
        bundle: &mut OscBundle,
    ) {
        bundle.send_parameter("ExpressionTrackingActive", OscType::Bool(true));
        bundle.send_parameter("LipTrackingActive", OscType::Bool(true));
        //bundle.send_parameter("EyeTrackingActive", OscType::Bool(true));

        for idx in 0..NUM_SHAPES {
            if let Some(param) = &params[idx] {
                param.send(self.shapes[idx], bundle)
            }
        }
    }
}

pub struct ExtTracking {
    pub hmd: Pose,
    pub controllers: [Pose; 2],
    pub hands: [Option<[Pose; 26]>; 2],
    pub eyes: [Quat; 2],
    pub face: UnifiedTrackingData,
    receiver: Receiver<TrackingMessage>,
    params: [Option<MysteryParam>; NUM_SHAPES],
}

impl ExtTracking {
    pub fn new() -> Self {
        let size = std::mem::size_of::<TrackingMessage>();
        let (sender, receiver) = sync_channel(size * 32);

        let default_combined = vec![
            CombinedExpression::BrowExpressionLeft,
            CombinedExpression::BrowExpressionRight,
            CombinedExpression::EyeLidLeft,
            CombinedExpression::EyeLidRight,
            CombinedExpression::JawX,
            CombinedExpression::LipFunnelLower,
            CombinedExpression::LipFunnelUpper,
            CombinedExpression::LipPucker,
            CombinedExpression::MouthLowerDown,
            CombinedExpression::MouthStretchTightenLeft,
            CombinedExpression::MouthStretchTightenRight,
            CombinedExpression::MouthUpperUp,
            CombinedExpression::MouthX,
            CombinedExpression::SmileSadLeft,
            CombinedExpression::SmileSadRight,
        ];

        let default_unified = vec![
            UnifiedExpressions::CheekPuffLeft,
            UnifiedExpressions::CheekPuffRight,
            UnifiedExpressions::EyeSquintLeft,
            UnifiedExpressions::EyeSquintRight,
            UnifiedExpressions::JawOpen,
            UnifiedExpressions::MouthClosed,
        ];

        let mut params = array::from_fn(|_| None);

        for e in default_combined.into_iter() {
            let name: &str = e.into();
            let new = MysteryParam {
                name: name.into(),
                main_address: Some(format!("FT/v2/{}", name).into()),
                addresses: array::from_fn(|_| None),
                neg_address: None,
                num_bits: 0,
            };
            params[UnifiedExpressions::COUNT + (e as usize)] = Some(new);
        }

        for e in default_unified.into_iter() {
            let name: &str = e.into();
            let new = MysteryParam {
                name: name.into(),
                main_address: Some(format!("FT/v2/{}", name).into()),
                addresses: array::from_fn(|_| None),
                neg_address: None,
                num_bits: 0,
            };
            params[e as usize] = Some(new);
        }

        thread::spawn(move || {
            receive(sender);
        });

        let me = Self {
            receiver,
            face: UnifiedTrackingData::new(),
            hmd: Pose::default(),
            controllers: [Pose::default(), Pose::default()],
            hands: [None, None],
            eyes: [Quat::default(), Quat::default()],
            params,
        };
        me.print_params();

        me
    }

    pub fn step(&mut self, parameters: &AvatarParameters, bundle: &mut OscBundle) {
        for tracking in self.receiver.try_iter() {
            match tracking {
                TrackingMessage::FaceFb(face_fb) => match parameters.get("Motion") {
                    Some(OscType::Int(1)) => {}
                    _ => {
                        self.face.load_face(&face_fb);
                    }
                },
                TrackingMessage::PoseHead(pose_head) => {
                    self.hmd = pose_head;
                }
                TrackingMessage::PoseConL(pose_con_l) => {
                    self.controllers[0] = pose_con_l;
                }
                TrackingMessage::PoseConR(pose_con_r) => {
                    self.controllers[1] = pose_con_r;
                }
                TrackingMessage::HandSkeL(skel_l) => {
                    self.hands[0] = Some(skel_l);
                }
                TrackingMessage::HandSkeR(skel_r) => {
                    self.hands[1] = Some(skel_r);
                }
                TrackingMessage::EyesQuat(eyes_quat) => match parameters.get("Motion") {
                    Some(OscType::Int(1)) => {}
                    _ => {
                        self.eyes = eyes_quat;
                    }
                },
            }
        }

        self.face.apply_to_bundle(&self.params, bundle);

        let left_euler = self.eyes[0].to_euler(glam::EulerRot::ZXY);
        let right_euler = self.eyes[1].to_euler(glam::EulerRot::ZXY);

        bundle.send_tracking(
            "/tracking/eye/LeftRightPitchYaw",
            vec![
                OscType::Float(-left_euler.1.to_degrees()),
                OscType::Float(-left_euler.2.to_degrees()),
                OscType::Float(-right_euler.1.to_degrees()),
                OscType::Float(-right_euler.2.to_degrees()),
            ],
        );
    }

    pub fn osc_json(&mut self, root_node: &OscJsonNode) {
        self.params.iter_mut().for_each(|p| *p = None);

        let _x: Option<()> = root_node
            .get("parameters")
            .and_then(|parameters| parameters.get("FT"))
            .and_then(|ft| ft.get("v2"))
            .and_then(|v2| {
                let Some(contents) = &v2.contents else {
                    return None;
                };
                contents.iter().for_each(|(name, node)| {
                    if let Some(m) = FT_PARAMS_REGEX.captures(name) {
                        let main: Arc<str> = m[1].into();

                        let Some(idx) = UnifiedExpressions::from_str(&main)
                            .map(|e| e as usize)
                            .or_else(|_| {
                                CombinedExpression::from_str(&main)
                                    .map(|e| UnifiedExpressions::COUNT + (e as usize))
                            })
                            .ok()
                        else {
                            warn!("Unknown expression: {}", &main);
                            return;
                        };

                        let create = self.params[idx].is_none();

                        if create {
                            let new = MysteryParam {
                                name: main.clone(),
                                main_address: None,
                                addresses: array::from_fn(|_| None),
                                neg_address: None,
                                num_bits: 0,
                            };
                            self.params[idx] = Some(new);
                        };

                        let stored = (&mut self.params[idx]).as_mut().unwrap();
                        match m.get(2).map(|s| s.as_str()) {
                            Some("Negative") => {
                                let addr = &node.full_path.as_ref()[super::PARAM_PREFIX.len()..];
                                stored.neg_address = Some(addr.into());
                            }
                            Some(digit) => {
                                let digit = digit.parse::<f32>().unwrap();
                                let idx = digit.log2() as usize;
                                let addr = &node.full_path.as_ref()[super::PARAM_PREFIX.len()..];
                                stored.num_bits = stored.num_bits.max(idx + 1);
                                stored.addresses[idx] = Some(addr.into());
                            }
                            None => {
                                let addr = &node.full_path.as_ref()[super::PARAM_PREFIX.len()..];
                                stored.main_address = Some(addr.into());
                            }
                        }
                    }
                });

                None
            });
        self.print_params();
    }

    fn print_params(&self) {
        for v in self.params.iter().filter_map(|p| p.as_ref()) {
            if v.main_address.as_ref().is_some() {
                info!("{}: float", v.name,);
            } else {
                info!(
                    "{}: {} bits {}",
                    v.name,
                    v.num_bits,
                    if v.neg_address.is_some() { "+ neg" } else { "" },
                );
            }
        }
    }
}

fn receive(sender: SyncSender<TrackingMessage>) {
    let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let listener = UdpSocket::bind(SocketAddr::new(ip, 0xA1F7)).unwrap();
    listener.connect("0.0.0.0:0").expect("listener connect");

    let mut buf = [0u8; 1500];

    info!(
        "Listening for ALVR-VRCFT messages on {}",
        listener.local_addr().unwrap()
    );

    loop {
        if let Ok(size) = listener.recv(&mut buf) {
            let mut cur = Cursor::new(&buf[..size]);
            let mut id = [0u8; 8];
            while let Ok(()) = cur.read_exact(&mut id) {
                let message: TrackingMessage;
                if id == *b"FaceFb\0\0" {
                    let Some(face_fb) = read_bin::<[f32; 63]>(&mut cur) else {
                        warn!("Failed to read FaceFb message");
                        break;
                    };
                    message = TrackingMessage::FaceFb(face_fb);
                } else if id == *b"EyesQuat" {
                    let Some(eyes_quat) = read_bin::<[Quat; 2]>(&mut cur) else {
                        warn!("Failed to read EyesQuat message");
                        break;
                    };
                    message = TrackingMessage::EyesQuat(eyes_quat);
                } else if id == *b"PoseHmd\0" {
                    let Some(pose_head) = read_bin::<[f32; 7]>(&mut cur) else {
                        warn!("Failed to read PoseHmd message");
                        break;
                    };
                    message = TrackingMessage::PoseHead(Pose::from_arr(&pose_head));
                } else if id == *b"PoseCnL\0" {
                    let Some(pose_con_l) = read_bin::<[f32; 7]>(&mut cur) else {
                        warn!("Failed to read PoseCnL message");
                        break;
                    };
                    message = TrackingMessage::PoseConL(Pose::from_arr(&pose_con_l));
                } else if id == *b"PoseCnR\0" {
                    let Some(pose_con_r) = read_bin::<[f32; 7]>(&mut cur) else {
                        warn!("Failed to read PoseCnR message");
                        break;
                    };
                    message = TrackingMessage::PoseConR(Pose::from_arr(&pose_con_r));
                } else if id == *b"HandSkL\0" {
                    let Some(skel_l) = read_bin::<[f32; 26 * 7]>(&mut cur) else {
                        warn!("Failed to read HandSkL message");
                        break;
                    };
                    let mut poses: [Pose; 26] = Default::default();
                    for (i, chunk) in skel_l.chunks_exact(7).enumerate() {
                        poses[i] = Pose::from_arr(chunk);
                    }
                    message = TrackingMessage::HandSkeL(poses);
                } else if id == *b"HandSkR\0" {
                    let Some(skel_r) = read_bin::<[f32; 26 * 7]>(&mut cur) else {
                        warn!("Failed to read HandSkR message");
                        break;
                    };
                    let mut poses: [Pose; 26] = Default::default();
                    for (i, chunk) in skel_r.chunks_exact(7).enumerate() {
                        poses[i] = Pose::from_arr(chunk);
                    }
                    message = TrackingMessage::HandSkeR(poses);
                } else {
                    warn!("Unknown tracking message type: {:?}", id);
                    break;
                };
                debug!("Received tracking message: {:?}", id);

                match sender.try_send(message) {
                    Ok(()) => {}
                    Err(TrySendError::Full(_)) => {
                        debug!("Tracking message queue full");
                        break;
                    }
                    Err(TrySendError::Disconnected(_)) => {
                        warn!("Tracking message queue disconnected");
                        return;
                    }
                }
            }
        }
        thread::sleep(Duration::from_millis(1));
    }
}

fn read_bin<T>(cursor: &mut Cursor<&[u8]>) -> Option<T> {
    let size = std::mem::size_of::<T>();
    unsafe {
        let mut t = std::mem::zeroed();
        let buf = slice::from_raw_parts_mut(&mut t as *mut _ as *mut u8, size);
        let Ok(()) = cursor.read_exact(buf) else {
            warn!("Failed to read {} message", std::any::type_name::<T>());
            return None;
        };
        Some(t)
    }
}

static FT_PARAMS_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(.+?)(Negative|\d+)?$").unwrap());
