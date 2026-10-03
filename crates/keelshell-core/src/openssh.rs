//! Safe, bounded import of the portable OpenSSH configuration format.
//!
//! The parser intentionally implements a small, deterministic subset.  It
//! understands exact `Host` aliases, `Host *` defaults, `HostName`, `Port`,
//! `User`, `IdentityFile`, `ProxyJump`, and explicitly supplied `Include`
//! documents. Shell expansion, patterns, conditional blocks and proxy
//! commands are skipped and returned as reviewable warnings because importing
//! those directives without the OpenSSH evaluator could silently connect to a
//! different endpoint.

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

use crate::{AuthMethod, Connection, Error, ValidationError};

/// Maximum bytes consumed across the root document and explicitly supplied includes.
pub const MAX_OPENSSH_CONFIG_BYTES: usize = 1024 * 1024;
/// Maximum bytes accepted for one physical line.
pub const MAX_OPENSSH_LINE_BYTES: usize = 16 * 1024;
/// Maximum number of imported exact aliases.
pub const MAX_OPENSSH_ENTRIES: usize = 1024;
/// Maximum include nesting depth.
pub const MAX_OPENSSH_INCLUDE_DEPTH: usize = 4;
const MAX_TOKENS_PER_LINE: usize = 32;
const MAX_TOKEN_BYTES: usize = 4096;

/// One imported profile and the optional exact alias used as its jump host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenSshEntry {
    /// Exact alias from a `Host` block.  This is also used as the display name.
    pub alias: String,
    /// Validated connection metadata.  It contains no password or vault reference.
    pub connection: Connection,
    /// Exact alias named by `ProxyJump`, if present.
    pub proxy_jump: Option<String>,
    /// One-based line where the alias block began.
    pub source_line: usize,
    /// Literal Include key that supplied this entry, or `None` for the root clipboard text.
    pub source: Option<String>,
}

/// A source item that was ignored during a safe import.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenSshWarning {
    /// One-based source line, or zero for a document-wide warning.
    pub line: usize,
    /// Literal Include key that supplied this warning, or `None` for the root clipboard text.
    pub source: Option<String>,
    /// Lower-case directive name when the warning concerns one directive.
    pub directive: Option<String>,
    /// Stable explanation suitable for a review UI.
    pub reason: &'static str,
}

/// Import outcome plus the source items that need user review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenSshImportReport {
    /// Profiles and duplicate counts produced by the normal library importer.
    pub imported: crate::ImportReport,
    /// Parsed profiles shown to the user before the candidate state is saved.
    pub entries: Vec<OpenSshEntry>,
    /// Unsupported directives and skipped blocks.
    pub warnings: Vec<OpenSshWarning>,
}

/// Result of parsing an OpenSSH-style document before it is merged into a library.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OpenSshConfig {
    /// Profiles in source order.
    pub entries: Vec<OpenSshEntry>,
    /// Include keys resolved through the caller-provided map, in encounter order.
    pub includes: Vec<String>,
    /// Unsupported or potentially semantic-changing items skipped by policy.
    pub warnings: Vec<OpenSshWarning>,
}

/// Parse a self-contained OpenSSH-style configuration.
///
/// `Include` directives are rejected unless the caller supplies their exact
/// contents through [`parse_openssh_config_with_includes`].  The function does
/// not read the filesystem, expand environment variables, or execute a proxy.
pub fn parse_openssh_config(input: &str) -> Result<OpenSshConfig, Error> {
    parse_openssh_config_with_includes(input, &BTreeMap::new())
}

