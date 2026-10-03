use super::*;

fn plan(marked: &str) -> CompletionPlan {
    let caret = marked
        .find('│')
        .unwrap_or_else(|| panic!("missing test caret"));
    let input = marked.replacen('│', "", 1);
    match analyze_completion(&input, caret) {
        Ok(CompletionAnalysis::Ready(plan)) => plan,
        other => panic!("expected literal completion: {other:?}"),
    }
}

fn unsupported(marked: &str) -> CompletionUnsupported {
    let caret = marked
        .find('│')
        .unwrap_or_else(|| panic!("missing test caret"));
    let input = marked.replacen('│', "", 1);
    match analyze_completion(&input, caret) {
        Ok(CompletionAnalysis::Unsupported(reason)) => reason,
        other => panic!("expected unsupported completion: {other:?}"),
    }
}

fn candidate(name: &str, path: &str, is_directory: bool) -> LiteralCandidate {
    LiteralCandidate {
        name: name.into(),
        path: path.into(),
        is_directory,
    }
}

fn bound(marked: &str, base: Option<&str>, resolved: &str) -> CompletionPlan {
    let mut plan = plan(marked);
    let CompletionQuery::Paths { directory, .. } =
        plan.query(base).unwrap_or_else(|e| panic!("query: {e}"))
    else {
        panic!("expected path query");
    };
    plan.bind_resolved_directory(&directory, resolved)
        .unwrap_or_else(|e| panic!("binding: {e}"))
}

fn apply(plan: &CompletionPlan, candidate: &LiteralCandidate) -> (String, CompletionEdit) {
    let edit = plan
        .edit(&plan.source, plan.caret, candidate)
        .unwrap_or_else(|e| panic!("edit: {e}"));
    let mut text = plan.source.clone();
    text.replace_range(edit.range.clone(), &edit.replacement);
    assert!(text.is_char_boundary(edit.caret_byte));
    assert_eq!(&text[..edit.range.start], &plan.source[..edit.range.start]);
    assert_eq!(
        &text[edit.range.start + edit.replacement.len()..],
        &plan.source[edit.range.end..]
    );
    (text, edit)
}

#[test]
fn commands_after_pipelines_lists_and_literal_assignments_are_detected()
-> Result<(), CompletionError> {
    for input in [
        "gr│",
        "echo x | gr│",
        "true&&gr│",
        "false||gr│",
        "echo x;gr│",
        "echo x\ngr│",
        "A='two words' B=x gr│",
        ">out A=x gr│",
    ] {
        assert_eq!(
            plan(input).query(None)?,
            CompletionQuery::Commands {
                prefix: "gr".into()
            },
            "{input}"
        );
    }
    Ok(())
}

#[test]
fn all_argument_positions_and_file_redirections_query_paths() -> Result<(), CompletionError> {
    for input in [
        "cp from /et│",
        "echo first second /et│",
        "cat</et│",
        "echo x 2>/et│",
        "echo x>>/et│",
        "exec 3<>/et│",
        "echo x >|/et│",
        "</et│ cat",
    ] {
        assert_eq!(
            plan(input).query(None)?,
            CompletionQuery::Paths {
                directory: "/".into(),
                prefix: "et".into(),
                directories_only: false
            },
            "{input}"
        );
    }
    Ok(())
}

#[test]
fn descriptor_duplication_is_never_a_path_query() {
    for input in ["cat 2>&│", "cat 2>&1│", "cat <&-│", "cat 2│>out"] {
        assert_eq!(
            unsupported(input),
            CompletionUnsupported::FileDescriptor,
            "{input}"
        );
    }
}

#[test]
fn relative_paths_require_an_explicit_base_and_do_not_collapse_dot_components()
-> Result<(), CompletionError> {
    let mut p = plan("cat ../link/../fi│");
    assert_eq!(p.query(None), Err(CompletionError::MissingBaseDirectory));
    assert_eq!(
        p.query(Some("/srv/work"))?,
        CompletionQuery::Paths {
            directory: "/srv/work/../link/../".into(),
            prefix: "fi".into(),
            directories_only: false
        }
    );
    assert_eq!(
        p.query(Some("C:\\local")),
        Err(CompletionError::InvalidDirectory)
    );
    Ok(())
}

