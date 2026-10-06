use keelshell_core::{
    AiApiStyle, AiAuthentication, AiModelReasoning, AiModelSampling, AiPreset, AiProfileCatalog,
    AiReasoningCapability, AiReasoningSelection, AiSamplingValue, NamedAiProfile, StateStore,
};
use serde_json::json;

type TestResult = Result<(), Box<dyn std::error::Error>>;
fn profile() -> NamedAiProfile {
    let mut p = NamedAiProfile::draft(AiPreset::Custom);
    p.name = "Inference fixture".into();
    p.endpoint = "https://provider.example/v1/messages".into();
    p.model = "explicit-model".into();
    p.authentication = AiAuthentication::None;
    p
}
fn reasoning(p: &mut NamedAiProfile, selection: AiReasoningSelection) {
    let capability = match &selection {
        AiReasoningSelection::Effort(v) => AiReasoningCapability::Effort {
            values: vec![v.clone()],
        },
        AiReasoningSelection::Budget(_) => AiReasoningCapability::TokenBudget {
            min: 1024,
            max: 1_000_000,
        },
        AiReasoningSelection::Thinking(_) => AiReasoningCapability::ThinkingToggle,
        _ => AiReasoningCapability::Unknown,
    };
    p.reasoning_by_model.insert(
        p.model.clone(),
        AiModelReasoning {
            capability,
            selection,
        },
    );
}

#[test]
fn exact_decimal_bounds_precision_and_nonfinite_input() -> TestResult {
    for (text, millis) in [
        ("0", 0),
        ("0.001", 1),
        ("0.123", 123),
        ("1.000", 1000),
        ("2", 2000),
    ] {
        let value = AiSamplingValue::parse(text)?;
        assert_eq!(value.millis(), millis);
        assert_eq!(AiSamplingValue::parse(&value.decimal())?, value);
        assert_eq!(
            serde_json::from_value::<AiSamplingValue>(serde_json::to_value(value)?)?,
            value
        );
    }
    for text in [
        "NaN",
        "inf",
        "-0.1",
        "+1",
        "1e0",
        "0.0001",
        "2.001",
        "9999999999",
        ".5",
        "",
    ] {
        assert!(AiSamplingValue::parse(text).is_err(), "{text}");
    }
    for value in [
        json!(-1),
        json!(0.5),
        json!(2001),
        json!("NaN"),
        json!(null),
    ] {
        assert!(serde_json::from_value::<AiSamplingValue>(value).is_err());
    }
    Ok(())
}

#[test]
fn protocol_reasoning_and_strict_messages_output_budget() -> TestResult {
    let mut p = profile();
    for protocol in [
        AiApiStyle::ChatCompletions,
        AiApiStyle::Responses,
        AiApiStyle::AnthropicMessages,
    ] {
        p.api_style = protocol;
        for effort in [
            "none", "minimal", "low", "medium", "high", "xhigh", "max", "invented",
        ] {
            reasoning(&mut p, AiReasoningSelection::Effort(effort.into()));
            let accepted = effort != "invented"
                && !(protocol == AiApiStyle::AnthropicMessages
                    && ["none", "minimal"].contains(&effort));
            assert_eq!(
                p.validate_current_transport().is_ok(),
                accepted,
                "{protocol:?}/{effort}"
            );
        }
    }
    for budget in [1024, 4095, 4096, 5000] {
        reasoning(&mut p, AiReasoningSelection::Budget(budget));
        assert_eq!(p.validate_current_transport().is_ok(), budget < 4096);
    }
    p.max_output_tokens = Some(16000);
    reasoning(&mut p, AiReasoningSelection::Budget(8192));
    p.validate_current_transport()?;
    p.model = "another-model".into();
    p.validate_current_transport()?;
    Ok(())
}

#[test]
fn sampling_declaration_omission_zero_and_combinations() -> TestResult {
    let mut p = profile();
    let mut sampling = AiModelSampling {
        declared_supported: false,
        temperature: Some(AiSamplingValue::parse("0")?),
        top_p: None,
    };
    assert!(sampling.validate().is_err());
    sampling.declared_supported = true;
    sampling.validate()?;
    p.sampling_by_model
        .insert(p.model.clone(), sampling.clone());
    p.validate_current_transport()?;
    reasoning(&mut p, AiReasoningSelection::Effort("high".into()));
    assert!(p.validate_current_transport().is_err());
    reasoning(&mut p, AiReasoningSelection::Effort("none".into()));
    p.validate_current_transport()?;
    sampling.top_p = Some(AiSamplingValue::parse("1")?);
    assert!(sampling.validate().is_err());
    sampling.temperature = None;
    sampling.top_p = Some(AiSamplingValue::parse("1.001")?);
    assert!(sampling.validate().is_err());
    p.api_style = AiApiStyle::AnthropicMessages;
    reasoning(&mut p, AiReasoningSelection::Thinking(false));
    p.validate_current_transport()?;
    p.sampling_by_model
        .get_mut(&p.model)
        .ok_or("sampling")?
        .temperature = Some(AiSamplingValue::parse("1.001")?);
    assert!(p.validate_current_transport().is_err());
    Ok(())
}

