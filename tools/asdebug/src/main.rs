// #![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::sync::{
    mpsc::{channel, Sender},
    Arc, RwLock, RwLockReadGuard,
};

use context::Context;
use disasm_view::DisasmView;
use eframe::egui::{self, ScrollArea};
use server::start_server;
use shared::scripting::angelscript::{
    debug::Response, disasm, AsInst, AsInstInstance, ScriptModule,
};
use utils::show_strings;

mod context;
mod disasm_view;
mod server;
mod utils;

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

fn setup_font(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    fonts.font_data.insert(
        "my_font".to_owned(),
        egui::FontData::from_static(include_bytes!(
            "../../../radiance/radiance-assets/src/embed/fonts/SourceHanSerif-Regular.ttf"
        )),
    );

    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .push("my_font".to_owned());

    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .push("my_font".to_owned());

    ctx.set_fonts(fonts);
}

enum AppState {
    Debugger,
    Disassembler,
}

struct AsDebugApp {
    state: AppState,
    dv: DisasmView,
    tx: Sender<Response>,
    context: Arc<RwLock<Context>>,
}

impl AsDebugApp {
    pub fn new(ec: eframe::egui::Context) -> Self {
        setup_font(&ec);

        let (tx, rx) = channel();
        let context = Arc::new(RwLock::new(Context::new(ec)));
        start_server(rx, context.clone());

        Self {
            state: AppState::Debugger,
            dv: DisasmView::new(),
            tx,
            context,
        }
    }

    fn show_toolbar(&self, ui: &mut egui::Ui) {
        if ui.button("Step Into").clicked() {
            let _ = self.tx.send(Response::SingleStep);
        }
    }

    fn show_inst_note(
        &self,
        ui: &mut egui::Ui,
        context: &RwLockReadGuard<Context>,
        inst: &AsInstInstance,
    ) {
        let note = match inst.inst {
            AsInst::CallSys { function_index } => {
                format!(
                    "// {}",
                    context
                        .functions
                        .get((-function_index - 1) as usize)
                        .unwrap_or(&"".to_string())
                )
            }

            _ => "".to_string(),
        };

        ui.label(note);
    }

    fn show_code(&self, ui: &mut egui::Ui) {
        let context = self.context.read().unwrap();
        if context.module.is_none() {
            return;
        }

        let module = context.module.as_ref().unwrap();

        let insts = disasm(&module.functions[context.function_id as usize]);

        ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    egui::Grid::new("my_grid")
                        .num_columns(4)
                        .spacing([4.0, 4.0])
                        .striped(false)
                        .show(ui, |ui| {
                            for inst in &insts {
                                ui.add(|ui: &mut egui::Ui| ui.label(format!("{}", inst.addr)));
                                if context.pc == inst.addr as usize {
                                    ui.label("▶"); //.scroll_to_me(None);
                                } else {
                                    ui.label("");
                                }

                                ui.label(format!("{:?}", inst.inst));
                                self.show_inst_note(ui, &context, inst);
                                ui.end_row();
                            }
                        });
                });

                let margin = ui.visuals().clip_rect_margin;

                let current_scroll = ui.clip_rect().top() - ui.min_rect().top() + margin;
                let max_scroll = ui.min_rect().height() - ui.clip_rect().height() + 2.0 * margin;
                (current_scroll, max_scroll)
            })
            .inner;
    }

    fn context_info(&self, ui: &mut egui::Ui) {
        let context = self.context.read().unwrap();

        if let Some(module) = context.module.as_ref() {
            ui.label(egui::RichText::new("Module").strong());
            ui.label(format!(
                "Function: {}",
                &module.functions[context.function_id as usize].name
            ));
            ui.label(format!("Strings"));

            ScrollArea::vertical()
                .auto_shrink([false; 2])
                .max_height(100.)
                .show(ui, |ui| {
                    show_strings(ui, module);
                });
            ui.separator();

            ui.label(egui::RichText::new("Registers").strong());
            egui::Grid::new("my_grid")
                .num_columns(2)
                .spacing([40.0, 4.0])
                .striped(false)
                .show(ui, |ui| {
                    ui.add(|ui: &mut egui::Ui| ui.label("pc"));
                    ui.label(format!("{}", &context.pc));
                    ui.end_row();

                    ui.add(|ui: &mut egui::Ui| ui.label("sp"));
                    ui.label(format!("{}", &context.sp));
                    ui.end_row();

                    ui.add(|ui: &mut egui::Ui| ui.label("fp"));
                    ui.label(format!("{}", &context.fp));
                    ui.end_row();

                    ui.add(|ui: &mut egui::Ui| ui.label("r1"));
                    ui.label(format!("{}", &context.r1));
                    ui.end_row();

                    ui.add(|ui: &mut egui::Ui| ui.label("r2"));
                    ui.label(format!("{}", &context.r2));
                    ui.end_row();

                    ui.add(|ui: &mut egui::Ui| ui.label("obj"));
                    ui.label(format!("{}", &context.object_register));
                    ui.end_row();
                });
        }
    }

    fn show_stack(&self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Stack").strong());
        ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                egui::Grid::new("my_grid")
                    .num_columns(3)
                    .spacing([4.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        let context = self.context.read().unwrap();
                        for i in 0..context.stack.len() / 4 + 1 {
                            let addr = i * 4;
                            let resp = ui.add(|ui: &mut egui::Ui| ui.label(format!("{}", addr)));

                            if context.sp >= addr && context.sp < addr + 4 {
                                resp.scroll_to_me(None);
                            }

                            ui.label(format!(
                                "{} {}",
                                if addr == context.sp { "sp" } else { "" },
                                if addr == context.fp { "fp" } else { "" },
                            ));

                            if addr + 4 > context.stack.len() {
                                ui.label(egui::RichText::new("Bottom of Stack").weak());
                            } else {
                                let mut line = "".to_string();
                                for j in 0..4 {
                                    let addr = addr + j;
                                    // let s = if addr == context.sp { "s" } else { " " };
                                    // let f = if addr == context.fp { "f" } else { " " };
                                    line = format!("{}{:02X}  ", line, context.stack[addr]);
                                }

                                ui.label(line);
                            }

                            ui.end_row();
                        }
                    });
            });
    }

    fn show_debugger(
        &mut self,
        /*ctx: &egui::Context*/ ui: &mut egui::Ui,
        frame: &mut eframe::Frame,
    ) {
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
            .show_inside(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(state);
                });

                self.show_toolbar(ui);
                ui.separator();
                self.show_code(ui);
            });

        egui::SidePanel::right("right_panel")
            .resizable(true)
            .default_width(100.0)
            .width_range((window_width / 6.)..=(window_width / 2.))
            .show_inside(ui, |ui| {
                self.show_stack(ui);
            });

        egui::CentralPanel::default().show_inside(ui, |ui| self.context_info(ui));
    }
}

impl eframe::App for AsDebugApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::SidePanel::left("side")
            .resizable(false)
            .exact_width(48.)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    if ui.button(egui::RichText::new("🚧").size(36.)).clicked() {
                        self.state = AppState::Debugger;
                    }
                    if ui.button(egui::RichText::new("📃").size(36.)).clicked() {
                        self.state = AppState::Disassembler;
                    }
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            match self.state {
                AppState::Debugger => self.show_debugger(ui, frame),
                AppState::Disassembler => self.dv.show(ctx, frame),
            };
        });
    }
}