#[test]
fn bindings_reject_unrelated_queries_and_require_canonical_remote_parents()
-> Result<(), CompletionError> {
    let mut p = plan("cat ./fi│");
    p.query(Some("/srv"))?;
    assert_eq!(
        p.bind_resolved_directory("/other/./", "/resolved"),
        Err(CompletionError::DirectoryMismatch)
    );
    for bad in ["relative", "/a/../b", "/a/./b", "//srv", "/srv/", "/a\nb"] {
        assert_eq!(
            p.bind_resolved_directory("/srv/./", bad),
            Err(CompletionError::InvalidDirectory)
        );
    }
    let bound = p.bind_resolved_directory("/srv/./", "/resolved")?;
    assert!(
        bound
            .edit(
                &bound.source,
                bound.caret,
                &candidate("file", "/resolved/file", false)
            )
            .is_ok()
    );
    assert_eq!(
        p.edit(
            &p.source,
            p.caret,
            &candidate("file", "/resolved/file", false)
        ),
        Err(CompletionError::DirectoryMismatch)
    );
    Ok(())
}

#[test]
fn a_new_query_revokes_the_old_resolved_directory_binding() -> Result<(), CompletionError> {
    let mut p = bound("cat fi│", Some("/a"), "/a");
    p.query(Some("/b"))?;
    assert_eq!(
        p.bind_resolved_directory("/a", "/a"),
        Err(CompletionError::DirectoryMismatch)
    );
    assert_eq!(
        p.edit(&p.source, p.caret, &candidate("file", "/a/file", false)),
        Err(CompletionError::DirectoryMismatch)
    );
    Ok(())
}

#[test]
fn mixed_quotes_and_unicode_decode_to_one_literal_component() -> Result<(), CompletionError> {
    let mut p = plan("cat /tmp/中\"文 空\"'间'\\ 名│ tail");
    assert_eq!(
        p.query(None)?,
        CompletionQuery::Paths {
            directory: "/tmp/".into(),
            prefix: "中文 空间 名".into(),
            directories_only: false
        }
    );
    let p = p.bind_resolved_directory("/tmp/", "/tmp")?;
    let (text, _) = apply(
        &p,
        &candidate("中文 空间 名😀", "/tmp/中文 空间 名😀", false),
    );
    assert_eq!(text, "cat '/tmp/中文 空间 名😀' tail");
    Ok(())
}

#[test]
fn unicode_whitespace_is_part_of_a_word_not_a_shell_separator() -> Result<(), CompletionError> {
    assert_eq!(
        plan("cat one\u{a0}two│").query(Some("/tmp"))?,
        CompletionQuery::Paths {
            directory: "/tmp".into(),
            prefix: "one\u{a0}two".into(),
            directories_only: false
        }
    );
    Ok(())
}

#[test]
fn double_quotes_preserve_backslashes_before_ordinary_characters() -> Result<(), CompletionError> {
    assert_eq!(
        plan("cat \"/tmp/a\\q│\"").query(None)?,
        CompletionQuery::Paths {
            directory: "/tmp/".into(),
            prefix: "a\\q".into(),
            directories_only: false
        }
    );
    assert_eq!(
        plan("cat \"/tmp/a\\$│\"").query(None)?,
        CompletionQuery::Paths {
            directory: "/tmp/".into(),
            prefix: "a$".into(),
            directories_only: false
        }
    );
    Ok(())
}

#[test]
fn escaped_newlines_join_words_without_modifying_other_lines() -> Result<(), CompletionError> {
    let mut p = plan("echo first\ncat /tm\\\np/fi│ second\necho last");
    assert_eq!(
        p.query(None)?,
        CompletionQuery::Paths {
            directory: "/tmp/".into(),
            prefix: "fi".into(),
            directories_only: false
        }
    );
    let p = p.bind_resolved_directory("/tmp/", "/tmp")?;
    assert_eq!(
        apply(&p, &candidate("file", "/tmp/file", false)).0,
        "echo first\ncat '/tmp/file' second\necho last"
    );
    Ok(())
}

