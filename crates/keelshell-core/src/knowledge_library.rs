//! Reviewed edits to saved knowledge without persistence or transport side effects.

use crate::{AppState, Error, SavedKnowledgeEntry, ValidationError};

#[cfg(test)]
mod tests;

impl AppState {
    /// Maximum entries in one explicitly reviewed knowledge deletion.
    pub const MAX_KNOWLEDGE_DELETE_ENTRIES: usize = 32;

    /// Replace one exactly reviewed knowledge entry without changing its order.
    ///
    /// `expected` must equal the complete current entry, including its content.
    /// The replacement must preserve its ID, complete source binding, session,
    /// capture type, occurrence timestamp and outcome. Construct edited content
    /// with [`SavedKnowledgeEntry::with_reviewed_content`] or change only its
    /// local category with [`SavedKnowledgeEntry::with_reviewed_category`].
    /// Both content and category participate in the complete original comparison.
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

    /// Remove an entire reviewed set of saved entries, or change nothing.
    ///
    /// Supply between one and [`Self::MAX_KNOWLEDGE_DELETE_ENTRIES`] distinct
    /// originals. Every field of every original must match its current record;
    /// an ID alone never authorizes deletion. All originals are checked before
    /// constructing the candidate. Remaining entries retain their complete
    /// content, metadata and relative order.
    ///
    /// This edits memory once and preserves the snapshot revision. Persist the
    /// resulting state once through the [`crate::StateStore`] that loaded it.
    /// Deletion does not erase historical copies in configuration backups.
    ///
    /// # Errors
    /// Empty, oversized or duplicate review sets return a validation error.
    /// Missing or changed originals return [`Error::Conflict`]; ambiguous IDs or
    /// an invalid candidate return a validation error. Every failure leaves the
    /// entire state, including its snapshot revision, unchanged.
    pub fn remove_knowledge_entries(
        &mut self,
        expected: &[SavedKnowledgeEntry],
    ) -> Result<(), Error> {
        if expected.is_empty() || expected.len() > Self::MAX_KNOWLEDGE_DELETE_ENTRIES {
            return Err(ValidationError::new(
                "knowledge_entries",
                "review between one and 32 saved entries",
            )
            .into());
        }
        let mut ids = std::collections::HashSet::with_capacity(expected.len());
        for entry in expected {
            if !ids.insert(entry.id()) {
                return Err(ValidationError::new(
                    "knowledge_entries.id",
                    "reviewed entry IDs must be unique",
                )
                .into());
            }
            reviewed_entry_index(&self.knowledge_entries, entry)?;
        }
        let mut candidate = self.clone();
        candidate
            .knowledge_entries
            .retain(|entry| !ids.contains(&entry.id()));
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
