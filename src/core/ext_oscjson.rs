use log::{info, warn};
use mdns_sd::{ServiceDaemon, ServiceEvent};
use rosc::{OscBundle, OscType};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Write},
    sync::Arc,
    thread,
    time::Duration,
};

use super::{bundle::AvatarBundle, folders::CONFIG_DIR};

pub struct ExtOscJson {
    mdns: ServiceDaemon,
    mdns_recv: mdns_sd::Receiver<ServiceEvent>,
    oscjson_addr: Option<Arc<str>>,
    next_run: std::time::Instant,
    client: reqwest::blocking::Client,
}

impl ExtOscJson {
    pub fn new() -> Self {
        let mdns = ServiceDaemon::new().unwrap();
        let mdns_recv = mdns.browse("_oscjson._tcp.local.").unwrap();
        let client = reqwest::blocking::Client::new();

        Self {
            mdns,
            mdns_recv,
            oscjson_addr: None,
            next_run: std::time::Instant::now(),
            client,
        }
    }

    pub fn step(&mut self) -> bool {
        let mut notify_avatar = false;
        if self.next_run > std::time::Instant::now() {
            return notify_avatar;
        }
        self.next_run = std::time::Instant::now() + std::time::Duration::from_secs(15);

        for event in self.mdns_recv.try_iter() {
            if let ServiceEvent::ServiceResolved(info) = event {
                info!("Discovered service: {} at {}:{}",
                    info.get_fullname(),
                    info.get_addresses().iter().next().unwrap(),
                    info.get_port()
                );

                if !info.get_fullname().starts_with("VRChat-Client-") {
                    info!("Skipping non-VRChat service: {}", info.get_fullname());
                    continue;
                }

                // Prefer IPv4 addresses over IPv6
                let addr = info.get_addresses().iter()
                    .find(|a| a.is_ipv4())
                    .or_else(|| info.get_addresses().iter().next())
                    .unwrap();
                info!(
                    "Found OSCJSON service: {} @ {}:{}",
                    info.get_fullname(),
                    addr,
                    info.get_port()
                );

                if self.oscjson_addr.is_none() {
                    notify_avatar = true;
                }

                // Handle IPv6 addresses by wrapping them in brackets
                let formatted_addr = if addr.to_string().contains(':') {
                    format!("[{}]", addr)
                } else {
                    addr.to_string()
                };

                self.oscjson_addr =
                    Some(format!("http://{}:{}/avatar", formatted_addr, info.get_port()).into());
                info!("Set OSCQuery URL to: {}", self.oscjson_addr.as_ref().unwrap());
            }
        }

        if self.oscjson_addr.is_some() && notify_avatar {
            self.avatar(&AvatarIdentifier::Default);
        }
        notify_avatar
    }

    pub fn avatar(&mut self, avatar: &AvatarIdentifier) -> Option<OscJsonNode> {
        let mut json = String::new();

        if let AvatarIdentifier::Path(path) = avatar {
            if let Err(e) = File::open(path).and_then(|mut f| f.read_to_string(&mut json)) {
                log::error!("Could not read file: {:?}", e);
                return None;
            }
        } else {
            let Some(addr) = self.oscjson_addr.as_ref() else {
                warn!("No avatar oscjson address.");
                return None;
            };

            info!("Attempting to fetch avatar JSON from: {}", addr);
            thread::sleep(Duration::from_millis(250));

            let Ok(resp) = self.client.get(addr.as_ref()).send() else {
                warn!("Failed to send avatar json request to: {}", addr);
                return None;
            };

            let Ok(text) = resp.text() else {
                warn!("No payload in avatar json response.");
                return None;
            };

            json = text;
        }

        let path = format!("{}/{}", CONFIG_DIR.as_ref(), "oscavmgr-avatar.json");
        if let Err(e) = File::create(path).and_then(|mut f| f.write_all(json.as_bytes())) {
            warn!("Could not write avatar json file: {:?}", e);
        }

        // Try to parse as VRChat format first, then fall back to OSCQuery format
        if let Ok(vrchat_avatar) = serde_json::from_str::<VRChatAvatarConfig>(&json) {
            info!("Parsed avatar as VRChat format");
            Some(convert_vrchat_to_oscquery(vrchat_avatar))
        } else {
            match serde_json::from_str(&json) {
                Ok(root_node) => {
                    info!("Parsed avatar as OSCQuery format");
                    Some(root_node)
                }
                Err(e) => {
                    warn!("Failed to deserialize avatar json: {}", e);
                    None
                }
            }
        }
    }
}

