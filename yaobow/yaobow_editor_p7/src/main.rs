mod ui_script;

use anyhow::{Context, Result};
use radiance::application::Application;
use radiance::comdef::IApplicationImpl;

fn main() -> Result<()> {
    let logger = simple_logger::SimpleLogger::new();

    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "android"))]
    let logger = logger.with_utc_timestamps();

    logger.init().unwrap();

    let args: Vec<String> = std::env::args().collect();

    let script_path = ui_script::resolve_ui_script_path(&args);
    let runner = ui_script::load_ui_script_runner(&script_path)
        .with_context(|| format!("Failed to load p7 UI script: {}", script_path.display()))?;

    let app = Application::new();
    app.set_title("Yaobow Editor (p7)");
    app.engine()
        .borrow()
        .set_ui_script_runner(runner)
        .context("Failed to initialize p7 UI script runner")?;

    app.initialize();
    app.run();

    Ok(())
}
