use std::{array, str::FromStr, sync::Arc};

use once_cell::sync::Lazy;
use regex::Regex;
use rosc::{OscBundle, OscType};
use sranipal::SRanipalExpression;

use crate::FaceSetup;

#[cfg(feature = "alvr")]
use self::alvr::AlvrReceiver;

#[cfg(feature = "babble")]
use self::babble::BabbleEtvrReceiver;

#[cfg(feature = "openxr")]
use self::openxr::OpenXrReceiver;

use self::unified::{CombinedExpression, UnifiedExpressions, UnifiedTrackingData, NUM_SHAPES};

use super::{
    ext_oscjson::{MysteryParam, OscJsonNode},
    AppState,
};

use strum::EnumCount;
use strum::IntoEnumIterator;

#[cfg(feature = "alvr")]
mod alvr;
#[cfg(feature = "babble")]
mod babble;
mod face2_fb;
#[cfg(feature = "openxr")]
mod htc;
#[cfg(feature = "openxr")]
mod openxr;
mod sranipal;
pub mod unified;

trait FaceReceiver {
    fn start_loop(&mut self);
    fn receive(&mut self, _data: &mut UnifiedTrackingData, _: &mut AppState);
}

struct DummyReceiver;

impl FaceReceiver for DummyReceiver {
    fn start_loop(&mut self) {}
    fn receive(&mut self, _data: &mut UnifiedTrackingData, _: &mut AppState) {}
}

pub struct ExtTracking {
    pub data: UnifiedTrackingData,
    params: [Option<MysteryParam>; NUM_SHAPES],
    receiver: Box<dyn FaceReceiver>,
}

impl ExtTracking {
    pub fn new(setup: FaceSetup) -> Self {
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
            params[e as usize] = Some(new);
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

        let receiver: Box<dyn FaceReceiver> = match setup {
            FaceSetup::Dummy => Box::new(DummyReceiver {}),
            #[cfg(feature = "alvr")]
            FaceSetup::Alvr => Box::new(AlvrReceiver::new()),
            #[cfg(feature = "openxr")]
            FaceSetup::Openxr => Box::new(OpenXrReceiver::new()),
            #[cfg(feature = "babble")]
            FaceSetup::Babble { listen } => Box::new(BabbleEtvrReceiver::new(listen)),
        };

        let mut me = Self {
            data: UnifiedTrackingData::default(),
            params,
            receiver,
        };

        log::info!("--- Default params ---");
        me.print_params();

        me.receiver.start_loop();

        me
    }

    pub fn step(&mut self, state: &mut AppState, bundle: &mut OscBundle) {
        let motion = matches!(state.params.get("Motion"), Some(OscType::Int(1)));
        let face_override = matches!(state.params.get("FaceFreeze"), Some(OscType::Bool(true)));
        let afk = matches!(state.params.get("AFK"), Some(OscType::Bool(true)))
            || matches!(state.params.get("IsAfk"), Some(OscType::Bool(true)));

        if afk {
            log::debug!("AFK");
        } else if motion ^ face_override {
            log::debug!("Freeze");
        } else {
            self.receiver.receive(&mut self.data, state);
            self.data.calc_combined(state);
        }

        if matches!(state.params.get("FacePause"), Some(OscType::Bool(true))) {
            log::debug!("FacePause");
            return;
        }

        self.data.apply_to_bundle(&mut self.params, bundle);
    }

    pub fn osc_json(&mut self, avatar_node: &OscJsonNode) {
        self.params.iter_mut().for_each(|p| *p = None);

        let Some(parameters) = avatar_node.get("parameters") else {
            log::warn!("oscjson: Could not read /avatar/parameters");
            return;
        };

        self.process_node_recursive("parameters", parameters);
        self.print_params();
    }

    fn process_node_recursive(&mut self, name: &str, node: &OscJsonNode) -> Option<()> {
        static FT_PARAMS_REGEX: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"^(.+?)(Negative|\d+)?$").unwrap());

        if let Some(contents) = node.contents.as_ref() {
            log::debug!("Checking {}", name);
            for (name, node) in contents.iter() {
                let _ = self.process_node_recursive(name, node);
            }
            return None;
        }

        if let Some(m) = FT_PARAMS_REGEX.captures(name) {
            let main: Arc<str> = m[1].into();

            log::debug!("Param: {}", name);
            let idx = UnifiedExpressions::from_str(&main)
                .map(|e| e as usize)
                .or_else(|_| CombinedExpression::from_str(&main).map(|e| e as usize))
                .or_else(|_| SRanipalExpression::from_str(&main).map(|e| e as usize))
                .ok()?;

            log::debug!(
                "Match: {}",
                UnifiedExpressions::iter()
                    .nth(idx)
                    .map(|e| format!("UnifiedExpressions::{:?}", e))
                    .or_else(|| CombinedExpression::iter()
                        .nth(idx - UnifiedExpressions::COUNT)
                        .map(|e| format!("CombinedExpression::{:?}", e)))
                    .or_else(|| Some("None".to_string()))
                    .unwrap()
            );

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
        None
    }

    fn print_params(&self) {
        for v in self.params.iter().filter_map(|p| p.as_ref()) {
            let mut elems = vec![];

            if v.main_address.is_some() {
                elems.push("float".into())
            }
            if v.num_bits > 0 {
                elems.push(if v.num_bits > 1 {
                    format!("{} bit", v.num_bits)
                } else {
                    format!("{} bits", v.num_bits)
                });
            }
            if v.neg_address.is_some() {
                elems.push("neg".into());
            }
            log::info!("{}: {}", v.name, elems.join(" + "))
        }
    }
}
