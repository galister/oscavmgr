use std::fs::File;
use std::time::Instant;

use log::info;
use rosc::{OscBundle, OscType};
use serde::{Deserialize, Serialize};

use super::bundle::AvatarBundle;
use super::folders::CONFIG_DIR;
use super::AvatarParameters;

const FILE_NAME: &str = "extGogo.json";

const STAND_PARAM: &str = "Go/StandIdle";
const CROUCH_PARAM: &str = "Go/CrouchIdle";
const PRONE_PARAM: &str = "Go/ProneIdle";
const LOCO_PARAM: &str = "Go/Locomotion";

const TRACKING_TYPE: &str = "TrackingType";

#[derive(Serialize, Deserialize, Default)]
pub struct ExtGogo {
    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    path: String,

    idle_stand: i32,
    idle_crouch: i32,
    idle_prone: i32,

    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    staging: Option<Staging>,

    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    avatar_changed: Option<Instant>,
}

impl ExtGogo {
    pub fn new() -> ExtGogo {
        let path = format!("{}/{}", CONFIG_DIR.as_ref(), FILE_NAME);

        let mut me = File::open(&path)
            .ok()
            .and_then(|file| serde_json::from_reader(file).ok())
            .unwrap_or_else(|| Some(ExtGogo::default()))
            .unwrap();

        me.path = path;

        me
    }

    fn save(&mut self) {
        info!("Saving ExtGogo to {}", &self.path);
        File::create(&self.path)
            .ok()
            .and_then(|file| serde_json::to_writer(file, self).ok());
    }

    pub fn notify(&mut self, name: &str, value: &OscType) {
        if self
            .avatar_changed
            .is_some_and(|t| t.elapsed().as_secs() < 5)
        {
            return;
        }

        if let OscType::Int(value) = value {
            match name {
                STAND_PARAM if self.idle_stand != *value => {
                    let staging = Staging::from_live(self);
                    staging.idle_stand = *value;
                }
                CROUCH_PARAM if self.idle_crouch != *value => {
                    let staging = Staging::from_live(self);
                    staging.idle_crouch = *value
                }
                PRONE_PARAM if self.idle_prone != *value => {
                    let staging = Staging::from_live(self);
                    staging.idle_prone = *value
                }
                _ => (),
            }
        }
    }

    pub fn avatar(&mut self, bundle: &mut OscBundle) {
        info!(
            "Setting Go Pose params: {} {} {}",
            self.idle_stand, self.idle_crouch, self.idle_prone
        );
        self.staging = None;
        self.avatar_changed = Some(Instant::now());
        bundle.send_parameter(STAND_PARAM, OscType::Int(self.idle_stand));
        bundle.send_parameter(CROUCH_PARAM, OscType::Int(self.idle_crouch));
        bundle.send_parameter(PRONE_PARAM, OscType::Int(self.idle_prone));
    }

    pub fn step(&mut self, parameters: &AvatarParameters, bundle: &mut OscBundle) {
        if let Some(OscType::Int(tracking)) = parameters.get(TRACKING_TYPE) {
            let want_loco = if 5 < *tracking {
                OscType::Bool(true)
            } else {
                OscType::Bool(false)
            };

            if parameters.get(LOCO_PARAM) != Some(&want_loco) {
                info!("Set Locomotion: {:?}", want_loco);
                bundle.send_parameter(LOCO_PARAM, want_loco);
            }
        }

        if let Some(staging) = self.staging.take() {
            let elapsed = staging.time.elapsed().as_secs();
            if elapsed < 5 {
                self.staging = Some(staging);
            } else {
                info!("Committing Go Pose params");
                staging.commit(self);
                self.save();
            }
        }
    }
}

#[derive(Clone)]
struct Staging {
    pub idle_stand: i32,
    pub idle_crouch: i32,
    pub idle_prone: i32,
    time: Instant,
}

impl Staging {
    fn from_live(gogo: &mut ExtGogo) -> &mut Self {
        if let Some(staging) = gogo.staging.as_mut() {
            staging.time = Instant::now();
        }
        gogo.staging.get_or_insert_with(|| Self {
            idle_stand: gogo.idle_stand,
            idle_crouch: gogo.idle_crouch,
            idle_prone: gogo.idle_prone,
            time: Instant::now(),
        })
    }

    fn commit(self, gogo: &mut ExtGogo) {
        gogo.idle_stand = self.idle_stand;
        gogo.idle_crouch = self.idle_crouch;
        gogo.idle_prone = self.idle_prone;
    }
}
