#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
#![allow(rustdoc::missing_crate_level_docs)] // it's an example

use crate::record::{Record, RecordData};
use crate::Event;
use eframe::egui;
use eframe::egui::{ProgressBar, Rgba, Slider};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};

pub fn init_gui(rx: Receiver<Event>, tx: Sender<Event>) -> eframe::Result {
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

    eframe::run_simple_native("Keyboard Companion", options, move |ctx, _frame| {
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
            _ => {}
        }

        egui::CentralPanel::default().show(ctx, |ui| {
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
