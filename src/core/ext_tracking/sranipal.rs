use strum::{EnumCount, EnumIter, EnumString, IntoStaticStr};

use super::unified::{CombinedExpression, UnifiedExpressions};

#[allow(unused)]
#[repr(usize)]
#[derive(Debug, Clone, Copy, EnumIter, EnumCount, EnumString, IntoStaticStr)]
pub enum SRanipalExpression {
    LeftEyeX = UnifiedExpressions::EyeLeftX as _,
    RightEyeX = UnifiedExpressions::EyeRightX as _,
    EyesY = UnifiedExpressions::EyeY as _,

    EyeLeftWide = UnifiedExpressions::EyeWideLeft as _,
    EyeRightWide = UnifiedExpressions::EyeWideRight as _,
    EyeLeftBlink = UnifiedExpressions::EyeClosedLeft as _,
    EyeRightBlink = UnifiedExpressions::EyeClosedRight as _,
    EyeLeftSqueeze = UnifiedExpressions::EyeSquintLeft as _,
    EyeRightSqueeze = UnifiedExpressions::EyeSquintRight as _,
    CheekSuck = UnifiedExpressions::CheekSuckLeft as _,
    MouthApeShape = UnifiedExpressions::MouthClosed as _,
    MouthUpperInside = CombinedExpression::LipSuckUpper as usize,
    MouthLowerInside = CombinedExpression::LipSuckLower as usize,
    MouthUpperOverturn = CombinedExpression::LipFunnelUpper as usize,
    MouthLowerOverturn = CombinedExpression::LipFunnelLower as usize,
    MouthPout = CombinedExpression::LipPucker as usize,
    MouthLowerOverlay = UnifiedExpressions::MouthRaiserLower as _,
    TongueLongStep1 = UnifiedExpressions::TongueOut as _,
    /* duplicate names
    CheekPuffLeft = UnifiedExpressions::CheekPuffLeft as _,
    CheekPuffRight = UnifiedExpressions::CheekPuffRight as _,
    JawLeft = UnifiedExpressions::JawLeft as _,
    JawRight = UnifiedExpressions::JawRight as _,
    JawForward = UnifiedExpressions::JawForward as _,
    JawOpen = UnifiedExpressions::JawOpen as _,
    MouthSmileLeft = CombinedExpression::MouthSmileLeft as usize,
    MouthSmileRight = CombinedExpression::MouthSmileRight as usize,
    MouthSadLeft = CombinedExpression::MouthSadLeft as usize,
    MouthSadRight = CombinedExpression::MouthSadRight as usize,
    MouthUpperUpLeft = UnifiedExpressions::MouthUpperUpLeft as _,
    MouthUpperUpRight = UnifiedExpressions::MouthUpperUpRight as _,
    MouthLowerDownLeft = UnifiedExpressions::MouthLowerDownLeft as _,
    MouthLowerDownRight = UnifiedExpressions::MouthLowerDownRight as _,
    TongueUp = UnifiedExpressions::TongueUp as _,
    TongueDown = UnifiedExpressions::TongueDown as _,
    TongueLeft = UnifiedExpressions::TongueLeft as _,
    TongueRight = UnifiedExpressions::TongueRight as _,
    TongueRoll = UnifiedExpressions::TongueRoll as _,
    */
}
