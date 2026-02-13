use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use p7::ModuleProvider;
use radiance::ui::UiScriptRunner;

#[derive(Clone)]
struct FileSystemModuleProvider {
    script_dir: PathBuf,
    std_dir: PathBuf,
}

impl FileSystemModuleProvider {
    fn new(script_path: &Path) -> Self {
        let script_dir = script_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        let std_dir = default_p7_std_dir();

        Self { script_dir, std_dir }
    }

    fn module_path_to_file_path(module_path: &str) -> PathBuf {
        let parts: Vec<&str> = module_path.split('.').collect();
        let mut path = PathBuf::new();
        for (i, part) in parts.iter().enumerate() {
            if i == parts.len() - 1 {
                path.push(format!("{}.p7", part));
            } else {
                path.push(part);
            }
        }
        path
    }

    fn load_from_directory(&self, base_dir: &Path, module_path: &str) -> Option<String> {
        let file_path = base_dir.join(Self::module_path_to_file_path(module_path));
        fs::read_to_string(&file_path).ok()
    }
}

impl ModuleProvider for FileSystemModuleProvider {
    fn load_module(&self, module_path: &str) -> Option<String> {
        if module_path.starts_with("std.") {
            let relative_path = &module_path[4..];
            return self.load_from_directory(&self.std_dir, relative_path);
        }

        if module_path == "std" {
            let mod_file = self.std_dir.join("mod.p7");
            if mod_file.is_file() {
                return fs::read_to_string(&mod_file).ok();
            }
        }

        self.load_from_directory(&self.script_dir, module_path)
    }

    fn clone_boxed(&self) -> Box<dyn ModuleProvider> {
        Box::new(self.clone())
    }
}

pub fn resolve_ui_script_path(args: &[String]) -> Option<PathBuf> {
    if let Ok(path) = std::env::var("YAOBOW_EDITOR_UI_SCRIPT") {
        if !path.trim().is_empty() {
            return Some(PathBuf::from(path));
        }
    }

    if let Ok(flag) = std::env::var("YAOBOW_EDITOR_P7_UI") {
        if is_truthy(&flag) {
            return Some(default_ui_script_path());
        }
    }

    for arg in args {
        if arg == "--p7-ui" {
            return Some(default_ui_script_path());
        }
        if let Some(path) = arg.strip_prefix("--p7-ui=") {
            if !path.trim().is_empty() {
                return Some(PathBuf::from(path));
            }
        }
    }

    None
}

pub fn load_ui_script_runner(script_path: &Path) -> Result<UiScriptRunner> {
    if !script_path.is_file() {
        return Err(anyhow!("UI script not found: {}", script_path.display()));
    }

    let script_source = fs::read_to_string(script_path)
        .with_context(|| format!("Failed to read script: {}", script_path.display()))?;

    let provider = FileSystemModuleProvider::new(script_path);
    UiScriptRunner::new(script_source, Box::new(provider))
        .with_context(|| format!("Failed to compile script: {}", script_path.display()))
}

fn default_ui_script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("ui")
        .join("welcome.p7")
}

fn default_p7_std_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("radiance")
        .join("p7lang")
        .join("std")
}

fn is_truthy(value: &str) -> bool {
    matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on")
}
