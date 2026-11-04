//! Bounded, explicit selection of saved historical knowledge for later review.
//!
//! This boundary performs no I/O and grants no authority over a source session.
//! Serialized records remain untrusted input: callers must redact and budget
//! them with the question before presenting an exact model request.

use std::fmt;

use uuid::Uuid;

use crate::SavedKnowledgeEntry;

/// Maximum saved records explicitly attached to one model-context draft.
pub const MAX_SAVED_CONTEXT_ENTRIES: usize = 8;
/// Maximum combined serialized UTF-8 bytes of selected historical records.
///
/// The inference adapter's input budget is a separate, potentially lower limit.
pub const MAX_SAVED_CONTEXT_BYTES: usize = 32 * 1024;

/// A non-content error while selecting or composing saved knowledge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SavedContextError {
    /// A record is missing, edited, or has an ambiguous identity in the library.
    #[error("saved knowledge changed; select the current record again")]
    StaleEntry,
    /// The explicit selection already contains the maximum number of records.
    #[error("select at most eight saved knowledge records")]
    TooManyEntries,
    /// The complete serialized records exceed the context-selection byte limit.
    #[error("selected saved knowledge exceeds 32 KiB")]
    TooLarge,
    /// A record could not be serialized; no partial composition is returned.
    #[error("saved knowledge could not be composed")]
    Serialization,
}

/// An ordered, ephemeral selection of exact saved records.
///
/// Records retain their original identity, source, session, capture kind,
/// timestamp and outcome. Neither selecting nor composing resolves those
/// historical identifiers to a live connection. This type is not serializable;
/// the selection itself is never restored as implicit permission after restart.
#[derive(Default, Clone, PartialEq, Eq)]
pub struct SavedKnowledgeContext {
    entries: Vec<SavedKnowledgeEntry>,
    bytes: usize,
}

impl fmt::Debug for SavedKnowledgeContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SavedKnowledgeContext")
            .field("entries", &self.entries.len())
            .field("bytes", &self.bytes)
            .finish()
    }
}

impl SavedKnowledgeContext {
    /// Borrow the exact records in the order explicitly selected.
    pub fn entries(&self) -> &[SavedKnowledgeEntry] {
        &self.entries
    }

    /// Return the combined serialized record size, including historical metadata.
    pub const fn bytes(&self) -> usize {
        self.bytes
    }

    /// Select an exact current record without implicitly replacing an old version.
    ///
    /// Selecting the same unchanged record twice is a no-op (`Ok(false)`).
    /// Errors leave the entire selection unchanged. The caller must supply the
    /// authoritative in-memory library, not search results or edited drafts.
    ///
    /// # Errors
    /// Returns [`SavedContextError`] for a stale/ambiguous record, a selection
    /// limit, or a serialization failure.
    pub fn select(
        &mut self,
        expected: &SavedKnowledgeEntry,
        library: &[SavedKnowledgeEntry],
    ) -> Result<bool, SavedContextError> {
        self.validate_current(library)?;
        validate_entry(expected, library)?;
        if self.entries.contains(expected) {
            return Ok(false);
        }
        if self.entries.len() >= MAX_SAVED_CONTEXT_ENTRIES {
            return Err(SavedContextError::TooManyEntries);
        }
        let bytes = self.bytes.saturating_add(serialize(expected)?.len());
        if bytes > MAX_SAVED_CONTEXT_BYTES {
            return Err(SavedContextError::TooLarge);
        }
        self.entries.push(expected.clone());
        self.bytes = bytes;
        Ok(true)
    }

    /// Remove one selected record by its stable saved identity.
    pub fn remove(&mut self, id: Uuid) -> bool {
        let Some(index) = self.entries.iter().position(|entry| entry.id() == id) else {
            return false;
        };
        self.entries.remove(index);
        self.recount();
        true
    }

    /// Clear all selected records. Returns whether the selection changed.
    pub fn clear(&mut self) -> bool {
        let changed = !self.entries.is_empty();
        self.entries.clear();
        self.bytes = 0;
        changed
    }

    /// Discard records whose exact saved versions are no longer current.
    ///
    /// Unrelated settings or view changes can retain unchanged selections.
    /// Explicit reloads should instead call [`Self::clear`], even if the loaded
    /// bytes happen to equal the previous snapshot.
    pub fn retain_current(&mut self, library: &[SavedKnowledgeEntry]) -> bool {
        let before = self.entries.len();
        self.entries
            .retain(|entry| validate_entry(entry, library).is_ok());
        self.recount();
        before != self.entries.len()
    }

    /// Check every selection against the complete current library.
    ///
    /// # Errors
    /// Returns [`SavedContextError::StaleEntry`] when any selected record is
    /// missing, edited or duplicated. Empty selections are valid.
    pub fn validate_current(
        &self,
        library: &[SavedKnowledgeEntry],
    ) -> Result<(), SavedContextError> {
        for entry in &self.entries {
            validate_entry(entry, library)?;
        }
        Ok(())
    }

    /// Compose complete historical records for a later redacted request preview.
    ///
    /// The flat record JSON preserves the saved schema's exact metadata and
    /// content; it is data, never a system instruction or execution grant.
    /// Empty selection returns no records. No truncation or partial result is
    /// permitted here. Callers must still apply current credential redaction,
    /// sensitive-content policy, and model budgets before exposing a preview.
    ///
    /// # Errors
    /// Returns [`SavedContextError`] if the selection is stale or cannot be
    /// serialized completely.
    pub fn compose(
        &self,
        library: &[SavedKnowledgeEntry],
    ) -> Result<Vec<String>, SavedContextError> {
        self.validate_current(library)?;
        self.entries.iter().map(serialize).collect()
    }

    fn recount(&mut self) {
        // These immutable entries were serialized successfully on admission.
        // Keep the limit conservative if serialization ever becomes fallible.
        self.bytes = self
            .entries
            .iter()
            .map(|entry| serialize(entry).map_or(MAX_SAVED_CONTEXT_BYTES, |text| text.len()))
            .fold(0usize, usize::saturating_add);
    }
}

fn serialize(entry: &SavedKnowledgeEntry) -> Result<String, SavedContextError> {
    serde_json::to_string(entry).map_err(|_| SavedContextError::Serialization)
}

fn validate_entry(
    expected: &SavedKnowledgeEntry,
    library: &[SavedKnowledgeEntry],
) -> Result<(), SavedContextError> {
    let mut matches = library.iter().filter(|entry| entry.id() == expected.id());
    if matches.next() != Some(expected) || matches.next().is_some() {
        return Err(SavedContextError::StaleEntry);
    }
    Ok(())
}
