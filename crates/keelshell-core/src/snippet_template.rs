//! Opt-in, literal POSIX shell parameters. This is not a shell evaluator.

use std::{collections::BTreeMap, ops::Range};

use crate::Snippet;

mod syntax;

/// Maximum template source and rendered command length, in UTF-8 bytes.
pub const MAX_SNIPPET_TEMPLATE_BYTES: usize = 65_536;
/// Maximum number of distinct parameter names in a template.
pub const MAX_SNIPPET_VARIABLES: usize = 32;
/// Maximum length of each supplied literal value, in UTF-8 bytes.
pub const MAX_SNIPPET_VALUE_BYTES: usize = 4_096;
const MAX_NAME_BYTES: usize = 64;

/// Unsupported syntax categories; offsets in errors refer to source UTF-8 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnippetTemplateContext {
    /// A parameter appears inside single or double quotes.
    Quoted,
    /// A parameter appears in a shell comment.
    Comment,
    /// A backslash escapes part of a parameter marker.
    Escaped,
    /// A parameter is concatenated with unsupported word prefixes or suffixes.
    WordFragment,
    /// Shell expansion would require interpretation beyond literal words.
    Expansion,
    /// A here-document or here-string changes how subsequent text is parsed.
    HereDocument,
    /// A compound command or unsupported operator was encountered.
    CompoundSyntax,
    /// A quote, escape, operator or redirection lacks its required continuation.
    IncompleteSyntax,
    /// A terminal control other than a line feed or tab occurs in the source.
    ControlCharacter,
    /// A dynamic file-descriptor operand cannot be treated as a literal path.
    FileDescriptor,
}

/// Checked template failures. Diagnostics never contain a supplied value or source.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SnippetTemplateError {
    /// The source contains only whitespace.
    #[error("template command is empty")]
    EmptyCommand,
    /// The source exceeds 64 KiB.
    #[error("template command exceeds the size limit")]
    CommandTooLong,
    /// A marker name is not `[A-Za-z_][A-Za-z0-9_]*` within 64 bytes.
    #[error("invalid parameter name at byte {offset}")]
    InvalidName {
        /// Byte offset of the invalid name in the original source.
        offset: usize,
    },
    /// The source contains an incomplete or unmatched double-brace marker.
    #[error("malformed parameter marker at byte {offset}")]
    MalformedPlaceholder {
        /// Byte offset of the marker in the original source.
        offset: usize,
    },
    /// The source cannot be handled by the bounded literal-word grammar.
    #[error("unsupported template context at byte {offset}: {context:?}")]
    UnsupportedContext {
        /// UTF-8 byte offset in the original source.
        offset: usize,
        /// Stable category for localized UI explanations.
        context: SnippetTemplateContext,
    },
    /// More than 32 distinct parameter names occur.
    #[error("template has too many distinct parameters")]
    TooManyVariables,
    /// A required key is absent; an explicitly provided empty value is allowed.
    #[error("missing parameter: {name}")]
    MissingValue {
        /// Validated parameter name; never its value.
        name: String,
    },
    /// A supplied valid identifier does not occur in the template.
    #[error("unexpected parameter: {name}")]
    UnexpectedValue {
        /// Validated extra key; never its value.
        name: String,
    },
    /// A supplied key is not a bounded parameter identifier.
    #[error("invalid parameter key")]
    InvalidValueKey,
    /// A supplied value exceeds 4 KiB before quoting.
    #[error("parameter exceeds the value size limit: {name}")]
    ValueTooLong {
        /// Validated parameter name; never its value.
        name: String,
    },
    /// A value contains terminal controls other than line feeds and tabs.
    #[error("parameter contains unsupported controls: {name}")]
    InvalidValue {
        /// Validated parameter name; never its value.
        name: String,
    },
    /// Quoting or repeated substitution would make the result exceed 64 KiB.
    #[error("rendered template exceeds the size limit")]
    RenderedTooLong,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Parameter {
    range: Range<usize>,
    variable: usize,
}

/// An immutable, checked source snapshot. It never stores supplied values.
///
/// Each parameter replaces one unquoted word, or the complete value following
/// `NAME=` or `--long-option=`. Rendered values are POSIX single-quoted literals.
/// Quoting prevents values from adding shell grammar at that interpolation site;
/// it does not neutralize the command's own semantics, such as `eval`, `sh -c`,
/// option processing, or an application interpreting an argument as source code.
/// Callers must still show the complete rendered command for explicit review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnippetTemplate {
    source: String,
    variables: Vec<String>,
    parameters: Vec<Parameter>,
}

impl SnippetTemplate {
    /// The exact original command; no whitespace or quoting is normalized.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Unique parameter names in first-occurrence order, at most 32 names.
    pub fn variables(&self) -> &[String] {
        &self.variables
    }

