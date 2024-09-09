mod audio;
mod hid_device_channel;
mod record;

use crate::audio::AudioManager;
use crate::hid_device_channel::HidDeviceChannel;
use hidapi::HidError;
use record::*;
use std::fmt::Debug;
use std::process::ExitCode;
use std::thread::sleep;
use std::time::Duration;
use windows::Win32::Media::Audio::{
    eCapture, eCommunications, eMultimedia, eRender, EDataFlow, ERole,
};

struct VolumeManager {
    previous_vol: Option<u8>,
    current_vol: Option<u8>,
    audio_manager: AudioManager,
    curr_mute: Option<bool>,
    prev_mute: Option<bool>,
    prev_mic_mute: Option<bool>,
    curr_mic_mute: Option<bool>,
}

impl Default for VolumeManager {
    fn default() -> Self {
        let mut manager = unsafe {
            Self {
                previous_vol: None,
                current_vol: None,
                audio_manager: AudioManager::new().unwrap(),
                curr_mute: None,
                prev_mute: None,
                prev_mic_mute: None,
                curr_mic_mute: None,
            }
        };

        manager.refresh();
        manager
    }
}

impl VolumeManager {
    fn get_system_volume(
        &self,
        device_type: Option<EDataFlow>,
        device_role: Option<ERole>,
    ) -> Option<u8> {
        let device_type = device_type.unwrap_or(eRender);
        let device_role = device_role.unwrap_or(eMultimedia);
        let device = self
            .audio_manager
            .get_default_device(device_type, device_role);

        match device.unwrap().activate() {
            Ok(device) => device.get_volume().map_err(|_| None::<u8>).ok(),
            Err(_) => None::<u8>,
        }
    }

    fn get_mute(&self, device_type: Option<EDataFlow>, device_role: Option<ERole>) -> Option<bool> {
        let device_type = device_type.unwrap_or(eRender);
        let device_role = device_role.unwrap_or(eMultimedia);

        let device = self
            .audio_manager
            .get_default_device(device_type, device_role);

        match device
            .expect("Failed to get default audio device")
            .activate()
        {
            Ok(device) => Some(
                device
                    .get_muted()
                    .expect("Failed to get mute state of device"),
            ),
            Err(e) => panic!("{e:?}"),
        }
    }

    fn refresh(&mut self) -> &Self {
        self.previous_vol = self.current_vol;
        self.current_vol = self.get_system_volume(Some(eRender), Some(eMultimedia));

        self.prev_mute = self.curr_mute;
        self.curr_mute = self.get_mute(Some(eRender), Some(eMultimedia));

        self.prev_mic_mute = self.curr_mic_mute;
        self.curr_mic_mute = self.get_mute(Some(eCapture), Some(eCommunications));

        self
    }

    fn get_vol_if_changed(&self) -> Option<u8> {
        match self.current_vol {
            Some(vol) if vol != self.previous_vol? => Some(vol),
            _ => None,
        }
    }

    fn get_mute_if_changed(&self) -> Option<bool> {
        match self.curr_mute {
            Some(mute) if mute != self.prev_mute? => Some(mute),
            _ => None,
        }
    }

    fn get_mic_mute_if_changed(&self) -> Option<bool> {
        match self.curr_mic_mute {
            Some(mute) if mute != self.prev_mic_mute? => Some(mute),
            _ => None,
        }
    }

    fn toggle_mic_mute(&mut self) {
        let curr_mute = self.curr_mic_mute.unwrap_or_else(|| {
            self.refresh()
                .curr_mic_mute
                .expect("Failed to refresh mic mute state")
        });

        self.set_mic_mute(!curr_mute)
    }

    fn set_mic_mute(&mut self, muted: bool) {
        self.audio_manager
            .get_mic()
            .expect("Failed to get microphone device")
            .activate()
            .expect("Failed to activate microphone device")
            .set_muted(muted)
    }
}

struct Application {
    volume_manager: VolumeManager,
    device: HidDeviceChannel,
    device_params: (u16, u16, u16, u16),
}

impl Application {
    pub fn new() -> Result<Self, HidError> {
        let device_params = (0x3434, 0x661, 0xFF60, 0x61);
        let (vendor_id, product_id, usage_page, usage) = device_params;

        Ok(Self {
            volume_manager: Default::default(),
            device: HidDeviceChannel::connect(vendor_id, product_id, usage_page, usage)?,
            device_params,
        })
    }

