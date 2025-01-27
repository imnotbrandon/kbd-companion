mod audio;
mod gui;
mod hid_device_channel;
mod record;
mod steelseries;

use crate::audio::AudioManager;
use crate::gui::init_gui;
use crate::hid_device_channel::{HidDeviceChannel, WriteError};
use crate::steelseries::api::sonar::types::{VolumeInfo, VolumeSettings};
use crate::steelseries::SteelSeriesEngineClient;
use hid_device_channel::WriteResult;
use hidapi::HidError;
use record::*;
use std::fmt::Debug;
use std::process::ExitCode;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, SendError, Sender, TryRecvError};
use std::thread::sleep;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;
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
                .expect("Failed to refresh Mic mute state")
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

trait ApplicationState {}
struct Connected {
    device: HidDeviceChannel,
}
struct Disconnected {
    error: Option<AppError>,
}

impl Default for Disconnected {
    fn default() -> Self {
        Self { error: None }
    }
}

impl ApplicationState for Connected {}
impl ApplicationState for Disconnected {}

#[derive(Debug)]
enum AppError {
    Write(WriteError),
    Connect(HidError),
}

impl Application<Disconnected> {
    pub fn new(tx: Sender<Event>, rx: Receiver<Event>) -> Application<Disconnected> {
        Application::<Disconnected> {
            volume_manager: Default::default(),
            state: Default::default(),
            tx,
            rx,
        }
    }

    pub fn connect(
        self,
        vendor_id: u16,
        product_id: u16,
        usage_page: u16,
        usage: u16,
    ) -> Result<Application<Connected>, Application<Disconnected>> {
        match HidDeviceChannel::connect(vendor_id, product_id, usage_page, usage) {
            Ok(device) => Ok(Application::<Connected> {
                volume_manager: self.volume_manager,
                state: { Connected { device } },
                tx: self.tx,
                rx: self.rx,
            }),
            Err(error) => Err(Application::<Disconnected> {
                volume_manager: self.volume_manager,
                state: Disconnected {
                    error: Some(AppError::Connect(error)),
                },
                tx: self.tx,
                rx: self.rx,
            }),
        }
    }
}

struct Application<S: ApplicationState = Disconnected> {
    volume_manager: VolumeManager,
    state: S,
    tx: Sender<Event>,
    rx: Receiver<Event>,
}

impl Application<Connected> {
    fn process_record(&mut self, record: &Record) {
        println!("DATA!: {:?}", record);

        match record.data {
            RecordData::Pong => {
                self.send_record(Record::new(record.serial + 1, RecordData::BatteryRequest));
                self.volume_manager
                    .get_mute(Some(eRender), Some(eMultimedia))
                    .inspect(|state| {
                        let (hid, gui) = self.send_record(Record::new(
                            record.serial + 2,
                            RecordData::SetOutputMuteState(*state),
                        ));
                        hid.expect("Failed to send record to device");
                        gui.expect("Failed to send record to gui");
                    });
                self.volume_manager
                    .get_mute(Some(eCapture), Some(eCommunications))
                    .and_then(|state| {
                        Some(self.send_record(Record::new(
                            record.serial + 3,
                            RecordData::SetInputMuteState(state),
                        )))
                    });
            }
            RecordData::BatteryResponse { percent, .. } => {
                let (hid, gui) = self.send_record(Record::new(
                    record.serial + 1,
                    RecordData::SetLedMeter {
                        percent,
                        danger_threshold: 2,
                        warning_threshold: 6,
                        invert: false,
                        linger_time: 1000,
                    },
                ));
                hid.expect("Failed to send record to device");
                gui.expect("Failed to send record to gui");
            }
            RecordData::ToggleInputMute => {
                self.volume_manager.toggle_mic_mute();
            }
            _ => {}
        }
    }

    fn send_record(&self, record: Record) -> (WriteResult, Result<(), SendError<Event>>) {
        (
            self.state.device.write_record(record),
            self.tx.send(Event::RecordToDevice(record.clone())),
        )
    }

    pub fn before_read(&mut self) {
        let manager = self.volume_manager.refresh();
        let new_vol = manager.get_vol_if_changed();
        let new_mute = manager.get_mute_if_changed();
        let new_mic_mute = manager.get_mic_mute_if_changed();

        match new_vol {
            None => {}
            Some(vol) => {
                let led_meter_record = Record::new(
                    123,
                    RecordData::SetLedMeter {
                        percent: vol,
                        warning_threshold: 0,
                        danger_threshold: 0,
                        invert: false,
                        linger_time: 1000,
                    },
                );

                self.send_record(led_meter_record);
            }
        }

        match new_mute {
            None => {}
            Some(mute) => {
                let (hid, gui) =
                    self.send_record(Record::new(456, RecordData::SetOutputMuteState(mute)));
                hid.expect("Failed to send new output mute value to device");
                gui.expect("Failed to send new output mute value to gui");
            }
        }

        match new_mic_mute {
            None => {}
            Some(mute) => {
                self.send_record(Record::new(789, RecordData::SetInputMuteState(mute)));
            }
        }
    }

