use std::{
    array,
    ops::Add,
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant},
};

use once_cell::sync::Lazy;
use regex::Regex;
use rosc::{OscBundle, OscType};
use sranipal::SRanipalExpression;

#[cfg(feature = "alvr")]
use self::alvr::AlvrReceiver;

#[cfg(feature = "babble")]
use self::babble::BabbleReceiver;

#[cfg(feature = "wivrn")]
use self::openxr::OpenXrReceiver;

use self::unified::{CombinedExpression, UnifiedExpressions, UnifiedTrackingData, NUM_SHAPES};

use super::{
    ext_oscjson::{MysteryParam, OscJsonNode},
    AppState,
};

#[cfg(feature = "alvr")]
mod alvr;
#[cfg(feature = "babble")]
mod babble;
mod face2_fb;
#[cfg(feature = "wivrn")]
mod openxr;
mod sranipal;
pub mod unified;

pub struct ExtTracking {
    pub data: UnifiedTrackingData,
    params: [Option<MysteryParam>; NUM_SHAPES],
    alvr_receiver: AlvrReceiver,
    babble_receiver: BabbleReceiver,
    openxr_receiver: Option<OpenXrReceiver>,
    openxr_next_try: Instant,
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

        let alvr_receiver = AlvrReceiver::new().unwrap(); // never fails
        alvr_receiver.start_loop();

        let babble_receiver = BabbleReceiver::new().unwrap(); // never fails
        babble_receiver.start_loop();

        let openxr_receiver = OpenXrReceiver::new().ok();

        let me = Self {
            data: UnifiedTrackingData::default(),
            params,
            alvr_receiver,
            babble_receiver,
            openxr_receiver,
            openxr_next_try: Instant::now().add(Duration::from_secs(10)),
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
            // don't remove this mut, idk why but rust-analyzer contradicts clippy
            if let Some(mut oxr) = self.openxr_receiver.take() {
                if let Err(e) = oxr.receive(&mut self.data, state) {
                    log::debug!("OpenXR error: {}", e);
                    self.openxr_next_try = Instant::now().add(Duration::from_secs(10));
                } else {
                    self.openxr_receiver = Some(oxr);
                }
            } else if self.openxr_next_try < Instant::now() {
                self.openxr_receiver = OpenXrReceiver::new().ok();
                self.openxr_next_try = Instant::now().add(Duration::from_secs(10));
            }

            self.alvr_receiver.receive(&mut self.data, state).ok();
            self.babble_receiver.receive(&mut self.data, state).ok();
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

            log::debug!("Match: {}", name);

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

struct DummyReceiver;
impl DummyReceiver {
    fn new() -> anyhow::Result<Self> {
        Ok(Self)
    }
    fn start_loop(&self) {}
    fn receive(&self, _data: &mut UnifiedTrackingData, _: &mut AppState) -> anyhow::Result<()> {
        Ok(())
    }
    fn restart(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
}

#[cfg(not(feature = "alvr"))]
type AlvrReceiver = DummyReceiver;

#[cfg(not(feature = "babble"))]
type BabbleReceiver = DummyReceiver;

#[cfg(not(feature = "wivrn"))]
type OpenXrReceiver = DummyReceiver;
