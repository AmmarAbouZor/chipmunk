//! UI-side state tracking for the search-values extraction pipeline.
//!
//! This keeps only operation lifecycle data (id + phase). Result payloads are handled
//! in chart-specific state.

use uuid::Uuid;

use crate::session::{types::OperationPhase, ui::definitions::UpdateOperationOutcome};

#[derive(Debug, Clone)]
/// Metadata for the currently running search-values backend operation.
struct SearchValuesOperation {
    /// Backend operation identifier used for updates/cancel.
    id: Uuid,
    /// Latest known operation phase from backend events.
    phase: OperationPhase,
}

impl SearchValuesOperation {
    /// Creates a new operation in the `Initializing` phase.
    fn new(id: Uuid) -> Self {
        Self {
            id,
            phase: OperationPhase::Initializing,
        }
    }
}

#[derive(Debug, Default)]
/// Shared state for search-values pipeline synchronization in a session.
pub struct SearchValuesState {
    /// Active search-values operation, if one is currently tracked.
    operation: Option<SearchValuesOperation>,
}

impl SearchValuesState {
    /// Starts tracking a new search-values operation and replaces any previous one.
    pub fn set_operation(&mut self, id: Uuid) {
        self.operation = Some(SearchValuesOperation::new(id));
    }

    /// Clears the currently tracked search-values operation.
    pub fn clear_operation(&mut self) {
        self.operation = None;
    }

    /// Returns the current operation id while it is still running.
    ///
    /// `Done` operations are treated as non-running and return `None`.
    pub fn processing_operation(&self) -> Option<Uuid> {
        self.operation.as_ref().and_then(|op| {
            if op.phase != OperationPhase::Done {
                Some(op.id)
            } else {
                None
            }
        })
    }

    pub fn update_operation(
        &mut self,
        operation_id: Uuid,
        phase: OperationPhase,
    ) -> UpdateOperationOutcome {
        // We only consume updates that belong to the active tracked operation.
        if let Some(operation) = self.operation.as_mut()
            && operation.id == operation_id
        {
            operation.phase = phase;
            if phase == OperationPhase::Done {
                self.clear_operation();
            }
            UpdateOperationOutcome::Consumed
        } else {
            UpdateOperationOutcome::None
        }
    }
}
