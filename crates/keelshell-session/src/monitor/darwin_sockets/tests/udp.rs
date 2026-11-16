use super::*;

fn udp_file(fd: usize, family: &str, name: &str) -> String {
    format!("f{fd}\0t{family}\0PUDP\0n{name}\0\n")
}

fn unbound_body() -> String {
    format!(
        "{PROCESS}{}{}",
        udp_file(4, "IPv4", "*:*"),
        udp_file(5, "IPv6", "*:*")
    )
}

fn mixed_body() -> String {
    format!(
        "{PROCESS}{FILE}{}{}{}",
        udp_file(4, "IPv4", "127.0.0.1:5353"),
        udp_file(5, "IPv6", "[::1]:5354->[::1]:53"),
        udp_file(6, "IPv6", "*:*")
    )
}

fn mixed_rows() -> Vec<SocketInfo> {
    vec![
        SocketInfo {
            protocol: "tcp".into(),
            state: "LISTEN".into(),
            local: "127.0.0.1:2222".into(),
            peer: "*:*".into(),
            process: Some("fixture (pid=42)".into()),
        },
        SocketInfo {
            protocol: "udp".into(),
            state: "BOUND".into(),
            local: "127.0.0.1:5353".into(),
            peer: "*:*".into(),
            process: Some("fixture (pid=42)".into()),
        },
        SocketInfo {
            protocol: "udp".into(),
            state: "PEER".into(),
            local: "[::1]:5354".into(),
            peer: "[::1]:53".into(),
            process: Some("fixture (pid=42)".into()),
        },
    ]
}

fn udp_reply(body: &str) -> Reply {
    Reply {
        stdout: frame(body, 0),
        stderr: Vec::new(),
        status: Some(0),
    }
}

#[test]
fn darwin_udp_bound_addresses_preserve_families_and_normalize_wildcards() {
    for (family, name, local) in [
        ("IPv4", "127.0.0.1:1", "127.0.0.1:1"),
        ("IPv4", "0.0.0.0:65535", "0.0.0.0:65535"),
        ("IPv4", "*:5353", "0.0.0.0:5353"),
        ("IPv6", "[::1]:5353", "[::1]:5353"),
        ("IPv6", "[::]:65535", "[::]:65535"),
        ("IPv6", "*:5353", "[::]:5353"),
        ("IPv6", "[fe80::1%lo0]:5353", "[fe80::1%lo0]:5353"),
        ("IPv6", "127.0.0.1:5353", "127.0.0.1:5353"),
    ] {
        let body = format!("{PROCESS}{}", udp_file(4, family, name));
        assert_eq!(
            parse(output(&body, 0)).unwrap(),
            vec![SocketInfo {
                protocol: "udp".into(),
                state: "BOUND".into(),
                local: local.into(),
                peer: "*:*".into(),
                process: Some("fixture (pid=42)".into()),
            }],
            "{family} {name}"
        );
    }
}

#[test]
fn darwin_udp_peer_address_shape_preserves_both_endpoints_without_kernel_state() {
    for (family, name, local, peer) in [
        (
            "IPv4",
            "127.0.0.1:5353->192.0.2.7:53",
            "127.0.0.1:5353",
            "192.0.2.7:53",
        ),
        (
            "IPv4",
            "*:5353->192.0.2.7:65535",
            "0.0.0.0:5353",
            "192.0.2.7:65535",
        ),
        ("IPv6", "[::1]:5353->[::1]:53", "[::1]:5353", "[::1]:53"),
        (
            "IPv6",
            "[fe80::1%lo0]:5353->[fe80::2%lo0]:53",
            "[fe80::1%lo0]:5353",
            "[fe80::2%lo0]:53",
        ),
        (
            "IPv6",
            "127.0.0.1:5353->127.0.0.2:53",
            "127.0.0.1:5353",
            "127.0.0.2:53",
        ),
    ] {
        let body = format!("{PROCESS}{}", udp_file(4, family, name));
        assert_eq!(
            parse(output(&body, 0)).unwrap(),
            vec![SocketInfo {
                protocol: "udp".into(),
                state: "PEER".into(),
                local: local.into(),
                peer: peer.into(),
                process: Some("fixture (pid=42)".into()),
            }],
            "{family} {name}"
        );
    }
}

