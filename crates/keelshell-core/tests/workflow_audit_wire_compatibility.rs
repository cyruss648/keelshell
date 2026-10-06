//! Preserve accepted legacy representations while rejecting attached context.

use keelshell_core::{WorkflowAuditNotStarted, WorkflowAuditOutcome, WorkflowAuditTrigger};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn assert_projection<T: DeserializeOwned + Serialize>(
    text: &str,
    expected: Option<Value>,
) -> TestResult {
    match expected {
        Some(expected) => {
            let parsed = serde_json::from_str::<T>(text)?;
            assert_eq!(serde_json::to_value(parsed)?, expected, "{text}");
        }
        None => assert!(serde_json::from_str::<T>(text).is_err(), "{text}"),
    }
    Ok(())
}

#[test]
fn legacy_forms_have_fixed_projections_and_cannot_attach_context() -> TestResult {
    for (text, expected) in [
        (r#"["succeeded"]"#, Some(json!({"kind":"succeeded"}))),
        (r#"["succeeded",null]"#, None),
        (r#"["succeeded",{}]"#, None),
        (r#"["succeeded",{"command":"extra"}]"#, None),
        (
            r#"["failed",7]"#,
            Some(json!({"kind":"failed","exit_code":7})),
        ),
        (r#"["failed",7,"extra"]"#, None),
        (
            r#"{"kind":"not_started","reason":{"deadline":null}}"#,
            Some(json!({"kind":"not_started","reason":"deadline"})),
        ),
    ] {
        assert_projection::<WorkflowAuditOutcome>(text, expected)?;
    }
    for (text, expected) in [
        (r#"["manual"]"#, Some(json!({"kind":"manual"}))),
        (r#"["manual",null]"#, None),
        (r#"["manual",{"command":"extra"}]"#, None),
        (
            r#"["scheduled","00000000-0000-0000-0000-000000000001",0,1725000000]"#,
            Some(json!({
                "kind":"scheduled","schedule_id":"00000000-0000-0000-0000-000000000001","occurrence":0,"scheduled_at":1_725_000_000,
            })),
        ),
    ] {
        assert_projection::<WorkflowAuditTrigger>(text, expected)?;
    }
    for (text, expected) in [
        (r#""deadline""#, Some(json!("deadline"))),
        (r#"{"deadline":null}"#, Some(json!("deadline"))),
        (r#"{"deadline":null,"command":"extra"}"#, None),
    ] {
        assert_projection::<WorkflowAuditNotStarted>(text, expected)?;
    }
    Ok(())
}
