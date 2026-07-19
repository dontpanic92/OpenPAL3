//! Asynchronous, UI-neutral mod-manager service.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use uuid::Uuid;

use crate::error::{PatcherError, Result};
use crate::transaction::{ApplyOptions, OperationPhase, OperationProgress, ProgressReporter};

const EXPECTED_GAME: &str = "pal3";

#[derive(Debug, Clone)]
pub struct ModEntry {
    pub patch_id: Uuid,
    pub source_name: String,
    pub label: String,
    pub details: String,
    pub validation: String,
    pub applied: bool,
    pub can_install: bool,
    pub can_uninstall: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobState {
    Idle,
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone)]
pub struct JobSnapshot {
    pub state: JobState,
    pub kind: String,
    pub message: String,
    pub completed: usize,
    pub total: usize,
}

impl Default for JobSnapshot {
    fn default() -> Self {
        Self {
            state: JobState::Idle,
            kind: String::new(),
            message: "Ready".to_string(),
            completed: 0,
            total: 0,
        }
    }
}

#[derive(Default)]
struct ServiceState {
    game_root: Option<PathBuf>,
    mods: Vec<ModEntry>,
    job: JobSnapshot,
}

#[derive(Clone, Default)]
pub struct ManagerService {
    state: Arc<Mutex<ServiceState>>,
}

impl ManagerService {
    pub fn new() -> Self {
        let service = Self::default();
        if let Some(root) =
            crate::environment::detect_pal3_root(crate::environment::candidate_roots(None))
        {
            service.lock().game_root = Some(root.root().to_path_buf());
        }
        service
    }

    pub fn with_root(game_root: impl Into<PathBuf>) -> Result<Self> {
        let service = Self::default();
        service.set_root(game_root.into())?;
        service.refresh_sync()?;
        Ok(service)
    }

