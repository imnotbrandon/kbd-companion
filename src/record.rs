use bincode::{Decode, Encode};

#[derive(Encode, Decode, PartialEq, Debug)]
#[repr(C)]
pub(crate) struct Record {
    pub(crate) serial: u32,
    pub(crate) data: RecordData,
}

impl Record {
    pub(crate) fn new(serial: u32, data: RecordData) -> Self {
        Self { serial, data }
    }
}

#[derive(Encode, Decode, PartialEq, Debug)]
pub(crate) enum RecordData {
    Empty,
    Ping,
    Pong,
    BatteryRequest,
    BatteryResponse {
        percent: u8,
        voltage: u16,
    },
    SetLedMeter {
        percent: u8,
        warning_threshold: u8,
        danger_threshold: u8,
        invert: bool,
        linger_time: u16,
    },
    SetOutputMuteState(bool),
    SetInputMuteState(bool),
    ToggleOutputMute,
    ToggleInputMute,
}

impl RecordData {
    fn set_led_meter_no_threshold(percent: u8) -> Self {
        Self::SetLedMeter {
            percent,
            warning_threshold: 0,
            danger_threshold: 0,
            invert: false,
            linger_time: 1000,
        }
    }
}
