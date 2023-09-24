use std::mem::{size_of, transmute};
use std::net::{UdpSocket, Ipv4Addr, IpAddr, SocketAddr};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::time::Duration;

use glam::Vec2;
use log::{info, warn};
use rosc::{OscBundle, OscType};
use super::bundle::AvatarBundle;

#[allow(unused)]
pub enum UnifiedExpressions
{
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
    NasalDilationLeft, // Left side nose's canal dilates. Basis on the alar nasalis muscle.
    NasalConstrictRight, // Right side nose's canal constricts. Basis on the transverse nasalis muscle.
    NasalConstrictLeft, // Left side nose's canal constricts. Basis on the transverse nasalis muscle.

    CheekSquintRight, // Raises the right side cheek. Basis on the main action of the lower outer part of the orbicularis oculi.
    CheekSquintLeft, // Raises the left side cheek. Basis on the main action of the lower outer part of the orbicularis oculi.
    CheekPuffRight, // Puffs the right side cheek. Basis on the cheeks' ability to stretch orbitally.
    CheekPuffLeft, // Puffs the left side cheek. Basis on the cheeks' ability to stretch orbitally.
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
    MouthStretchLeft,  // Stretches the left side lips together horizontally and thins them vertically slightly. Basis on the risorius muscle.

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
    TongueUp, // Tongue points in an upward direction.
    TongueDown, // Tongue points in an downward direction.
    TongueRight, // Tongue points in an rightward direction.
    TongueLeft, // Tongue points in an leftward direction.

    // Based on https://www.naun.org/main/NAUN/computers/2018/a042007-060.pdf
    TongueRoll, // Rolls up the sides of the tongue into a 'hotdog bun' shape.
    TongueBendDown, // Pushes tip of the tongue below the rest of the tongue in an arch.
    TongueCurlUp, // Pushes tip of the tongue above the rest of the tongue in an arch.
    TongueSquish, // Tongue becomes thinner width-wise and slightly thicker height-wise.
    TongueFlat, // Tongue becomes thicker width-wise and slightly thinner height-wise.

    TongueTwistRight, // Tongue tip rotates clockwise from POV with the rest of the tongue following gradually.
    TongueTwistLeft, // Tongue tip rotates counter-clockwise from POV with the rest of the tongue following gradually.

    SoftPalateClose, // Visibly lowers the back of the throat (soft palate) inside the mouth to close off the throat.
    ThroatSwallow, // Visibly causes the Adam's apple to pull upward into the throat as if swallowing.
    
    NeckFlexRight, // Flexes the Right side of the neck and face (causes the right corner of the face to stretch towards.)
    NeckFlexLeft, // Flexes the left side of the neck and face (causes the left corner of the face to stretch towards.)

    Max,
}

#[allow(non_snake_case, unused)]
pub struct FaceFb {
    Name: [u8; 8],
    BrowLowererL: f32,
    BrowLowererR: f32,
    CheekPuffL: f32,
    CheekPuffR: f32,
    CheekRaiserL: f32,
    CheekRaiserR: f32,
    CheekSuckL: f32,
    CheekSuckR: f32,
    ChinRaiserB: f32,
    ChinRaiserT: f32,
    DimplerL: f32,
    DimplerR: f32,
    EyesClosedL: f32,
    EyesClosedR: f32,
    EyesLookDownL: f32,
    EyesLookDownR: f32,
    EyesLookLeftL: f32,
    EyesLookLeftR: f32,
    EyesLookRightL: f32,
    EyesLookRightR: f32,
    EyesLookUpL: f32,
    EyesLookUpR: f32,
    InnerBrowRaiserL: f32,
    InnerBrowRaiserR: f32,
    JawDrop: f32,
    JawSidewaysLeft: f32,
    JawSidewaysRight: f32,
    JawThrust: f32,
    LidTightenerL: f32,
    LidTightenerR: f32,
    LipCornerDepressorL: f32,
    LipCornerDepressorR: f32,
    LipCornerPullerL: f32,
    LipCornerPullerR: f32,
    LipFunnelerLB: f32,
    LipFunnelerLT: f32,
    LipFunnelerRB: f32,
    LipFunnelerRT: f32,
    LipPressorL: f32,
    LipPressorR: f32,
    LipPuckerL: f32,
    LipPuckerR: f32,
    LipStretcherL: f32,
    LipStretcherR: f32,
    LipSuckLB: f32,
    LipSuckLT: f32,
    LipSuckRB: f32,
    LipSuckRT: f32,
    LipTightenerL: f32,
    LipTightenerR: f32,
    LipsToward: f32,
    LowerLipDepressorL: f32,
    LowerLipDepressorR: f32,
    MouthLeft: f32,
    MouthRight: f32,
    NoseWrinklerL: f32,
    NoseWrinklerR: f32,
    OuterBrowRaiserL: f32,
    OuterBrowRaiserR: f32,
    UpperLidRaiserL: f32,
    UpperLidRaiserR: f32,
    UpperLipRaiserL: f32,
    UpperLipRaiserR: f32,
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
    pub max_dilation: f32,
    pub min_dilation: f32,
}

