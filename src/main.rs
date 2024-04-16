#![allow(dead_code)]

use crate::core::AvatarOsc;
use env_logger::Env;
use indicatif::MultiProgress;
use indicatif_log_bridge::LogWrapper;

mod core;

const VRC_PORT: u16 = 9000;
const OSC_PORT: u16 = 9002;

fn main() {
    let log = env_logger::Builder::from_env(Env::default().default_filter_or("info")).build();
    let multi = MultiProgress::new();
    LogWrapper::new(multi.clone(), log).try_init().unwrap();

    let mut osc = AvatarOsc::new(OSC_PORT, VRC_PORT, multi);

    osc.handle_messages();
}
