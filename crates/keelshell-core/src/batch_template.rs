//! Bounded per-target command rendering for reviewed SSH batches.
//!
//! The template is rendered locally from immutable connection metadata. It never
//! evaluates shell syntax or performs I/O; callers must still display the
//! complete per-target commands and obtain an explicit execution confirmation.

use std::collections::BTreeMap;

use thiserror::Error;

use crate::{SnippetTemplate, SnippetTemplateError, compile_snippet_template};

/// Supported metadata markers in a batch command template.
///
/// Values are sourced from the selected target's saved route or one-time session
/// label. No credentials, environment variables, or remote state are exposed.
pub const BATCH_TEMPLATE_VARIABLES: [&str; 5] = ["endpoint", "host", "name", "port", "user"];

/// A bounded metadata snapshot used for one target's command rendering.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BatchTargetContext {
    /// Display name of the selected target.
    pub name: String,
    /// Hostname or address, when known.
    pub host: String,
    /// SSH port rendered as decimal text, when known.
    pub port: String,
    /// Remote account name, when known.
    pub user: String,
    /// Display endpoint, including the user and port when known.
    pub endpoint: String,
}

/// A validated command source containing only the supported metadata markers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchCommandTemplate {
    source: String,
    template: SnippetTemplate,
}

/// Failure while compiling or rendering a per-target batch template.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BatchTemplateError {
    /// The source is not a supported literal command template.
    #[error("invalid batch command template: {0}")]
    Invalid(#[from] SnippetTemplateError),
    /// A marker is outside the intentionally small metadata allow-list.
    #[error("unsupported batch template variable: {name}")]
    UnsupportedVariable {
        /// Variable name from the source; never a value or credential.
        name: String,
    },
}

impl BatchCommandTemplate {
    /// Compile a source that contains one or more `{{name}}` markers.
    ///
    /// Sources without markers return `Ok(None)` so existing batch commands keep
    /// their previous shell syntax acceptance. Marker syntax is delegated to the
    /// same bounded literal parser used by parameterized snippets.
    pub fn compile(source: &str) -> Result<Option<Self>, BatchTemplateError> {
        if !source.contains("{{") {
            return Ok(None);
        }
        let template = compile_snippet_template(source)?;
        for name in template.variables() {
            if !BATCH_TEMPLATE_VARIABLES.contains(&name.as_str()) {
                return Err(BatchTemplateError::UnsupportedVariable { name: name.clone() });
            }
        }
        Ok(Some(Self {
            source: source.to_owned(),
            template,
        }))
    }

    /// Return the exact source submitted for review.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Return marker names in first-occurrence order.
    pub fn variables(&self) -> &[String] {
        self.template.variables()
    }

    /// Render one target's command with shell-safe literal values.
    ///
    /// Rendering is deterministic and local. The returned text is an ephemeral
    /// review value and is not persisted by this type.
    pub fn render(&self, context: &BatchTargetContext) -> Result<String, BatchTemplateError> {
        let values = BTreeMap::from([
            ("endpoint".to_owned(), context.endpoint.clone()),
            ("host".to_owned(), context.host.clone()),
            ("name".to_owned(), context.name.clone()),
            ("port".to_owned(), context.port.clone()),
            ("user".to_owned(), context.user.clone()),
        ]);
        let values = values
            .into_iter()
            .filter(|(name, _)| self.template.variables().contains(name))
            .collect();
        Ok(self.template.render(&values)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> BatchTargetContext {
        BatchTargetContext {
            name: "生产 west".into(),
            host: "node.example".into(),
            port: "2201".into(),
            user: "deploy".into(),
            endpoint: "deploy@node.example:2201".into(),
        }
    }

    #[test]
    fn no_marker_preserves_existing_commands() {
        assert!(matches!(
            BatchCommandTemplate::compile("printf '%s\\n' ok"),
            Ok(None)
        ));
    }

    #[test]
    fn renders_each_supported_target_value_as_a_literal() {
        let template = match BatchCommandTemplate::compile(
            "printf '%s\\n' {{name}} {{host}} {{port}} {{user}} {{endpoint}}",
        ) {
            Ok(Some(template)) => template,
            other => panic!("expected marker template, got {other:?}"),
        };
        let rendered = match template.render(&context()) {
            Ok(rendered) => rendered,
            Err(error) => panic!("expected render to succeed, got {error}"),
        };
        assert_eq!(
            rendered,
            "printf '%s\\n' '生产 west' 'node.example' '2201' 'deploy' 'deploy@node.example:2201'"
        );
    }

    #[test]
    fn rejects_unknown_markers_before_any_rendering() {
        let error = match BatchCommandTemplate::compile("echo {{secret}}") {
            Err(error) => error,
            Ok(value) => panic!("expected unknown marker error, got {value:?}"),
        };
        assert_eq!(
            error,
            BatchTemplateError::UnsupportedVariable {
                name: "secret".into()
            }
        );
    }

    #[test]
    fn rejects_unsupported_marker_context() {
        assert!(BatchCommandTemplate::compile("echo \"{{host}}\"").is_err());
    }
}