    /// Render all occurrences from exactly the expected keys, without execution.
    ///
    /// Empty values are accepted only when their key is explicitly present. Each
    /// value is at most 4,096 UTF-8 bytes; line feeds and tabs are preserved while
    /// other controls are refused. Neither this object nor the snippet is mutated.
    /// The returned plaintext command is intended for an ephemeral review draft,
    /// not automatic persistence, history capture or logging of entered secrets.
    ///
    /// # Errors
    /// Rejects malformed, extra or missing keys, invalid/oversized values and any
    /// rendered command beyond 65,536 bytes, including all quote escaping.
    pub fn render(
        &self,
        values: &BTreeMap<String, String>,
    ) -> Result<String, SnippetTemplateError> {
        for name in values.keys() {
            if !valid_name(name) {
                return Err(SnippetTemplateError::InvalidValueKey);
            }
            if !self.variables.contains(name) {
                return Err(SnippetTemplateError::UnexpectedValue { name: name.clone() });
            }
        }
        let values = self
            .variables
            .iter()
            .map(|name| {
                let value = values
                    .get(name)
                    .ok_or_else(|| SnippetTemplateError::MissingValue { name: name.clone() })?;
                if value.len() > MAX_SNIPPET_VALUE_BYTES {
                    return Err(SnippetTemplateError::ValueTooLong { name: name.clone() });
                }
                if value.chars().any(invalid_control) {
                    return Err(SnippetTemplateError::InvalidValue { name: name.clone() });
                }
                let quoted_len =
                    value.len() + 2 + 3 * value.bytes().filter(|&b| b == b'\'').count();
                Ok((value.as_str(), quoted_len))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut size = self.source.len()
            - self
                .parameters
                .iter()
                .map(|parameter| parameter.range.len())
                .sum::<usize>();
        for parameter in &self.parameters {
            size += values[parameter.variable].1;
            // Bound before allocating a rendered string, including repeated values.
            if size > MAX_SNIPPET_TEMPLATE_BYTES {
                return Err(SnippetTemplateError::RenderedTooLong);
            }
        }
        let mut rendered = String::with_capacity(size);
        let mut end = 0;
        for parameter in &self.parameters {
            rendered.push_str(&self.source[end..parameter.range.start]);
            quote_value(values[parameter.variable].0, &mut rendered);
            end = parameter.range.end;
        }
        rendered.push_str(&self.source[end..]);
        Ok(rendered)
    }
}

impl Snippet {
    /// Compile parameters only when this saved snippet explicitly enables them.
    ///
    /// `false` returns `None` without parsing, so old literal `{{name}}` text keeps
    /// its original meaning. This method never stores or supplies parameter values.
    pub fn compile_template(&self) -> Result<Option<SnippetTemplate>, SnippetTemplateError> {
        self.parameterized
            .then(|| compile_snippet_template(&self.command))
            .transpose()
    }
}

/// Compile a bounded literal template without evaluating shell syntax or doing I/O.
///
/// Markers use ASCII identifiers `[A-Za-z_][A-Za-z0-9_]*` of at most 64 bytes.
/// Supports simple command lists, pipelines, literal words and common file
/// redirections; quoted/commented markers, word fragments, shell expansions,
/// here-documents and compound syntax are refused. A source with no markers is
/// valid if its syntax is supported and renders only with an empty value map.
///
/// # Errors
/// Returns a typed source offset for malformed markers or unsupported syntax, or
/// a size/count error. No supplied values are involved in compilation.
///
/// ```
/// use std::collections::BTreeMap;
/// use keelshell_core::compile_snippet_template;
/// let template = compile_snippet_template("printf '%s\\n' {{path}}")?;
/// let values = BTreeMap::from([("path".into(), "a'b".into())]);
/// assert_eq!(template.render(&values)?, "printf '%s\\n' 'a'\\''b'");
/// # Ok::<(), keelshell_core::SnippetTemplateError>(())
/// ```
pub fn compile_snippet_template(command: &str) -> Result<SnippetTemplate, SnippetTemplateError> {
    if command.len() > MAX_SNIPPET_TEMPLATE_BYTES {
        return Err(SnippetTemplateError::CommandTooLong);
    }
    if command.trim().is_empty() {
        return Err(SnippetTemplateError::EmptyCommand);
    }
    if let Some((offset, _)) = command.char_indices().find(|(_, c)| invalid_control(*c)) {
        return Err(unsupported(
            offset,
            SnippetTemplateContext::ControlCharacter,
        ));
    }
    let (variables, parameters) = syntax::compile(command)?;
    Ok(SnippetTemplate {
        source: command.to_owned(),
        variables,
        parameters,
    })
}

fn valid_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    name.len() <= MAX_NAME_BYTES
        && bytes
            .next()
            .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

fn invalid_control(c: char) -> bool {
    c.is_control() && !matches!(c, '\n' | '\t')
}

fn unsupported(offset: usize, context: SnippetTemplateContext) -> SnippetTemplateError {
    SnippetTemplateError::UnsupportedContext { offset, context }
}

fn quote_value(value: &str, output: &mut String) {
    output.push('\'');
    for ch in value.chars() {
        if ch == '\'' {
            output.push_str("'\\''");
        } else {
            output.push(ch);
        }
    }
    output.push('\'');
}
