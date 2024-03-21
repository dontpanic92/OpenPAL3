use components::window::Window;
use eframe::egui;

mod components;
mod init;

const WINDOW_XML: &str = include_str!("./window.xml");

fn main() -> Result<(), eframe::Error> {
    env_logger::init();
    let mut window = Window::from_xml(WINDOW_XML);
    init::init_window(&mut window);
    run(window)
}

fn run(window: Window) -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        ..Default::default()
    };

    eframe::run_native(
        &window.title.clone(),
        options,
        Box::new(|cc| {
            let ec = cc.egui_ctx.clone();
            Box::new(App::new(ec, window))
        }),
    )
}

struct App {
    ec: egui::Context,
    window: Window,
}

impl App {
    fn new(ec: egui::Context, window: Window) -> Self {
        setup_font(&ec);
        Self { ec, window }
    }
}

impl eframe::App for App {
    fn update(&mut self, _ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.window.update(&self.ec, frame);
    }
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
