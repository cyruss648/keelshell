use std::{collections::BTreeMap, fs};

use keelshell_core::{
    AppState, Error, MAX_SNIPPET_TEMPLATE_BYTES, MAX_SNIPPET_VALUE_BYTES, Snippet,
    SnippetTemplateContext as Context, SnippetTemplateError as TemplateError, StateStore,
    compile_snippet_template as compile,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn values(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
    entries
        .iter()
        .map(|(name, value)| ((*name).into(), (*value).into()))
        .collect()
}

fn assert_context(source: &str, expected: Context) {
    assert!(
        matches!(compile(source), Err(TemplateError::UnsupportedContext { context, .. }) if context == expected),
        "{source:?}: {:?}",
        compile(source)
    );
}

#[test]
fn parameters_follow_first_occurrence_order_and_share_repeated_values() -> TestResult {
    let source = "printf '%s\\n' {{beta}} {{alpha}} {{beta}}";
    let template = compile(source)?;
    assert_eq!(template.source(), source);
    assert_eq!(template.variables(), &["beta", "alpha"]);
    assert_eq!(
        template.render(&values(&[("alpha", "中文"), ("beta", "a'b")]))?,
        "printf '%s\\n' 'a'\\''b' '中文' 'a'\\''b'"
    );
    assert_eq!(template.source(), source);
    Ok(())
}

#[test]
fn empty_values_are_explicit_and_missing_or_extra_keys_are_rejected() -> TestResult {
    let template = compile("tool {{one}} {{two}}")?;
    assert_eq!(
        template.render(&values(&[("one", ""), ("two", "")]))?,
        "tool '' ''"
    );
    assert_eq!(
        template.render(&values(&[("one", "")])),
        Err(TemplateError::MissingValue { name: "two".into() })
    );
    assert_eq!(
        template.render(&values(&[("one", ""), ("two", ""), ("three", "")])),
        Err(TemplateError::UnexpectedValue {
            name: "three".into()
        })
    );
    assert_eq!(
        template.render(&values(&[("invalid key", "")])),
        Err(TemplateError::InvalidValueKey)
    );
    Ok(())
}

#[test]
fn literal_zero_parameter_templates_require_an_empty_value_map() -> TestResult {
    let template = compile("printf '%s\\n' 'literal with spaces' # retained\n")?;
    assert!(template.variables().is_empty());
    assert_eq!(template.render(&BTreeMap::new())?, template.source());
    assert!(matches!(
        template.render(&values(&[("unexpected", "")])),
        Err(TemplateError::UnexpectedValue { .. })
    ));
    Ok(())
}

#[test]
fn ordinary_quotes_comments_whitespace_and_unicode_are_preserved_exactly() -> TestResult {
    let source =
        "  printf '%s\\n' \"中文\" a\\ b {{path}}\n\t# untouched comment\nprintf '%s' {{path}}\n";
    let template = compile(source)?;
    assert_eq!(
        template.render(&values(&[("path", "行一\n\t行二🦀")]))?,
        source.replace("{{path}}", "'行一\n\t行二🦀'")
    );
    Ok(())
}

#[test]
fn full_assignment_and_long_option_values_preserve_their_prefixes() -> TestResult {
    for source in [
        "FILE={{path}} tool",
        "env FILE={{path}} tool",
        "tool --file-name={{path}}",
        "FILE={{path}}",
        "FILE=\\\n{{path}} tool",
        "tool --file=\\\n{{path}}",
        "{{path}}",
    ] {
        let template = compile(source)?;
        assert_eq!(
            template.render(&values(&[("path", "a b'c")]))?,
            source.replace("{{path}}", "'a b'\\''c'")
        );
    }
    Ok(())
}

#[test]
fn lists_pipelines_boolean_operators_and_file_redirections_are_supported() -> TestResult {
    for source in [
        "tool {{x}} | other {{y}}",
        "tool {{x}} &&\n other {{y}} || final",
        "tool {{x}}; other {{y}} &\nfinal",
        "<{{x}} tool 2>{{y}} 1>&2",
        "{{x}} >>{{y}}",
        "2>{{x}} FILE={{y}} tool",
        "tool <> {{x}} >| {{y}}",
        "tool \\\n{{x}} {{y}}",
    ] {
        let template = compile(source)?;
        assert!(
            template
                .variables()
                .iter()
                .all(|name| name == "x" || name == "y")
        );
    }
    Ok(())
}

#[test]
fn quoted_escaped_and_commented_markers_are_refused() {
    for source in [
        "tool '{{x}}'",
        "tool \"{{x}}\"",
        "tool 'prefix {{x}} suffix'",
    ] {
        assert_context(source, Context::Quoted);
    }
    for source in ["# {{x}}\ntool", "tool # ignored {{x}}"] {
        assert_context(source, Context::Comment);
    }
    assert_context("tool \\{{x}}", Context::Escaped);
}

#[test]
fn marker_fragments_and_multiple_markers_in_one_word_are_refused() {
    for source in [
        "tool pre{{x}}",
        "tool {{x}}post",
        "tool {{x}}{{y}}",
        "tool '{{literal}}'{{x}}",
        "tool -x={{x}}",
        "tool --={{x}}",
        "tool /{{x}}",
        "tool {{x}}/tail",
        "tool NAME=pre{{x}}",
        "tool {{x}}''",
    ] {
        assert!(compile(source).is_err(), "{source}");
    }
}

#[test]
fn expansions_compound_syntax_and_here_documents_fail_closed() {
    for source in [
        "tool $HOME {{x}}",
        "tool \"$HOME\" {{x}}",
        "tool $(anything) {{x}}",
        "tool `anything` {{x}}",
        "tool *.txt {{x}}",
        "tool ~ {{x}}",
        "tool ${HOME} {{x}}",
    ] {
        assert_context(source, Context::Expansion);
    }
    for source in [
        "if tool; then other {{x}}; fi",
        "for x in a; do tool {{x}}; done",
        "(tool {{x}})",
        "{ tool {{x}}; }",
        "tool {{x}} |& other",
        "tool {{x}} &>out",
        "tool {{x}};; other",
        "i\\\nf tool {{x}}",
    ] {
        assert_context(source, Context::CompoundSyntax);
    }
    for source in [
        "tool <<EOF\n{{x}}\nEOF",
        "tool <<-EOF\n{{x}}\nEOF",
        "tool <<<{{x}}",
    ] {
        assert_context(source, Context::HereDocument);
    }
}

#[test]
fn incomplete_quotes_operators_and_dynamic_descriptors_are_refused() {
    for source in [
        "tool '{{x}}",
        "tool {{x}} 'unterminated",
        "tool {{x}} \\",
        "tool {{x}} |",
        "tool {{x}} &&",
        "tool {{x}} >",
        "tool {{x}} > ;",
        "; tool {{x}}",
        "tool {{x}} | | other",
    ] {
        assert!(compile(source).is_err(), "{source}");
    }
    for source in [
        "tool >&{{fd}}",
        "tool 2<&{{fd}}",
        "tool 2>&name",
        "tool 2>&''",
    ] {
        assert_context(source, Context::FileDescriptor);
    }
}

#[test]
fn names_are_bounded_ascii_identifiers_and_offsets_are_utf8_bytes() -> TestResult {
    for name in ["", "9first", "has-dash", "has space", "中文"] {
        let source = format!("tool 中文 {{{{{name}}}}}");
        assert_eq!(
            compile(&source),
            Err(TemplateError::InvalidName {
                offset: "tool 中文 {{".len()
            })
        );
    }
    let accepted = "x".repeat(64);
    assert_eq!(
        compile(&format!("tool {{{{{accepted}}}}}"))?.variables(),
        &[accepted]
    );
    assert!(matches!(
        compile(&format!("tool {{{{{}}}}}", "x".repeat(65))),
        Err(TemplateError::InvalidName { .. })
    ));
    for source in ["tool {{x", "tool x}}", "tool {{x}}}", "tool {{{x}}"] {
        assert!(compile(source).is_err(), "{source}");
    }
    Ok(())
}

#[test]
fn variable_limit_counts_unique_names_not_occurrences() -> TestResult {
    let source = format!(
        "tool {}",
        (0..32)
            .map(|n| format!("{{{{v{n}}}}}"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    let template = compile(&(source.clone() + " {{v0}} {{v31}}"))?;
    assert_eq!(template.variables().len(), 32);
    assert_eq!(
        compile(&(source + " {{extra}}")),
        Err(TemplateError::TooManyVariables)
    );
    Ok(())
}

#[test]
fn source_and_value_limits_measure_bytes_and_preserve_empty_values() -> TestResult {
    assert_eq!(compile(" \n\t"), Err(TemplateError::EmptyCommand));
    assert_eq!(
        compile(&"x".repeat(MAX_SNIPPET_TEMPLATE_BYTES + 1)),
        Err(TemplateError::CommandTooLong)
    );
    assert_eq!(
        compile(&"x".repeat(MAX_SNIPPET_TEMPLATE_BYTES))?
            .source()
            .len(),
        MAX_SNIPPET_TEMPLATE_BYTES
    );
    let template = compile("tool {{x}}")?;
    assert_eq!(
        template
            .render(&values(&[("x", &"中".repeat(1_365))]))?
            .len(),
        5 + 4_095 + 2
    );
    assert_eq!(
        template
            .render(&values(&[("x", &"x".repeat(MAX_SNIPPET_VALUE_BYTES))]))?
            .len(),
        5 + MAX_SNIPPET_VALUE_BYTES + 2
    );
    assert!(matches!(
        template.render(&values(&[("x", &"中".repeat(1_366))])),
        Err(TemplateError::ValueTooLong { .. })
    ));
    Ok(())
}

#[test]
fn controls_are_refused_without_echoing_supplied_values_in_errors() -> TestResult {
    let template = compile("tool {{x}}")?;
    for control in ['\0', '\r', '\u{1b}', '\u{7f}', '\u{85}'] {
        assert_context(
            &format!("tool {control} {{{{x}}}}"),
            Context::ControlCharacter,
        );
        let value = format!("private-value{control}never-print");
        let error = template
            .render(&values(&[("x", &value)]))
            .err()
            .ok_or("invalid control was accepted")?;
        assert_eq!(error, TemplateError::InvalidValue { name: "x".into() });
        assert!(!format!("{error:?} {error}").contains("private-value"));
    }
    Ok(())
}

#[test]
fn rendered_limit_includes_repetitions_and_quote_escaping() -> TestResult {
    let template = compile(&format!("tool {}", vec!["{{x}}"; 16].join(" ")))?;
    assert_eq!(
        template.render(&values(&[("x", &"'".repeat(1_024))])),
        Err(TemplateError::RenderedTooLong)
    );
    let source = format!("tool {}", vec!["{{x}}"; 100].join(" "));
    let padding = MAX_SNIPPET_TEMPLATE_BYTES - source.len();
    let template = compile(&(source + &" ".repeat(padding)))?;
    let rendered = template.render(&values(&[("x", "")]))?;
    assert_eq!(rendered.len(), MAX_SNIPPET_TEMPLATE_BYTES - 300);
    Ok(())
}

#[test]
fn final_render_budget_is_not_rejected_by_temporary_growth_before_later_shrinkage() -> TestResult {
    let source = "tool {{expand}} {{long_parameter_that_shrinks}}";
    let source = format!(
        "{source}{}",
        " ".repeat(MAX_SNIPPET_TEMPLATE_BYTES - source.len())
    );
    let template = compile(&source)?;
    let result = template.render(&values(&[
        ("expand", "01234567890123456789"),
        ("long_parameter_that_shrinks", ""),
    ]))?;
    assert!(result.len() < MAX_SNIPPET_TEMPLATE_BYTES);
    Ok(())
}

#[test]
fn old_snippet_json_keeps_double_braces_literal_and_opt_in_is_strict() -> TestResult {
    let snippet = Snippet::new("Literal", "echo '{{x}}' ${HOME}");
    assert!(!snippet.parameterized);
    assert_eq!(snippet.compile_template()?, None);
    let mut json = serde_json::to_value(&snippet)?;
    json.as_object_mut()
        .ok_or("expected object")?
        .remove("parameterized");
    let restored: Snippet = serde_json::from_value(json.clone())?;
    assert_eq!(restored, snippet);
    restored.validate()?;
    json["parameterized"] = serde_json::json!(true);
    let opted_in: Snippet = serde_json::from_value(json.clone())?;
    assert!(opted_in.validate().is_err());
    json["parameterized"] = serde_json::json!("true");
    assert!(serde_json::from_value::<Snippet>(json).is_err());
    Ok(())
}

#[test]
fn real_legacy_state_without_opt_in_loads_literal_text_and_refuses_value_fields() -> TestResult {
    let temp = tempfile::tempdir()?;
    let store = StateStore::new(temp.path().join("state.json"));
    let snippet = Snippet::new("Legacy", "tool '{{x}}' $HOME");
    let original = AppState {
        snippets: vec![snippet.clone()],
        ..AppState::default()
    };
    let saved = store.save(&original)?;
    let mut json = serde_json::to_value(saved)?;
    json["snippets"][0]
        .as_object_mut()
        .ok_or("snippet object")?
        .remove("parameterized");
    fs::write(store.path(), serde_json::to_vec(&json)?)?;
    let loaded = StateStore::new(store.path()).load()?;
    assert_eq!(loaded.snippets, vec![snippet]);
    assert!(loaded.snippets[0].compile_template()?.is_none());
    json["snippets"][0]["values"] = serde_json::json!({"x": "never-a-model-field"});
    fs::write(store.path(), serde_json::to_vec(&json)?)?;
    assert!(matches!(
        StateStore::new(store.path()).load(),
        Err(Error::Json(_))
    ));
    Ok(())
}

#[test]
fn explicit_template_crud_and_persistence_never_store_supplied_values() -> TestResult {
    let temp = tempfile::tempdir()?;
    let store = StateStore::new(temp.path().join("state.json"));
    let mut state = store.load()?;
    let mut snippet = Snippet::new("参数", "printf '%s' {{value}}");
    snippet.parameterized = true;
    state.insert_snippet(snippet.clone())?;
    state = store.save(&state)?;
    assert_eq!(StateStore::new(store.path()).load()?, state);
    let template = snippet
        .compile_template()?
        .ok_or("missing opted-in template")?;
    let rendered = template.render(&values(&[("value", "ephemeral-never-saved-测试")]))?;
    assert!(rendered.contains("ephemeral-never-saved-测试"));
    assert!(!fs::read_to_string(store.path())?.contains("ephemeral-never-saved"));
    let before = state.clone();
    let mut invalid = snippet.clone();
    invalid.command = "tool '{{value}}'".into();
    assert!(matches!(
        state.update_snippet(invalid.clone()),
        Err(Error::Validation(_))
    ));
    assert_eq!(state, before);
    invalid.id = uuid::Uuid::new_v4();
    assert!(state.insert_snippet(invalid).is_err());
    assert_eq!(state, before);
    snippet.parameterized = false;
    snippet.command = "tool '{{value}}'".into();
    state.update_snippet(snippet.clone())?;
    state = store.save(&state)?;
    assert_eq!(StateStore::new(store.path()).load()?, state);
    assert_eq!(state.remove_snippet(snippet.id)?, snippet);
    store.save(&state)?;
    assert!(
        !StateStore::new(store.path())
            .load()?
            .snippets
            .iter()
            .any(|entry| entry.id == snippet.id)
    );
    assert!(!fs::read_to_string(store.path())?.contains("ephemeral-never-saved"));
    Ok(())
}

#[test]
fn invalid_opt_in_disk_data_fails_validation_and_stale_changes_cannot_overwrite() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("state.json");
    let first = StateStore::new(&path);
    let second = StateStore::new(&path);
    let mut initial = first.load()?;
    let mut snippet = Snippet::new("Template", "tool {{x}}");
    snippet.parameterized = true;
    initial.insert_snippet(snippet.clone())?;
    first.save(&initial)?;
    let mut winner = first.load()?;
    let mut stale = second.load()?;
    let mut winning = snippet.clone();
    winning.command = "tool {{y}}".into();
    winner.update_snippet(winning)?;
    let saved = first.save(&winner)?;
    snippet.parameterized = false;
    stale.update_snippet(snippet)?;
    assert!(matches!(second.save(&stale), Err(Error::Conflict)));
    assert_eq!(StateStore::new(&path).load()?, saved);
    let mut json = serde_json::to_value(saved)?;
    let entries = json["snippets"].as_array_mut().ok_or("snippets array")?;
    let entry = entries
        .iter_mut()
        .find(|entry| entry["parameterized"] == true)
        .ok_or("template entry")?;
    entry["command"] = serde_json::json!("tool '{{y}}'");
    fs::write(&path, serde_json::to_vec(&json)?)?;
    assert!(matches!(
        StateStore::new(&path).load(),
        Err(Error::Validation(_))
    ));
    Ok(())
}