/// Parse an OpenSSH-style configuration with explicitly supplied Include data.
///
/// The map keys are matched literally.  This makes a caller's file-selection
/// policy explicit and prevents a config document from causing arbitrary path
/// traversal or filesystem reads.  Include paths may use `~` and relative
/// components, but shell globs, absolute paths and `..` components are
/// rejected.  Include cycles and duplicate includes are rejected.
pub fn parse_openssh_config_with_includes(
    input: &str,
    includes: &BTreeMap<String, String>,
) -> Result<OpenSshConfig, Error> {
    let mut parser = Parser {
        includes,
        total_bytes: 0,
        used_includes: HashSet::new(),
        include_names: Vec::new(),
        aliases: Vec::new(),
        seen_aliases: HashSet::new(),
        directives: Vec::new(),
        warnings: Vec::new(),
    };
    parser.parse_document(input, 0, None, Scope::Global)?;
    parser.finish()
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Scope {
    Global,
    Alias(String),
    Ignored,
}

#[derive(Debug, Clone)]
struct Directive {
    scope: Option<String>,
    key: String,
    args: Vec<String>,
    line: usize,
    source: Option<String>,
}

struct Parser<'a> {
    includes: &'a BTreeMap<String, String>,
    total_bytes: usize,
    used_includes: HashSet<String>,
    include_names: Vec<String>,
    aliases: Vec<(String, usize, Option<String>)>,
    seen_aliases: HashSet<String>,
    directives: Vec<Directive>,
    warnings: Vec<OpenSshWarning>,
}

impl<'a> Parser<'a> {
    fn parse_document(
        &mut self,
        input: &str,
        depth: usize,
        source_name: Option<&str>,
        initial_scope: Scope,
    ) -> Result<Scope, Error> {
        if depth > MAX_OPENSSH_INCLUDE_DEPTH {
            return Err(config_error(0, "include nesting is too deep"));
        }
        self.total_bytes = self
            .total_bytes
            .checked_add(input.len())
            .ok_or_else(|| config_error(0, "configuration is too large"))?;
        if self.total_bytes > MAX_OPENSSH_CONFIG_BYTES {
            return Err(config_error(0, "configuration exceeds the size limit"));
        }

        let mut scope = initial_scope;
        for (index, raw_line) in input.split('\n').enumerate() {
            let line = index + 1;
            let raw_line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
            if raw_line.len() > MAX_OPENSSH_LINE_BYTES {
                return Err(config_error(line, "line exceeds the size limit"));
            }
            if raw_line.contains('\0') {
                return Err(config_error(line, "NUL bytes are not allowed"));
            }
            let tokens = tokenize(raw_line, line)?;
            if tokens.is_empty() {
                continue;
            }
            let (raw_key, args) = split_directive(tokens, line)?;
            let key = raw_key.to_ascii_lowercase();
            match key.as_str() {
                "host" => {
                    if args.len() != 1 {
                        self.push_warning(
                            line,
                            Some("host"),
                            "Host pattern block was skipped",
                            source_name,
                        );
                        scope = Scope::Ignored;
                        continue;
                    }
                    let alias = args[0].clone();
                    if alias == "*" {
                        scope = Scope::Global;
                        continue;
                    }
                    if alias.contains(['*', '?', '!']) {
                        self.push_warning(
                            line,
                            Some("host"),
                            "wildcard Host block was skipped",
                            source_name,
                        );
                        scope = Scope::Ignored;
                        continue;
                    }
                    validate_alias(&alias, line)?;
                    if !self.seen_aliases.insert(alias.clone()) {
                        return Err(config_error(line, "duplicate Host aliases are ambiguous"));
                    }
                    if self.aliases.len() >= MAX_OPENSSH_ENTRIES {
                        return Err(config_error(line, "too many Host entries"));
                    }
                    self.aliases
                        .push((alias.clone(), line, source_name.map(ToOwned::to_owned)));
                    scope = Scope::Alias(alias);
                }
                "include" => {
                    if !matches!(&scope, Scope::Global) {
                        self.push_warning(
                            line,
                            Some("include"),
                            "Include inside a Host or ignored block was skipped",
                            source_name,
                        );
                        continue;
                    }
                    if args.len() != 1 {
                        self.push_warning(
                            line,
                            Some("include"),
                            "Include directive was skipped because its exact content was not supplied",
                            source_name,
                        );
                        continue;
                    }
                    let include_name = args[0].clone();
                    if validate_include_name(&include_name, line).is_err() {
                        self.push_warning(
                            line,
                            Some("include"),
                            "Include path pattern was skipped",
                            source_name,
                        );
                        continue;
                    }
                    if !self.used_includes.insert(include_name.clone()) {
                        return Err(config_error(line, "duplicate or cyclic Include"));
                    }
                    let Some(included) = self.includes.get(&include_name) else {
                        self.push_warning(
                            line,
                            Some("include"),
                            "Include directive was skipped because its exact content was not supplied",
                            source_name,
                        );
                        continue;
                    };
                    self.include_names.push(include_name.clone());
                    let _ = self.parse_document(
                        included,
                        depth + 1,
                        Some(&include_name),
                        Scope::Global,
                    )?;
                }
                "match" => {
                    self.push_warning(line, Some("match"), "Match block was skipped", source_name);
                    scope = Scope::Ignored;
                    continue;
                }
                "proxycommand"
                | "localcommand"
                | "remotecommand"
                | "hostkeyalias"
                | "canonicalizehostname"
                | "canonicalizemaxdots"
                | "canonicaldomains"
                | "identityagent" => {
                    self.push_warning(
                        line,
                        Some(key.as_str()),
                        "connection-semantic directive was skipped",
                        source_name,
                    );
                    continue;
                }
                "hostname" | "port" | "user" | "identityfile" | "proxyjump" => {
                    if matches!(&scope, Scope::Ignored) {
                        continue;
                    }
                    let scope_name = match &scope {
                        Scope::Global => None,
                        Scope::Alias(alias) => Some(alias.clone()),
                        Scope::Ignored => continue,
                    };
                    self.directives.push(Directive {
                        scope: scope_name,
                        key,
                        args,
                        line,
                        source: source_name.map(ToOwned::to_owned),
                    });
                }
                _ => {
                    self.push_warning(
                        line,
                        Some(key.as_str()),
                        "unsupported directive was skipped",
                        source_name,
                    );
                }
            }
        }
        Ok(scope)
    }

