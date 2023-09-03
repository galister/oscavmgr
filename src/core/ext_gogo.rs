use std::{path::Path, fs::File};

use log::info;
use rosc::{OscType, OscBundle};
use serde::{Serialize, Deserialize};

use super::bundle::AvatarBundle;

const FILE_NAME: &str = "extGogo.json";

const STAND_PARAM: &str = "Go/StandIdle";
const CROUCH_PARAM: &str = "Go/CrouchIdle";
const PRONE_PARAM: &str = "Go/ProneIdle";

#[derive(Serialize, Deserialize, Default)]
pub struct ExtGogo { 
    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    path: Option<String>,

    idle_stand: i32,
    idle_crouch: i32,
    idle_prone: i32,
}

impl ExtGogo {
    pub fn new() -> ExtGogo {
        let path = std::env::var("XDG_CONFIG_HOME")
            .or_else(|_| std::env::var("HOME").map(|home| format!("{}/.config", home)))
            .map(|config| Path::new(&config).join(FILE_NAME))
            .ok()
            .and_then(|path| path.to_str().map(|path| path.to_string()));

        let mut me = path
            .as_ref()
            .and_then(|path| {
                File::open(path)
                    .ok()
                    .and_then(|file| serde_json::from_reader(file).ok())
            })
            .unwrap_or_else(|| Some(ExtGogo::default()))
            .unwrap();
        me.path = path;

        me
    }

    fn save(&mut self) {
        self.path.as_ref().and_then(|path| {
            info!("Saving ExtGogo to {}", path);
            File::create(path)
                .ok()
                .and_then(|file| serde_json::to_writer(file, self).ok())
        });
    }

    pub fn notify(&mut self, name: &str, value: &OscType) {
        if let OscType::Int(value) = value {
            match name {
                STAND_PARAM if self.idle_stand != *value => {
                    self.save();
                    self.idle_stand = *value
                },
                CROUCH_PARAM if self.idle_crouch != *value => {
                    self.save();
                    self.idle_crouch = *value
                },
                PRONE_PARAM if self.idle_prone != *value => {
                    self.save();
                    self.idle_prone = *value
                },
                _ => (),
            }
        }
    }

    pub fn avatar(&self, bundle: &mut OscBundle) {
        info!("Setting Go Pose params: {} {} {}", self.idle_stand, self.idle_crouch, self.idle_prone);
        bundle.send_parameter(STAND_PARAM, OscType::Int(self.idle_stand));
        bundle.send_parameter(CROUCH_PARAM, OscType::Int(self.idle_crouch));
        bundle.send_parameter(PRONE_PARAM, OscType::Int(self.idle_prone));
    }
}
