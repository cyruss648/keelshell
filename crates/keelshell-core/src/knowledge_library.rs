//! Reviewed edits to saved knowledge without persistence or transport side effects.

use crate::{AppState, Error, SavedKnowledgeEntry, ValidationError};

impl AppState {
    /// Replace one exactly reviewed knowledge entry without changing its order.
    ///
    /// `expected` must equal the complete current entry, including its content.
    /// The replacement must preserve its ID, complete source binding, session,
    /// capture type, occurrence timestamp and outcome. Construct edited content
    /// with [`SavedKnowledgeEntry::with_reviewed_content`].
    ///
    /// This only edits memory and preserves the snapshot revision. Persist the
    /// candidate through the same [`crate::StateStore`] that loaded the snapshot;
    /// its revision gate independently rejects a stale writer.
    ///
    /// # Errors
    /// Returns [`Error::Conflict`] if the reviewed entry is missing or changed.
    /// Duplicate IDs or altered immutable metadata return a validation error.
    /// Invalid candidate state is also rejected. Any failure leaves the entire
    /// state, including its snapshot revision, unchanged.
    pub fn replace_knowledge_entry(
        &mut self,
        expected: &SavedKnowledgeEntry,
        replacement: SavedKnowledgeEntry,
    ) -> Result<(), Error> {
        let index = reviewed_entry_index(&self.knowledge_entries, expected)?;
        if replacement.id() != expected.id()
            || replacement.entry().source() != expected.entry().source()
            || replacement.session_id() != expected.session_id()
            || replacement.capture_type() != expected.capture_type()
            || replacement.entry().occurred_at() != expected.entry().occurred_at()
            || replacement.entry().outcome() != expected.entry().outcome()
        {
            return Err(ValidationError::new(
                "knowledge_entries",
                "replacement must preserve saved entry metadata",
            )
            .into());
        }
        let mut candidate = self.clone();
        candidate.knowledge_entries[index] = replacement;
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    /// Remove one exactly reviewed knowledge entry, preserving all other order.
    ///
    /// `expected` must equal the complete current entry. A missing entry is
    /// rejected, including a repeated removal. This only edits memory and
    /// preserves the snapshot revision; persist it through the same
    /// [`crate::StateStore`] that loaded the snapshot to retain its stale-writer
    /// protection.
    ///
    /// # Errors
    /// Returns [`Error::Conflict`] if the reviewed entry is missing or changed.
    /// Duplicate IDs return a validation error; invalid remaining state is also
    /// rejected. Any failure leaves the entire state and snapshot revision
    /// unchanged.
    pub fn remove_knowledge_entry(&mut self, expected: &SavedKnowledgeEntry) -> Result<(), Error> {
        let index = reviewed_entry_index(&self.knowledge_entries, expected)?;
        let mut candidate = self.clone();
        candidate.knowledge_entries.remove(index);
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }
}

fn reviewed_entry_index(
    entries: &[SavedKnowledgeEntry],
    expected: &SavedKnowledgeEntry,
) -> Result<usize, Error> {
    let mut matching = entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.id() == expected.id());
    let (index, current) = matching.next().ok_or(Error::Conflict)?;
    if matching.next().is_some() {
        return Err(
            ValidationError::new("knowledge_entries.id", "saved entry IDs must be unique").into(),
        );
    }
    if current != expected {
        return Err(Error::Conflict);
    }
    Ok(index)
}