    fn push_warning(
        &mut self,
        line: usize,
        directive: Option<&str>,
        reason: &'static str,
        source_name: Option<&str>,
    ) {
        self.warnings.push(OpenSshWarning {
            line,
            source: source_name.map(ToOwned::to_owned),
            directive: directive.map(ToOwned::to_owned),
            reason,
        });
    }

    fn finish(self) -> Result<OpenSshConfig, Error> {
        if self.aliases.is_empty() {
            return Err(config_error(
                0,
                "configuration contains no exact Host entries",
            ));
        }
        let mut warnings = self.warnings;
        let mut entries = Vec::with_capacity(self.aliases.len());
        let mut warned_duplicates = HashSet::new();
        for (alias, source_line, source) in self.aliases {
            let mut host = alias.clone();
            let mut port = 22_u16;
            let mut username = String::new();
            let mut identity: Option<PathBuf> = None;
            let mut proxy_jump = None;
            let mut seen = HashSet::new();
            let mut seen_in_scope = HashSet::new();

            // OpenSSH evaluates matching blocks in source order and keeps the
            // first value obtained for each option. Specific blocks should be
            // placed before `Host *` defaults when they must override them.
            for directive in self.directives.iter().filter(|directive| {
                directive.scope.is_none() || directive.scope.as_deref() == Some(alias.as_str())
            }) {
                // OpenSSH uses the first obtained value for these options.  We
                // preserve that deterministic rule instead of silently applying
                // a later block's value.
                let scope_key = (directive.key.clone(), directive.scope.clone());
                let duplicate_in_scope = !seen_in_scope.insert(scope_key);
                if !seen.insert(directive.key.as_str()) {
                    let warning_key = (
                        directive.source.clone(),
                        directive.line,
                        directive.key.clone(),
                    );
                    if duplicate_in_scope && warned_duplicates.insert(warning_key) {
                        warnings.push(OpenSshWarning {
                            line: directive.line,
                            source: directive.source.clone(),
                            directive: Some(directive.key.clone()),
                            reason: "duplicate directive was ignored after the first value",
                        });
                    }
                    continue;
                }
                let value = one_arg(directive, &directive.key)?;
                match directive.key.as_str() {
                    "hostname" => {
                        validate_host_value(value, directive.line)?;
                        host = value.to_owned();
                    }
                    "port" => {
                        port = value.parse::<u16>().map_err(|_| {
                            config_error(
                                directive.line,
                                "Port must be a decimal value from 1 to 65535",
                            )
                        })?;
                        if port == 0 {
                            return Err(config_error(directive.line, "Port must be nonzero"));
                        }
                    }
                    "user" => {
                        username = value.to_owned();
                    }
                    "identityfile" => {
                        identity = Some(parse_identity_path(value, directive.line)?);
                    }
                    "proxyjump" => {
                        if value.eq_ignore_ascii_case("none") {
                            proxy_jump = None;
                        } else {
                            if value.contains(',') || value.contains('@') || value.contains(':') {
                                return Err(config_error(
                                    directive.line,
                                    "ProxyJump must name one exact imported Host alias",
                                ));
                            }
                            validate_alias(value, directive.line)?;
                            proxy_jump = Some(value.to_owned());
                        }
                    }
                    // Unrelated options are intentionally ignored after token
                    // validation.  They do not alter the endpoint imported here.
                    _ => {}
                }
            }
            if username.is_empty() {
                return Err(config_error(source_line, "each Host entry needs User"));
            }
            let mut connection = Connection::new(alias.clone(), host, username);
            connection.port = port;
            connection.auth = match identity {
                Some(path) => AuthMethod::PrivateKey { path },
                None => AuthMethod::Agent,
            };
            connection
                .validate()
                .map_err(|error| map_validation(error, source_line))?;
            entries.push(OpenSshEntry {
                alias,
                connection,
                proxy_jump,
                source_line,
                source,
            });
        }
        Ok(OpenSshConfig {
            entries,
            includes: self.include_names,
            warnings,
        })
    }
}

