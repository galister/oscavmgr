#![allow(dead_code)]
use std::env;

use crate::core::AvatarOsc;
use env_logger::Env;

mod core;

const VRC_PORT: u16 = 9000;
const OSC_PORT: u16 = 9002;

fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let mut osc = AvatarOsc::new(OSC_PORT, VRC_PORT);

    let headless = env::args().any(|arg| arg == "--headless");

    if headless {
        osc.run_headless();
    } else {
        osc.handle_messages();
    }
}
