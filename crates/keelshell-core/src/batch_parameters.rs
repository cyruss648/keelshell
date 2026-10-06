//! Transient named literal values for individually reviewed SSH targets.
use std::{collections::BTreeMap, fmt};

use crate::{
    BATCH_TEMPLATE_VARIABLES, BatchTargetContext, MAX_SNIPPET_VALUE_BYTES, MAX_SNIPPET_VARIABLES,
    SnippetTemplate, SnippetTemplateError, compile_snippet_template,
};

/// Checked mapping failures. Diagnostics never contain a supplied value.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BatchParameterError {
    /// The shared literal-template grammar refused the source or value.
    #[error("invalid target parameter: {0}")]
    Template(#[from] SnippetTemplateError),
    /// More than 32 named values were supplied for a target.
    #[error("target parameter count exceeds 32")]
    TooManyValues,
    /// One identifier was supplied twice rather than silently overwritten.
    #[error("duplicate target parameter: {name}")]
    DuplicateValue {
        /// Validated identifier, never the value.
        name: String,
    },
    /// Immutable connection metadata cannot be overridden by an input value.
    #[error("reserved target metadata parameter: {name}")]
    ReservedName {
        /// Validated reserved metadata identifier.
        name: String,
    },
}

/// Validated in-memory values; intentionally has no persistence or serialization API.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct BatchParameterValues(BTreeMap<String, String>);

impl fmt::Debug for BatchParameterValues {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BatchParameterValues")
            .field("names", &self.0.keys().collect::<Vec<_>>())
            .field("value_bytes", &self.byte_len())
            .finish()
    }
}

impl BatchParameterValues {
    /// Validate an exact target mapping before rendering any command.
    ///
    /// Names follow `[A-Za-z_][A-Za-z0-9_]*`, with at most 64 UTF-8 bytes.
    /// Each value allows at most 4,096 UTF-8 bytes, including explicit empty
    /// strings, line feeds and tabs. Other controls, reserved metadata names and
    /// duplicate entries are refused. This performs no I/O and stores nothing.
    pub fn new(values: Vec<(String, String)>) -> Result<Self, BatchParameterError> {
        if values.len() > MAX_SNIPPET_VARIABLES {
            return Err(BatchParameterError::TooManyValues);
        }
        let mut checked = BTreeMap::new();
        for (name, value) in values {
            let mut bytes = name.bytes();
            if name.len() > 64
                || !bytes
                    .next()
                    .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
                || !bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_')
            {
                return Err(SnippetTemplateError::InvalidValueKey.into());
            }
            if BATCH_TEMPLATE_VARIABLES.contains(&name.as_str()) {
                return Err(BatchParameterError::ReservedName { name });
            }
            if checked.contains_key(&name) {
                return Err(BatchParameterError::DuplicateValue { name });
            }
            if value.len() > MAX_SNIPPET_VALUE_BYTES {
                return Err(SnippetTemplateError::ValueTooLong { name }.into());
            }
            if value
                .chars()
                .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
            {
                return Err(SnippetTemplateError::InvalidValue { name }.into());
            }
            checked.insert(name, value);
        }
        Ok(Self(checked))
    }

    /// Aggregate value bytes, for a caller's bounded transient draft budget.
    pub fn byte_len(&self) -> usize {
        self.0.values().map(String::len).sum()
    }

    /// Require exactly the names used across this target's tasks.
    pub fn validate_names(&self, names: &[String]) -> Result<(), BatchParameterError> {
        validate_expected_names(names)?;
        for name in self.0.keys() {
            if !names.contains(name) {
                return Err(SnippetTemplateError::UnexpectedValue { name: name.clone() }.into());
            }
        }
        for name in names {
            if !self.0.contains_key(name) {
                return Err(SnippetTemplateError::MissingValue { name: name.clone() }.into());
            }
        }
        Ok(())
    }

    /// Copy only names required by one task, after validating the whole target
    /// mapping with [`Self::validate_names`]. Missing values still fail closed.
    pub fn for_task(&self, names: &[String]) -> Result<Self, BatchParameterError> {
        validate_expected_names(names)?;
        let mut values = BTreeMap::new();
        for name in names {
            let value = self
                .0
                .get(name)
                .ok_or_else(|| SnippetTemplateError::MissingValue { name: name.clone() })?;
            values.insert(name.clone(), value.clone());
        }
        Ok(Self(values))
    }
}

fn validate_expected_names(names: &[String]) -> Result<(), BatchParameterError> {
    if names.len() > MAX_SNIPPET_VARIABLES {
        return Err(BatchParameterError::TooManyValues);
    }
    let mut seen = std::collections::BTreeSet::new();
    for name in names {
        let mut bytes = name.bytes();
        if name.len() > 64
            || !bytes
                .next()
                .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
            || !bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err(SnippetTemplateError::InvalidValueKey.into());
        }
        if BATCH_TEMPLATE_VARIABLES.contains(&name.as_str()) {
            return Err(BatchParameterError::ReservedName { name: name.clone() });
        }
        if !seen.insert(name) {
            return Err(BatchParameterError::DuplicateValue { name: name.clone() });
        }
    }
    Ok(())
}

/// Locally compiled source supporting metadata and user-named literal markers.
/// Values are never held by the compiled template, evaluated or inherited.
#[derive(Clone, PartialEq, Eq)]
pub struct BatchParameterizedTemplate {
    template: SnippetTemplate,
    parameters: Vec<String>,
}

impl fmt::Debug for BatchParameterizedTemplate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BatchParameterizedTemplate")
            .field("source_bytes", &self.template.source().len())
            .field("parameters", &self.parameters)
            .finish()
    }
}

impl BatchParameterizedTemplate {
    /// Preserve arbitrary existing literal command syntax when there are no
    /// markers. With markers, use exactly the snippet literal-word grammar and
    /// its 32-name / 64 KiB bounds; metadata names retain their previous meaning.
    pub fn compile(source: &str) -> Result<Option<Self>, BatchParameterError> {
        if !source.contains("{{") {
            return Ok(None);
        }
        let template = compile_snippet_template(source)?;
        let parameters = template
            .variables()
            .iter()
            .filter(|name| !BATCH_TEMPLATE_VARIABLES.contains(&name.as_str()))
            .cloned()
            .collect();
        Ok(Some(Self {
            template,
            parameters,
        }))
    }

    /// User parameter names, in first-occurrence order; excludes metadata names.
    pub fn parameters(&self) -> &[String] {
        &self.parameters
    }

    /// Render the complete command as an ephemeral review value. Each named
    /// value occupies one literal word and is POSIX single-quoted by the existing
    /// parser. Shell/program semantics such as `eval` still require human review.
    pub fn render(
        &self,
        context: &BatchTargetContext,
        values: &BatchParameterValues,
    ) -> Result<String, BatchParameterError> {
        values.validate_names(&self.parameters)?;
        let metadata = [
            ("name", &context.name),
            ("host", &context.host),
            ("port", &context.port),
            ("user", &context.user),
            ("endpoint", &context.endpoint),
        ];
        let mut supplied = values.0.clone();
        for (name, value) in metadata {
            if self
                .template
                .variables()
                .iter()
                .any(|variable| variable == name)
            {
                supplied.insert(name.to_owned(), value.clone());
            }
        }
        Ok(self.template.render(&supplied)?)
    }
}