#[test]
fn darwin_udp_and_tcp_share_pid_and_accept_udp_field_order() {
    let body = mixed_body();
    assert_eq!(parse(output(&body, 0)).unwrap(), mixed_rows());
    let reordered = body.replace("tIPv4\0PUDP\0", "PUDP\0tIPv4\0");
    assert_eq!(parse(output(&reordered, 0)).unwrap(), mixed_rows());
}

#[test]
fn darwin_udp_nonempty_all_unbound_exit_zero_is_a_valid_empty_collection() {
    let body = format!(
        "{}p43\0cother\0\n{}",
        unbound_body(),
        udp_file(4, "IPv4", "*:*")
    );
    assert!(parse(output(&body, 0)).unwrap().is_empty());
    assert!(matches!(
        parse(output("", 0)),
        Err(MonitorError::InvalidData(_))
    ));
}

#[test]
fn darwin_udp_requires_each_of_four_fields_once_and_forbids_tcp_state() {
    for name in ["127.0.0.1:5353", "127.0.0.1:5353->192.0.2.7:53", "*:*"] {
        let file = udp_file(4, "IPv4", name);
        let body = format!("{PROCESS}{file}");
        let name_field = format!("n{name}\0");
        for field in ["f4\0", "tIPv4\0", "PUDP\0", name_field.as_str()] {
            assert_invalid(&body.replace(field, ""));
            assert_invalid(&body.replace(field, &format!("{field}{field}")));
        }
        for extra in [
            "TST=LISTEN\0",
            "TST=ESTABLISHED\0",
            "TQR=0\0",
            "p43\0",
            "cother\0",
            "u501\0",
        ] {
            let invalid = file.replace("\0\n", &format!("\0{extra}\n"));
            assert_invalid(&format!("{PROCESS}{invalid}"));
        }
    }
}

#[test]
fn darwin_udp_protocol_and_family_are_exact_even_for_unbound_objects() {
    for name in ["127.0.0.1:5353", "127.0.0.1:5353->192.0.2.7:53", "*:*"] {
        let body = format!("{PROCESS}{}", udp_file(4, "IPv4", name));
        for protocol in ["udp", "Udp", "TCP", "SCTP", ""] {
            assert_invalid(&body.replace("PUDP\0", &format!("P{protocol}\0")));
        }
        for family in ["ipv4", "IPV4", "ipv6", "IPv5", "unknown", ""] {
            assert_invalid(&body.replace("tIPv4\0", &format!("t{family}\0")));
        }
        for (field, replacement) in [("PUDP\0", "pUDP\0"), ("tIPv4\0", "TIPv4\0"), ("n", "N")] {
            assert_invalid(&body.replace(field, replacement));
        }
    }
}

#[test]
fn darwin_udp_rejects_malformed_endpoints_arrows_wildcard_peers_and_ports() {
    for (family, name) in [
        ("IPv4", "localhost:53"),
        ("IPv4", "999.1.2.3:53"),
        ("IPv4", "[::1]:53"),
        ("IPv4", "127.0.0.1"),
        ("IPv4", "127.0.0.1:53:54"),
        ("IPv4", "127.0.0.1:53->"),
        ("IPv4", "->192.0.2.7:53"),
        ("IPv4", "127.0.0.1:53->192.0.2.7:53->192.0.2.8:53"),
        ("IPv4", "127.0.0.1:53=>192.0.2.7:53"),
        ("IPv4", "127.0.0.1:53<-192.0.2.7:53"),
        ("IPv4", "127.0.0.1:53->localhost:53"),
        ("IPv4", "127.0.0.1:53->[::1]:53"),
        ("IPv4", "127.0.0.1:53->*:53"),
        ("IPv4", "127.0.0.1:53->*:*"),
        ("IPv4", "*:*->192.0.2.7:53"),
        ("IPv4", "*:* "),
        ("IPv6", "::1:53"),
        ("IPv6", "[::gg]:53"),
        ("IPv6", "[127.0.0.1]:53"),
        ("IPv6", "[::1%]:53"),
        ("IPv6", "[::1%lo 0]:53"),
        ("IPv6", "[::1%lo/0]:53"),
        ("IPv6", "[::1%lo0%other]:53"),
        ("IPv6", "[::1]:53->::1:53"),
        ("IPv6", "[::1]:53->[::1%bad/zone]:53"),
        ("IPv6", "[::1]:53->*:53"),
        ("IPv6", "[::1]:53->*:*"),
        ("IPv6", "*:*->[::1]:53"),
        ("IPv6", "*:*->*:*"),
    ] {
        assert_invalid(&format!("{PROCESS}{}", udp_file(4, family, name)));
    }
    for (family, local, peer) in [
        ("IPv4", "127.0.0.1", "192.0.2.7"),
        ("IPv6", "[::1]", "[::2]"),
    ] {
        for port in [
            "0",
            "-1",
            "+53",
            "65536",
            "4294967296",
            "domain",
            "*",
            "",
            "53 ",
            " 53",
        ] {
            for name in [
                format!("{local}:{port}"),
                format!("{local}:{port}->{peer}:53"),
                format!("{local}:5353->{peer}:{port}"),
            ] {
                assert_invalid(&format!("{PROCESS}{}", udp_file(4, family, &name)));
            }
        }
        for name in [
            "1".repeat(MAX_ENDPOINT + 1),
            format!("{local}:5353->{}", "1".repeat(MAX_ENDPOINT + 1)),
        ] {
            assert_invalid(&format!("{PROCESS}{}", udp_file(4, family, &name)));
        }
    }
}