#[test]
fn completion_in_the_middle_preserves_component_suffix_and_following_path() {
    let p = bound("cat /et│c/ssh/config tail", None, "/");
    let (text, edit) = apply(&p, &candidate("etc", "/etc", true));
    assert_eq!(text, "cat '/etc/ssh/config' tail");
    assert_eq!(&text[..edit.caret_byte], "cat '/etc");
    assert_eq!(
        p.edit(&p.source, p.caret, &candidate("eternal", "/eternal", true)),
        Err(CompletionError::InvalidCandidate)
    );
    assert_eq!(
        p.edit(&p.source, p.caret, &candidate("etc", "/etc", false)),
        Err(CompletionError::InvalidCandidate)
    );
}

#[test]
fn middle_command_completion_matches_both_sides_without_duplicating_suffix() {
    let p = plan("true && gr│ep pattern");
    assert_eq!(
        apply(&p, &candidate("grep", "/usr/bin/grep", false)).0,
        "true && 'grep' pattern"
    );
    assert_eq!(
        p.edit(
            &p.source,
            p.caret,
            &candidate("group", "/usr/bin/group", false)
        ),
        Err(CompletionError::InvalidCandidate)
    );
}

#[test]
fn assignment_value_replacement_keeps_the_name_and_command_position() {
    let p = bound("PATH_PART=/tm│p cmd arg", None, "/");
    assert_eq!(
        apply(&p, &candidate("tmp", "/tmp", true)).0,
        "PATH_PART='/tmp/' cmd arg"
    );
    assert_eq!(
        unsupported("PATH_PA│RT=/tmp cmd"),
        CompletionUnsupported::AssignmentName
    );
}

#[test]
fn directory_completion_places_caret_inside_closed_quotes_for_continuation() {
    let p = bound("cd /tm│", None, "/");
    let (text, edit) = apply(&p, &candidate("tmp", "/tmp", true));
    assert_eq!(text, "cd '/tmp/'");
    assert_eq!(&text[..edit.caret_byte], "cd '/tmp/");
    assert_eq!(
        p.edit(&p.source, p.caret, &candidate("tmp", "/tmp", false)),
        Err(CompletionError::InvalidCandidate)
    );
}

#[test]
fn unclosed_quote_at_end_can_be_completed_but_not_swallow_later_text() {
    let p = bound("cat '/tmp/fi│", None, "/tmp");
    assert_eq!(
        apply(&p, &candidate("file", "/tmp/file", false)).0,
        "cat '/tmp/file'"
    );
    assert_eq!(
        unsupported("cat '/tmp/fi│ next\nlast"),
        CompletionUnsupported::IncompleteQuote
    );
}

#[test]
fn comments_are_contextual_and_hash_inside_a_word_is_literal() -> Result<(), CompletionError> {
    assert_eq!(
        unsupported("echo x # cat /et│"),
        CompletionUnsupported::Comment
    );
    assert_eq!(
        plan("# ignored\ncat a#b│").query(Some("/tmp"))?,
        CompletionQuery::Paths {
            directory: "/tmp".into(),
            prefix: "a#b".into(),
            directories_only: false
        }
    );
    Ok(())
}

#[test]
fn expansions_and_compound_syntax_fail_closed_without_a_lookup_plan() {
    for input in [
        "cat $HOME/fi│",
        "cat \"$HOME/fi│\"",
        "cat $(touch sentinel)/fi│",
        "echo `pwd`; ca│",
        "cat <(echo x) fi│",
        "cat ~/fi│",
        "cat ~other/fi│",
        "cat *.rs│",
        "cat $'ab│'",
        "if true; then ca│",
        "cat <<EOF\n/et│\nEOF",
        "cat <<< x│",
        "echo x &>/tm│",
        "echo x |& ca│",
        "(cat /tm│)",
    ] {
        let reason = unsupported(input);
        assert!(
            matches!(
                reason,
                CompletionUnsupported::Expansion | CompletionUnsupported::ComplexSyntax
            ),
            "{input}: {reason:?}"
        );
    }
}