#[test]
fn named_inference_roundtrip_missing_field_and_strict_metadata() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = StateStore::new(dir.path().join("state.json"));
    let mut state = store.load()?;
    let mut p = profile();
    reasoning(&mut p, AiReasoningSelection::Effort("none".into()));
    p.sampling_by_model.insert(
        p.model.clone(),
        AiModelSampling {
            declared_supported: true,
            temperature: Some(AiSamplingValue::parse("0")?),
            top_p: None,
        },
    );
    state.settings.ai_profiles = AiProfileCatalog {
        active_id: Some(p.id),
        profiles: vec![p.clone()],
    };
    let saved = store.save(&state)?;
    assert_eq!(StateStore::new(store.path()).load()?, saved);
    let mut legacy = serde_json::to_value(&p)?;
    legacy
        .as_object_mut()
        .ok_or("profile")?
        .remove("sampling_by_model");
    assert!(
        serde_json::from_value::<NamedAiProfile>(legacy)?
            .sampling_by_model
            .is_empty()
    );
    assert!(
        serde_json::from_value::<AiModelSampling>(
            json!({"declared_supported":true,"temperature":0,"top_p":null,"password":"never"})
        )
        .is_err()
    );
    assert!(p.legacy_projection(true).is_err());
    Ok(())
}

#[test]
fn messages_composition_declarations_migration_limits_and_storage() -> TestResult {
    use keelshell_core::{AiMessagesEffort, AiMessagesInference, AiMessagesThinking};
    let mut p = profile();
    p.api_style = AiApiStyle::AnthropicMessages;
    let make = |thinking| AiModelReasoning {
        capability: AiReasoningCapability::Messages {
            efforts: vec![AiMessagesEffort::Medium],
            adaptive: true,
            disabled: true,
            manual_budget: true,
        },
        selection: AiReasoningSelection::Messages(AiMessagesInference {
            effort: Some(AiMessagesEffort::Medium),
            thinking,
        }),
    };
    for mode in [
        AiMessagesThinking::ProviderDefault,
        AiMessagesThinking::Adaptive,
        AiMessagesThinking::Disabled,
        AiMessagesThinking::LegacyBudget(1024),
    ] {
        p.reasoning_by_model.insert(p.model.clone(), make(mode));
        p.validate_current_transport()?;
    }
    for budget in [1023, 1024, 4095, 4096] {
        p.reasoning_by_model.insert(
            p.model.clone(),
            make(AiMessagesThinking::LegacyBudget(budget)),
        );
        assert_eq!(
            p.validate_current_transport().is_ok(),
            (1024..4096).contains(&budget)
        );
    }
    p.reasoning_by_model
        .insert(p.model.clone(), make(AiMessagesThinking::Adaptive));
    p.api_style = AiApiStyle::Responses;
    assert!(p.validate_current_transport().is_err());
    p.api_style = AiApiStyle::AnthropicMessages;
    let mut missing = make(AiMessagesThinking::Adaptive);
    if let AiReasoningCapability::Messages { adaptive, .. } = &mut missing.capability {
        *adaptive = false;
    }
    assert!(missing.validate().is_err());
    let mut duplicate = make(AiMessagesThinking::Adaptive);
    if let AiReasoningCapability::Messages { efforts, .. } = &mut duplicate.capability {
        efforts.push(AiMessagesEffort::Medium);
    }
    assert!(duplicate.validate().is_err());
    p.sampling_by_model.insert(
        p.model.clone(),
        AiModelSampling {
            declared_supported: true,
            temperature: Some(AiSamplingValue::parse("0")?),
            top_p: None,
        },
    );
    assert!(p.validate_current_transport().is_err());
    p.reasoning_by_model.insert(
        p.model.clone(),
        AiModelReasoning {
            capability: AiReasoningCapability::Messages {
                efforts: vec![],
                adaptive: false,
                disabled: true,
                manual_budget: false,
            },
            selection: AiReasoningSelection::Messages(AiMessagesInference {
                effort: None,
                thinking: AiMessagesThinking::Disabled,
            }),
        },
    );
    p.validate_current_transport()?;
    p.sampling_by_model.clear();
    // Existing choices deserialize as before and are interpreted without adding another field.
    for old in [
        AiReasoningSelection::ProviderDefault,
        AiReasoningSelection::Effort("medium".into()),
        AiReasoningSelection::Thinking(true),
        AiReasoningSelection::Thinking(false),
        AiReasoningSelection::Budget(2048),
    ] {
        reasoning(&mut p, old.clone());
        let loaded: NamedAiProfile = serde_json::from_value(serde_json::to_value(&p)?)?;
        let selected = &loaded.reasoning_by_model[&loaded.model].selection;
        assert_eq!(selected, &old);
        let interpreted = AiMessagesInference::from_selection(selected)?;
        assert_eq!(
            interpreted.effort,
            matches!(old, AiReasoningSelection::Effort(_)).then_some(AiMessagesEffort::Medium)
        );
        p.validate_current_transport()?;
    }
    p.reasoning_by_model
        .insert(p.model.clone(), make(AiMessagesThinking::Adaptive));
    let dir = tempfile::tempdir()?;
    let store = StateStore::new(dir.path().join("state.json"));
    let mut state = store.load()?;
    state.settings.ai_profiles = AiProfileCatalog {
        active_id: Some(p.id),
        profiles: vec![p.clone()],
    };
    let saved = store.save(&state)?;
    assert_eq!(StateStore::new(store.path()).load()?, saved);
    assert!(p.legacy_projection(true).is_err());
    for value in [
        json!({"effort":"invented","thinking":{"kind":"adaptive"}}),
        json!({"effort":"medium","thinking":{"kind":"adaptive"},"raw_body":{}}),
        json!({"effort":"medium","thinking":{"kind":"adaptive","password":"never"}}),
    ] {
        assert!(serde_json::from_value::<AiMessagesInference>(value).is_err());
    }
    Ok(())
}
