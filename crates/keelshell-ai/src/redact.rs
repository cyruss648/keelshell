use zeroize::Zeroizing;

const MASK: &str = "[REDACTED]";

/// Counts of redaction events, not a claim to have found every secret.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RedactionReport {
    /// Number of PEM private-key blocks, including unterminated blocks.
    pub private_key_blocks: usize,
    /// Number of whole lines removed for credential markers or token prefixes.
    pub credential_lines: usize,
    /// Number of exact occurrences of explicitly supplied secrets replaced.
    pub explicit_matches: usize,
    /// Number of sanitized UTF-8 bytes omitted due to the context budget.
    pub truncated_bytes: usize,
}

impl RedactionReport {
    /// Number of redaction events, excluding truncation.
    pub fn total_redactions(&self) -> usize {
        self.private_key_blocks + self.credential_lines + self.explicit_matches
    }

    pub(crate) fn merge(&mut self, other: Self) {
        self.private_key_blocks += other.private_key_blocks;
        self.credential_lines += other.credential_lines;
        self.explicit_matches += other.explicit_matches;
        self.truncated_bytes += other.truncated_bytes;
    }
}

/// Conservative redaction of common credential lines and explicit secret values.
///
/// A matching credential line is removed in full, so even quoted passwords with
/// spaces are covered. Encoded, obfuscated and unknown secret formats may remain.
/// This type borrows the caller's secret list and never reads the environment.
pub struct Redactor<'a> {
    secrets: Vec<&'a str>,
}