    pub fn root_path(&self) -> String {
        self.lock()
            .game_root
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    pub fn set_root(&self, path: PathBuf) -> Result<()> {
        if !path.is_dir() {
            return Err(PatcherError::ValidationFailed(format!(
                "{} is not a directory",
                path.display()
            )));
        }
        let root = crate::environment::GameRoot::open(&path);
        if !root.looks_like_pal3() {
            return Err(PatcherError::ValidationFailed(format!(
                "{} does not look like a PAL3 install",
                path.display()
            )));
        }
        let mut state = self.lock();
        if state.job.state == JobState::Running {
            return Err(PatcherError::Other(
                "cannot change game root while an operation is running".to_string(),
            ));
        }
        state.game_root = Some(path);
        state.mods.clear();
        state.job = JobSnapshot::default();
        Ok(())
    }

    pub fn mod_count(&self) -> usize {
        self.lock().mods.len()
    }

    pub fn mod_entry(&self, index: usize) -> Option<ModEntry> {
        self.lock().mods.get(index).cloned()
    }

    pub fn job(&self) -> JobSnapshot {
        self.lock().job.clone()
    }

    pub fn acknowledge_job(&self) {
        let mut state = self.lock();
        if state.job.state != JobState::Running {
            state.job = JobSnapshot::default();
        }
    }

    pub fn start_refresh(&self) -> Result<()> {
        let root = self.require_root()?;
        self.start_job("Refresh", move || {
            crate::startup::recover_all_pending(&root)?;
            Ok(())
        })
    }

    pub fn start_import(&self, source_path: PathBuf) -> Result<()> {
        let root = self.require_root()?;
        self.start_job("Import", move || {
            crate::manager::import_patch(&root, &source_path, EXPECTED_GAME)?;
            Ok(())
        })
    }

    pub fn start_install(&self, index: usize) -> Result<()> {
        let root = self.require_root()?;
        let patch = self
            .mod_entry(index)
            .ok_or_else(|| PatcherError::Other(format!("invalid mod index {index}")))?;
        if !patch.can_install {
            return Err(PatcherError::ValidationFailed(patch.validation.clone()));
        }
        let managed_path = crate::manager::managed_patch_path(&root, patch.patch_id);
        let reporter: Arc<dyn ProgressReporter> = Arc::new(ServiceProgress {
            service: self.clone(),
        });
        self.start_job("Install", move || {
            crate::transaction::apply(
                &managed_path,
                &root,
                EXPECTED_GAME,
                ApplyOptions {
                    progress_reporter: Some(reporter),
                    ..ApplyOptions::default()
                },
            )?;
            Ok(())
        })
    }

    pub fn start_uninstall(&self, index: usize) -> Result<()> {
        let root = self.require_root()?;
        let patch = self
            .mod_entry(index)
            .ok_or_else(|| PatcherError::Other(format!("invalid mod index {index}")))?;
        if !patch.can_uninstall {
            return Err(PatcherError::PatchNotApplied(patch.patch_id));
        }
        let reporter: Arc<dyn ProgressReporter> = Arc::new(ServiceProgress {
            service: self.clone(),
        });
        self.start_job("Uninstall", move || {
            crate::transaction::uninstall(
                &root,
                patch.patch_id,
                ApplyOptions {
                    progress_reporter: Some(reporter),
                    ..ApplyOptions::default()
                },
            )?;
            Ok(())
        })
    }

    pub fn refresh_sync(&self) -> Result<()> {
        let root = self.require_root()?;
        let mods = load_catalog(&root)?;
        self.lock().mods = mods;
        Ok(())
    }

    fn start_job<F>(&self, kind: &str, operation: F) -> Result<()>
    where
        F: FnOnce() -> Result<()> + Send + 'static,
    {
        {
            let mut state = self.lock();
            if state.job.state == JobState::Running {
                return Err(PatcherError::Other(
                    "another mod-manager operation is already running".to_string(),
                ));
            }
            state.job = JobSnapshot {
                state: JobState::Running,
                kind: kind.to_string(),
                message: format!("{kind} started"),
                completed: 0,
                total: 0,
            };
        }

        let service = self.clone();
        std::thread::spawn(move || {
            let result = operation().and_then(|()| service.refresh_sync());
            let mut state = service.lock();
            match result {
                Ok(()) => {
                    state.job.state = JobState::Succeeded;
                    state.job.message = format!("{} completed", state.job.kind);
                }
                Err(error) => {
                    state.job.state = JobState::Failed;
                    state.job.message = error.to_string();
                }
            }
        });
        Ok(())
    }

    fn require_root(&self) -> Result<PathBuf> {
        self.lock()
            .game_root
            .clone()
            .ok_or_else(|| PatcherError::ValidationFailed("no PAL3 root selected".to_string()))
    }

    fn lock(&self) -> MutexGuard<'_, ServiceState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

struct ServiceProgress {
    service: ManagerService,
}

impl ProgressReporter for ServiceProgress {
    fn report(&self, progress: OperationProgress) {
        let mut state = self.service.lock();
        if state.job.state != JobState::Running {
            return;
        }
        state.job.completed = progress.completed;
        state.job.total = progress.total;
        state.job.message = format_progress(&progress);
    }
}

fn format_progress(progress: &OperationProgress) -> String {
    let phase = match progress.phase {
        OperationPhase::Validating => "Validating",
        OperationPhase::BackingUp => "Backing up",
        OperationPhase::Building => "Building",
        OperationPhase::Swapping => "Installing",
        OperationPhase::Committing => "Committing",
    };
    match &progress.target_package {
        Some(package) => format!(
            "{phase} {package} ({}/{})",
            progress.completed + 1,
            progress.total
        ),
        None => phase.to_string(),
    }
}

fn load_catalog(game_root: &Path) -> Result<Vec<ModEntry>> {
    let manager = crate::manager::ManagerState::load_or_default(game_root)?;
    let mut entries = Vec::new();
    for patch in crate::manager::list_managed_patches(game_root)? {
        let applied = manager.is_applied(patch.patch_id());
        let source_name = manager
            .source_name(patch.patch_id())
            .unwrap_or_else(|| {
                patch
                    .path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("patch.ybpatch")
            })
            .to_string();
        let plan = crate::plan::PatchPlan::from_manifest(&patch.manifest);
        let details = format!(
            "Patch: {}\nSource: {}\nCreated: {}\nBase project version: {}\nPackages: {}\nChanges: {}",
            patch.patch_id(),
            source_name,
            patch.manifest.created_at,
            patch.manifest.base_project_version,
            plan.packages.len(),
            plan.total_changes()
        );

        let (can_install, validation) = if applied {
            (false, "Installed".to_string())
        } else {
            let (_, summary) =
                crate::transaction::validate_patch(&patch.path, game_root, EXPECTED_GAME)?;
            let issues = summary
                .all_issues()
                .map(|issue| issue.message.clone())
                .collect::<Vec<_>>();
            let message = if issues.is_empty() {
                "Ready to install".to_string()
            } else {
                issues.join("\n")
            };
            (summary.is_ok(), message)
        };

        entries.push(ModEntry {
            patch_id: patch.patch_id(),
            label: format!("{} ({})", source_name, patch.patch_id()),
            source_name,
            details,
            validation,
            applied,
            can_install,
            can_uninstall: applied,
        });
    }
    entries.sort_by(|left, right| left.source_name.cmp(&right.source_name));
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::FixtureChange;

    fn wait_for_job(service: &ManagerService) -> JobSnapshot {
        for _ in 0..500 {
            let job = service.job();
            if job.state != JobState::Running {
                return job;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("job did not finish");
    }

    #[test]
    fn imports_installs_and_uninstalls_asynchronously() {
        let root = crate::test_scratch::dir("service-async");
        crate::fixtures::write_fixture_cpk(
            &root.join("basedata"),
            "basedata.cpk",
            &[("marker.txt", b"marker")],
        );
        let package =
            crate::fixtures::write_fixture_cpk(&root, "scene.cpk", &[("base.dat", b"base")]);
        let patch_path = root.join("mod.ybpatch");
        crate::fixtures::build_fixture_ybpatch(
            &patch_path,
            EXPECTED_GAME,
            1,
            &[(
                "scene.cpk",
                crate::fingerprint::package_fingerprint(&package).unwrap(),
            )],
            &[FixtureChange::add("scene.cpk", "mod.dat", b"mod")],
        );

        let service = ManagerService::with_root(&root).unwrap();
        service.start_import(patch_path).unwrap();
        assert_eq!(wait_for_job(&service).state, JobState::Succeeded);
        assert_eq!(service.mod_count(), 1);

        service.start_install(0).unwrap();
        assert_eq!(wait_for_job(&service).state, JobState::Succeeded);
        assert!(service.mod_entry(0).unwrap().applied);

        service.start_uninstall(0).unwrap();
        assert_eq!(wait_for_job(&service).state, JobState::Succeeded);
        assert!(!service.mod_entry(0).unwrap().applied);

        let _ = std::fs::remove_dir_all(root);
    }
}
