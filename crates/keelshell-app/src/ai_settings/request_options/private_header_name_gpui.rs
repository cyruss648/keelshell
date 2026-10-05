#[gpui_kit::test]
fn independent_retained_header_name_cannot_enter_another_profile_delivery(cx: &mut TestAppContext) {
    const KNOWN: &str = "reviewer-retained-metadata-value";
    const SAFE: &str = "independently-approved-header-value";
    let (handle, panel) = mount_sized(cx, fixture_profile(), 900., 580.);
    cx.update_window(handle, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            for (index, value) in [KNOWN, "second-independent-retained-value"].into_iter().enumerate() {
                panel.add_request_header(window, cx);
                let id = panel.selected.unwrap_or_else(|| panic!("owned draft profile"));
                panel.request_editors[&id].headers[index].name.update(cx, |field, cx| {
                    field.set_value("x-duplicate", window, cx)
                });
                panel.sync_editor(cx);
                panel.clear_pending_request_fields(window, cx);
                put_value(panel, index, value, window, cx);
            }
            let owner = panel.selected.unwrap_or_else(|| panic!("owner"));
            let owner_profile = panel.profile().unwrap_or_else(|| panic!("owner profile")).clone();
            assert!(!panel.request_draft_valid(owner));
            assert!(crate::ai_request_options::resolve_options(&owner_profile, &panel.credentials).is_err());
            assert_eq!(panel.request_editors[&owner].headers[0].value.read(cx).value(), KNOWN);
            assert!(panel.request_editors[&owner].headers[0].value.read(cx).presentation().is_masked());
            assert!(panel.credentials.all_secrets().contains(&KNOWN));
            let mut active = fixture_profile();
            active.custom_headers.push(keelshell_core::AiCustomHeader {
                name: KNOWN.into(),
                value_ref: keelshell_core::AiSecretRef::Ephemeral { id: Uuid::new_v4() },
            });
            let active_id = active.id;
            panel.catalog.profiles.push(active);
            panel.select(active_id, window, cx);
            put_value(panel, 0, SAFE, window, cx);
            assert!(panel.request_draft_valid(active_id));
            assert!(panel.credentials.all_secrets().contains(&KNOWN));
            let active = panel.profile().unwrap_or_else(|| panic!("active profile"));
            let options = crate::ai_request_options::resolve_options(active, &panel.credentials)
                .unwrap_or_else(|error| panic!("active resolution: {error}"));
            let header_name_admitted = options.header_names().any(|name| name == KNOWN);
            let summary_hidden = !options.review_summary().contains(KNOWN);
            let provider = ProviderConfig::new(&active.endpoint, &active.model)
                .unwrap_or_else(|error| panic!("provider: {error}"))
                .with_request_options(options);
            let prepared = ContextDraft::new("ordinary owned prompt").prepare(&provider, &[], 4096);
            println!("gpui_header_metadata retained_masked=true owner_invalid=true active_valid=true known_pool_contains=true header_name_admitted={header_name_admitted} summary_hidden={summary_hidden} prepared_ok={} network_requests=0", prepared.is_ok());
            assert!(prepared.is_err(), "known retained metadata header name entered another profile review");
        });
    }).unwrap_or_else(|error| panic!("owned GPUI metadata admission: {error}"));
}