fn one_arg<'a>(directive: &'a Directive, key: &str) -> Result<&'a str, Error> {
    if directive.args.len() != 1 {
        return Err(config_error(
            directive.line,
            match key {
                "hostname" => "HostName must contain one value",
                "port" => "Port must contain one value",
                "user" => "User must contain one value",
                "identityfile" => "IdentityFile must contain one value",
                "proxyjump" => "ProxyJump must contain one value",
                _ => "directive must contain one value",
            },
        ));
    }
    Ok(directive.args[0].as_str())
}

/// Split the directive keyword from its arguments, accepting OpenSSH's
/// optional `=` separator.  The separator may be attached to either side or
/// surrounded by whitespace (`Port=2200`, `Port =2200`, `Port= 2200`, and
/// `Port = 2200`).  Keeping this normalization before directive dispatch
/// prevents a supported option from being silently treated as an unknown key.
fn split_directive(tokens: Vec<String>, line: usize) -> Result<(String, Vec<String>), Error> {
    let mut tokens = tokens.into_iter();
    let first = tokens
        .next()
        .ok_or_else(|| config_error(line, "directive is missing a keyword"))?;
    if let Some((key, value)) = first.split_once('=') {
        if key.is_empty() {
            return Err(config_error(line, "directive keyword is empty"));
        }
        let mut args = Vec::new();
        if !value.is_empty() {
            args.push(value.to_owned());
        }
        args.extend(tokens);
        return Ok((key.to_owned(), args));
    }

    let mut args: Vec<String> = tokens.collect();
    if let Some(second) = args.first_mut() {
        if second == "=" {
            args.remove(0);
        } else if let Some(value) = second.strip_prefix('=') {
            let value = value.to_owned();
            args[0] = value;
        }
    }
    Ok((first, args))
}

