use crate::core::ext_tracking::face2_fb::face2_fb_to_unified;

use super::unified::{UnifiedExpressions, UnifiedShapeAccessors, UnifiedShapes, NUM_ALL_SHAPES};

enum FaceAndroid {
    TongueOut = 63,
    TongueLeft,
    TongueRight,
    TongueUp,
    TongueDown,
}

pub(crate) fn android_to_unified(face_android: &[f32]) -> Option<UnifiedShapes> {

    let shapes: Option<UnifiedShapes> = face2_fb_to_unified(face_android);

    match shapes {
        None => None,
        Some(mut shapes) => {
            let getf = |index| face_android[index as usize];

            shapes.setu(
                UnifiedExpressions::TongueOut,
                getf(FaceAndroid::TongueOut),
            );
            shapes.setu(
                UnifiedExpressions::TongueLeft,
                getf(FaceAndroid::TongueLeft),
            );
            shapes.setu(
                UnifiedExpressions::TongueRight,
                getf(FaceAndroid::TongueRight),
            );
            shapes.setu(
                UnifiedExpressions::TongueUp,
                getf(FaceAndroid::TongueUp),
            );
            shapes.setu(
                UnifiedExpressions::TongueDown,
                getf(FaceAndroid::TongueDown),
            );
            Some(shapes)
        }
    }
}
