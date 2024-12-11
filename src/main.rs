#![allow(dead_code)]

use crate::core::AvatarOsc;

use clap::Parser;
use env_logger::Env;
use indicatif::MultiProgress;
use indicatif_log_bridge::LogWrapper;

mod core;

fn main() {
    let log = env_logger::Builder::from_env(Env::default().default_filter_or("info"))
        .filter_module("mdns_sd", log::LevelFilter::Warn)
        .format_target(false)
        .format_module_path(false)
        .build();
    let multi = MultiProgress::new();
    LogWrapper::new(multi.clone(), log).try_init().unwrap();

    let args = Args::parse();

    let mut osc = AvatarOsc::new(args, multi);

    osc.handle_messages();
}

#[derive(Default, Debug, Clone, clap::Subcommand)]
pub enum FaceSetup {
    #[default]
    #[clap(subcommand, hide = true)]
    /// Do not use face tracking
    Dummy,
    #[cfg(feature = "openxr")]
    /// Retrieve face data from OpenXR (WiVRn / Monado)
    Openxr,

    #[cfg(feature = "alvr")]
    /// Retrieve face data from ALVR
    Alvr,

    #[cfg(feature = "babble")]
    /// Retrieve face data from Babble and Etvr
    Babble {
        /// The port to listen on for Babble and ETVR packets.
        #[arg(short, long, default_value = "9400")]
        listen: u16,
    },
}

/// OSC Avatar Manager
#[derive(Default, clap::Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Provider to use for face data
    #[command(subcommand)]
    face: FaceSetup,

    /// OSC port for VRC
    #[arg(long, default_value = "9000")]
    vrc_port: u16,

    /// OSC listen port. Set this same port for VrcAdvert's osc_port!
    #[arg(long, default_value = "9002")]
    osc_port: u16,

    /// The OSC-JSON avatar file to use. See ~/.config/oscavmgr-avatar.json
    #[arg(long)]
    avatar: Option<String>,
}
