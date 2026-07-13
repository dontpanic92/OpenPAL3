//! `IUiLayer` implementation that hosts the whole patcher GUI as a
//! single `Scene`-band layer (see `src/bin/yaobow_asset_patcher.rs` for
//! how it's registered). Kept in its own module — mirroring
//! `yaobow/yaobow/src/openpal3/debug_layer.rs`'s
//! `ComObject_OpenPal3DebugLayer!(super::OpenPal3DebugLayer);` pattern
//! — so the `ComObject_AssetPatcherUiLayer!` macro (generated from
//! `idl/yaobow_asset_patcher.idl` into `crate::comdef`, and made
//! available crate-wide via `#[macro_use] pub mod comdef` in `lib.rs`)
//! finds its `super::AssetPatcherUiLayer` target in the same file it's
//! invoked from.
//!
//! All state lives in [`GuiState`], driven entirely through
//! [`crate::transaction`], [`crate::startup`], [`crate::environment`]
//! and [`crate::config`] — this file only renders imgui widgets and
//! wires button clicks to those library calls. No transaction logic is
//! duplicated here.

use std::path::PathBuf;
use std::rc::Rc;

use crosscom::ComRc;
use radiance::comdef::{IUiHost, IUiLayerImpl};
use radiance::radiance::UiManager;
use uuid::Uuid;

use crate::environment::GameRoot;
use crate::fault::NoFaults;
use crate::plan::PatchPlan;
use crate::startup::{self, PendingInstall};
use crate::transaction::{self, ApplyOptions};
use crate::validate::{Severity, ValidationSummary};

/// Game/config key this build of the GUI installs against. The
/// transaction/validation library is game-agnostic (it just compares
/// this string against `PatchManifest::target_game`); PAL3 is the only
/// target wired up in the GUI today, matching the task's "select/detect
/// PAL3 root" requirement.
const EXPECTED_GAME: &str = "pal3";

pub struct AssetPatcherUiLayer {
    ui: Rc<UiManager>,
    state: std::cell::RefCell<GuiState>,
}

#[derive(Default)]
struct GuiState {
    game_root: Option<PathBuf>,
    root_looks_like_pal3: bool,
    yapatch_path: Option<PathBuf>,
    manifest: Option<asset_project::patch::PatchManifest>,
    plan: Option<PatchPlan>,
    validation: Option<ValidationSummary>,
    pending_installs: Vec<PendingInstall>,
    journal_entries: Vec<asset_project::journal::JournalEntry>,
    status: Option<String>,
    status_is_error: bool,
}

impl AssetPatcherUiLayer {
    pub fn new(ui: Rc<UiManager>) -> Self {
        let mut state = GuiState::default();

        // Best-effort auto-detect on startup: an explicit config
        // override, the current directory, and the platform config
        // file's remembered `asset_path` for `pal3` (see
        // `environment::candidate_roots`). The user can always
        // override with "Select PAL3 Root..." below.
        let candidates = crate::environment::candidate_roots(None);
        if let Some(root) = crate::environment::detect_pal3_root(candidates) {
            state.game_root = Some(root.root().to_path_buf());
        }

        let mut layer = Self {
            ui,
            state: std::cell::RefCell::new(state),
        };
        layer.refresh_root_derived_state();
        layer
    }

    /// Recomputes everything that depends on `game_root` alone
    /// (pending-install detection, journal entries) — called whenever
    /// the root changes.
    fn refresh_root_derived_state(&mut self) {
        let mut state = self.state.borrow_mut();
        if let Some(root) = state.game_root.clone() {
            state.root_looks_like_pal3 = GameRoot::open(&root).looks_like_pal3();
            state.pending_installs = startup::detect_pending(&root).unwrap_or_default();
            state.journal_entries = transaction::list_journal_entries(&root).unwrap_or_default();
        } else {
            state.root_looks_like_pal3 = false;
            state.pending_installs.clear();
            state.journal_entries.clear();
        }
    }

    fn pick_root(&self) {
        if let Some(path) = pick_folder_native() {
            self.state.borrow_mut().game_root = Some(path);
            // SAFETY-free: `refresh_root_derived_state` takes `&mut
            // self`, but we only have `&self` here (imgui render calls
            // are all through `&self`) — reborrow through the RefCell
            // instead via a small helper that doesn't need `&mut
            // Self`.
            self.refresh_root_derived_state_shared();
        }
    }