#[derive(Debug)]
pub enum AvatarIdentifier {
    Default,
    Uid(String),
    Path(String),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OscJsonNode {
    #[serde(alias = "FULL_PATH")]
    pub full_path: Arc<str>,
    #[serde(alias = "ACCESS")]
    pub access: i32,
    #[serde(alias = "TYPE")]
    pub data_type: Option<Arc<str>>,
    #[serde(alias = "CONTENTS")]
    pub contents: Option<HashMap<Arc<str>, OscJsonNode>>,
}

impl OscJsonNode {
    pub fn get(&self, path: &str) -> Option<&OscJsonNode> {
        let mut node = self;
        for part in path.split('/') {
            if let Some(contents) = &node.contents {
                node = contents.get(part)?;
            } else {
                return None;
            }
        }
        Some(node)
    }

    pub fn has_vsync(&self) -> bool {
        self.get("parameters")
            .and_then(|parameters| parameters.get("VSync"))
            .is_some()
    }
}

// VRChat avatar JSON format structures
#[derive(Serialize, Deserialize, Debug)]
pub struct VRChatAvatarConfig {
    pub id: String,
    pub name: String,
    pub parameters: Vec<VRChatParameter>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct VRChatParameter {
    pub name: String,
    pub input: Option<VRChatParameterIO>,
    pub output: Option<VRChatParameterIO>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct VRChatParameterIO {
    pub address: String,
    #[serde(rename = "type")]
    pub param_type: String,
}

// Convert VRChat format to OSCQuery format
fn convert_vrchat_to_oscquery(vrchat_config: VRChatAvatarConfig) -> OscJsonNode {
    let mut parameters_contents = HashMap::new();

    for param in vrchat_config.parameters {
        // Only process input parameters (the ones we can send to)
        if let Some(input) = param.input {
            // Convert VRChat type to OSCQuery type
            let osc_type = match input.param_type.as_str() {
                "Float" => "f",
                "Int" => "i",
                "Bool" => "b",
                _ => "f", // Default to float
            };

            // Extract parameter name from the full path
            // "/avatar/parameters/FT/v2/JawOpen" -> "FT/v2/JawOpen"
            let param_name = if input.address.starts_with("/avatar/parameters/") {
                &input.address[19..] // Remove "/avatar/parameters/"
            } else {
                &param.name
            };

            let full_path_str = input.address.clone();
            let param_name_str = param_name.to_string();

            let node = OscJsonNode {
                full_path: full_path_str.into(),
                access: 2, // Write access
                data_type: Some(osc_type.into()),
                contents: None,
            };

            parameters_contents.insert(param_name_str.into(), node);
        }
    }

    // Create the parameters node
    let parameters_node = OscJsonNode {
        full_path: "/avatar/parameters".into(),
        access: 0,
        data_type: None,
        contents: Some(parameters_contents),
    };

    // Create the root avatar node
    let mut avatar_contents = HashMap::new();
    avatar_contents.insert("parameters".into(), parameters_node);

    OscJsonNode {
        full_path: "/avatar".into(),
        access: 0,
        data_type: None,
        contents: Some(avatar_contents),
    }
}

#[derive(Clone)]
pub struct MysteryParam {
    pub name: Arc<str>,
    pub main_address: Option<Arc<str>>,
    pub addresses: [Option<Arc<str>>; 7],
    pub neg_address: Option<Arc<str>>,
    pub num_bits: usize,
    pub last_value: f32,
    pub last_bits: [bool; 8],
}

impl MysteryParam {
    pub fn send(&mut self, value: f32, bundle: &mut OscBundle) {
        if let Some(addr) = self.main_address.as_ref() {
            if (value - self.last_value).abs() > 0.01 {
                bundle.send_parameter(addr, OscType::Float(value));
                self.last_value = value;
            }
        }

        let mut value = value;
        if let Some(addr) = self.neg_address.as_ref() {
            let send_val = value < 0.;
            if self.last_bits[7] != send_val {
                bundle.send_parameter(addr, OscType::Bool(send_val));
                self.last_bits[7] = send_val;
            }
            value = value.abs();
        } else if value < 0. {
            value = 0.;
        }

        let value = (value * ((1 << self.num_bits) - 1) as f32) as i32;

        self.addresses
            .iter()
            .enumerate()
            .take(self.num_bits)
            .for_each(|(idx, param)| {
                if let Some(addr) = param.as_ref() {
                    let send_val = value & (1 << idx) != 0;
                    if self.last_bits[idx] != send_val {
                        bundle.send_parameter(addr, OscType::Bool(send_val));
                        self.last_bits[idx] = send_val;
                    }
                }
            });
    }
}