#[test]
fn quoted_or_escaped_expansion_characters_remain_literal_data() -> Result<(), CompletionError> {
    for input in ["cat '/tmp/$(x);`y`*│'", "cat /tmp/\\$\\(x\\)\\;\\`y\\`\\*│"] {
        assert_eq!(
            plan(input).query(None)?,
            CompletionQuery::Paths {
                directory: "/tmp/".into(),
                prefix: "$(x);`y`*".into(),
                directories_only: false
            }
        );
    }
    Ok(())
}

#[test]
fn candidate_quoting_never_exposes_shell_metacharacters() {
    let p = bound("cat │ tail", Some("/tmp"), "/tmp");
    let name = "a b'c\";$(`x`)!-";
    let (text, edit) = apply(&p, &candidate(name, &format!("/tmp/{name}"), false));
    assert_eq!(text, "cat '/tmp/a b'\\''c\";$(`x`)!-' tail");
    assert_eq!(edit.caret_byte, "cat ".len() + edit.replacement.len());
}

#[test]
fn path_candidates_cannot_escape_the_bound_parent_or_change_the_name() {
    let p = bound("cat │", Some("/tmp"), "/tmp");
    for c in [
        candidate("file", "/elsewhere/file", false),
        candidate("file", "/tmp/other", false),
        candidate("../file", "/tmp/../file", false),
        candidate(".", "/tmp/.", true),
        candidate("file", "/tmp//file", false),
        candidate("file", "relative/file", false),
    ] {
        assert_eq!(
            p.edit(&p.source, p.caret, &c),
            Err(CompletionError::InvalidCandidate)
        );
    }
}

#[test]
fn command_candidates_insert_only_the_basename_and_reject_directories() {
    let p = plan("gr│ pattern");
    assert_eq!(
        apply(&p, &candidate("grep", "/usr/local/bin/grep", false)).0,
        "'grep' pattern"
    );
    assert_eq!(
        p.edit(
            &p.source,
            p.caret,
            &candidate("grep", "/usr/local/bin/grep", true)
        ),
        Err(CompletionError::InvalidCandidate)
    );
    assert_eq!(
        p.edit(
            &p.source,
            p.caret,
            &candidate("grep", "/usr/local/bin/other", false)
        ),
        Err(CompletionError::InvalidCandidate)
    );
}

#[test]
fn controls_invalid_utf8_markers_and_bidi_candidates_are_rejected() {
    let p = bound("cat │", Some("/tmp"), "/tmp");
    for name in [
        "new\nline",
        "tab\tname",
        "nul\0name",
        "escape\u{1b}",
        "bad\u{fffd}",
        "bidi\u{202e}",
        "isolate\u{2066}",
        "line\u{2028}",
    ] {
        assert_eq!(
            p.edit(
                &p.source,
                p.caret,
                &candidate(name, &format!("/tmp/{name}"), false)
            ),
            Err(CompletionError::InvalidCandidate)
        );
    }
}

#[test]
fn stale_text_or_caret_cannot_apply_even_when_the_current_word_still_matches() {
    let p = bound("cat fi│", Some("/tmp"), "/tmp");
    let c = candidate("file", "/tmp/file", false);
    assert_eq!(
        p.edit("bat fi", p.caret, &c),
        Err(CompletionError::StaleInput)
    );
    assert_eq!(
        p.edit(&p.source, p.caret - 1, &c),
        Err(CompletionError::StaleInput)
    );
}

#[test]
fn caret_boundaries_and_size_limits_are_checked_before_lexing() {
    assert_eq!(
        analyze_completion("中", 1),
        Err(CompletionError::InvalidCursor)
    );
    assert_eq!(
        analyze_completion("x", 2),
        Err(CompletionError::InvalidCursor)
    );
    assert_eq!(
        analyze_completion(&"x".repeat(MAX_COMPLETION_INPUT_BYTES + 1), 0),
        Err(CompletionError::InputTooLong)
    );
    assert_eq!(
        unsupported("cat a\\│ b"),
        CompletionUnsupported::EscapeBoundary
    );
    assert_eq!(
        unsupported("cat a\\│"),
        CompletionUnsupported::EscapeBoundary
    );
}