    /// Same as [`Self::refresh_root_derived_state`] but callable
    /// through `&self` (everything it touches is already behind a
    /// `RefCell`).
    fn refresh_root_derived_state_shared(&self) {
        let root = self.state.borrow().game_root.clone();
        let mut state = self.state.borrow_mut();
        if let Some(root) = root {
            state.root_looks_like_pal3 = GameRoot::open(&root).looks_like_pal3();
            state.pending_installs = startup::detect_pending(&root).unwrap_or_default();
            state.journal_entries = transaction::list_journal_entries(&root).unwrap_or_default();
        } else {
            state.root_looks_like_pal3 = false;
            state.pending_installs.clear();
            state.journal_entries.clear();
        }
    }

    fn pick_patch(&self) {
        let Some(path) = pick_open_file_native() else {
            return;
        };

        let mut state = self.state.borrow_mut();
        state.yapatch_path = Some(path.clone());
        state.manifest = None;
        state.plan = None;
        state.validation = None;
        state.status = None;

        match crate::transaction::plan_patch(&path) {
            Ok(plan) => state.plan = Some(plan),
            Err(e) => {
                state.status = Some(format!("failed to open patch: {e}"));
                state.status_is_error = true;
                return;
            }
        }

        if let Some(root) = state.game_root.clone() {
            drop(state);
            self.validate_against(&root);
        }
    }

    fn validate_against(&self, root: &std::path::Path) {
        let yapatch_path = self.state.borrow().yapatch_path.clone();
        let Some(yapatch_path) = yapatch_path else {
            return;
        };

        match transaction::validate_patch(&yapatch_path, root, EXPECTED_GAME) {
            Ok((manifest, summary)) => {
                let mut state = self.state.borrow_mut();
                state.manifest = Some(manifest);
                state.validation = Some(summary);
            }
            Err(e) => {
                let mut state = self.state.borrow_mut();
                state.status = Some(format!("validation failed: {e}"));
                state.status_is_error = true;
            }
        }
    }

    fn do_apply(&self) {
        let (yapatch_path, root) = {
            let state = self.state.borrow();
            match (state.yapatch_path.clone(), state.game_root.clone()) {
                (Some(p), Some(r)) => (p, r),
                _ => return,
            }
        };

        let result = transaction::apply(
            &yapatch_path,
            &root,
            EXPECTED_GAME,
            ApplyOptions {
                fault_injector: Some(Box::new(NoFaults)),
            },
        );

        let mut state = self.state.borrow_mut();
        match result {
            Ok(report) => {
                state.status = Some(format!(
                    "applied patch {} ({} package(s), {} change(s))",
                    report.patch_id,
                    report.touched_packages.len(),
                    report.changes_applied
                ));
                state.status_is_error = false;
            }
            Err(e) => {
                state.status = Some(format!("apply failed: {e}"));
                state.status_is_error = true;
            }
        }
        drop(state);
        self.refresh_root_derived_state_shared();
    }

    fn do_rollback(&self, patch_id: Uuid) {
        let root = self.state.borrow().game_root.clone();
        let Some(root) = root else { return };

        let result = transaction::rollback(&root, patch_id);
        let mut state = self.state.borrow_mut();
        match result {
            Ok(report) => {
                state.status = Some(format!(
                    "rolled back patch {} ({} package(s) restored)",
                    report.patch_id,
                    report.packages_restored.len()
                ));
                state.status_is_error = false;
            }
            Err(e) => {
                state.status = Some(format!("rollback failed: {e}"));
                state.status_is_error = true;
            }
        }
        drop(state);
        self.refresh_root_derived_state_shared();
    }

    fn do_recover_all_pending(&self) {
        let root = self.state.borrow().game_root.clone();
        let Some(root) = root else { return };

        let result = startup::recover_all_pending(&root);
        let mut state = self.state.borrow_mut();
        match result {
            Ok(recovered) => {
                state.status = Some(format!(
                    "recovered {} interrupted install(s)",
                    recovered.len()
                ));
                state.status_is_error = false;
            }
            Err(e) => {
                state.status = Some(format!("recovery failed: {e}"));
                state.status_is_error = true;
            }
        }
        drop(state);
        self.refresh_root_derived_state_shared();
    }

