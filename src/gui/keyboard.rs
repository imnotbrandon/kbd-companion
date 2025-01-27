use crate::gui::View;
use crate::record::{Record, RecordData};
use crate::Event;
use eframe::egui::{ProgressBar, Rgba, Slider, Ui};
use std::sync::mpsc::{Receiver, Sender};

pub(super) struct KeyboardView {
    bat_pc: u8,
    set_bat_pc: u8,
    led_meter_pc: u8,
    muted: bool,
    tx: Sender<Event>,
}
impl KeyboardView {
    pub(super) fn new(tx: Sender<Event>) -> Self {
        Self {
            bat_pc: 0,
            set_bat_pc: 0,
            led_meter_pc: 0,
            muted: false,
            tx,
        }
    }
}

impl View for KeyboardView {
    fn init(&mut self) {}

    fn render(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            ui.group(|ui| {
                ui.heading("Device State");
                ui.horizontal(|ui| {
                    let name_label = ui.label("Battery Level:");
                    ui.colored_label(
                        Rgba::from_rgb(0f32, 255f32, 0f32),
                        format!("{}%", self.bat_pc),
                    )
                    .labelled_by(name_label.id);
                });
                ui.horizontal(|ui| {
                    let led_label = ui.label("Led meter:");
                    ui.add(ProgressBar::new(self.led_meter_pc as f32 / 100f32))
                        .labelled_by(led_label.id)
                });
            });
            ui.add_space(10f32);
            ui.group(|ui| {
                ui.heading("System State");
                ui.horizontal(|ui| {
                    let name_label = ui.label("Mute:");
                    let col = if self.muted {
                        Rgba::from_rgb(255f32, 0f32, 0f32)
                    } else {
                        Rgba::from_rgb(0f32, 255f32, 0f32)
                    };
                    ui.colored_label(col, format!("{}", self.muted))
                        .labelled_by(name_label.id);
                });
            });
            ui.add_space(10f32);
            ui.group(|ui| {
                ui.heading("Actions");
                ui.horizontal(|ui| {
                    ui.add(Slider::new(&mut self.set_bat_pc, 0..=100));
                    let btn = ui.button("Illuminate battery %");
                    if btn.clicked() {
                        self.tx
                            .send(Event::RecordToDevice(Record::new(
                                456,
                                RecordData::SetLedMeter {
                                    percent: self.set_bat_pc,
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
    }

    fn process_event(&mut self, event: &Event) {
        match event {
            Event::RecordFromDevice(rec) => match rec.data {
                RecordData::BatteryResponse { percent, .. } => self.bat_pc = percent,
                _ => {}
            },
            Event::RecordToDevice(rec) => match rec.data {
                RecordData::SetOutputMuteState(state) => self.muted = state,
                RecordData::SetLedMeter { percent, .. } => self.led_meter_pc = percent,
                _ => {}
            },
            _ => {}
        }
    }
}
