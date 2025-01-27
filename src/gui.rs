#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use crate::gui::keyboard::KeyboardView;
use crate::gui::steelseries::SonarView;
use crate::Event;
use eframe::egui;
use eframe::egui::Button;
use std::cmp::PartialEq;
use std::default::Default;
use std::sync::mpsc::{Receiver, Sender};
use tokio::sync::mpsc::UnboundedSender;

mod keyboard;
mod steelseries;

trait View {
    fn init(&mut self);
    fn render(&mut self, ui: &mut egui::Ui);
    fn process_event(&mut self, event: &Event);
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
    let mut tab = Tab::Device;

    let mut sonar_view = SonarView::new(ss_tx.clone());
    let mut keyboard_view = KeyboardView::new(tx);

    sonar_view.init();
    keyboard_view.init();

    eframe::run_simple_native("Controller", options, move |ctx, _frame| {
        match rx.try_recv() {
            Ok(event) => {
                sonar_view.process_event(&event);
                keyboard_view.process_event(&event);
            }
            _ => {}
        }

        egui::CentralPanel::default().show(ctx, |mut ui| {
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
                    sonar_view.render(&mut ui);
                }
                Tab::Device => {
                    keyboard_view.render(&mut ui);
                }
            }
        });
    })
}