    fn listen_for_data(&mut self) {
        loop {
            self.before_read();

            let response = match self.state.device.read_record(Some(10)) {
                Ok(res) => res,
                Err(err) => {
                    eprintln!("Error!: {:?}", err);

                    return;
                }
            };

            match response {
                Some(res) => {
                    self.tx
                        .send(Event::RecordFromDevice(res.clone()))
                        .expect("Failed to send record to gui");
                    self.process_record(&res);
                }
                None => {}
            }

            match self.rx.try_recv() {
                Ok(Event::RecordToDevice(record)) => {
                    self.send_record(record);
                }
                Err(TryRecvError::Disconnected) => return,
                _ => {} // No data or not interested
            }
        }
    }

    fn run(mut self) -> Application<Disconnected> {
        loop {
            let result = self
                .state
                .device
                .write_record(Record::new(0, RecordData::Ping));

            match result {
                Ok(size) => {
                    println!("Wrote {size} bytes");

                    self.listen_for_data();
                }
                Err(err) => {
                    eprintln!("Error during write: {err:?}");
                    return Application::<Disconnected> {
                        volume_manager: self.volume_manager,
                        state: Disconnected {
                            error: Some(AppError::Write(err)),
                        },
                        tx: self.tx,
                        rx: self.rx,
                    };
                }
            }
        }
    }
}

#[derive(Debug)]
pub(crate) enum SonarRequest {
    FetchDevices {
        remove_steelseries_vad: Option<bool>,
    },
    FetchClassicRedirections,
    FetchDeviceVolume,
}

#[derive(Debug)]
pub(crate) enum SonarResponse {
    FetchDevices(Vec<steelseries::api::sonar::types::AudioDevice>),
    FetchClassicRedirections(Vec<steelseries::api::sonar::types::ClassicRedirection>),
    FetchDeviceVolume(VolumeInfo),
}

#[derive(Debug)]
pub(crate) enum Event {
    DeviceConnected,
    DeviceDisconnected,
    RecordFromDevice(Record),
    RecordToDevice(Record),
    SonarRequest(SonarRequest),
    SonarResponse(SonarResponse),
}

async fn ss_comms(mut rx: UnboundedReceiver<Event>, gui_tx: Sender<Event>) {
    let engine_client = SteelSeriesEngineClient::new_autodetect();
    let new_client = crate::steelseries::api::sonar::Client::new(
        engine_client
            .get_subapp_url("sonar")
            .await
            .expect("Couldn't get sonar URL")
            .as_str(),
    );

    loop {
        let event = rx.recv().await;
        println!("Got event: {event:?}");
        let response = match event {
            Some(Event::SonarRequest(request)) => match request {
                SonarRequest::FetchDevices {
                    remove_steelseries_vad,
                } => Some(SonarResponse::FetchDevices(
                    new_client
                        .list_audio_devices(None, None, remove_steelseries_vad)
                        .await
                        .expect("idk")
                        .to_owned(),
                )),
                SonarRequest::FetchClassicRedirections => {
                    Some(SonarResponse::FetchClassicRedirections(
                        new_client
                            .list_classic_redirections()
                            .await
                            .expect("idk")
                            .to_owned(),
                    ))
                }
                SonarRequest::FetchDeviceVolume => Some(SonarResponse::FetchDeviceVolume(
                    new_client
                        .get_classic_volume_settings()
                        .await
                        .expect("idk vol")
                        .to_owned(),
                )),
            },
            _ => return,
        };

        if let Some(response) = response {
            gui_tx
                .send(Event::SonarResponse(response))
                .expect("Failed to send response");
        }
    }
}

fn main() -> ExitCode {
    // Sending data to GUI
    let (gui_tx, gui_rx) = mpsc::channel();
    // Sending data to USB
    let (usb_tx, usb_rx) = mpsc::channel();
    let (ss_tx, ss_rx) = tokio::sync::mpsc::unbounded_channel();

    let gui_tx_for_kbd = gui_tx.clone();
    let thread = std::thread::spawn(move || {
        let mut retry = 0;
        let mut application = Application::new(gui_tx_for_kbd, usb_rx);

        loop {
            application = match application.connect(0x3434, 0x661, 0xFF60, 0x61) {
                Ok(app) => app.run(),
                Err(e) => {
                    retry += 1;
                    eprintln!("Error during connect: {:?}", e.state.error);

                    if retry > 100 {
                        return ExitCode::FAILURE;
                    }
                    sleep(Duration::from_millis(100));

                    e
                }
            }
        }
    });

    let thread2 = std::thread::spawn(move || {
        let tokio = tokio::runtime::Runtime::new().unwrap();
        tokio.block_on(ss_comms(ss_rx, gui_tx));
    });

    init_gui(gui_rx, usb_tx, ss_tx).expect("wat");

    thread2.join().unwrap();
    thread.join().unwrap();

    ExitCode::SUCCESS
}