    fn render_window(&self) {
        let ui = self.ui.ui();
        ui.window("Yaobow Asset Patcher").build(|| {
            let mut state = self.state.borrow_mut();

            if !state.pending_installs.is_empty() {
                ui.text_colored(
                    [1.0, 0.6, 0.1, 1.0],
                    format!(
                        "{} interrupted install(s) detected from a previous run.",
                        state.pending_installs.len()
                    ),
                );
                drop(state);
                if ui.button("Recover Now") {
                    self.do_recover_all_pending();
                }
                ui.separator();
                state = self.state.borrow_mut();
            }

            ui.text("PAL3 root:");
            ui.same_line();
            match &state.game_root {
                Some(root) => ui.text(root.to_string_lossy().to_string()),
                None => ui.text_colored([0.9, 0.3, 0.3, 1.0], "(not selected)"),
            }
            if state.game_root.is_some() && !state.root_looks_like_pal3 {
                ui.text_colored(
                    [0.9, 0.3, 0.3, 1.0],
                    "This does not look like a PAL3 install (basedata/basedata.cpk not found).",
                );
            }
            drop(state);
            if ui.button("Select PAL3 Root...") {
                self.pick_root();
            }

            ui.separator();

            let state = self.state.borrow();
            ui.text(".yapatch:");
            ui.same_line();
            match &state.yapatch_path {
                Some(p) => ui.text(p.to_string_lossy().to_string()),
                None => ui.text_colored([0.9, 0.3, 0.3, 1.0], "(none opened)"),
            }
            drop(state);
            if ui.button("Open .yapatch...") {
                self.pick_patch();
            }

            ui.separator();

            let state = self.state.borrow();
            if let Some(plan) = &state.plan {
                ui.text("Dry-run plan (grouped by package):");
                for package_plan in &plan.packages {
                    ui.text(format!(
                        "  {} - {} add, {} replace, {} bytes total",
                        package_plan.target_package.as_str(),
                        package_plan.add_count(),
                        package_plan.replace_count(),
                        package_plan.total_payload_size()
                    ));
                }
            }

            if let Some(summary) = &state.validation {
                ui.separator();
                ui.text("Validation summary:");
                for issue in summary.all_issues() {
                    let color = match issue.severity {
                        Severity::Error => [0.9, 0.3, 0.3, 1.0],
                        Severity::Warning => [0.9, 0.7, 0.2, 1.0],
                    };
                    ui.text_colored(color, format!("  {}", issue.message));
                }
                if summary.is_ok() {
                    ui.text_colored([0.3, 0.9, 0.3, 1.0], "Ready to apply.");
                }
            }

            let can_apply = state
                .validation
                .as_ref()
                .map(|v| v.is_ok())
                .unwrap_or(false);
            drop(state);

            ui.separator();
            if !can_apply {
                ui.text_colored(
                    [0.6, 0.6, 0.6, 1.0],
                    "(select a valid root + patch with no validation errors to enable Apply)",
                );
            }
            if ui.button("Apply") {
                self.do_apply();
            }

            ui.separator();
            let state = self.state.borrow();
            if !state.journal_entries.is_empty() {
                ui.text("Installation history (most recent first):");
                let entries = state.journal_entries.clone();
                drop(state);
                for entry in &entries {
                    ui.text(format!("  {} - {:?}", entry.patch_id, entry.status));
                    if entry.status == asset_project::journal::InstallStatus::Applied {
                        ui.same_line();
                        if ui.button(&format!("Rollback##{}", entry.patch_id)) {
                            self.do_rollback(entry.patch_id);
                        }
                    }
                }
            } else {
                drop(state);
            }

            let state = self.state.borrow();
            if let Some(status) = &state.status {
                ui.separator();
                let color = if state.status_is_error {
                    [0.9, 0.3, 0.3, 1.0]
                } else {
                    [0.3, 0.9, 0.3, 1.0]
                };
                ui.text_colored(color, status);
            }
        });
    }
}

impl IUiLayerImpl for AssetPatcherUiLayer {
    fn render(&self, _ui_host: ComRc<IUiHost>, _delta_sec: f32) {
        self.render_window();
    }
}

ComObject_AssetPatcherUiLayer!(super::AssetPatcherUiLayer);

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn pick_folder_native() -> Option<PathBuf> {
    use native_dialog::FileDialogBuilder;
    match FileDialogBuilder::default().open_single_dir().show() {
        Ok(Some(path)) => Some(path),
        Ok(None) => None,
        Err(e) => {
            log::warn!("folder picker failed: {e}");
            None
        }
    }
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn pick_folder_native() -> Option<PathBuf> {
    None
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn pick_open_file_native() -> Option<PathBuf> {
    use native_dialog::FileDialogBuilder;
    match FileDialogBuilder::default()
        .add_filter("Yaobow Asset Patch", &["yapatch"])
        .open_single_file()
        .show()
    {
        Ok(Some(path)) => Some(path),
        Ok(None) => None,
        Err(e) => {
            log::warn!("open-file picker failed: {e}");
            None
        }
    }
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn pick_open_file_native() -> Option<PathBuf> {
    None
}
