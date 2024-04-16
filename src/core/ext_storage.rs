use std::{fs::File, time::Instant};

use log::{debug, info};
use rosc::{OscBundle, OscType};

use super::{bundle::AvatarBundle, folders::CONFIG_DIR};

const FILE_NAME: &str = "extMem.json";
const LENGTH: usize = 255;

pub struct ExtStorage {
    path: String,
    data: Vec<f32>,
    ext_index: usize,
    ext_value: f32,
    int_index: usize,
    last_save: Instant,
    last_tick: Instant,
}

impl ExtStorage {
    pub fn new() -> ExtStorage {
        let path = format!("{}/{}", CONFIG_DIR.as_ref(), FILE_NAME);

        let data: Vec<f32> = File::open(&path)
            .ok()
            .and_then(|file| serde_json::from_reader(file).ok())
            .unwrap_or_else(|| Some(vec![-1.; LENGTH]))
            .unwrap();

        ExtStorage {
            path,
            data,
            ext_index: 0,
            ext_value: 0.0,
            last_save: Instant::now(),
            last_tick: Instant::now(),
            int_index: 0,
        }
    }

    fn save(&mut self) {
        self.last_save = Instant::now();
        info!("Saving ExtStorage to {}", &self.path);
        File::create(&self.path)
            .ok()
            .and_then(|file| serde_json::to_writer(file, &self.data).ok());
    }

    pub fn notify(&mut self, name: &str, value: &OscType) {
        match (name, value) {
            ("ExtIndex", OscType::Int(index)) => {
                self.ext_index = *index as _;
                if self.ext_value > f32::EPSILON {
                    self.data[self.ext_index] = self.ext_value;
                    self.int_index = 0;
                }
            }
            ("ExtValue", OscType::Float(value)) => {
                self.ext_value = *value;
                if self.ext_index > 0 {
                    self.data[self.ext_index] = self.ext_value;
                    self.int_index = 0;
                }
            }
            _ => (),
        }
    }

    fn next(&mut self) -> Option<f32> {
        let start_idx = self.int_index;
        loop {
            self.int_index += 1;
            if self.int_index == start_idx {
                return None;
            }
            if self.int_index >= LENGTH {
                self.int_index = 0;
                return None;
            }
            let value = self.data[self.int_index];
            if value >= 0. {
                return Some(value);
            }
        }
    }

    pub fn step(&mut self, bundle: &mut OscBundle) {
        if Instant::now()
            .saturating_duration_since(self.last_tick)
            .as_millis()
            < 250
        {
            return;
        }
        if self.ext_index != 0 {
            self.int_index = 0;
            debug!("ExtIndex {}", self.ext_index);
            return;
        }

        if let Some(value) = self.next() {
            self.last_tick = Instant::now();
            debug!("Sending {} {}", self.int_index, value);

            bundle.send_parameter("IntIndex", OscType::Int(self.int_index as _));
            bundle.send_parameter("IntValue", OscType::Float(value));
        }

        if Instant::now()
            .saturating_duration_since(self.last_save)
            .as_secs()
            > 300
        {
            self.save();
        }
    }
}
