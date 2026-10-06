use keelshell_core::{
    BatchParameterError, BatchParameterValues, BatchParameterizedTemplate, BatchTargetContext,
    SnippetTemplateError,
};

trait Checked<T> {
    fn checked(self, operation: &str) -> T;
}
impl<T, E: std::fmt::Debug> Checked<T> for Result<T, E> {
    fn checked(self, operation: &str) -> T {
        self.unwrap_or_else(|error| panic!("{operation}: {error:?}"))
    }
}
impl<T> Checked<T> for Option<T> {
    fn checked(self, operation: &str) -> T {
        self.unwrap_or_else(|| panic!("{operation}: missing value"))
    }
}
trait Rejected<E> {
    fn checked_error(self, operation: &str) -> E;
}
impl<T, E> Rejected<E> for Result<T, E> {
    fn checked_error(self, operation: &str) -> E {
        match self {
            Err(error) => error,
            Ok(_) => panic!("{operation}: unexpected success"),
        }
    }
}

fn values(items: &[(&str, &str)]) -> BatchParameterValues {
    BatchParameterValues::new(
        items
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect(),
    )
    .checked("test values")
}
fn template(source: &str) -> BatchParameterizedTemplate {
    BatchParameterizedTemplate::compile(source)
        .checked("test source")
        .checked("markers")
}

#[test]
fn metadata_and_distinct_target_literals_keep_the_original_quote_contract() {
    let source = template("printf '%s\\n' {{host}} {{path}} --release={{version}} {{path}}");
    assert_eq!(source.parameters(), &["path", "version"]);
    let context = BatchTargetContext {
        host: "node.invalid".into(),
        ..Default::default()
    };
    let first = source
        .render(
            &context,
            &values(&[("path", "中文'$(touch no)\n\t"), ("version", "v1; echo no")]),
        )
        .checked("render first");
    assert_eq!(
        first,
        "printf '%s\\n' 'node.invalid' '中文'\\''$(touch no)\n\t' --release='v1; echo no' '中文'\\''$(touch no)\n\t'"
    );
    let second = source
        .render(&context, &values(&[("path", ""), ("version", "v2")]))
        .checked("render explicit empty");
    assert_eq!(second, "printf '%s\\n' 'node.invalid' '' --release='v2' ''");
}

#[test]
fn missing_extra_duplicate_reserved_invalid_and_oversized_inputs_fail_without_values() {
    let secret = "fixture-sensitive-value";
    let source = template("echo {{path}}");
    assert!(matches!(
        source.render(&Default::default(), &values(&[])),
        Err(BatchParameterError::Template(
            SnippetTemplateError::MissingValue { .. }
        ))
    ));
    assert!(matches!(
        source.render(
            &Default::default(),
            &values(&[("path", secret), ("unused", secret)])
        ),
        Err(BatchParameterError::Template(
            SnippetTemplateError::UnexpectedValue { .. }
        ))
    ));
    for entries in [
        vec![("path", secret), ("path", secret)],
        vec![("host", secret)],
        vec![("9invalid", secret)],
        vec![("path", "bad\0")],
    ] {
        let error = BatchParameterValues::new(
            entries
                .into_iter()
                .map(|(n, v)| (n.into(), v.into()))
                .collect(),
        )
        .checked_error("reject mapping");
        assert!(!format!("{error:?} {error}").contains(secret));
    }
    assert!(BatchParameterValues::new(vec![("p".into(), "中".repeat(1366))]).is_err());
    assert!(BatchParameterValues::new(vec![("p".into(), "x".repeat(4096))]).is_ok());
    assert!(
        BatchParameterValues::new((0..33).map(|i| (format!("p{i}"), String::new())).collect())
            .is_err()
    );
    assert!(
        BatchParameterValues::new((0..32).map(|i| (format!("p{i}"), String::new())).collect())
            .is_ok()
    );
    assert!(BatchParameterValues::new(vec![("x".repeat(64), String::new())]).is_ok());
    assert!(BatchParameterValues::new(vec![("x".repeat(65), String::new())]).is_err());
}

#[test]
fn task_subset_requires_validated_union_and_never_exposes_values_in_debug() {
    let map = values(&[("path", "fixture-private"), ("release", "v3")]);
    map.validate_names(&["path".into(), "release".into()])
        .checked("union exact");
    let task = map.for_task(&["path".into()]).checked("task subset");
    assert_eq!(
        template("echo {{path}}")
            .render(&Default::default(), &task)
            .checked("render"),
        "echo 'fixture-private'"
    );
    assert!(!format!("{map:?} {task:?}").contains("fixture-private"));
    assert!(map.for_task(&["missing".into()]).is_err());
    assert!(map.validate_names(&["path".into()]).is_err());
}

#[test]
fn grammar_and_render_budgets_cannot_be_bypassed_by_named_parameters() {
    for source in [
        "echo \"{{p}}\"",
        "echo prefix{{p}}",
        "echo $({{p}})",
        "echo {{9invalid}}",
        "cat <<{{p}}",
        "echo {{p",
    ] {
        assert!(
            BatchParameterizedTemplate::compile(source).is_err(),
            "{source}"
        );
    }
    assert!(
        BatchParameterizedTemplate::compile("if true; then echo $HOME; fi")
            .checked("old literal syntax")
            .is_none()
    );
    let source = template(&format!("echo {}", "{{p}} ".repeat(17)));
    assert!(matches!(
        source.render(&Default::default(), &values(&[("p", &"x".repeat(4096))])),
        Err(BatchParameterError::Template(
            SnippetTemplateError::RenderedTooLong
        ))
    ));
    assert!(!format!("{:?}", template("echo fixture-private {{p}}")).contains("fixture-private"));
}

#[test]
fn public_required_name_validation_rejects_untrusted_names_without_echoing_them() {
    let map = BatchParameterValues::default();
    for name in [
        "秘密-value",
        "value\nsecret",
        "x\0y",
        "x".repeat(65).as_str(),
    ] {
        let names = [name.to_owned()];
        for error in [
            map.validate_names(&names)
                .checked_error("reject invalid required name"),
            map.for_task(&names)
                .checked_error("reject invalid task name"),
        ] {
            assert!(!format!("{error:?} {error}").contains(name));
        }
    }
    assert!(matches!(
        map.validate_names(&["p".into(), "p".into()]),
        Err(BatchParameterError::DuplicateValue { .. })
    ));
    assert!(matches!(
        map.for_task(&["host".into()]),
        Err(BatchParameterError::ReservedName { .. })
    ));
}