pub type UnifiedExpressionShape = f32;

#[derive(Debug, Clone)]
pub struct UnifiedTrackingData {
    pub eye: UnifiedEyeData,
    pub shapes: [UnifiedExpressionShape; UnifiedExpressions::Max as usize], 
}

impl UnifiedTrackingData {
    pub fn new() -> Self {
        Self {
            eye: UnifiedEyeData::default(),
            shapes: [0.0; UnifiedExpressions::Max as usize],
        }
    }

    pub fn load_face(&mut self, face_fb: &FaceFb) {

            self.eye.left.openness = 1. - (face_fb.EyesClosedL + face_fb.EyesClosedL * face_fb.LidTightenerL).clamp(0.0, 1.0);
            self.eye.right.openness = 1. - (face_fb.EyesClosedR + face_fb.EyesClosedR * face_fb.LidTightenerR).clamp(0.0, 1.0);

            self.eye.left.gaze.x = (face_fb.EyesLookRightL - face_fb.EyesLookLeftL) * 0.5;
            self.eye.left.gaze.y = (face_fb.EyesLookUpL - face_fb.EyesLookDownL) * 0.5;
            self.eye.right.gaze.x = (face_fb.EyesLookRightR - face_fb.EyesLookLeftR) * 0.5;
            self.eye.right.gaze.y = (face_fb.EyesLookUpR - face_fb.EyesLookDownR) * 0.5;

            self.eye.left.pupil_diameter_mm = 0.5;
            self.eye.right.pupil_diameter_mm = 0.5;

            self.shapes[UnifiedExpressions::EyeSquintRight as usize] = face_fb.LidTightenerR;
            self.shapes[UnifiedExpressions::EyeSquintLeft as usize] = face_fb.LidTightenerL;
            self.shapes[UnifiedExpressions::EyeWideRight as usize] = face_fb.UpperLidRaiserR;
            self.shapes[UnifiedExpressions::EyeWideLeft as usize] = face_fb.UpperLidRaiserL;

            self.shapes[UnifiedExpressions::BrowPinchRight as usize] = face_fb.BrowLowererR;
            self.shapes[UnifiedExpressions::BrowPinchLeft as usize] = face_fb.BrowLowererL;
            self.shapes[UnifiedExpressions::BrowLowererRight as usize] = face_fb.BrowLowererR;
            self.shapes[UnifiedExpressions::BrowLowererLeft as usize] = face_fb.BrowLowererL;
            self.shapes[UnifiedExpressions::BrowInnerUpRight as usize] = face_fb.InnerBrowRaiserR;
            self.shapes[UnifiedExpressions::BrowInnerUpLeft as usize] = face_fb.InnerBrowRaiserL;
            self.shapes[UnifiedExpressions::BrowOuterUpRight as usize] = face_fb.OuterBrowRaiserR;
            self.shapes[UnifiedExpressions::BrowOuterUpLeft as usize] = face_fb.OuterBrowRaiserL;

            self.shapes[UnifiedExpressions::CheekSquintRight as usize] = face_fb.CheekRaiserR;
            self.shapes[UnifiedExpressions::CheekSquintLeft as usize] = face_fb.CheekRaiserL;
            self.shapes[UnifiedExpressions::CheekPuffRight as usize] = face_fb.CheekPuffR;
            self.shapes[UnifiedExpressions::CheekPuffLeft as usize] = face_fb.CheekPuffL;
            self.shapes[UnifiedExpressions::CheekSuckRight as usize] = face_fb.CheekSuckR;
            self.shapes[UnifiedExpressions::CheekSuckLeft as usize] = face_fb.CheekSuckL;

            self.shapes[UnifiedExpressions::JawOpen as usize] = face_fb.JawDrop;
            self.shapes[UnifiedExpressions::JawRight as usize] = face_fb.JawSidewaysRight;
            self.shapes[UnifiedExpressions::JawLeft as usize] = face_fb.JawSidewaysLeft;
            self.shapes[UnifiedExpressions::JawForward as usize] = face_fb.JawThrust;
            self.shapes[UnifiedExpressions::MouthClosed as usize] = face_fb.LipsToward;

            self.shapes[UnifiedExpressions::LipSuckLowerRight as usize] = face_fb.LipSuckRB;
            self.shapes[UnifiedExpressions::LipSuckLowerLeft as usize] = face_fb.LipSuckLB;
            self.shapes[UnifiedExpressions::LipFunnelUpperRight as usize] = face_fb.LipFunnelerRT;
            self.shapes[UnifiedExpressions::LipFunnelUpperLeft as usize] = face_fb.LipFunnelerLT;
            self.shapes[UnifiedExpressions::LipFunnelLowerRight as usize] = face_fb.LipFunnelerRB;
            self.shapes[UnifiedExpressions::LipFunnelLowerLeft as usize] = face_fb.LipFunnelerLB;
            self.shapes[UnifiedExpressions::LipPuckerUpperRight as usize] = face_fb.LipPuckerR;
            self.shapes[UnifiedExpressions::LipPuckerUpperLeft as usize] = face_fb.LipPuckerL;
            self.shapes[UnifiedExpressions::LipPuckerLowerRight as usize] = face_fb.LipPuckerR;
            self.shapes[UnifiedExpressions::LipPuckerLowerLeft as usize] = face_fb.LipPuckerL;

            self.shapes[UnifiedExpressions::NoseSneerRight as usize] = face_fb.NoseWrinklerR;
            self.shapes[UnifiedExpressions::NoseSneerLeft as usize] = face_fb.NoseWrinklerL;

            self.shapes[UnifiedExpressions::MouthLowerDownRight as usize] = face_fb.LowerLipDepressorR;
            self.shapes[UnifiedExpressions::MouthLowerDownLeft as usize] = face_fb.LowerLipDepressorL;

            self.shapes[UnifiedExpressions::MouthUpperRight as usize] = face_fb.MouthRight;
            self.shapes[UnifiedExpressions::MouthUpperLeft as usize] = face_fb.MouthLeft;
            self.shapes[UnifiedExpressions::MouthLowerRight as usize] = face_fb.MouthRight;
            self.shapes[UnifiedExpressions::MouthLowerLeft as usize] = face_fb.MouthLeft;

            self.shapes[UnifiedExpressions::MouthCornerPullRight as usize] = face_fb.LipCornerPullerR;
            self.shapes[UnifiedExpressions::MouthCornerPullLeft as usize] = face_fb.LipCornerPullerL;
            self.shapes[UnifiedExpressions::MouthCornerSlantRight as usize] = face_fb.LipCornerPullerR;
            self.shapes[UnifiedExpressions::MouthCornerSlantLeft as usize] = face_fb.LipCornerPullerL;

            self.shapes[UnifiedExpressions::MouthFrownRight as usize] = face_fb.LipCornerDepressorR;
            self.shapes[UnifiedExpressions::MouthFrownLeft as usize] = face_fb.LipCornerDepressorL;
            self.shapes[UnifiedExpressions::MouthStretchRight as usize] = face_fb.LipStretcherR;
            self.shapes[UnifiedExpressions::MouthStretchLeft as usize] = face_fb.LipStretcherL;

            self.shapes[UnifiedExpressions::MouthDimpleLeft as usize] = face_fb.DimplerL;
            self.shapes[UnifiedExpressions::MouthDimpleRight as usize] = face_fb.DimplerR;

            self.shapes[UnifiedExpressions::MouthRaiserUpper as usize] = face_fb.ChinRaiserT;
            self.shapes[UnifiedExpressions::MouthRaiserLower as usize] = face_fb.ChinRaiserB;
            self.shapes[UnifiedExpressions::MouthPressRight as usize] = face_fb.LipPressorR;
            self.shapes[UnifiedExpressions::MouthPressLeft as usize] = face_fb.LipPressorL;
            self.shapes[UnifiedExpressions::MouthTightenerRight as usize] = face_fb.LipTightenerR;
            self.shapes[UnifiedExpressions::MouthTightenerLeft as usize] = face_fb.LipTightenerL;
    }
    
