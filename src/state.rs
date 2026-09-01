//! Runtime state machine and fatal-failure reporting.
//!
//! The prototype has exactly four states. `Failed` is terminal: nothing in the
//! application transitions out of it, and the first recorded failure is the one
//! that is kept, so a later cascading symptom can never mask the original
//! actionable cause.

use bevy::prelude::*;

/// High-level phase of the humanoid prototype.
#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum PrototypeState {
    /// The root glTF and its dependencies are loading.
    #[default]
    Loading,
    /// The glTF contract was accepted and the scene has been spawned; the
    /// spawned hierarchy is still being validated.
    Validating,
    /// The humanoid is spawned, validated, and looping its walk clip.
    Running,
    /// A fatal, non-recoverable contract violation. Terminal.
    Failed,
}

/// The single fatal failure the application will report, on screen and in the
/// log. Empty until something actually fails.
#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct FailureReport {
    /// One-line description of what went wrong.
    pub summary: String,
    /// Actionable specifics: paths, expected values, and discovered values.
    pub details: Vec<String>,
}

impl FailureReport {
    /// Creates a populated report.
    pub fn new(summary: impl Into<String>, details: Vec<String>) -> Self {
        Self {
            summary: summary.into(),
            details,
        }
    }

    /// True once a failure has been recorded.
    pub fn is_recorded(&self) -> bool {
        !self.summary.is_empty()
    }

    /// Records the *first* failure only, and reports whether it did so.
    ///
    /// Failure is terminal, so a second failure is always a consequence of the
    /// first. Overwriting would replace the diagnosable root cause with its
    /// symptom.
    pub fn record(&mut self, summary: impl Into<String>, details: Vec<String>) -> bool {
        if self.is_recorded() {
            return false;
        }
        self.summary = summary.into();
        self.details = details;
        true
    }

    /// Renders the report as the lines shown on screen and written to the log.
    pub fn to_display_string(&self) -> String {
        if !self.is_recorded() {
            return String::new();
        }
        let mut text = self.summary.clone();
        for detail in &self.details {
            text.push_str("\n  ");
            text.push_str(detail);
        }
        text
    }
}

/// Records a fatal failure and enters the terminal [`PrototypeState::Failed`].
pub fn fail(
    next_state: &mut NextState<PrototypeState>,
    report: &mut FailureReport,
    summary: impl Into<String>,
    details: Vec<String>,
) {
    report.record(summary, details);
    next_state.set(PrototypeState::Failed);
}