#[test]
fn edited_command_size_limit_includes_quote_escaping() {
    let source = format!("cat {}│", " ".repeat(MAX_COMPLETION_INPUT_BYTES - 4));
    let p = bound(&source, Some("/tmp"), "/tmp");
    assert_eq!(
        p.edit(&p.source, p.caret, &candidate("'", "/tmp/'", false)),
        Err(CompletionError::InputTooLong)
    );
}

#[test]
fn invisible_candidate_rejection_matches_the_transport_contract() {
    let p = bound("cat │", Some("/tmp"), "/tmp");
    for ch in [
        '\u{200b}', '\u{200c}', '\u{200d}', '\u{2060}', '\u{2065}', '\u{206a}', '\u{206f}',
        '\u{feff}',
    ] {
        let name = format!("before{ch}after");
        assert_eq!(
            p.edit(
                &p.source,
                p.caret,
                &candidate(&name, &format!("/tmp/{name}"), false)
            ),
            Err(CompletionError::InvalidCandidate)
        );
        assert_eq!(
            unsupported(&format!("cat before{ch}│")),
            CompletionUnsupported::ControlCharacter
        );
    }
}

#[test]
fn a_standalone_continuation_does_not_consume_the_command_position() -> Result<(), CompletionError>
{
    assert_eq!(
        plan("\\\n gr│").query(None)?,
        CompletionQuery::Commands {
            prefix: "gr".into()
        }
    );
    assert_eq!(
        plan("echo x | \\\n gr│").query(None)?,
        CompletionQuery::Commands {
            prefix: "gr".into()
        }
    );
    Ok(())
}

#[test]
fn redirection_targets_are_not_reserved_words_or_cd_arguments() -> Result<(), CompletionError> {
    assert_eq!(
        plan(">if│ cat").query(Some("/tmp"))?,
        CompletionQuery::Paths {
            directory: "/tmp".into(),
            prefix: "if".into(),
            directories_only: false
        }
    );
    assert_eq!(
        plan("cd 2>/tm│").query(None)?,
        CompletionQuery::Paths {
            directory: "/".into(),
            prefix: "tm".into(),
            directories_only: false
        }
    );
    Ok(())
}

#[test]
fn attached_long_option_and_leading_assignment_preserve_their_prefix() {
    for (input, expected) in [
        ("tool --config=/tm│p tail", "tool --config='/tmp/' tail"),
        ("CONFIG=/tm│p tool tail", "CONFIG='/tmp/' tool tail"),
    ] {
        let p = bound(input, None, "/");
        assert_eq!(apply(&p, &candidate("tmp", "/tmp", true)).0, expected);
    }
}

#[test]
fn assignment_looking_arguments_are_complete_literal_filenames() -> Result<(), CompletionError> {
    let mut p = plan("cat a=fi│ tail");
    assert_eq!(
        p.query(Some("/base"))?,
        CompletionQuery::Paths {
            directory: "/base".into(),
            prefix: "a=fi".into(),
            directories_only: false,
        }
    );
    let p = p.bind_resolved_directory("/base", "/base")?;
    assert_eq!(
        apply(&p, &candidate("a=file", "/base/a=file", false)).0,
        "cat '/base/a=file' tail"
    );
    let p = bound("A=fi│ cmd", Some("/base"), "/base");
    assert_eq!(
        apply(&p, &candidate("file", "/base/file", false)).0,
        "A='/base/file' cmd"
    );
    Ok(())
}

#[test]
fn query_and_candidate_name_limits_match_transport_bounds() {
    let mut p = plan(&format!("{}│", "a".repeat(MAX_NAME_BYTES + 1)));
    assert_eq!(p.query(None), Err(CompletionError::InputTooLong));
    let p = bound("cat │", Some("/tmp"), "/tmp");
    let name = "a".repeat(MAX_NAME_BYTES + 1);
    assert_eq!(
        p.edit(
            &p.source,
            p.caret,
            &candidate(&name, &format!("/tmp/{name}"), false)
        ),
        Err(CompletionError::InvalidCandidate)
    );
}