fn tokenize(line: &str, line_number: usize) -> Result<Vec<String>, Error> {
    let bytes = line.as_bytes();
    let mut index = 0;
    let mut tokens = Vec::new();
    while index < bytes.len() {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index == bytes.len() || bytes[index] == b'#' {
            break;
        }
        if tokens.len() >= MAX_TOKENS_PER_LINE {
            return Err(config_error(line_number, "too many tokens on one line"));
        }
        let mut value = String::new();
        let mut quote = None;
        while index < bytes.len() {
            let byte = bytes[index];
            if let Some(delimiter) = quote {
                if byte == delimiter {
                    quote = None;
                    index += 1;
                    continue;
                }
            } else if byte.is_ascii_whitespace() {
                break;
            } else if byte == b'\'' || byte == b'"' {
                quote = Some(byte);
                index += 1;
                continue;
            } else if byte == b'#' && value.is_empty() {
                break;
            }
            if byte == b'\\' {
                index += 1;
                if index == bytes.len() {
                    return Err(config_error(line_number, "unterminated escape"));
                }
            }
            let ch = line[index..]
                .chars()
                .next()
                .ok_or_else(|| config_error(line_number, "invalid UTF-8"))?;
            if ch.is_control() {
                return Err(config_error(
                    line_number,
                    "control characters are not allowed",
                ));
            }
            value.push(ch);
            if value.len() > MAX_TOKEN_BYTES {
                return Err(config_error(line_number, "token exceeds the size limit"));
            }
            index += ch.len_utf8();
        }
        if quote.is_some() {
            return Err(config_error(line_number, "unterminated quote"));
        }
        if value.is_empty() {
            // A leading # is a comment; any other empty token is malformed.
            if index < bytes.len() && bytes[index] == b'#' {
                break;
            }
            return Err(config_error(line_number, "empty token"));
        }
        tokens.push(value);
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index < bytes.len() && bytes[index] == b'#' {
            break;
        }
    }
    Ok(tokens)
}

fn validate_alias(alias: &str, line: usize) -> Result<(), Error> {
    if alias.is_empty()
        || alias.len() > 253
        || alias.starts_with('-')
        || alias.starts_with('!')
        || alias
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace() || "*?!/@\\:$%".contains(ch))
    {
        return Err(config_error(line, "Host alias must be one exact safe name"));
    }
    Ok(())
}

fn validate_include_name(name: &str, line: usize) -> Result<(), Error> {
    if name.is_empty()
        || name.len() > 4096
        || name.starts_with('/')
        || name.contains("..")
        || name.contains(['*', '?', '[', ']', '!', '%', '$', '\\'])
        || name.chars().any(char::is_control)
    {
        return Err(config_error(
            line,
            "Include path must be explicit and bounded",
        ));
    }
    Ok(())
}

fn validate_host_value(value: &str, line: usize) -> Result<(), Error> {
    if value.contains(['%', '$']) || value.starts_with('-') {
        return Err(config_error(
            line,
            "HostName expansions and options are not allowed",
        ));
    }
    if value.is_empty() || value.chars().any(char::is_control) {
        return Err(config_error(line, "HostName must be a safe endpoint"));
    }
    Ok(())
}

fn parse_identity_path(value: &str, line: usize) -> Result<PathBuf, Error> {
    if value.eq_ignore_ascii_case("none")
        || value.contains(['%', '$'])
        || value.starts_with('-')
        || value.is_empty()
        || value.len() > 4096
        || value.chars().any(char::is_control)
    {
        return Err(config_error(line, "IdentityFile must be one explicit path"));
    }
    Ok(PathBuf::from(value))
}

fn config_error(line: usize, reason: &'static str) -> Error {
    Error::OpenSshConfig { line, reason }
}