    pub fn apply_to_bundle(&self, bundle: &mut OscBundle) {
        bundle.send_tracking("/tracking/eye/LeftRightPitchYaw", vec![
            OscType::Float((-self.eye.left.gaze.y.atan()).to_degrees()),
            OscType::Float(self.eye.left.gaze.x.atan().to_degrees()),
            OscType::Float((-self.eye.right.gaze.y.atan()).to_degrees()),
            OscType::Float(self.eye.right.gaze.x.atan().to_degrees()),
        ]);

        bundle.send_parameter("v2/EyeLidLeft", 
            OscType::Float(
                self.eye.left.openness * 0.75 
                + self.shapes[UnifiedExpressions::EyeWideLeft as usize] * 0.25
            ));

        bundle.send_parameter("v2/EyeLidRight",
            OscType::Float(
                self.eye.right.openness * 0.75 
                + self.shapes[UnifiedExpressions::EyeWideRight as usize] * 0.25
            ));

        bundle.send_parameter("v2/EyeSquintLeft", OscType::Float(self.shapes[UnifiedExpressions::EyeSquintLeft as usize]));
        bundle.send_parameter("v2/EyeSquintRight", OscType::Float(self.shapes[UnifiedExpressions::EyeSquintRight as usize]));

        let brow_down_left = self.shapes[UnifiedExpressions::BrowLowererLeft as usize] * 0.75 + self.shapes[UnifiedExpressions::BrowPinchLeft as usize] * 0.25;
        let brow_down_right = self.shapes[UnifiedExpressions::BrowLowererRight as usize] * 0.75 + self.shapes[UnifiedExpressions::BrowPinchRight as usize] * 0.25;

        bundle.send_parameter("v2/BrowExpressionLeft",
            OscType::Float(
                (self.shapes[UnifiedExpressions::BrowInnerUpLeft as usize] * 0.5 
                + self.shapes[UnifiedExpressions::BrowOuterUpLeft as usize] * 0.5) 
                - brow_down_left
        ));

        bundle.send_parameter("v2/BrowExpressionRight",
            OscType::Float(
                (self.shapes[UnifiedExpressions::BrowInnerUpRight as usize] * 0.5 
                + self.shapes[UnifiedExpressions::BrowOuterUpRight as usize] * 0.5) 
                - brow_down_right
            ));

        let mouth_smile_left = self.shapes[UnifiedExpressions::MouthCornerPullLeft as usize] * 0.75 + self.shapes[UnifiedExpressions::MouthCornerSlantLeft as usize] * 0.25;
        let mouth_smile_right = self.shapes[UnifiedExpressions::MouthCornerPullRight as usize] * 0.75 + self.shapes[UnifiedExpressions::MouthCornerSlantRight as usize] * 0.25;
        let mouth_sad_left = self.shapes[UnifiedExpressions::MouthFrownLeft as usize] * 0.75 + self.shapes[UnifiedExpressions::MouthStretchLeft as usize] * 0.25;
        let mouth_sad_right = self.shapes[UnifiedExpressions::MouthFrownRight as usize] * 0.75 + self.shapes[UnifiedExpressions::MouthStretchRight as usize] * 0.25;

        bundle.send_parameter("v2/SmileSadLeft", OscType::Float(mouth_smile_left - mouth_sad_left));
        bundle.send_parameter("v2/SmileSadRight", OscType::Float(mouth_smile_right - mouth_sad_right));

        bundle.send_parameter("v2/MouthOpen", OscType::Float(
            (self.shapes[UnifiedExpressions::MouthUpperUpLeft as usize] * 0.25 + self.shapes[UnifiedExpressions::MouthUpperUpRight as usize] * 0.25
            + self.shapes[UnifiedExpressions::MouthLowerDownLeft as usize] * 0.25 + self.shapes[UnifiedExpressions::MouthLowerDownRight as usize] * 0.25).clamp(0.0, 1.0)
        ));

        bundle.send_parameter("v2/MouthStretch", OscType::Float(
            (self.shapes[UnifiedExpressions::MouthStretchLeft as usize] + self.shapes[UnifiedExpressions::MouthStretchRight as usize]) * 0.5
        ));

        bundle.send_parameter("v2/MouthTightener", OscType::Float(
            (self.shapes[UnifiedExpressions::MouthTightenerLeft as usize] + self.shapes[UnifiedExpressions::MouthTightenerRight as usize]) * 0.5
        ));

        bundle.send_parameter("v2/MouthClosed", OscType::Float(
            self.shapes[UnifiedExpressions::MouthClosed as usize]
        ));

        bundle.send_parameter("v2/MouthUpperUp", OscType::Float(
            (self.shapes[UnifiedExpressions::MouthUpperUpLeft as usize] + self.shapes[UnifiedExpressions::MouthUpperUpRight as usize]) * 0.5
        ));

        bundle.send_parameter("v2/MouthLowerDown", OscType::Float(
            (self.shapes[UnifiedExpressions::MouthLowerDownLeft as usize] + self.shapes[UnifiedExpressions::MouthLowerDownRight as usize]) * 0.5
        ));

        bundle.send_parameter("v2/MouthX", 
            OscType::Float((self.shapes[UnifiedExpressions::MouthUpperRight as usize] + self.shapes[UnifiedExpressions::MouthLowerRight as usize]) / 2.0 -
            (self.shapes[UnifiedExpressions::MouthUpperLeft as usize] + self.shapes[UnifiedExpressions::MouthLowerLeft as usize]) / 2.0)
        );

        bundle.send_parameter("v2/JawX", 
            OscType::Float(self.shapes[UnifiedExpressions::JawRight as usize] - self.shapes[UnifiedExpressions::JawLeft as usize])
        );

        bundle.send_parameter("v2/JawOpen", OscType::Float(self.shapes[UnifiedExpressions::JawOpen as usize]));

        bundle.send_parameter("v2/CheekPuffLeft", OscType::Float(self.shapes[UnifiedExpressions::CheekPuffLeft as usize]));
        bundle.send_parameter("v2/CheekPuffRight", OscType::Float(self.shapes[UnifiedExpressions::CheekPuffRight as usize]));

        let lip_pucker_left = (self.shapes[UnifiedExpressions::LipPuckerLowerLeft as usize] + self.shapes[UnifiedExpressions::LipPuckerUpperLeft as usize]) * 0.5;
        let lip_pucker_right = (self.shapes[UnifiedExpressions::LipPuckerLowerRight as usize] + self.shapes[UnifiedExpressions::LipPuckerUpperRight as usize]) * 0.5;

        bundle.send_parameter("v2/LipPuckerLeft", OscType::Float(lip_pucker_left));
        bundle.send_parameter("v2/LipPuckerRight", OscType::Float(lip_pucker_right));
        bundle.send_parameter("v2/LipPucker", OscType::Float((lip_pucker_left + lip_pucker_right) * 0.5));

        let lip_funnel_upper = (self.shapes[UnifiedExpressions::LipFunnelUpperLeft as usize] + self.shapes[UnifiedExpressions::LipFunnelUpperRight as usize]) * 0.5;
        let lip_funnel_lower = (self.shapes[UnifiedExpressions::LipFunnelLowerLeft as usize] + self.shapes[UnifiedExpressions::LipFunnelLowerRight as usize]) * 0.5;

        bundle.send_parameter("v2/LipFunnelUpper", OscType::Float(lip_funnel_upper));
        bundle.send_parameter("v2/LipFunnelLower", OscType::Float(lip_funnel_lower));
        bundle.send_parameter("v2/LipFunnel", OscType::Float((lip_funnel_upper + lip_funnel_lower) * 0.5));
    }
}

