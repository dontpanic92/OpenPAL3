use std::path::{Path, PathBuf};

use p7::ModuleProvider;
use p7::embedding::{CallOutcome, Runtime};
use yaobow_asset_patcher::service::ManagerService;

const UI_SOURCE: &str = include_str!("../../scripts/main.p7");

fn main() {
    let logger = simple_logger::SimpleLogger::new();
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "android"))]
    let logger = logger.with_utc_timestamps();
    let _ = logger.init();

    if let Err(error) = run() {
        eprintln!("yaobow_asset_patcher: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let resources = P7LclResources::locate()?;
    let lcl_source = std::fs::read_to_string(&resources.module).map_err(|error| {
        format!(
            "failed to read p7-lcl module {}: {error}",
            resources.module.display()
        )
    })?;

    let module = compile_ui(lcl_source)?;

    let mut runtime = Runtime::new();
    let result = (|| {
        runtime
            .load_native_extension(&resources.native_library)
            .map_err(|error| {
                format!(
                    "failed to load p7-lcl native library {}: {error}",
                    resources.native_library.display()
                )
            })?;
        yaobow_asset_patcher::bridge::register(&mut runtime, ManagerService::new());
        runtime.load_module(module);
        match runtime.call("main", Vec::new()) {
            Ok(CallOutcome::Returned(_)) => Ok(()),
            Ok(CallOutcome::Threw(value)) => Err(format!("the mod-manager UI threw {value:?}")),
            Ok(CallOutcome::Trapped(error)) => Err(format!("the mod-manager UI trapped: {error}")),
            Err(error) => Err(format!("failed to start the mod-manager UI: {error}")),
        }
    })();

    let shutdown = runtime
        .shutdown()
        .map_err(|error| format!("p7-lcl shutdown failed: {error}"));
    match (result, shutdown) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(error),
        (Err(error), Err(shutdown_error)) => Err(format!("{error}\n{shutdown_error}")),
    }
}

fn compile_ui(lcl_source: String) -> Result<p7::bytecode::Module, String> {
    let mut provider = p7::InMemoryModuleProvider::new();
    provider.add_module("lcl".to_string(), lcl_source);
    p7::compile_module_with_provider(
        UI_SOURCE.to_string(),
        "yaobow_asset_patcher.main",
        provider.clone_boxed(),
    )
    .map_err(|error| format!("failed to compile the mod-manager UI: {error}"))
}

#[derive(Debug)]
struct P7LclResources {
    module: PathBuf,
    native_library: PathBuf,
}

impl P7LclResources {
    fn locate() -> Result<Self, String> {
        let root = if let Some(override_path) = std::env::var_os("YAOBOW_P7_LCL_DIR") {
            PathBuf::from(override_path)
        } else {
            let executable = std::env::current_exe()
                .map_err(|error| format!("failed to locate the current executable: {error}"))?;
            executable
                .parent()
                .ok_or_else(|| {
                    format!(
                        "current executable has no parent directory: {}",
                        executable.display()
                    )
                })?
                .join("p7-lcl")
        };
        Self::from_root(&root).map_err(|error| {
            format!(
                "{error}\nSet YAOBOW_P7_LCL_DIR to an extracted p7-lcl v0.1.0 package when developing."
            )
        })
    }

    fn from_root(root: &Path) -> Result<Self, String> {
        let module = root.join("src").join("mod.p7");
        let native_library = root.join("native").join("lib").join(native_library_name());
        if !module.is_file() {
            return Err(format!(
                "p7-lcl v0.1.0 module is missing: {}",
                module.display()
            ));
        }
        if !native_library.is_file() {
            return Err(format!(
                "p7-lcl v0.1.0 native library is missing: {}",
                native_library.display()
            ));
        }
        Ok(Self {
            module,
            native_library,
        })
    }
}

#[cfg(target_os = "windows")]
fn native_library_name() -> &'static str {
    "p7lcl.dll"
}

#[cfg(target_os = "linux")]
fn native_library_name() -> &'static str {
    "libp7lcl.so"
}

#[cfg(target_os = "macos")]
fn native_library_name() -> &'static str {
    "libp7lcl.dylib"
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
compile_error!("yaobow_asset_patcher supports only Windows, Linux, and macOS");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_compiles_against_staged_release() {
        let Some(root) = std::env::var_os("YAOBOW_P7_LCL_DIR") else {
            return;
        };
        let resources = P7LclResources::from_root(Path::new(&root)).unwrap();
        let source = std::fs::read_to_string(resources.module).unwrap();
        compile_ui(source).unwrap();
        let mut runtime = Runtime::new();
        runtime
            .load_native_extension(&resources.native_library)
            .unwrap();
        runtime.shutdown().unwrap();
    }
}
