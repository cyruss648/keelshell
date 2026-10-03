//! Explicit command-template edits without transport or execution side effects.

use uuid::Uuid;

use crate::{AppState, Error, Snippet, ValidationError};

impl AppState {
    /// Append a validated command template, preserving its caller-supplied ID.
    ///
    /// This edits memory only; persist the edited state through [`crate::StateStore`]
    /// after explicit user review. Template commands are plaintext application data
    /// and must not be populated automatically from terminal output or history.
    ///
    /// # Errors
    /// Returns an error for a duplicate ID, an invalid template, a
    /// collection beyond 2,000 entries, or invalid application state. Any error
    /// leaves this snapshot, including its persistence revision, unchanged.
    pub fn insert_snippet(&mut self, snippet: Snippet) -> Result<(), Error> {
        if self.snippets.iter().any(|entry| entry.id == snippet.id) {
            return Err(ValidationError::new("snippet.id", "must be unique").into());
        }
        let mut candidate = self.clone();
        candidate.snippets.push(snippet);
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    /// Replace the template with the same ID without changing its display order.
    ///
    /// An unknown ID is rejected, never inserted. Saving this snapshot is a
    /// separate operation and does not execute or select the template.
    ///
    /// # Errors
    /// Returns an error for a missing or ambiguous ID, invalid replacement, or
    /// invalid application state. Any error leaves this snapshot and revision
    /// unchanged.
    pub fn update_snippet(&mut self, snippet: Snippet) -> Result<(), Error> {
        let index = unique_index(&self.snippets, snippet.id)?;
        let mut candidate = self.clone();
        candidate.snippets[index] = snippet;
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    /// Remove one template by stable ID and return its original contents.
    ///
    /// This preserves the order of the remaining templates. Removal is an
    /// in-memory edit until the caller explicitly saves the state.
    ///
    /// # Errors
    /// Returns an error for a missing or ambiguous ID or invalid remaining
    /// application state. Any error leaves this snapshot and revision unchanged.
    pub fn remove_snippet(&mut self, id: Uuid) -> Result<Snippet, Error> {
        let index = unique_index(&self.snippets, id)?;
        let mut candidate = self.clone();
        let removed = candidate.snippets.remove(index);
        candidate.validate()?;
        *self = candidate;
        Ok(removed)
    }
}

fn unique_index(snippets: &[Snippet], id: Uuid) -> Result<usize, ValidationError> {
    let mut matching = snippets
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.id == id);
    let (index, _) = matching
        .next()
        .ok_or_else(|| ValidationError::new("snippet.id", "was not found"))?;
    if matching.next().is_some() {
        return Err(ValidationError::new("snippet.id", "must be unique"));
    }
    Ok(index)
}
