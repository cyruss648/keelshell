#![allow(clippy::unwrap_used, clippy::expect_used)]
use super::*;

fn run(limits: AgentLimits) -> AgentRun {
    AgentRun::new(
        AgentTarget::new("fixture", "session-a").unwrap(),
        "Find cause",
        "selected output",
        limits,
    )
    .unwrap()
}
fn command() -> &'static str {
    r#"{"explanation":"inspect selected evidence","action":{"kind":"command","command":"printf 'observed'"}}"#
}

#[test]
fn closed_loop_requires_both_reviews_and_includes_confirmed_result() {
    let mut run = run(AgentLimits::default());
    assert!(run.next_prompt().unwrap().contains("selected output"));
    assert_eq!(run.rounds_used(), 0, "preparation is not a request");
    let round = run.begin_round().unwrap();
    assert!(run.next_prompt().is_err());
    run.receive_decision(run.id(), round, command()).unwrap();
    let id = run.pending_step().unwrap().id();
    assert_eq!(run.phase(), AgentPhase::AwaitingAction);
    assert!(run.begin_round().is_err());
    run.approve_action(id).unwrap();
    assert!(run.approve_action(id).is_err(), "approval cannot be reused");
    run.action_result(
        run.id(),
        id,
        AgentOutcome::Completed {
            exit_status: Some(7),
            output: "confirmed failure output".into(),
        },
    )
    .unwrap();
    let prompt = run.next_prompt().unwrap();
    assert!(prompt.contains("confirmed failure output") && prompt.contains("exit_status"));
    assert_eq!(run.rounds_used(), 1);
    let round = run.begin_round().unwrap();
    run.receive_decision(run.id(), round, r#"{"explanation":"supplied evidence supports conclusion","action":{"kind":"finish","summary":"Exit 7; cause remains uncertain"}}"#).unwrap();
    assert_eq!(run.phase(), AgentPhase::Completed);
    assert_eq!(run.steps().len(), 2);
}

#[test]
fn rejected_action_is_feedback_without_dispatch_and_consumes_action_allowance() {
    let mut run = run(AgentLimits::new(3, 1, 8192).unwrap());
    let round = run.begin_round().unwrap();
    run.receive_decision(run.id(), round, command()).unwrap();
    let id = run.pending_step().unwrap().id();
    run.action_result(run.id(), id, AgentOutcome::Rejected)
        .unwrap();
    assert!(run.next_prompt().unwrap().contains("rejected"));
    let round = run.begin_round().unwrap();
    assert_eq!(
        run.receive_decision(run.id(), round, command()),
        Err(AgentError::BudgetExceeded)
    );
    assert_eq!(run.phase(), AgentPhase::BudgetExceeded);
}

#[test]
fn exhausted_round_allowance_does_not_admit_another_request() {
    let mut run = run(AgentLimits::new(1, 2, 8192).unwrap());
    let round = run.begin_round().unwrap();
    run.receive_decision(run.id(), round, command()).unwrap();
    let id = run.pending_step().unwrap().id();
    run.action_result(run.id(), id, AgentOutcome::Rejected)
        .unwrap();
    assert_eq!(run.begin_round(), Err(AgentError::BudgetExceeded));
    assert_eq!(run.rounds_used(), 1);
}

#[test]
fn unknown_and_stopped_dispatch_never_become_automatic_retry() {
    for stop in [false, true] {
        let mut run = run(AgentLimits::default());
        let round = run.begin_round().unwrap();
        run.receive_decision(run.id(), round, command()).unwrap();
        let id = run.pending_step().unwrap().id();
        run.approve_action(id).unwrap();
        if stop {
            run.stop();
        } else {
            run.action_result(run.id(), id, AgentOutcome::Unknown)
                .unwrap();
        }
        assert_eq!(run.phase(), AgentPhase::OutcomeUnknown);
        assert!(run.begin_round().is_err());
        assert!(
            run.action_result(
                run.id(),
                id,
                AgentOutcome::Completed {
                    exit_status: Some(0),
                    output: "late success".into()
                }
            )
            .is_err()
        );
    }
}

#[test]
fn stale_identity_round_action_and_target_loss_are_rejected() {
    let mut run = run(AgentLimits::default());
    let round = run.begin_round().unwrap();
    assert_eq!(
        run.receive_decision(Uuid::new_v4(), round, command()),
        Err(AgentError::StaleEvent)
    );
    assert_eq!(
        run.receive_decision(run.id(), round + 1, command()),
        Err(AgentError::StaleEvent)
    );
    run.receive_decision(run.id(), round, command()).unwrap();
    assert!(run.approve_action(Uuid::new_v4()).is_err());
    run.lose_target();
    assert_eq!(run.phase(), AgentPhase::TargetLost);
    assert!(run.approve_action(run.steps()[0].id()).is_err());
}

#[test]
fn malformed_model_protocol_fails_closed_without_creating_action() {
    for text in [
        "```json\n{}\n```",
        "{}",
        "[]",
        "{} trailing",
        r#"{"explanation":"x","action":{"kind":"command","command":"id"},"approve":true}"#,
        r#"{"explanation":"x","explanation":"y","action":{"kind":"command","command":"id"}}"#,
        r#"{"explanation":"x","action":{"kind":"command","command":"id","target":"other"}}"#,
        r#"{"explanation":"x","action":{"kind":"read_file","path":"/etc/../shadow"}}"#,
        r#"{"explanation":"x","action":{"kind":"read_file","path":"//etc/passwd"}}"#,
        r#"{"explanation":"x","action":{"kind":"read_file","path":"/"}}"#,
        r#"{"explanation":"x","action":{"kind":"automatic_tool","command":"id"}}"#,
        r#"{"explanation":"x","action":{"kind":"command","command":"id\u0000"}}"#,
    ] {
        let mut run = run(AgentLimits::default());
        let round = run.begin_round().unwrap();
        assert_eq!(
            run.receive_decision(run.id(), round, text),
            Err(AgentError::InvalidDecision),
            "{text}"
        );
        assert!(run.steps().is_empty());
        assert_eq!(run.phase(), AgentPhase::Failed);
    }
}

#[test]
fn transcript_limit_rejects_intact_evidence_instead_of_silent_truncation() {
    let mut run = AgentRun::new(
        AgentTarget::new("fixture", "session-a").unwrap(),
        "question",
        "🙂".repeat(3000),
        AgentLimits::new(6, 4, 8192).unwrap(),
    )
    .unwrap();
    assert_eq!(run.next_prompt(), Err(AgentError::BudgetExceeded));
    assert_eq!(run.rounds_used(), 0);
}

#[test]
fn debug_never_includes_selected_output_commands_or_file_content() {
    let mut run = run(AgentLimits::default());
    let round = run.begin_round().unwrap();
    run.receive_decision(run.id(), round, command()).unwrap();
    for text in [
        format!("{run:?}"),
        format!("{:?}", run.steps()[0].decision()),
    ] {
        for secret in ["selected output", "printf", "inspect selected evidence"] {
            assert!(!text.contains(secret));
        }
    }
}