#[test]
fn darwin_udp_unbound_filter_still_validates_process_and_descriptor() {
    for family in ["IPv4", "IPv6"] {
        let file = udp_file(4, family, "*:*");
        let body = format!("{PROCESS}{file}");
        assert_invalid(&file);
        for field in ["p42\0", "cfixture\0"] {
            assert_invalid(&body.replace(field, ""));
            assert_invalid(&body.replace(field, &format!("{field}{field}")));
        }
        for pid in ["0", "-1", "+42", "2147483648", "4294967296", "42x", " 42"] {
            assert_invalid(&body.replace("p42\0", &format!("p{pid}\0")));
        }
        for fd in ["", "-1", "+4", "2147483648", "cwd", "4u"] {
            assert_invalid(&body.replace("f4\0", &format!("f{fd}\0")));
        }
        for command in ["", "secret\u{001b}[31m", "line\rbreak"] {
            assert_invalid(&body.replace("cfixture\0", &format!("c{command}\0")));
        }
        assert_invalid(&body.replace("fixture", &"c".repeat(MAX_COMMAND + 1)));
        assert_invalid(&format!("{body}p43\0cother\0\n"));
    }
}

#[test]
fn darwin_udp_unbound_objects_do_not_hide_duplicate_pid_or_descriptor() {
    let unbound = udp_file(4, "IPv4", "*:*");
    let bound = udp_file(4, "IPv4", "127.0.0.1:5353");
    let tcp = FILE.replacen("f3\0", "f4\0", 1);
    for body in [
        format!("{PROCESS}{unbound}{unbound}"),
        format!("{PROCESS}{unbound}{bound}"),
        format!("{PROCESS}{bound}{unbound}"),
        format!("{PROCESS}{unbound}{tcp}"),
        format!("{PROCESS}{tcp}{unbound}"),
        format!("{PROCESS}{unbound}{}", udp_file(4, "IPv6", "*:*")),
        format!("{PROCESS}{unbound}{PROCESS}{}", udp_file(5, "IPv6", "*:*")),
        format!(
            "{PROCESS}{unbound}p042\0cother\0\n{}",
            udp_file(5, "IPv4", "*:*")
        ),
    ] {
        assert_invalid(&body);
    }
}

#[test]
fn darwin_udp_unbound_filter_never_silently_discards_invalid_mixed_records() {
    let unbound = udp_file(4, "IPv4", "*:*");
    for invalid in [
        FILE.replace("127.0.0.1:2222", "*:*"),
        FILE.replace("127.0.0.1:2222", "127.0.0.1:0"),
        FILE.replace("TST=LISTEN\0", ""),
        udp_file(5, "unknown", "*:*"),
        udp_file(5, "IPv4", "*:0"),
        udp_file(5, "IPv4", "*:*->192.0.2.7:53"),
        udp_file(5, "IPv6", "[::1]:5353->*:*"),
        udp_file(5, "IPv4", "*:*").replace("PUDP\0", "PUDP\0TST=LISTEN\0"),
    ] {
        assert_invalid(&format!("{PROCESS}{unbound}{invalid}"));
        assert_invalid(&format!("{PROCESS}{invalid}{unbound}"));
    }
}

