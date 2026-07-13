//! Fault-injection hooks used by this crate's own failure-injection
//! tests to force a specific step of [`crate::transaction::apply`] to
//! fail, so the recovery behavior (already-swapped packages restored
//! from backup, journal marked `Failed`, temp files cleaned up) can be
//! exercised deterministically instead of relying on flaky real I/O
//! failures.
//!
//! Production callers (the GUI binary) never construct a
//! [`FaultInjector`] — [`crate::transaction::ApplyOptions::default`]
//! leaves it `None`, which is a complete no-op.

use asset_project::manifest::TargetPackage;

/// A specific point in the apply pipeline where a test can ask for an
/// injected failure. Named after the guarantee being tested rather
/// than the implementation step, so tests read as specifications.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailurePoint {
    /// After the pre-patch backup of `package` has been written to the
    /// patch-specific backup directory (but before any temp `.cpk` is
    /// built).
    AfterBackup(TargetPackage),
    /// After the sibling temp `.cpk` for `package` has been built and
    /// verified (but before any package's swap phase begins).
    AfterTempBuild(TargetPackage),
    /// Immediately before the atomic rename that swaps `package`'s
    /// temp file into place.
    BeforeSwap(TargetPackage),
    /// Immediately after `package` has been atomically swapped into
    /// place.
    AfterSwap(TargetPackage),
}

impl FailurePoint {
    pub fn describe(&self) -> String {
        match self {
            FailurePoint::AfterBackup(p) => format!("after backing up {}", p.as_str()),
            FailurePoint::AfterTempBuild(p) => {
                format!("after building temp cpk for {}", p.as_str())
            }
            FailurePoint::BeforeSwap(p) => format!("before swapping {}", p.as_str()),
            FailurePoint::AfterSwap(p) => format!("after swapping {}", p.as_str()),
        }
    }
}

/// Test seam: implementors decide whether the transaction should fail
/// at a given [`FailurePoint`]. The default (used in production)
/// never fails anything.
pub trait FaultInjector {
    fn should_fail(&self, point: &FailurePoint) -> bool;
}

/// No-op injector used whenever the caller doesn't pass one.
pub struct NoFaults;

impl FaultInjector for NoFaults {
    fn should_fail(&self, _point: &FailurePoint) -> bool {
        false
    }
}

/// Fails at exactly one specific [`FailurePoint`] (compared by
/// variant + package), and never again — matches the common test
/// shape of "fail once, partway through a multi-package apply".
pub struct FailAt(pub FailurePoint);

impl FaultInjector for FailAt {
    fn should_fail(&self, point: &FailurePoint) -> bool {
        &self.0 == point
    }
}

/// Fails at every [`FailurePoint`] matching any of a list — useful for
/// forcing failure regardless of package iteration order.
pub struct FailAtAny(pub Vec<FailurePoint>);

impl FaultInjector for FailAtAny {
    fn should_fail(&self, point: &FailurePoint) -> bool {
        self.0.contains(point)
    }
}