pub struct ExtFacetrack {
    data: UnifiedTrackingData,
    receiver: Receiver<FaceFb>,
}

impl ExtFacetrack {
    pub fn new() -> Self {
        let (sender, receiver) = channel();

        thread::spawn(move || {
            receive(sender);
        });

        Self {
            receiver,
            data: UnifiedTrackingData::new(),
        }
    }

    pub fn step(&mut self, bundle: &mut OscBundle) {
        let Some(face_fb) = self.receiver.try_iter().last() else {
            return;
        };

        self.data.load_face(&face_fb);
        self.data.apply_to_bundle(bundle);
    }
}

fn receive(sender: Sender<FaceFb>) {
    let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let listener = UdpSocket::bind(SocketAddr::new(ip, 0xA1F7)).unwrap();
    listener.connect("0.0.0.0:0").expect("listener connect");

    let mut buf = [0u8; size_of::<FaceFb>()];

    info!(
        "Listening for ALVR-VRCFT messages on {}",
        listener.local_addr().unwrap()
    );

    loop {
        if let Ok(size) = listener.recv(&mut buf) {
            if size == size_of::<FaceFb>() {
                let face_fb = unsafe { transmute::<[u8; size_of::<FaceFb>()], FaceFb>(buf) };
                if face_fb.Name != *b"FaceFb\0\0" {
                    warn!("Received ALVR-VRCFT message with unexpected name {:?}", face_fb.Name);
                    continue;
                }
                sender.send(face_fb).unwrap();
            } else {
                warn!("Received ALVR-VRCFT message of unexpected size {}", size);
            }
        }
        thread::sleep(Duration::from_millis(1));
    }    
}