#[test]
fn darwin_udp_truncation_encoding_and_failed_status_never_become_empty_success() {
    for name in ["127.0.0.1:5353", "127.0.0.1:5353->192.0.2.7:53", "*:*"] {
        let body = format!("{PROCESS}{}", udp_file(4, "IPv4", name));
        for end in 0..body.len() {
            assert_invalid(&body[..end]);
        }
        let framed = frame(&body, 0);
        for end in 0..framed.len() {
            let mut truncated = output(&body, 0);
            truncated.stdout = framed[..end].to_vec();
            assert!(matches!(
                parse(truncated),
                Err(MonitorError::InvalidData(_))
            ));
        }
        let mut invalid_utf8 = output(&body, 0);
        invalid_utf8.stdout[START.len() + body.find("\0n").unwrap() + 2] = 0xff;
        assert!(matches!(
            parse(invalid_utf8),
            Err(MonitorError::InvalidData(_))
        ));
        let mut legacy = output(&body, 0);
        legacy.stdout = format!("@@KS:darwin-sockets:1@@\n{body}{STATUS}0{END}").into_bytes();
        assert!(matches!(parse(legacy), Err(MonitorError::InvalidData(_))));
        for status in [1, 2, 126, 127] {
            assert!(matches!(
                parse(output(&body, status)),
                Err(MonitorError::CapabilityUnavailable { .. })
            ));
        }
        for status in [None, Some(1), Some(64), Some(66), Some(255)] {
            let mut failed = output(&body, 0);
            failed.exit_status = status;
            assert!(matches!(
                parse(failed),
                Err(MonitorError::RemoteFailure { exit_status, .. }) if exit_status == status
            ));
        }
    }
}

#[test]
fn darwin_udp_descriptor_budget_includes_filtered_objects_across_processes() {
    let mut body = PROCESS.to_owned();
    for fd in 0..10_000 {
        if fd == 5_000 {
            body.push_str("p43\0cother\0\n");
        }
        let family = if fd % 2 == 0 { "IPv4" } else { "IPv6" };
        body.push_str(&udp_file(fd, family, "*:*"));
    }
    assert!(parse(output(&body, 0)).unwrap().is_empty());
    let tcp = FILE.replacen("f3\0", "f0\0", 1);
    let mixed = body.replacen(&udp_file(0, "IPv4", "*:*"), &tcp, 1);
    assert_eq!(
        parse(output(&mixed, 0)).unwrap(),
        vec![SocketInfo {
            protocol: "tcp".into(),
            state: "LISTEN".into(),
            local: "127.0.0.1:2222".into(),
            peer: "*:*".into(),
            process: Some("fixture (pid=42)".into()),
        }]
    );
    for mut at_limit in [body, mixed] {
        at_limit.push_str(&udp_file(10_000, "IPv4", "*:*"));
        assert_invalid(&at_limit);
    }
}

#[tokio::test]
async fn ssh_darwin_udp_mixed_and_all_unbound_results_are_available_and_refetched()
-> Result<(), Box<dyn std::error::Error>> {
    let (fixture, monitor) = fixture(udp_reply(&mixed_body())).await?;
    assert_eq!(monitor.listening_sockets().await?, mixed_rows());
    assert_eq!(fixture.control.calls.load(Ordering::Acquire), 1);
    *fixture.control.reply.lock().unwrap() = udp_reply(&unbound_body());
    assert!(monitor.listening_sockets().await?.is_empty());
    assert_eq!(fixture.control.calls.load(Ordering::Acquire), 2);
    let current = monitor.capabilities().await;
    assert_eq!(current.sockets.state, CapabilityState::Available);
    assert_eq!(current.resources.state, CapabilityState::Unknown);
    assert_eq!(current.filesystems.state, CapabilityState::Unknown);
    assert_eq!(current.processes.state, CapabilityState::Unknown);
    assert!(!fixture.control.unexpected_command.load(Ordering::Acquire));
    monitor.session.close().await?;
    Ok(())
}