    pub fn reconnect(self) -> Result<Self, HidError> {
        let device_params = self.device_params;
        let (vendor_id, product_id, usage_page, usage) = device_params;
        Ok(Self {
            volume_manager: self.volume_manager,
            device_params: self.device_params,
            device: HidDeviceChannel::connect(vendor_id, product_id, usage_page, usage)?,
        })
    }

    fn process_record(&mut self, record: &Record) {
        println!("DATA!: {:?}", record);

        match record.data {
            RecordData::Pong => {
                self.device
                    .write_record(Record::new(record.serial + 1, RecordData::BatteryRequest))
                    .expect("Failed to write battery request");
                self.volume_manager
                    .get_mute(Some(eRender), Some(eMultimedia))
                    .and_then(|state| {
                        Some(
                            self.device
                                .write_record(Record::new(
                                    record.serial + 2,
                                    RecordData::SetOutputMuteState(state),
                                ))
                                .expect("Failed to write data"),
                        )
                    });
                self.volume_manager
                    .get_mute(Some(eCapture), Some(eCommunications))
                    .and_then(|state| {
                        Some(
                            self.device
                                .write_record(Record::new(
                                    record.serial + 3,
                                    RecordData::SetInputMuteState(state),
                                ))
                                .expect("Failed to write data"),
                        )
                    });
            }
            RecordData::BatteryResponse { percent, .. } => {
                self.device
                    .write_record(Record::new(
                        record.serial + 1,
                        RecordData::SetLedMeter {
                            percent,
                            danger_threshold: 2,
                            warning_threshold: 6,
                            invert: false,
                            linger_time: 1000,
                        },
                    ))
                    .expect("ok");
            }
            RecordData::ToggleInputMute => {
                self.volume_manager.toggle_mic_mute();
            }
            _ => {}
        }
    }

    pub fn before_read(&mut self) {
        let manager = self.volume_manager.refresh();
        let new_vol = manager.get_vol_if_changed();
        let new_mute = manager.get_mute_if_changed();
        let new_mic_mute = manager.get_mic_mute_if_changed();

        match new_vol {
            None => {}
            Some(vol) => {
                self.device
                    .write_record(Record::new(
                        123,
                        RecordData::SetLedMeter {
                            percent: vol,
                            warning_threshold: 0,
                            danger_threshold: 0,
                            invert: false,
                            linger_time: 1000,
                        },
                    ))
                    .expect("TODO: panic message");
            }
        }

        match new_mute {
            None => {}
            Some(mute) => {
                self.device
                    .write_record(Record::new(456, RecordData::SetOutputMuteState(mute)))
                    .expect("Failed to send new output mute value");
            }
        }

        match new_mic_mute {
            None => {}
            Some(mute) => {
                self.device
                    .write_record(Record::new(789, RecordData::SetInputMuteState(mute)))
                    .expect("Failed to send new mic mute value");
            }
        }
    }

    fn listen_for_data(&mut self) {
        loop {
            self.before_read();

            let response = match self.device.read_record(Some(100)) {
                Ok(res) => res,
                Err(err) => {
                    eprintln!("Error!: {:?}", err);

                    return;
                }
            };

            match response {
                Some(res) => self.process_record(&res),
                None => {}
            }
        }
    }

    fn run(&mut self) {
        let mut retry = 0;
        loop {
            if retry > 100 {
                return;
            }

            let result = self.device.write_record(Record::new(0, RecordData::Ping));

            match result {
                Ok(size) => {
                    println!("Wrote {size} bytes");

                    self.listen_for_data();
                }
                Err(err) => {
                    retry += 1;
                    eprintln!("Error during write: {err:?}");
                    sleep(Duration::from_millis(1000));
                }
            }
        }
    }
}

fn main() -> ExitCode {
    let mut retry = 0;
    let mut application = Application::new();

    loop {
        match application {
            Ok(mut app) => {
                app.run();
                application = app.reconnect();
            }
            Err(e) => {
                retry += 1;
                eprintln!("Error during connect: {e:?}");

                if retry > 100 {
                    return ExitCode::FAILURE;
                }
                sleep(Duration::from_millis(100));

                application = Application::new();
            }
        }
    }
}
