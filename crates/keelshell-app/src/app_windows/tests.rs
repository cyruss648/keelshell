use super::*;

#[::core::prelude::v1::test]
fn configuration_write_lease_excludes_competing_window_and_rejects_old_revision()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = StateStore::new(directory.path().join("state.json"));
    let initial = store.save(&AppState::default())?;
    let gate = Arc::new(Mutex::new(ConfigurationGate::new(initial.clone())));
    let owner = Uuid::new_v4();
    let lease = ConfigurationGate::begin(&gate, owner, initial.snapshot, 0)?;
    assert!(ConfigurationGate::begin(&gate, Uuid::new_v4(), initial.snapshot, 0).is_err());
    let mut candidate = initial.clone();
    candidate.settings.language = keelshell_core::Language::En;
    let result = store.save(&candidate);
    lease.finish(&result);
    let saved = result?;
    assert!(ConfigurationGate::begin(&gate, owner, initial.snapshot, 0).is_err());
    assert!(
        gate.lock()
            .map_err(|_| "gate poisoned")?
            .current(saved.snapshot, 1)
    );
    Ok(())
}

#[::core::prelude::v1::test]
fn configuration_abandoned_write_is_unknown_until_explicit_reload()
-> Result<(), Box<dyn std::error::Error>> {
    let state = AppState::default();
    let gate = Arc::new(Mutex::new(ConfigurationGate::new(state.clone())));
    drop(ConfigurationGate::begin(
        &gate,
        Uuid::new_v4(),
        state.snapshot,
        0,
    )?);
    {
        let shared = gate.lock().map_err(|_| "gate poisoned")?;
        assert!(!shared.current(state.snapshot, 0));
        assert!(shared.available());
    }
    let owner = Uuid::new_v4();
    let reload = ConfigurationGate::begin_reload_lease(&gate, owner)?;
    reload.finish(Some(&state));
    let shared = gate.lock().map_err(|_| "gate poisoned")?;
    assert!(shared.current(state.snapshot, 2));
    Ok(())
}

#[::core::prelude::v1::test]
fn abandoned_reload_releases_writer_and_requires_explicit_recovery()
-> Result<(), Box<dyn std::error::Error>> {
    let state = AppState::default();
    let gate = Arc::new(Mutex::new(ConfigurationGate::new(state.clone())));
    drop(ConfigurationGate::begin_reload_lease(
        &gate,
        Uuid::new_v4(),
    )?);
    {
        let shared = gate.lock().map_err(|_| "gate poisoned")?;
        assert!(shared.available());
        assert!(!shared.current(state.snapshot, 0));
    }
    let reload = ConfigurationGate::begin_reload_lease(&gate, Uuid::new_v4())?;
    reload.finish(Some(&state));
    assert!(
        gate.lock()
            .map_err(|_| "gate poisoned")?
            .current(state.snapshot, 2)
    );
    Ok(())
}

#[::core::prelude::v1::test]
fn configuration_conflict_never_retries_or_reopens_old_authority()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = StateStore::new(directory.path().join("state.json"));
    let original = store.save(&AppState::default())?;
    let gate = Arc::new(Mutex::new(ConfigurationGate::new(original.clone())));
    let lease = ConfigurationGate::begin(&gate, Uuid::new_v4(), original.snapshot, 0)?;
    let competitor = StateStore::new(store.path().to_owned());
    let mut external = competitor.load()?;
    external.settings.language = keelshell_core::Language::En;
    let external = competitor.save(&external)?;
    let result = store.save(&original);
    assert!(matches!(result, Err(keelshell_core::Error::Conflict)));
    lease.finish(&result);
    assert!(ConfigurationGate::begin(&gate, Uuid::new_v4(), original.snapshot, 0).is_err());
    assert_eq!(
        competitor.load()?.settings.language,
        external.settings.language
    );
    Ok(())
}

#[::core::prelude::v1::test]
fn maintenance_lease_excludes_new_windows_writes_and_reload()
-> Result<(), Box<dyn std::error::Error>> {
    let state = AppState::default();
    let gate = Arc::new(Mutex::new(ConfigurationGate::new(state.clone())));
    let owner = Uuid::new_v4();
    {
        let mut shared = gate.lock().map_err(|_| "gate poisoned")?;
        assert!(!shared.begin_maintenance(owner, 2));
        assert!(shared.begin_maintenance(owner, 1));
        assert!(!shared.available());
        shared.end_maintenance(Uuid::new_v4());
        assert!(!shared.available());
    }
    assert!(
        ConfigurationGate::begin_reload_lease(&gate, Uuid::new_v4()).is_err(),
        "maintenance must reject a reload lease"
    );
    assert!(ConfigurationGate::begin(&gate, owner, state.snapshot, 0).is_err());
    gate.lock()
        .map_err(|_| "gate poisoned")?
        .end_maintenance(owner);
    let lease = ConfigurationGate::begin(&gate, owner, state.snapshot, 0)?;
    lease.cancel_before_dispatch();
    assert!(
        gate.lock()
            .map_err(|_| "gate poisoned")?
            .current(state.snapshot, 0)
    );
    Ok(())
}
