use log::{info, warn};
use mdns_sd::{ServiceDaemon, ServiceEvent};
use rosc::{OscBundle, OscType};
use serde::Deserialize;
use std::{collections::HashMap, sync::Arc, thread, time::Duration};

use super::bundle::AvatarBundle;

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
        self.next_run = std::time::Instant::now() + std::time::Duration::from_secs(5);

        loop {
            for event in self.mdns_recv.try_iter() {
                if let ServiceEvent::ServiceResolved(info) = event {
                    if !info.get_fullname().starts_with("VRChat-Client-") {
                        continue;
                    }
                    let addr = info.get_addresses().iter().next().unwrap();
                    info!(
                        "Found OSCJSON service: {} @ {}:{}",
                        info.get_fullname(),
                        addr,
                        info.get_port()
                    );

                    if self.oscjson_addr.is_none() {
                        notify_avatar = true;
                    }

                    self.oscjson_addr =
                        Some(format!("http://{}:{}/avatar", addr, info.get_port()).into());
                }
            }

            if self.oscjson_addr.is_some() {
                if notify_avatar {
                    self.avatar();
                }
                break;
            } else {
                thread::sleep(Duration::from_millis(10));
            }
        }
        notify_avatar
    }

    pub fn avatar(&mut self) -> Option<OscJsonNode> {
        let Some(addr) = self.oscjson_addr.as_ref() else {
            warn!("No avatar oscjson address.");
            return None;
        };

        thread::sleep(Duration::from_millis(250));

        let Ok(resp) = self.client.get(addr.as_ref()).send() else {
            warn!("Failed to send avatar json request.");
            return None;
        };

        resp.text()
            .ok()
            .map(|json| match serde_json::from_str(&json) {
                Ok(root_node) => Some(root_node),
                Err(e) => {
                    warn!("Failed to deserialize avatar json: {}", e);
                    None
                }
            })
            .flatten()
    }
}

#[derive(Deserialize, Debug)]
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

#[derive(Clone)]
pub struct MysteryParam {
    pub name: Arc<str>,
    pub main_address: Option<Arc<str>>,
    pub addresses: [Option<Arc<str>>; 7],
    pub neg_address: Option<Arc<str>>,
    pub num_bits: usize,
}

impl MysteryParam {
    pub fn send(&self, value: f32, bundle: &mut OscBundle) {
        if let Some(addr) = self.main_address.as_ref() {
            bundle.send_parameter(addr, OscType::Float(value));
            return;
        }

        let mut value = value;
        if let Some(addr) = self.neg_address.as_ref() {
            bundle.send_parameter(addr, OscType::Bool(value < 0.));
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
                    bundle.send_parameter(addr, OscType::Bool(value & (1 << idx) != 0));
                }
            });
    }
}
