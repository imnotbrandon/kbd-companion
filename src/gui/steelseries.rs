use crate::{
    gui::View,
    steelseries::api::sonar,
    steelseries::api::sonar::types::DeviceRole,
    steelseries::api::sonar::types::RedirectionId,
    steelseries::api::sonar::types::RedirectionVolumes
    ,
    Event,
    SonarRequest,
    SonarResponse
};
use eframe::egui;
use eframe::egui::ComboBox;
use std::collections::HashMap;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::mpsc::UnboundedSender;

pub type AudioDevice = sonar::types::AudioDevice;
pub type ClassicRedirection = sonar::types::ClassicRedirection;

struct ChannelState {
    is_muted: bool,
    volume: f32,
    device: String,
}

pub(super) struct SonarView {
    audio_devices: Vec<AudioDevice>,
    redirections: Vec<ClassicRedirection>,
    channel_tx: UnboundedSender<Event>,
    vad_volume: HashMap<DeviceRole, RedirectionVolumes>,
    sonar_url: String,
}

impl SonarView {
    pub(crate) fn new(channel_tx: UnboundedSender<Event>) -> Self {
        Self {
            audio_devices: Vec::new(),
            redirections: Vec::new(),
            channel_tx,
            vad_volume: HashMap::new(),
            sonar_url: String::new(),
        }
    }

    fn redirect_device(&mut self, redirection: &RedirectionId, device: &String) {
        self.sonar_request(SonarRequest::RedirectDevice {
            redirection: redirection.clone(),
            device: device.clone(),
        })
        .expect("failed to redirect device");
    }

    fn fetch_volume(&mut self) -> Result<(), SendError<Event>> {
        self.sonar_request(SonarRequest::FetchDeviceVolume)
    }

    fn sonar_request(&mut self, request: SonarRequest) -> Result<(), SendError<Event>> {
        self.send_event(Event::SonarRequest(request))
    }

    fn send_event(&mut self, event: Event) -> Result<(), SendError<Event>> {
        self.channel_tx.send(event)
    }
}
impl View for SonarView {
    fn init(&mut self) {
        self.sonar_request(SonarRequest::FetchDevices {
            remove_steelseries_vad: Some(true),
        });
        self.sonar_request(SonarRequest::FetchClassicRedirections);
        self.fetch_volume().expect("Failed to fetch volume");
        self.sonar_request(SonarRequest::GetSonarUrl)
            .expect("Failed to get SonarUrl");
    }

    fn render(&mut self, ui: &mut egui::Ui) {
        let addr_label = ui.label("Sonar server address:".to_string());
        ui.hyperlink(format!("{}", self.sonar_url))
            .labelled_by(addr_label.id);

        let device_list = &self.audio_devices;
        let vad_volume = &self.vad_volume;
        let mut redirections = Vec::new();

        self.redirections.iter_mut().for_each(|redirection| {
            ui.group(|ui| {
                ui.vertical(|ui| {
                    let device = device_list.iter().find(|x| redirection.eq(x));
                    let device = if let Some(device) = device {
                        device
                    } else {
                        return;
                    };

                    let device_name = if let Some(device_name) = &device.friendly_name {
                        device_name
                    } else {
                        return;
                    };
                    let redirection_dataflow = redirection.get_dataflow();

                    ui.heading(&redirection.to_string());
                    ui.horizontal(|ui| {
                        ui.label("Active Device");
                        ComboBox::from_id_salt(redirection.to_string())
                            .wrap()
                            .selected_text(device_name.to_string())
                            .show_ui(ui, |ui| {
                                self.audio_devices
                                    .iter()
                                    .filter(|d| {
                                        d.data_flow.is_some_and(|f| f == redirection_dataflow)
                                    })
                                    .for_each(|device| {
                                        ui.selectable_value(
                                            &mut redirection.device_id,
                                            Some(device.id.as_ref().unwrap().clone()),
                                            device.friendly_name.as_ref().expect("idk"),
                                        );
                                    })
                            });
                    });
                    ui.horizontal(|ui| {
                        let redirection_id = redirection.id.unwrap();
                        let role = DeviceRole::try_from(redirection_id.to_string());
                        if let Ok(role) = role {
                            if let Some(vad_volume) = vad_volume.get(&role) {
                                ui.label(format!(
                                    "Volume: {}%",
                                    vad_volume
                                        .classic
                                        .as_ref()
                                        .unwrap()
                                        .volume
                                        .expect("missing volume")
                                        * 100.0
                                ));
                            }
                        }
                    });

                    if !redirection.eq(&device) {
                        redirections.push((redirection.id.clone(), redirection.device_id.clone()));
                    }
                })
            });
            ui.add_space(10.0);
        });

        redirections.into_iter().for_each(|(id, device)| {
            if let Some(redirection_id) = id {
                if let Some(device_id) = device {
                    println!("{:?}", self.redirect_device(&redirection_id, &device_id));
                }
            }
        });
    }

    fn process_event(&mut self, event: &Event) {
        match event {
            Event::SonarResponse(SonarResponse::FetchDevices(devices)) => {
                self.audio_devices = devices.clone();
                println!("Got sonar device list: {:?}", self.audio_devices);
            }
            Event::SonarResponse(SonarResponse::FetchClassicRedirections(redirections)) => {
                self.redirections = redirections.clone();
                println!("Got classic redirections: {:?}", self.redirections);
            }
            Event::SonarResponse((SonarResponse::FetchDeviceVolume(response))) => {
                println!("Got sonar device volume: {:?}", response);
                let devices = response.clone().devices.unwrap();
                self.vad_volume = devices
                    .into_iter()
                    .map(|(k, v)| (DeviceRole::try_from(k).expect("invalid channel"), v))
                    .collect();
            }
            Event::SonarResponse(SonarResponse::GetSonarUrl(url)) => {
                self.sonar_url = url.clone();
            }
            _ => {}
        }
    }
}