#[tokio::test]
async fn ssh_darwin_udp_parser_and_command_failures_are_cached_until_reset()
-> Result<(), Box<dyn std::error::Error>> {
    let body = mixed_body();
    let malformed = udp_reply(&body.replace("[::1]:5354->[::1]:53", "[::1]:5354->*:*"));
    let mut permission = udp_reply(&body);
    permission.stderr = b"permission denied: PRIVATE_REMOTE_TEXT".to_vec();
    let mut unsupported = udp_reply(&body);
    unsupported.stderr = b"unknown option: PRIVATE_REMOTE_TEXT".to_vec();
    let mut failed = udp_reply(&body);
    failed.stdout = frame(&body, 2);
    let (fixture, monitor) = fixture(udp_reply(&body)).await?;
    for (reply, reason) in [
        (malformed, CapabilityReason::InvalidOutput),
        (permission, CapabilityReason::PermissionDenied),
        (unsupported, CapabilityReason::UnsupportedOptions),
        (failed, CapabilityReason::CommandFailed),
    ] {
        monitor.reset_capabilities().await;
        let before = fixture.control.calls.load(Ordering::Acquire);
        *fixture.control.reply.lock().unwrap() = reply;
        let error = monitor.listening_sockets().await.unwrap_err();
        if reason == CapabilityReason::InvalidOutput {
            assert!(matches!(&error, MonitorError::InvalidData(_)), "{error:?}");
        } else {
            assert!(
                matches!(
                    &error,
                    MonitorError::CapabilityUnavailable {
                        capability: Capability::Sockets,
                        reason: actual,
                    } if *actual == reason
                ),
                "{error:?}"
            );
        }
        assert!(!error.to_string().contains("PRIVATE_REMOTE_TEXT"));
        let previous = monitor.capability_report().await;
        assert_eq!(
            previous.observations().sockets.state,
            CapabilityState::Unavailable(reason)
        );
        *fixture.control.reply.lock().unwrap() = udp_reply(&body);
        for _ in 0..2 {
            assert!(matches!(
                monitor.listening_sockets().await,
                Err(MonitorError::CapabilityUnavailable {
                    capability: Capability::Sockets,
                    reason: actual,
                }) if actual == reason
            ));
        }
        assert_eq!(fixture.control.calls.load(Ordering::Acquire), before + 1);
        assert_eq!(
            monitor.capabilities().await.sockets,
            previous.observations().sockets
        );
        monitor.reset_capabilities().await;
        assert!(!monitor.accepts_capability_report(&previous));
        assert_eq!(
            monitor.capabilities().await.sockets.state,
            CapabilityState::Unknown
        );
        assert_eq!(monitor.listening_sockets().await?, mixed_rows());
        assert_eq!(fixture.control.calls.load(Ordering::Acquire), before + 2);
        assert_eq!(
            monitor.capabilities().await.sockets.state,
            CapabilityState::Available
        );
    }
    assert!(!fixture.control.unexpected_command.load(Ordering::Acquire));
    monitor.session.close().await?;
    Ok(())
}

#[tokio::test]
async fn ssh_darwin_udp_combined_output_overflow_is_transport_and_retries_without_reset()
-> Result<(), Box<dyn std::error::Error>> {
    let body = mixed_body();
    let mut oversized = udp_reply(&body);
    oversized.stderr = vec![b'x'; MAX_OUTPUT];
    let (fixture, monitor) = fixture(oversized).await?;
    assert!(matches!(
        monitor.listening_sockets().await,
        Err(MonitorError::Session(crate::SessionError::OutputLimit(limit))) if limit == MAX_OUTPUT
    ));
    assert_eq!(
        monitor.capabilities().await.sockets.state,
        CapabilityState::Unavailable(CapabilityReason::Transport)
    );
    *fixture.control.reply.lock().unwrap() = udp_reply(&body);
    assert_eq!(monitor.listening_sockets().await?, mixed_rows());
    assert_eq!(fixture.control.calls.load(Ordering::Acquire), 2);
    assert_eq!(
        monitor.capabilities().await.sockets.state,
        CapabilityState::Available
    );
    assert!(!fixture.control.unexpected_command.load(Ordering::Acquire));
    monitor.session.close().await?;
    Ok(())
}
