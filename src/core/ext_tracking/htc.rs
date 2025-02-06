use super::unified::{UnifiedExpressions, UnifiedShapeAccessors, UnifiedShapes, NUM_SHAPES};
use openxr as xr;

pub(crate) struct HtcFacialData {
    pub(super) eye: Option<[f32; xr::sys::FACIAL_EXPRESSION_EYE_COUNT_HTC]>,
    pub(super) lip: Option<[f32; xr::sys::FACIAL_EXPRESSION_LIP_COUNT_HTC]>,
}

impl HtcFacialData {
    #[inline(always)]
    fn eyef(&self, idx: xr::EyeExpressionHTC) -> f32 {
        self.eye
            .map(|arr| arr[idx.into_raw() as usize])
            .unwrap_or_default()
    }

    #[inline(always)]
    fn lipf(&self, idx: xr::LipExpressionHTC) -> f32 {
        self.lip
            .map(|arr| arr[idx.into_raw() as usize])
            .unwrap_or_default()
    }
}

pub(crate) fn htc_to_unified(d: &HtcFacialData) -> UnifiedShapes {
    let mut shapes: UnifiedShapes = [0.0; NUM_SHAPES];

    shapes.setu(
        UnifiedExpressions::EyeRightX,
        d.eyef(xr::EyeExpressionHTC::RIGHT_OUT) - d.eyef(xr::EyeExpressionHTC::RIGHT_IN),
    );
    shapes.setu(
        UnifiedExpressions::EyeLeftX,
        d.eyef(xr::EyeExpressionHTC::LEFT_IN) - d.eyef(xr::EyeExpressionHTC::LEFT_OUT),
    );
    shapes.setu(
        UnifiedExpressions::EyeY,
        (d.eyef(xr::EyeExpressionHTC::LEFT_UP) + d.eyef(xr::EyeExpressionHTC::RIGHT_UP)
            - d.eyef(xr::EyeExpressionHTC::LEFT_DOWN)
            - d.eyef(xr::EyeExpressionHTC::RIGHT_DOWN))
            / 2.0,
    );

    shapes.setu(
        UnifiedExpressions::EyeClosedLeft,
        d.eyef(xr::EyeExpressionHTC::LEFT_BLINK),
    );
    shapes.setu(
        UnifiedExpressions::EyeClosedRight,
        d.eyef(xr::EyeExpressionHTC::RIGHT_BLINK),
    );

    shapes.setu(
        UnifiedExpressions::EyeSquintRight,
        d.eyef(xr::EyeExpressionHTC::RIGHT_SQUEEZE),
    );
    shapes.setu(
        UnifiedExpressions::EyeSquintLeft,
        d.eyef(xr::EyeExpressionHTC::LEFT_SQUEEZE),
    );
    shapes.setu(
        UnifiedExpressions::EyeWideRight,
        d.eyef(xr::EyeExpressionHTC::RIGHT_WIDE),
    );
    shapes.setu(
        UnifiedExpressions::EyeWideLeft,
        d.eyef(xr::EyeExpressionHTC::LEFT_WIDE),
    );

    shapes.setu(
        UnifiedExpressions::BrowPinchRight,
        d.eyef(xr::EyeExpressionHTC::RIGHT_SQUEEZE),
    );
    shapes.setu(
        UnifiedExpressions::BrowPinchLeft,
        d.eyef(xr::EyeExpressionHTC::LEFT_SQUEEZE),
    );
    shapes.setu(
        UnifiedExpressions::BrowLowererRight,
        d.eyef(xr::EyeExpressionHTC::RIGHT_BLINK),
    );
    shapes.setu(
        UnifiedExpressions::BrowLowererLeft,
        d.eyef(xr::EyeExpressionHTC::LEFT_BLINK),
    );

    shapes.setu(
        UnifiedExpressions::CheekPuffRight,
        d.lipf(xr::LipExpressionHTC::CHEEK_PUFF_RIGHT),
    );
    shapes.setu(
        UnifiedExpressions::CheekPuffLeft,
        d.lipf(xr::LipExpressionHTC::CHEEK_PUFF_LEFT),
    );
    shapes.setu(
        UnifiedExpressions::CheekSuckRight,
        d.lipf(xr::LipExpressionHTC::CHEEK_SUCK),
    );
    shapes.setu(
        UnifiedExpressions::CheekSuckLeft,
        d.lipf(xr::LipExpressionHTC::CHEEK_SUCK),
    );

    shapes.setu(
        UnifiedExpressions::JawOpen,
        d.lipf(xr::LipExpressionHTC::JAW_OPEN),
    );
    shapes.setu(
        UnifiedExpressions::JawRight,
        d.lipf(xr::LipExpressionHTC::JAW_RIGHT),
    );
    shapes.setu(
        UnifiedExpressions::JawLeft,
        d.lipf(xr::LipExpressionHTC::JAW_LEFT),
    );
    shapes.setu(
        UnifiedExpressions::JawForward,
        d.lipf(xr::LipExpressionHTC::JAW_FORWARD),
    );
    shapes.setu(
        UnifiedExpressions::MouthClosed,
        d.lipf(xr::LipExpressionHTC::MOUTH_APE_SHAPE),
    );

    let suck_upper = d.lipf(xr::LipExpressionHTC::MOUTH_UPPER_INSIDE);
    shapes.setu(UnifiedExpressions::LipSuckUpperRight, suck_upper);
    shapes.setu(UnifiedExpressions::LipSuckUpperLeft, suck_upper);

    let suck_lower = d.lipf(xr::LipExpressionHTC::MOUTH_LOWER_INSIDE);
    shapes.setu(UnifiedExpressions::LipSuckLowerRight, suck_lower);
    shapes.setu(UnifiedExpressions::LipSuckLowerLeft, suck_lower);

    let upper_funnel = d.lipf(xr::LipExpressionHTC::MOUTH_LOWER_OVERTURN);
    shapes.setu(UnifiedExpressions::LipFunnelUpperRight, upper_funnel);
    shapes.setu(UnifiedExpressions::LipFunnelUpperLeft, upper_funnel);

    let lower_funnel = d.lipf(xr::LipExpressionHTC::MOUTH_LOWER_OVERTURN);
    shapes.setu(UnifiedExpressions::LipFunnelLowerRight, lower_funnel);
    shapes.setu(UnifiedExpressions::LipFunnelLowerLeft, lower_funnel);

    let pout = d.lipf(xr::LipExpressionHTC::MOUTH_POUT);
    shapes.setu(UnifiedExpressions::LipPuckerUpperRight, pout);
    shapes.setu(UnifiedExpressions::LipPuckerUpperLeft, pout);
    shapes.setu(UnifiedExpressions::LipPuckerLowerRight, pout);
    shapes.setu(UnifiedExpressions::LipPuckerLowerLeft, pout);

    shapes.setu(
        UnifiedExpressions::MouthLowerDownRight,
        d.lipf(xr::LipExpressionHTC::MOUTH_LOWER_DOWNRIGHT),
    );
    shapes.setu(
        UnifiedExpressions::MouthLowerDownLeft,
        d.lipf(xr::LipExpressionHTC::MOUTH_LOWER_DOWNLEFT),
    );

    let mouth_upper_up_right = d.lipf(xr::LipExpressionHTC::MOUTH_UPPER_UPRIGHT);
    let mouth_upper_up_left = d.lipf(xr::LipExpressionHTC::MOUTH_UPPER_UPLEFT);

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
        d.lipf(xr::LipExpressionHTC::MOUTH_UPPER_RIGHT),
    );
    shapes.setu(
        UnifiedExpressions::MouthUpperLeft,
        d.lipf(xr::LipExpressionHTC::MOUTH_UPPER_LEFT),
    );
    shapes.setu(
        UnifiedExpressions::MouthLowerRight,
        d.lipf(xr::LipExpressionHTC::MOUTH_LOWER_RIGHT),
    );
    shapes.setu(
        UnifiedExpressions::MouthLowerLeft,
        d.lipf(xr::LipExpressionHTC::MOUTH_LOWER_LEFT),
    );

    let smile_left = d.lipf(xr::LipExpressionHTC::MOUTH_SMILE_RIGHT);
    shapes.setu(UnifiedExpressions::MouthCornerPullLeft, smile_left);
    shapes.setu(UnifiedExpressions::MouthCornerSlantLeft, smile_left);

    let smile_right = d.lipf(xr::LipExpressionHTC::MOUTH_SMILE_RIGHT);
    shapes.setu(UnifiedExpressions::MouthCornerPullRight, smile_right);
    shapes.setu(UnifiedExpressions::MouthCornerSlantRight, smile_right);

    let sad_left = d.lipf(xr::LipExpressionHTC::MOUTH_SAD_LEFT);
    shapes.setu(UnifiedExpressions::MouthFrownLeft, sad_left);
    shapes.setu(UnifiedExpressions::MouthStretchLeft, sad_left);

    let sad_right = d.lipf(xr::LipExpressionHTC::MOUTH_SAD_RIGHT);
    shapes.setu(UnifiedExpressions::MouthFrownRight, sad_right);
    shapes.setu(UnifiedExpressions::MouthStretchRight, sad_right);

    let press = (d.lipf(xr::LipExpressionHTC::MOUTH_UPPER_INSIDE)
        + d.lipf(xr::LipExpressionHTC::MOUTH_LOWER_INSIDE))
        / 2.0;
    shapes.setu(UnifiedExpressions::MouthPressRight, press);
    shapes.setu(UnifiedExpressions::MouthPressLeft, press);

    shapes.setu(
        UnifiedExpressions::TongueOut,
        (d.lipf(xr::LipExpressionHTC::TONGUE_LONGSTEP1)
            + d.lipf(xr::LipExpressionHTC::TONGUE_LONGSTEP2))
            / 2.0,
    );
    shapes.setu(
        UnifiedExpressions::TongueUp,
        d.lipf(xr::LipExpressionHTC::TONGUE_UP),
    );
    shapes.setu(
        UnifiedExpressions::TongueDown,
        d.lipf(xr::LipExpressionHTC::TONGUE_DOWN),
    );
    shapes.setu(
        UnifiedExpressions::TongueLeft,
        d.lipf(xr::LipExpressionHTC::TONGUE_LEFT),
    );
    shapes.setu(
        UnifiedExpressions::TongueRight,
        d.lipf(xr::LipExpressionHTC::TONGUE_RIGHT),
    );
    shapes.setu(
        UnifiedExpressions::TongueRoll,
        d.lipf(xr::LipExpressionHTC::TONGUE_ROLL),
    );
    shapes.setu(
        UnifiedExpressions::TongueTwistLeft,
        d.lipf(xr::LipExpressionHTC::TONGUE_UPLEFT_MORPH)
            + d.lipf(xr::LipExpressionHTC::TONGUE_DOWNRIGHT_MORPH),
    );
    shapes.setu(
        UnifiedExpressions::TongueTwistRight,
        d.lipf(xr::LipExpressionHTC::TONGUE_UPRIGHT_MORPH)
            + d.lipf(xr::LipExpressionHTC::TONGUE_DOWNLEFT_MORPH),
    );

    /* does not map:
        UnifiedExpressions::BrowInnerUpLeft,
        UnifiedExpressions::BrowInnerUpRight,
        UnifiedExpressions::BrowOuterUpLeft,
        UnifiedExpressions::BrowOuterUpRight,
        UnifiedExpressions::CheekSquintLeft,
        UnifiedExpressions::CheekSquintRight,
        UnifiedExpressions::NoseSneerLeft,
        UnifiedExpressions::NoseSneerRight,
        UnifiedExpressions::MouthDimpleLeft,
        UnifiedExpressions::MouthDimpleRight,
        UnifiedExpressions::MouthTightenerLeft,
        UnifiedExpressions::MouthTightenerRight,
    */

    shapes
}