impl<'a> Redactor<'a> {
    /// Build a redactor. Empty secrets are ignored; longer values match first.
    pub fn new(secrets: &[&'a str]) -> Self {
        let mut secrets: Vec<_> = secrets.iter().copied().filter(|s| !s.is_empty()).collect();
        secrets.sort_unstable_by_key(|s| std::cmp::Reverse(s.len()));
        secrets.dedup();
        Self { secrets }
    }

    /// Redact selected text and return event counts. Input is never logged.
    pub fn redact(&self, input: &str) -> (String, RedactionReport) {
        let mut report = RedactionReport::default();
        let text = Zeroizing::new(redact_private_keys(input, &mut report));
        let mut output = String::with_capacity(text.len());
        for line in text.split_inclusive('\n') {
            if credential_line(line) {
                report.credential_lines += 1;
                output.push_str(MASK);
                if line.ends_with('\n') {
                    output.push('\n');
                }
            } else {
                output.push_str(line);
            }
        }
        // Explicit values run last so a secret equal to a marker such as
        // "password" cannot hide that marker before its value is removed.
        for secret in &self.secrets {
            let occurrences = output.matches(secret).count();
            if occurrences != 0 {
                report.explicit_matches += occurrences;
                let previous = Zeroizing::new(output);
                output = previous.replace(secret, MASK);
            }
        }
        (output, report)
    }
}

fn redact_private_keys(input: &str, report: &mut RedactionReport) -> String {
    let mut output = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find("-----BEGIN ") {
        output.push_str(&rest[..start]);
        let block = &rest[start..];
        let after_begin = &block["-----BEGIN ".len()..];
        let label_end = after_begin.find("-----").unwrap_or(after_begin.len());
        let label = &after_begin[..label_end];
        if !label.ends_with("PRIVATE KEY") || label.contains(['\r', '\n']) {
            output.push_str("-----BEGIN ");
            rest = &block["-----BEGIN ".len()..];
            continue;
        }
        report.private_key_blocks += 1;
        output.push_str(MASK);
        // An incomplete key block must remove the remainder, rather than reveal
        // key material merely because its footer was outside the selection.
        let footer = format!("-----END {label}-----");
        if let Some(end) = block.find(footer.as_str()) {
            rest = &block[end + footer.len()..];
            continue;
        }
        rest = "";
        break;
    }
    output.push_str(rest);
    output
}

fn credential_line(line: &str) -> bool {
    let lower = Zeroizing::new(line.to_ascii_lowercase());
    if lower.contains("bearer ") || lower.contains("bearer\t") {
        return true;
    }
    if [
        "sk-",
        "ghp_",
        "github_pat_",
        "xoxb-",
        "xoxp-",
        "AKIA",
        "ASIA",
    ]
    .iter()
    .any(|prefix| {
        line.match_indices(prefix).any(|(start, _)| {
            line[start + prefix.len()..]
                .bytes()
                .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'-')
                .count()
                >= 8
        })
    }) {
        return true;
    }
    [
        "password",
        "passwd",
        "pwd",
        "token",
        "api_key",
        "api-key",
        "apikey",
        "secret",
        "access_key",
        "authorization",
    ]
    .iter()
    .any(|marker| {
        lower.match_indices(marker).any(|(offset, _)| {
            let tail = &lower[offset + marker.len()..];
            let command_flag = lower[..offset].ends_with("--") && tail.starts_with([' ', '\t']);
            command_flag
                || tail
                    .trim_start_matches([' ', '\t', '\'', '"'])
                    .starts_with(['=', ':'])
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_remove_entire_lines_with_quoted_spaces() {
        let (output, report) = Redactor::new(&[]).redact(
            "keep this\nexport DB_PASSWORD = 'two words'\n{\"api_key\":\"abc\",\"other\":42}\nAuthorization: Bearer opaque\n",
        );
        assert_eq!(output, "keep this\n[REDACTED]\n[REDACTED]\n[REDACTED]\n");
        assert_eq!(report.credential_lines, 3);
    }

    #[test]
    fn private_key_blocks_and_unterminated_keys_are_removed() {
        for key in [
            "-----BEGIN OPENSSH PRIVATE KEY-----\nencoded-private-key\n-----END OPENSSH PRIVATE KEY-----",
            "-----BEGIN RSA PRIVATE KEY-----\nencoded-private-key",
        ] {
            let (output, report) = Redactor::new(&[]).redact(key);
            assert_eq!(output, MASK);
            assert_eq!(report.private_key_blocks, 1);
        }
    }

    #[test]
    fn explicit_unicode_values_are_removed_longest_first() {
        let (output, report) =
            Redactor::new(&["密钥", "密钥-abcdef", "", "密钥"]).redact("密钥-abcdef twice 密钥");
        assert_eq!(output, "[REDACTED] twice [REDACTED]");
        assert_eq!(report.explicit_matches, 2);
    }

    #[test]
    fn recognizable_token_prefixes_are_removed() {
        let (output, report) =
            Redactor::new(&[]).redact("received ghp_abcdefghijklmnop\nnormal output");
        assert_eq!(output, "[REDACTED]\nnormal output");
        assert_eq!(report.credential_lines, 1);
    }

    #[test]
    fn ordinary_diagnostics_remain_readable() {
        let input =
            "permission denied (publickey)\nHTTP 503 from upstream\npassword authentication failed";
        assert_eq!(Redactor::new(&[]).redact(input).0, input);
    }

    #[test]
    fn explicit_values_cannot_hide_credential_markers() {
        let input = "password=hidden\n-----BEGIN PRIVATE KEY-----\nkey-material";
        let (output, _) = Redactor::new(&["password", "-----BEGIN PRIVATE KEY-----"]).redact(input);
        assert_eq!(output, "[REDACTED]\n[REDACTED]");
    }

    #[test]
    fn unrelated_pem_footer_does_not_end_private_key_redaction() {
        let input = "-----BEGIN RSA PRIVATE KEY-----\nfirst\n-----END PUBLIC KEY-----\nremaining-private-material";
        assert_eq!(Redactor::new(&[]).redact(input).0, MASK);
    }

    #[test]
    fn credential_command_flags_are_redacted() {
        assert_eq!(
            Redactor::new(&[]).redact("client --password 'two words'").0,
            MASK
        );
    }
}
