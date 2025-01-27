use crate::steelseries::api::sonar::types::{
    AudioDevice, ClassicRedirection, DeviceDataFlow, RedirectionId,
};
use std::fmt::Display;

include!(concat!(env!("OUT_DIR"), "/codegen.rs"));

impl Display for ClassicRedirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = if let Some(id) = &self.id {
            id.to_string()
        } else {
            "".to_string()
        };
        write!(f, "{}", str)
    }
}

impl PartialEq<AudioDevice> for ClassicRedirection {
    fn eq(&self, other: &AudioDevice) -> bool {
        match &self.device_id {
            Some(id) => {
                if let Some(other_id) = &other.id {
                    id == other_id
                } else {
                    false
                }
            }
            _ => false,
        }
    }
}

impl ClassicRedirection {
    pub(crate) fn get_dataflow(&self) -> DeviceDataFlow {
        if let Some(id) = self.id {
            match id {
                RedirectionId::Mic => DeviceDataFlow::Capture,
                _ => DeviceDataFlow::Render,
            }
        } else {
            unreachable!("Id should not be None")
        }
    }
}
