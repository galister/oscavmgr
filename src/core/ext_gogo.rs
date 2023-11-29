use std::fs::File;

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
        if let OscType::Int(value) = value {
            match name {
                STAND_PARAM if self.idle_stand != *value => {
                    self.save();
                    self.idle_stand = *value
                }
                CROUCH_PARAM if self.idle_crouch != *value => {
                    self.save();
                    self.idle_crouch = *value
                }
                PRONE_PARAM if self.idle_prone != *value => {
                    self.save();
                    self.idle_prone = *value
                }
                _ => (),
            }
        }
    }

    pub fn avatar(&self, bundle: &mut OscBundle) {
        info!(
            "Setting Go Pose params: {} {} {}",
            self.idle_stand, self.idle_crouch, self.idle_prone
        );
        bundle.send_parameter(STAND_PARAM, OscType::Int(self.idle_stand));
        bundle.send_parameter(CROUCH_PARAM, OscType::Int(self.idle_crouch));
        bundle.send_parameter(PRONE_PARAM, OscType::Int(self.idle_prone));
    }

    pub fn step(&self, parameters: &AvatarParameters, bundle: &mut OscBundle) {
        if let Some(OscType::Int(tracking)) = parameters.get(TRACKING_TYPE) {
            let want_loco = if 5 < *tracking {
                OscType::Bool(false)
            } else {
                OscType::Bool(true)
            };

            if parameters.get(LOCO_PARAM) != Some(&want_loco) {
                info!("Set Locomotion: {:?}", want_loco);
                bundle.send_parameter(LOCO_PARAM, want_loco);
            }
        }
    }
}
