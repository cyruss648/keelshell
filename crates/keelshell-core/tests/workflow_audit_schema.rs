//! New non-author parser compatibility and hostile JSON controls.
use keelshell_core::{
    AppState, StateStore, WorkflowAuditNotStarted as Reason, WorkflowAuditOutcome as Outcome,
    WorkflowAuditRecord, WorkflowAuditTrigger as Trigger, WorkflowTaskAudit,
};
use serde_json::{Value, json};
use uuid::Uuid;
type Result = std::result::Result<(), Box<dyn std::error::Error>>;
fn outcomes() -> Vec<(Outcome, Value)> {
    let mut values = vec![
        (Outcome::Succeeded, json!({"kind":"succeeded"})),
        (
            Outcome::Failed {
                exit_code: u32::MAX,
            },
            json!({"kind":"failed","exit_code":u32::MAX}),
        ),
        (Outcome::Rejected, json!({"kind":"rejected"})),
        (Outcome::Unknown, json!({"kind":"unknown"})),
        (Outcome::Cancelled, json!({"kind":"cancelled"})),
        (
            Outcome::DependencyBlocked {
                dependency: Uuid::from_u128(1),
            },
            json!({"kind":"dependency_blocked","dependency":Uuid::from_u128(1)}),
        ),
        (
            Outcome::StoppedAfterFailure,
            json!({"kind":"stopped_after_failure"}),
        ),
    ];
    for (reason, text) in [
        (Reason::AdmissionRejected, "admission_rejected"),
        (Reason::SessionUnavailable, "session_unavailable"),
        (Reason::Deadline, "deadline"),
        (Reason::WorkerFailed, "worker_failed"),
        (Reason::StartRejected, "start_rejected"),
        (Reason::ScheduleMissed, "schedule_missed"),
        (Reason::ScheduleBusy, "schedule_busy"),
        (Reason::ScheduleInvalidated, "schedule_invalidated"),
    ] {
        values.push((
            Outcome::NotStarted { reason },
            json!({"kind":"not_started","reason":text}),
        ));
    }
    values
}
fn triggers() -> Vec<(Trigger, Value)> {
    vec![
        (Trigger::Manual, json!({"kind":"manual"})),
        (
            Trigger::Scheduled {
                schedule_id: Uuid::from_u128(101),
                occurrence: 31,
                scheduled_at: 1_725_000_000,
            },
            json!({"kind":"scheduled","schedule_id":Uuid::from_u128(101),"occurrence":31,"scheduled_at":1_725_000_000}),
        ),
    ]
}
#[test]
fn reviewed_parser_preserves_exact_documented_wire_for_every_valid_variant() -> Result {
    for (outcome, expected) in outcomes() {
        assert_eq!(serde_json::to_value(outcome)?, expected);
        assert_eq!(
            serde_json::from_value::<Outcome>(expected.clone())?,
            outcome
        );
        let mut object = expected.as_object().ok_or("object")?.clone();
        let kind = object.remove("kind").ok_or("kind")?;
        let mut pairs = object.into_iter().collect::<Vec<_>>();
        pairs.push(("kind".into(), kind));
        let reordered = pairs
            .into_iter()
            .map(|(k, v)| format!("{}:{}", serde_json::to_string(&k).unwrap_or_default(), v))
            .collect::<Vec<_>>()
            .join(",");
        assert_eq!(
            serde_json::from_str::<Outcome>(&format!("{{{reordered}}}"))?,
            outcome
        );
    }
    for (trigger, expected) in triggers() {
        assert_eq!(serde_json::to_value(trigger)?, expected);
        assert_eq!(serde_json::from_value::<Trigger>(expected)?, trigger);
    }
    assert_eq!(
        serde_json::to_string(&Outcome::Succeeded)?,
        r#"{"kind":"succeeded"}"#
    );
    assert_eq!(
        serde_json::to_string(&Trigger::Manual)?,
        r#"{"kind":"manual"}"#
    );
    println!("nonauthor documented compatibility: 15 outcome + 2 trigger representations");
    Ok(())
}
#[test]
fn reviewed_parser_rejects_extra_keys_regardless_of_json_value_type() -> Result {
    let mut count = 0;
    for (_, value) in outcomes() {
        for payload in [
            Value::Null,
            json!(false),
            json!(42),
            json!("中文 private context"),
            json!([1]),
            json!({"nested":true}),
        ] {
            for key in [
                "extra",
                "command",
                "exit_code_if_other_variant",
                "空字段",
                "\0",
            ] {
                let mut hostile = value.clone();
                hostile[key] = payload.clone();
                assert!(serde_json::from_value::<Outcome>(hostile).is_err());
                count += 1;
            }
        }
    }
    for (_, value) in triggers() {
        for payload in [
            Value::Null,
            json!(false),
            json!(42),
            json!("private context"),
            json!([1]),
            json!({"nested":true}),
        ] {
            for key in [
                "extra",
                "command",
                "schedule_if_other_variant",
                "空字段",
                "\0",
            ] {
                let mut hostile = value.clone();
                hostile[key] = payload.clone();
                assert!(serde_json::from_value::<Trigger>(hostile).is_err());
                count += 1;
            }
        }
    }
    assert_eq!(count, 510);
    println!("nonauthor hostile extra-field controls: {count} rejected");
    Ok(())
}
#[test]
fn reviewed_parser_rejects_duplicate_tags_missing_fields_and_wrong_scalar_types() {
    for hostile in [
        r#"{"kind":"succeeded","kind":"succeeded"}"#,
        r#"{"kind":"unknown","kind":"succeeded"}"#,
        r#"{"kind":"failed","exit_code":7,"exit_code":8}"#,
        r#"{"kind":"failed"}"#,
        r#"{"kind":"failed","exit_code":-1}"#,
        r#"{"kind":"failed","exit_code":4294967296}"#,
        r#"{"kind":"failed","exit_code":"7"}"#,
        r#"{"kind":"dependency_blocked","dependency":null}"#,
        r#"{"kind":"not_started","reason":{"deadline":null,"command":"extra"}}"#,
        r#"{"kind":"not_started","reason":"new_unknown_reason"}"#,
        r#"{"kind":"new_unknown_kind"}"#,
        r#"{}"#,
        r#""succeeded""#,
        r#"["succeeded",{"command":"extra"}]"#,
        r#"{"kind":"succeeded","exit_code":0}"#,
    ] {
        assert!(
            serde_json::from_str::<Outcome>(hostile).is_err(),
            "{hostile}"
        );
    }
    for hostile in [
        r#"{"kind":"manual","kind":"manual"}"#,
        r#"{"kind":"manual","occurrence":0}"#,
        r#"{"kind":"scheduled","schedule_id":null,"occurrence":0,"scheduled_at":0}"#,
        r#"{"kind":"scheduled","schedule_id":"00000000-0000-0000-0000-000000000001","occurrence":0}"#,
        r#"{"kind":"new_unknown_kind"}"#,
        r#"{}"#,
        r#""manual""#,
        r#"{"kind":null}"#,
    ] {
        assert!(
            serde_json::from_str::<Trigger>(hostile).is_err(),
            "{hostile}"
        );
    }
}
#[test]
fn reviewed_parser_loads_complete_valid_history_and_fails_closed_on_extra_context() -> Result {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("state.json");
    let store = StateStore::new(&path);
    let mut state = store.load()?;
    let scheduled = triggers()[1].0;
    for (index, (outcome, _)) in outcomes().into_iter().enumerate() {
        let trigger = match scheduled {
            Trigger::Scheduled {
                schedule_id,
                scheduled_at,
                ..
            } => Trigger::Scheduled {
                schedule_id,
                occurrence: index as u32,
                scheduled_at,
            },
            Trigger::Manual => Trigger::Manual,
        };
        state.record_workflow_audit(WorkflowAuditRecord {
            id: Uuid::from_u128(index as u128 + 1000),
            recorded_at: 1_725_000_001,
            trigger,
            tasks: vec![
                WorkflowTaskAudit {
                    id: Uuid::from_u128(1),
                    target_id: Uuid::from_u128(200),
                    outcome: Outcome::Succeeded,
                },
                WorkflowTaskAudit {
                    id: Uuid::from_u128(2),
                    target_id: Uuid::from_u128(200),
                    outcome,
                },
            ],
            cancelled: true,
            stopped_after_failure: true,
        })?;
    }
    let saved = store.save(&state)?;
    assert_eq!(
        StateStore::new(&path).load()?.workflow_audits,
        saved.workflow_audits
    );
    let bytes = std::fs::read(&path)?;
    let mut wire: Value = serde_json::from_slice(&bytes)?;
    wire["workflow_audits"][0]["tasks"][1]["outcome"]["command"] =
        json!("never persisted producer context");
    std::fs::write(&path, serde_json::to_vec(&wire)?)?;
    assert!(StateStore::new(&path).load().is_err());
    assert!(serde_json::from_value::<AppState>(wire).is_err());
    println!(
        "nonauthor real temporary StateStore: 15 valid history records load; unknown context rejects whole state"
    );
    Ok(())
}
