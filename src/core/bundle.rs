use rosc::{OscBundle, OscMessage, OscPacket, OscType};

use super::{INPUT_PREFIX, PARAM_PREFIX};

pub trait AvatarBundle {
    fn new_bundle() -> Self;
    fn send_parameter(&mut self, name: &str, value: OscType);
    fn send_tracking(&mut self, addr: &str, args: Vec<OscType>);
    fn send_input_axis(&mut self, name: &str, value: f32);
    fn send_input_button(&mut self, name: &str, value: bool);
    fn send_chatbox_message(&mut self, message: String, open_keyboard: bool, play_sound: bool);
    fn serialize(self) -> Option<Vec<u8>>;
}

impl AvatarBundle for OscBundle {
    fn new_bundle() -> OscBundle {
        OscBundle {
            timetag: rosc::OscTime {
                seconds: 0,
                fractional: 0,
            },
            content: Vec::new(),
        }
    }
    fn send_parameter(&mut self, name: &str, value: OscType) {
        log::trace!("Sending parameter {} = {:?}", name, value);
        self.content.push(OscPacket::Message(OscMessage {
            addr: format!("{}{}", PARAM_PREFIX, name),
            args: vec![value],
        }));
    }
    fn send_tracking(&mut self, addr: &str, args: Vec<OscType>) {
        log::trace!("Sending tracking {} = {:?}", addr, args);
        self.content.push(OscPacket::Message(OscMessage {
            addr: addr.to_string(),
            args,
        }));
    }
    fn send_input_axis(&mut self, name: &str, value: f32) {
        log::trace!("Sending input axis {} = {:?}", name, value);
        self.content.push(OscPacket::Message(OscMessage {
            addr: format!("{}{}", INPUT_PREFIX, name),
            args: vec![OscType::Float(value)],
        }));
    }
    fn send_input_button(&mut self, name: &str, value: bool) {
        log::trace!("Sending input button {} = {:?}", name, value);
        self.content.push(OscPacket::Message(OscMessage {
            addr: format!("{}{}", INPUT_PREFIX, name),
            args: vec![OscType::Bool(value)],
        }));
    }
    fn send_chatbox_message(&mut self, message: String, open_keyboard: bool, play_sound: bool) {
        log::trace!(
            "Sending chatbox message {} (kbd: {:?}, sfx: {:?})",
            message,
            open_keyboard,
            play_sound
        );
        self.content.insert(
            0,
            OscPacket::Message(OscMessage {
                addr: "/chatbox/input/".to_string(),
                args: vec![
                    OscType::String(message),
                    OscType::Bool(open_keyboard),
                    OscType::Bool(play_sound),
                ],
            }),
        );
    }
    fn serialize(self) -> Option<Vec<u8>> {
        if !self.content.is_empty() {
            rosc::encoder::encode(&OscPacket::Bundle(self)).ok()
        } else {
            None
        }
    }
}
