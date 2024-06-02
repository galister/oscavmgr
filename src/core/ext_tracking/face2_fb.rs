use super::unified::{UnifiedExpressions, UnifiedShapeAccessors, UnifiedShapes, NUM_SHAPES};

#[allow(non_snake_case, unused)]
#[repr(usize)]
enum FaceFb {
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
enum Face2Fb {
    TongueTipInterdental = 63,
    TongueTipAlveolar,
    TongueFrontDorsalPalate,
    TongueMidDorsalPalate,
    TongueBackDorsalPalate,
    TongueOut,
    TongueRetreat,
    Max,
}

pub(crate) fn face2_fb_to_unified(face_fb: &[f32]) -> Option<UnifiedShapes> {
    let mut shapes: UnifiedShapes = [0.0; NUM_SHAPES];
    if face_fb.len() < FaceFb::Max as usize {
        log::warn!(
            "Face tracking data is too short: {} < {}",
            face_fb.len(),
            FaceFb::Max as usize
        );
        return None;
    }

    let getf = |index| face_fb[index as usize];
    let getf2 = |index| face_fb[index as usize];

    shapes.setu(
        UnifiedExpressions::EyeRightX,
        getf(FaceFb::EyesLookRightR) - getf(FaceFb::EyesLookLeftR),
    );
    shapes.setu(
        UnifiedExpressions::EyeLeftX,
        getf(FaceFb::EyesLookRightL) - getf(FaceFb::EyesLookLeftL),
    );
    shapes.setu(
        UnifiedExpressions::EyeY,
        getf(FaceFb::EyesLookUpR) - getf(FaceFb::EyesLookDownR),
    );

    shapes.setu(UnifiedExpressions::EyeClosedLeft, getf(FaceFb::EyesClosedL));
    shapes.setu(
        UnifiedExpressions::EyeClosedRight,
        getf(FaceFb::EyesClosedR),
    );

    shapes.setu(
        UnifiedExpressions::EyeSquintRight,
        getf(FaceFb::LidTightenerR) - getf(FaceFb::EyesClosedR),
    );
    shapes.setu(
        UnifiedExpressions::EyeSquintLeft,
        getf(FaceFb::LidTightenerL) - getf(FaceFb::EyesClosedL),
    );
    shapes.setu(
        UnifiedExpressions::EyeWideRight,
        getf(FaceFb::UpperLidRaiserR),
    );
    shapes.setu(
        UnifiedExpressions::EyeWideLeft,
        getf(FaceFb::UpperLidRaiserL),
    );

    shapes.setu(
        UnifiedExpressions::BrowPinchRight,
        getf(FaceFb::BrowLowererR),
    );
    shapes.setu(
        UnifiedExpressions::BrowPinchLeft,
        getf(FaceFb::BrowLowererL),
    );
    shapes.setu(
        UnifiedExpressions::BrowLowererRight,
        getf(FaceFb::BrowLowererR),
    );
    shapes.setu(
        UnifiedExpressions::BrowLowererLeft,
        getf(FaceFb::BrowLowererL),
    );
    shapes.setu(
        UnifiedExpressions::BrowInnerUpRight,
        getf(FaceFb::InnerBrowRaiserR),
    );
    shapes.setu(
        UnifiedExpressions::BrowInnerUpLeft,
        getf(FaceFb::InnerBrowRaiserL),
    );
    shapes.setu(
        UnifiedExpressions::BrowOuterUpRight,
        getf(FaceFb::OuterBrowRaiserR),
    );
    shapes.setu(
        UnifiedExpressions::BrowOuterUpLeft,
        getf(FaceFb::OuterBrowRaiserL),
    );

    shapes.setu(
        UnifiedExpressions::CheekSquintRight,
        getf(FaceFb::CheekRaiserR),
    );
    shapes.setu(
        UnifiedExpressions::CheekSquintLeft,
        getf(FaceFb::CheekRaiserL),
    );
    shapes.setu(UnifiedExpressions::CheekPuffRight, getf(FaceFb::CheekPuffR));
    shapes.setu(UnifiedExpressions::CheekPuffLeft, getf(FaceFb::CheekPuffL));
    shapes.setu(UnifiedExpressions::CheekSuckRight, getf(FaceFb::CheekSuckR));
    shapes.setu(UnifiedExpressions::CheekSuckLeft, getf(FaceFb::CheekSuckL));

    shapes.setu(UnifiedExpressions::JawOpen, getf(FaceFb::JawDrop));
    shapes.setu(UnifiedExpressions::JawRight, getf(FaceFb::JawSidewaysRight));
    shapes.setu(UnifiedExpressions::JawLeft, getf(FaceFb::JawSidewaysLeft));
    shapes.setu(UnifiedExpressions::JawForward, getf(FaceFb::JawThrust));
    shapes.setu(UnifiedExpressions::MouthClosed, getf(FaceFb::LipsToward));

    shapes.setu(
        UnifiedExpressions::LipSuckUpperRight,
        (1.0 - getf(FaceFb::UpperLipRaiserR).powf(0.1666)).min(getf(FaceFb::LipSuckRT)),
    );
    shapes.setu(
        UnifiedExpressions::LipSuckUpperLeft,
        (1.0 - getf(FaceFb::UpperLipRaiserL).powf(0.1666)).min(getf(FaceFb::LipSuckLT)),
    );

    shapes.setu(
        UnifiedExpressions::LipSuckLowerRight,
        getf(FaceFb::LipSuckRB),
    );
    shapes.setu(
        UnifiedExpressions::LipSuckLowerLeft,
        getf(FaceFb::LipSuckLB),
    );
    shapes.setu(
        UnifiedExpressions::LipFunnelUpperRight,
        getf(FaceFb::LipFunnelerRT),
    );
    shapes.setu(
        UnifiedExpressions::LipFunnelUpperLeft,
        getf(FaceFb::LipFunnelerLT),
    );
    shapes.setu(
        UnifiedExpressions::LipFunnelLowerRight,
        getf(FaceFb::LipFunnelerRB),
    );
    shapes.setu(
        UnifiedExpressions::LipFunnelLowerLeft,
        getf(FaceFb::LipFunnelerLB),
    );
    shapes.setu(
        UnifiedExpressions::LipPuckerUpperRight,
        getf(FaceFb::LipPuckerR),
    );
    shapes.setu(
        UnifiedExpressions::LipPuckerUpperLeft,
        getf(FaceFb::LipPuckerL),
    );
    shapes.setu(
        UnifiedExpressions::LipPuckerLowerRight,
        getf(FaceFb::LipPuckerR),
    );
    shapes.setu(
        UnifiedExpressions::LipPuckerLowerLeft,
        getf(FaceFb::LipPuckerL),
    );

    shapes.setu(
        UnifiedExpressions::NoseSneerRight,
        getf(FaceFb::NoseWrinklerR),
    );
    shapes.setu(
        UnifiedExpressions::NoseSneerLeft,
        getf(FaceFb::NoseWrinklerL),
    );

    shapes.setu(
        UnifiedExpressions::MouthLowerDownRight,
        getf(FaceFb::LowerLipDepressorR),
    );
    shapes.setu(
        UnifiedExpressions::MouthLowerDownLeft,
        getf(FaceFb::LowerLipDepressorL),
    );

    let mouth_upper_up_right = getf(FaceFb::UpperLipRaiserR);
    let mouth_upper_up_left = getf(FaceFb::UpperLipRaiserL);

    shapes.setu(UnifiedExpressions::MouthUpperUpRight, mouth_upper_up_right);
    shapes.setu(UnifiedExpressions::MouthUpperUpLeft, mouth_upper_up_left);
    shapes.setu(
        UnifiedExpressions::MouthUpperDeepenRight,
        mouth_upper_up_right,
    );
    shapes.setu(
        UnifiedExpressions::MouthUpperDeepenLeft,
        mouth_upper_up_left,
    );

    shapes.setu(
        UnifiedExpressions::MouthUpperRight,
        getf(FaceFb::MouthRight),
    );
    shapes.setu(UnifiedExpressions::MouthUpperLeft, getf(FaceFb::MouthLeft));
    shapes.setu(
        UnifiedExpressions::MouthLowerRight,
        getf(FaceFb::MouthRight),
    );
    shapes.setu(UnifiedExpressions::MouthLowerLeft, getf(FaceFb::MouthLeft));

    shapes.setu(
        UnifiedExpressions::MouthCornerPullRight,
        getf(FaceFb::LipCornerPullerR),
    );
    shapes.setu(
        UnifiedExpressions::MouthCornerPullLeft,
        getf(FaceFb::LipCornerPullerL),
    );
    shapes.setu(
        UnifiedExpressions::MouthCornerSlantRight,
        getf(FaceFb::LipCornerPullerR),
    );
    shapes.setu(
        UnifiedExpressions::MouthCornerSlantLeft,
        getf(FaceFb::LipCornerPullerL),
    );

    shapes.setu(
        UnifiedExpressions::MouthFrownRight,
        getf(FaceFb::LipCornerDepressorR),
    );
    shapes.setu(
        UnifiedExpressions::MouthFrownLeft,
        getf(FaceFb::LipCornerDepressorL),
    );
    shapes.setu(
        UnifiedExpressions::MouthStretchRight,
        getf(FaceFb::LipStretcherR),
    );
    shapes.setu(
        UnifiedExpressions::MouthStretchLeft,
        getf(FaceFb::LipStretcherL),
    );

    shapes.setu(
        UnifiedExpressions::MouthDimpleLeft,
        (getf(FaceFb::DimplerL) * 2.0).min(1.0),
    );
    shapes.setu(
        UnifiedExpressions::MouthDimpleRight,
        (getf(FaceFb::DimplerR) * 2.0).min(1.0),
    );

    shapes.setu(
        UnifiedExpressions::MouthRaiserUpper,
        getf(FaceFb::ChinRaiserT),
    );
    shapes.setu(
        UnifiedExpressions::MouthRaiserLower,
        getf(FaceFb::ChinRaiserB),
    );
    shapes.setu(
        UnifiedExpressions::MouthPressRight,
        getf(FaceFb::LipPressorR),
    );
    shapes.setu(
        UnifiedExpressions::MouthPressLeft,
        getf(FaceFb::LipPressorL),
    );
    shapes.setu(
        UnifiedExpressions::MouthTightenerRight,
        getf(FaceFb::LipTightenerR),
    );
    shapes.setu(
        UnifiedExpressions::MouthTightenerLeft,
        getf(FaceFb::LipTightenerL),
    );

    if face_fb.len() >= Face2Fb::Max as usize {
        shapes.setu(UnifiedExpressions::TongueOut, getf2(Face2Fb::TongueOut));
        shapes.setu(
            UnifiedExpressions::TongueCurlUp,
            getf2(Face2Fb::TongueTipAlveolar),
        );
    }

    Some(shapes)
}
