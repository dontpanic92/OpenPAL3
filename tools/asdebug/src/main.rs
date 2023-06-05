#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::sync::{Arc, Mutex, RwLock};

use context::Context;
use eframe::egui;
use server::start_server;

mod context;
mod server;

fn main() -> Result<(), eframe::Error> {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(1024.0, 768.0)),
        ..Default::default()
    };

    eframe::run_native(
        "PAL4 AngelScript Debugger",
        options,
        Box::new(|cc| {
            let frame = cc.egui_ctx.clone();
            Box::new(AsDebugApp::new(frame))
        }),
    )
}

struct AsDebugApp {
    name: String,
    age: u32,
    context: Arc<RwLock<Context>>,
}

impl AsDebugApp {
    pub fn new(ec: eframe::egui::Context) -> Self {
        let context = Arc::new(RwLock::new(Context::new(ec)));
        start_server(context.clone());

        Self {
            name: "Arthur".to_owned(),
            age: 42,
            context,
        }
    }
}

impl eframe::App for AsDebugApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let window_width = frame.info().window_info.size.x;

        let state = match &self.context.read().unwrap().connection_state {
            context::ServerConnectionState::NotStarted
            | context::ServerConnectionState::Listening => "Waiting for connection...".to_string(),
            context::ServerConnectionState::Connected => "✔Connected".to_string(),
            context::ServerConnectionState::Error(e) => format!("Error: {}", e),
        };

        egui::SidePanel::left("left_panel")
            .resizable(true)
            .default_width(400.0)
            .width_range((window_width / 4.)..=(window_width / 4. * 3.))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(state);
                });
                egui::ScrollArea::vertical().show(ui, |ui| {
                    lorem_ipsum(ui);
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                let name_label = ui.label("Your name: ");
                ui.text_edit_singleline(&mut self.name)
                    .labelled_by(name_label.id);
            });
            ui.add(egui::Slider::new(&mut self.age, 0..=120).text("age"));
            if ui.button("Click each year").clicked() {
                self.age += 1;
            }
            ui.label(format!("Hello '{}', age {}", self.name, self.age));
        });
    }
}

fn lorem_ipsum(ui: &mut egui::Ui) {
    ui.with_layout(
        egui::Layout::top_down(egui::Align::LEFT).with_cross_justify(true),
        |ui| {
            ui.label(egui::RichText::new("aaaaaaaaaaaaaaaaaaa").small().weak());
            ui.add(egui::Separator::default().grow(8.0));
            ui.label(egui::RichText::new("aaaaaaaaaaaaaaaaaaa").small().weak());
        },
    );
}
