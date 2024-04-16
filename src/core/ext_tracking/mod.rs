use std::{array, str::FromStr, sync::Arc};

use once_cell::sync::Lazy;
use regex::Regex;
use rosc::{OscBundle, OscType};
use strum::EnumCount;

#[cfg(feature = "alvr")]
use self::alvr::AlvrReceiver;

#[cfg(feature = "babble")]
use self::babble::BabbleReceiver;

use self::unified::{CombinedExpression, UnifiedExpressions, UnifiedTrackingData, NUM_SHAPES};

use super::{
    ext_oscjson::{MysteryParam, OscJsonNode},
    AppState,
};

#[cfg(feature = "alvr")]
mod alvr;
#[cfg(feature = "babble")]
mod babble;
pub mod unified;

pub struct ExtTracking {
    pub data: UnifiedTrackingData,
    params: [Option<MysteryParam>; NUM_SHAPES],
    alvr_receiver: AlvrReceiver,
    babble_receiver: BabbleReceiver,
}

impl ExtTracking {
    pub fn new() -> Self {
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
                last_value: 0.,
                last_bits: [false; 8],
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
                last_value: 0.,
                last_bits: [false; 8],
            };
            params[e as usize] = Some(new);
        }

        let alvr_receiver = AlvrReceiver::new();
        alvr_receiver.start_loop();

        let babble_receiver = BabbleReceiver::new();
        babble_receiver.start_loop();

        let me = Self {
            data: UnifiedTrackingData::default(),
            params,
            alvr_receiver,
            babble_receiver,
        };
        me.print_params();

        me
    }

    pub fn step(&mut self, state: &mut AppState, bundle: &mut OscBundle) {
        let motion = matches!(state.params.get("Motion"), Some(OscType::Int(1)));
        let face_override = matches!(state.params.get("FaceFreeze"), Some(OscType::Bool(true)));

        if motion ^ face_override {
            log::debug!("Freeze");
        } else {
            self.alvr_receiver.receive(&mut self.data, state);
            self.babble_receiver
                .receive(&mut self.data, &mut state.status);
            self.data.calc_combined();
        }

        if matches!(state.params.get("FacePause"), Some(OscType::Bool(true))) {
            log::debug!("FacePause");
            return;
        }

        self.data.apply_to_bundle(&mut self.params, bundle);
    }

    pub fn osc_json(&mut self, root_node: &OscJsonNode) {
        static FT_PARAMS_REGEX: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"^(.+?)(Negative|\d+)?$").unwrap());

        self.params.iter_mut().for_each(|p| *p = None);

        let _x: Option<()> = root_node
            .get("parameters")
            .and_then(|parameters| parameters.get("FT"))
            .and_then(|ft| ft.get("v2"))
            .and_then(|v2| {
                v2.contents.as_ref()?.iter().for_each(|(name, node)| {
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
                            log::warn!("Unknown expression: {}", &main);
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
                                last_value: 0.,
                                last_bits: [false; 8],
                            };
                            self.params[idx] = Some(new);
                        };

                        let stored = self.params[idx].as_mut().unwrap();
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
                log::info!("{}: float", v.name,);
            } else {
                log::info!(
                    "{}: {} bits {}",
                    v.name,
                    v.num_bits,
                    if v.neg_address.is_some() { "+ neg" } else { "" },
                );
            }
        }
    }
}

#[cfg(not(feature = "alvr"))]
struct AlvrReceiver;
#[cfg(not(feature = "alvr"))]
impl AlvrReceiver {
    fn new() -> Self {
        Self
    }
    fn start_loop(&self) {}
    fn receive(&self, _data: &mut UnifiedTrackingData, _: &mut AppState) {}
}

#[cfg(not(feature = "babble"))]
struct BabbleReceiver;
#[cfg(not(feature = "babble"))]
impl BabbleReceiver {
    fn new() -> Self {
        Self
    }
    fn start_loop(&self) {}
    fn receive(&self, _: &mut UnifiedTrackingData, _: &mut super::status::StatusBar) {}
}