fn map_validation(error: ValidationError, line: usize) -> Error {
    let _ = error;
    config_error(line, "imported connection metadata is invalid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_exact_hosts_with_first_value_defaults_and_identity() {
        let input = r#"
Host app
    HostName 10.0.0.8
    User deploy
    Port 2208
    IdentityFile ~/.ssh/id_ed25519

Host *
    User fallback
    Port 22
"#;
        let config = match parse_openssh_config(input) {
            Ok(config) => config,
            Err(error) => panic!("config should parse: {error}"),
        };
        assert!(config.warnings.is_empty());
        let entry = &config.entries[0];
        assert_eq!(entry.alias, "app");
        assert_eq!(entry.connection.host, "10.0.0.8");
        assert_eq!(entry.connection.username, "deploy");
        assert_eq!(entry.connection.port, 2208);
        assert_eq!(
            entry.connection.auth,
            AuthMethod::PrivateKey {
                path: PathBuf::from("~/.ssh/id_ed25519")
            }
        );
    }

    #[test]
    fn imports_jump_aliases_only_after_all_exact_hosts_are_indexed() {
        let input = "Host target\n HostName target.internal\n User deploy\n ProxyJump bastion\nHost bastion\n HostName bastion.internal\n User jump\n";
        let mut state = crate::AppState::default();
        let report = match state.import_openssh_config_report(input) {
            Ok(report) => report,
            Err(error) => panic!("import should resolve exact jump alias: {error}"),
        };
        assert_eq!(report.imported.added, 2);
        let Some(target) = state
            .connections
            .iter()
            .find(|connection| connection.name == "target")
        else {
            panic!("target profile");
        };
        let Some(bastion) = state
            .connections
            .iter()
            .find(|connection| connection.name == "bastion")
        else {
            panic!("bastion profile");
        };
        assert_eq!(target.jump_host, Some(bastion.id));
    }

    #[test]
    fn skips_patterns_and_reports_dangerous_semantics_without_execution() {
        let input = "Host *\n User default\nHost web-*\n HostName example.internal\n User deploy\nProxyCommand sh -c 'echo unsafe'\nHost exact\n HostName exact.internal\n User deploy\n";
        let config = match parse_openssh_config(input) {
            Ok(config) => config,
            Err(error) => panic!("safe exact entry should remain: {error}"),
        };
        assert_eq!(config.entries.len(), 1);
        assert_eq!(config.entries[0].alias, "exact");
        assert!(
            config
                .warnings
                .iter()
                .any(|warning| warning.reason.contains("wildcard"))
        );
        assert!(
            config
                .warnings
                .iter()
                .any(|warning| warning.reason.contains("connection-semantic"))
        );
    }

    #[test]
    fn include_requires_explicit_selected_content_and_preserves_warning() {
        let input = "Include conf.d/team\nHost app\n HostName app.internal\n User deploy\n";
        let missing = match parse_openssh_config(input) {
            Ok(config) => config,
            Err(error) => panic!("missing include is reviewable: {error}"),
        };
        assert!(missing.entries.iter().any(|entry| entry.alias == "app"));
        assert!(
            missing
                .warnings
                .iter()
                .any(|warning| warning.reason.contains("Include"))
        );

        let mut includes = BTreeMap::new();
        includes.insert(
            "conf.d/team".to_owned(),
            "Host team\n HostName team.internal\n User ops\n".to_owned(),
        );
        let selected = match parse_openssh_config_with_includes(input, &includes) {
            Ok(config) => config,
            Err(error) => panic!("selected include should parse: {error}"),
        };
        assert_eq!(selected.entries.len(), 2);
        assert_eq!(selected.includes, vec!["conf.d/team"]);
    }

    #[test]
    fn include_inside_an_exact_host_is_skipped_without_guessing_scope() {
        let mut includes = BTreeMap::new();
        includes.insert("conf.d/team".to_owned(), "User included\n".to_owned());
        let input = "Host app\n Include conf.d/team\n User outer\n HostName app.internal\n\nHost other\n User other-root\n HostName other.internal\n";
        let parsed = match parse_openssh_config_with_includes(input, &includes) {
            Ok(parsed) => parsed,
            Err(error) => panic!("scoped include should parse: {error}"),
        };
        assert_eq!(parsed.entries[0].connection.username, "outer");
        assert_eq!(parsed.entries[1].connection.username, "other-root");
        assert!(parsed.warnings.iter().any(|warning| {
            warning.reason == "Include inside a Host or ignored block was skipped"
        }));
    }

    #[test]
    fn unsupported_and_duplicate_directives_are_review_warnings() {
        let input = "Host app\n HostName app.internal\n User deploy\n AddressFamily inet6\n IdentityFile ~/.ssh/one\n IdentityFile ~/.ssh/two\n StrictHostKeyChecking yes\n";
        let parsed = match parse_openssh_config(input) {
            Ok(parsed) => parsed,
            Err(error) => panic!("unsupported directives should remain reviewable: {error}"),
        };
        assert!(parsed.warnings.iter().any(|warning| {
            warning.directive.as_deref() == Some("addressfamily")
                && warning.reason == "unsupported directive was skipped"
        }));
        assert!(parsed.warnings.iter().any(|warning| {
            warning.directive.as_deref() == Some("identityfile")
                && warning.reason == "duplicate directive was ignored after the first value"
        }));
        assert!(
            parsed
                .warnings
                .iter()
                .any(|warning| { warning.directive.as_deref() == Some("stricthostkeychecking") })
        );
    }

    #[test]
    fn accepts_open_ssh_equals_separators() {
        let input = "Host=app\n HostName = app.internal\n User=deploy\n Port =2208\n IdentityFile= ~/.ssh/id_ed25519\n";
        let parsed = match parse_openssh_config(input) {
            Ok(parsed) => parsed,
            Err(error) => panic!("equals-separated directives should parse: {error}"),
        };
        let entry = &parsed.entries[0];
        assert_eq!(entry.alias, "app");
        assert_eq!(entry.connection.host, "app.internal");
        assert_eq!(entry.connection.username, "deploy");
        assert_eq!(entry.connection.port, 2208);
        assert_eq!(
            entry.connection.auth,
            AuthMethod::PrivateKey {
                path: PathBuf::from("~/.ssh/id_ed25519")
            }
        );
    }

    #[test]
    fn warns_when_global_directives_are_duplicated() {
        let input = "Host *\n IdentityFile ~/.ssh/one\n IdentityFile ~/.ssh/two\nHost app\n HostName app.internal\n User deploy\n";
        let parsed = match parse_openssh_config(input) {
            Ok(parsed) => parsed,
            Err(error) => panic!("duplicate global directives should remain reviewable: {error}"),
        };
        assert!(parsed.warnings.iter().any(|warning| {
            warning.directive.as_deref() == Some("identityfile")
                && warning.reason == "duplicate directive was ignored after the first value"
        }));
    }

    #[test]
    fn include_inside_ignored_host_pattern_is_not_imported() {
        let mut includes = BTreeMap::new();
        includes.insert(
            "conf.d/team".to_owned(),
            "Host included\n User inc\n HostName inc.internal\n".to_owned(),
        );
        let parsed = match parse_openssh_config_with_includes(
            "Host *.skip\n Include conf.d/team\nHost app\n User deploy\n HostName app.internal\n",
            &includes,
        ) {
            Ok(parsed) => parsed,
            Err(error) => panic!("ignored include should remain reviewable: {error}"),
        };
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].alias, "app");
        assert!(parsed.warnings.iter().any(|warning| {
            warning.reason == "Include inside a Host or ignored block was skipped"
        }));
    }

    #[test]
    fn rejects_shell_expansions_and_unresolved_jump_aliases() {
        let expansion = "Host app\n HostName %h.internal\n User deploy\n";
        assert!(matches!(
            parse_openssh_config(expansion),
            Err(Error::OpenSshConfig { .. })
        ));
        let unresolved = "Host app\n HostName app.internal\n User deploy\n ProxyJump missing\n";
        let mut state = crate::AppState::default();
        assert!(matches!(
            state.import_openssh_config_report(unresolved),
            Err(Error::OpenSshConfig {
                reason: "ProxyJump references an unknown Host alias",
                ..
            })
        ));
    }
}
