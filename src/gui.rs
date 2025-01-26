#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use crate::record::{Record, RecordData};
use crate::steelseries::api::sonar::types::RedirectionId;
use crate::{steelseries, Event, SonarRequest, SonarResponse};
use eframe::egui;
use eframe::egui::{Button, ComboBox, ProgressBar, Rgba, Slider};
use std::cmp::PartialEq;
use std::default::Default;
use std::sync::mpsc::{Receiver, Sender};
use tokio::sync::mpsc::UnboundedSender;

type AudioDevice = steelseries::api::sonar::types::AudioDevice;
type ClassicRedirection = steelseries::api::sonar::types::ClassicRedirection;
struct SonarState {
    audio_devices: Vec<AudioDevice>,
    redirections: Vec<ClassicRedirection>,
}

impl SonarState {
    fn new() -> Self {
        Self {
            audio_devices: Vec::new(),
            redirections: Vec::new(),
        }
    }

    fn redirect_device(&self, redirection: &RedirectionId, device: &String) -> ClassicRedirection {
        todo!("implement");
    }
}

fn ss_page(ui: &mut egui::Ui, state: &mut SonarState) {
    let addr_label = ui.label("Sonar server address:".to_string());
    ui.hyperlink(format!("{}", "")).labelled_by(addr_label.id);

    let device_list = &state.audio_devices;
    let mut redirections = Vec::new();

    ui.horizontal(|ui| {
        state.redirections.iter_mut().for_each(|redirection| {
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

                    let device_id = if let Some(device_id) = &device.id {
                        device_id
                    } else {
                        return;
                    };

                    ui.label(&redirection.to_string());
                    ComboBox::from_label(redirection.to_string())
                        .selected_text(device_name.to_string())
                        .show_ui(ui, |ui| {
                            state.audio_devices.iter().for_each(|device| {
                                ui.selectable_value(
                                    &mut redirection.device_id,
                                    Some(device.id.as_ref().unwrap().clone()),
                                    device.friendly_name.as_ref().expect("idk"),
                                );
                            })
                        });

                    if !redirection.eq(&device) {
                        redirections.push((redirection.id.clone(), redirection.device_id.clone()));
                    }
                })
            });
        })
    });

    redirections.into_iter().for_each(|(id, device)| {
        if let Some(redirection_id) = id {
            if let Some(device_id) = device {
                println!("{:?}", state.redirect_device(&redirection_id, &device_id));
            }
        }
    });
}

#[derive(PartialEq)]
enum Tab {
    Device,
    Sonar,
}

pub fn init_gui(
    rx: Receiver<Event>,
    tx: Sender<Event>,
    ss_tx: UnboundedSender<Event>,
) -> eframe::Result {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([640.0, 480.0])
            .with_drag_and_drop(false),
        ..Default::default()
    };

    // TODO: refactor into state struct
    let mut bat_pc = 0;
    let mut set_bat_pc = 0;
    let mut led_meter_pc = 0;
    let mut muted = false;
    let mut tab = Tab::Device;

    let mut sonar_state = SonarState::new();
    ss_tx.send(Event::SonarRequest(SonarRequest::FetchDevices {
        remove_steelseries_vad: Some(true),
    }));
    ss_tx.send(Event::SonarRequest(SonarRequest::FetchClassicRedirections));

    eframe::run_simple_native("Controller", options, move |ctx, _frame| {
        // TODO: Refactor into function
        match rx.try_recv() {
            Ok(Event::RecordFromDevice(rec)) => match rec.data {
                RecordData::BatteryResponse { percent, .. } => bat_pc = percent,
                _ => {}
            },
            Ok(Event::RecordToDevice(rec)) => match rec.data {
                RecordData::SetOutputMuteState(state) => muted = state,
                RecordData::SetLedMeter { percent, .. } => led_meter_pc = percent,
                _ => {}
            },
            Ok(Event::SonarResponse(SonarResponse::FetchDevices(devices))) => {
                sonar_state.audio_devices = devices;
                println!("Got sonar device list: {:?}", sonar_state.audio_devices);
            }
            Ok(Event::SonarResponse(SonarResponse::FetchClassicRedirections(redirections))) => {
                sonar_state.redirections = redirections;
                println!("Got sonar device list: {:?}", sonar_state.audio_devices);
            }
            _ => {}
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.scope(|ui| {
                    let device_btn = ui.add(Button::new("Keyboard").selected(tab == Tab::Device));
                    let sonar_btn = ui.add(Button::new("Sonar").selected(tab == Tab::Sonar));

                    if device_btn.clicked() {
                        tab = Tab::Device
                    } else if sonar_btn.clicked() {
                        tab = Tab::Sonar;
                    }
                })
            });

            ui.separator();
            match tab {
                Tab::Sonar => {
                    ss_page(ui, &mut sonar_state);
                    return;
                }
                Tab::Device => {}
            }
            ui.vertical(|ui| {
                ui.group(|ui| {
                    ui.heading("Device State");
                    ui.horizontal(|ui| {
                        let name_label = ui.label("Battery Level:");
                        ui.colored_label(Rgba::from_rgb(0f32, 255f32, 0f32), format!("{bat_pc}%"))
                            .labelled_by(name_label.id);
                    });
                    ui.horizontal(|ui| {
                        let led_label = ui.label("Led meter:");
                        ui.add(ProgressBar::new(led_meter_pc as f32 / 100f32))
                            .labelled_by(led_label.id)
                    });
                });
                ui.add_space(10f32);
                ui.group(|ui| {
                    ui.heading("System State");
                    ui.horizontal(|ui| {
                        let name_label = ui.label("Mute:");
                        let col = if muted {
                            Rgba::from_rgb(255f32, 0f32, 0f32)
                        } else {
                            Rgba::from_rgb(0f32, 255f32, 0f32)
                        };
                        ui.colored_label(col, format!("{muted}"))
                            .labelled_by(name_label.id);
                    });
                });
                ui.add_space(10f32);
                ui.group(|ui| {
                    ui.heading("Actions");
                    ui.horizontal(|ui| {
                        ui.add(Slider::new(&mut set_bat_pc, 0..=100));
                        let btn = ui.button("Illuminate battery %");
                        if btn.clicked() {
                            tx.send(Event::RecordToDevice(Record::new(
                                456,
                                RecordData::SetLedMeter {
                                    percent: set_bat_pc,
                                    invert: false,
                                    linger_time: 2000,
                                    danger_threshold: 2,
                                    warning_threshold: 7,
                                },
                            )))
                            .expect("Failed to send Illuminate");
                        }
                    })
                });
            });
        });
    })
}
