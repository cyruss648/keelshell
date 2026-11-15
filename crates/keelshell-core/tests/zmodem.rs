#![expect(clippy::unwrap_used, reason = "test fixtures and assertions")]

use std::collections::VecDeque;

use keelshell_core::zmodem::{
    DetectEvent, Detector, Error, Event, FileOffer, FrameEnd, FrameType, Header, HeaderFormat,
    MAX_FEED_BYTES, MAX_FILE_BYTES, MAX_FILES, MAX_RETRIES, MAX_SUBPACKET_BYTES, Receiver, Role,
    SendFile, Sender, encode_data, encode_header, validate_basename,
};

fn header(kind: FrameType, position: u32) -> Vec<u8> {
    encode_header(Header::new(kind, position, HeaderFormat::Hex))
}

fn file_header(format: HeaderFormat) -> Vec<u8> {
    encode_header(Header::new(FrameType::File, 1 << 24, format))
}

fn offer_bytes(name: &str, size: u64, format: HeaderFormat) -> Vec<u8> {
    metadata_bytes(format!("{name}\0{size} 0 100644 0\0").as_bytes(), format)
}

fn metadata_bytes(metadata: &[u8], format: HeaderFormat) -> Vec<u8> {
    let mut bytes = file_header(format);
    bytes.extend(encode_data(metadata, FrameEnd::EndAck, format).unwrap());
    bytes
}

fn frame(position: u32, bytes: &[u8], end: FrameEnd, format: HeaderFormat) -> Vec<u8> {
    let mut wire = encode_header(Header::new(FrameType::Data, position, format));
    wire.extend(encode_data(bytes, end, format).unwrap());
    wire
}

fn wire(events: Vec<Event>) -> Vec<u8> {
    events
        .into_iter()
        .filter_map(|event| match event {
            Event::Send(bytes) => Some(bytes),
            _ => None,
        })
        .flatten()
        .collect()
}

fn receiver() -> Receiver {
    let mut receiver = Receiver::new();
    receiver.start().unwrap();
    receiver
}

fn sender(bytes: &[u8]) -> Sender {
    let mut sender = Sender::new(vec![SendFile::new("data.bin", bytes.to_vec()).unwrap()]).unwrap();
    sender.start().unwrap();
    sender
        .feed(&header(FrameType::ReceiverInit, 0x2300_0000))
        .unwrap();
    sender
}

fn accepted(name: &str, size: u64, format: HeaderFormat) -> Receiver {
    let mut receiver = receiver();
    let events = receiver.feed(&offer_bytes(name, size, format)).unwrap();
    assert_eq!(
        events,
        vec![Event::Offer(FileOffer {
            name: name.to_owned(),
            size,
        })]
    );
    receiver.accept().unwrap();
    receiver
}

fn complete(receiver: &mut Receiver, bytes: &[u8], format: HeaderFormat) -> Vec<Event> {
    let mut output = receiver
        .feed(&frame(0, bytes, FrameEnd::End, format))
        .unwrap();
    output.extend(
        receiver
            .feed(&header(FrameType::EndOfFile, bytes.len() as u32))
            .unwrap(),
    );
    output
}

#[test]
fn hex_header_matches_independent_standard_crc16_vector() {
    assert_eq!(
        header(FrameType::ReceiverInit, 0x2300_0000),
        b"**\x18B0100000023be50\r\n\x11"
    );
}

#[test]
fn binary16_header_matches_independently_calculated_vector() {
    assert_eq!(
        encode_header(Header::new(FrameType::Data, 0, HeaderFormat::Binary16)),
        b"*\x18A\x18J\x18@\x18@\x18@\x18@F\xae"
    );
}

#[test]
fn binary32_header_matches_independently_calculated_vector() {
    assert_eq!(
        encode_header(Header::new(FrameType::Data, 0, HeaderFormat::Binary32)),
        b"*\x18C\x18J\x18@\x18@\x18@\x18@\xbc\xef\x18\xd2\x18\xcc"
    );
}

#[test]
fn crc16_data_includes_the_terminator_in_the_checksum() {
    assert_eq!(
        encode_data(b"123456789", FrameEnd::EndAck, HeaderFormat::Binary16).unwrap(),
        b"123456789\x18k8\xbf\x11"
    );
}

#[test]
fn crc32_data_uses_complemented_little_endian_checksum() {
    assert_eq!(
        encode_data(b"123456789", FrameEnd::EndAck, HeaderFormat::Binary32).unwrap(),
        b"123456789\x18k\x18\xd9\x26\xa4\xda\x11"
    );
}

#[test]
fn receiver_decodes_independent_hex_metadata_and_binary16_data_wire_vectors() {
    // CRCs independently checked with Python binascii.crc_hqx, not these
    // production encoders. This remains a golden-vector test, not rz/sz QA.
    let mut receiver = receiver();
    let metadata = b"**\x18B04000000019927\r\n\x11a\x18@3 0 100644\x18@\x18k\xa5\xb6\x11";
    assert_eq!(
        receiver.feed(metadata).unwrap(),
        vec![Event::Offer(FileOffer {
            name: "a".to_owned(),
            size: 3
        })]
    );
    receiver.accept().unwrap();
    receiver
        .feed(b"*\x18A\x18J\x18@\x18@\x18@\x18@F\xaeabc\x18hi\xba")
        .unwrap();
    assert!(matches!(
        receiver
            .feed(&header(FrameType::EndOfFile, 3))
            .unwrap()
            .as_slice(),
        [Event::FileReceived { bytes, .. }] if bytes == b"abc"
    ));
}

#[test]
fn detector_preserves_plain_text_and_incomplete_literal_signatures() {
    let mut detector = Detector::new();
    let plain = b"rz\r\nB00 **\\x18B00000000000000\n$ ";
    assert_eq!(
        detector.feed(plain).unwrap(),
        vec![DetectEvent::Terminal(plain.to_vec())]
    );
    assert!(detector.flush().is_empty());
}

#[test]
fn detector_detects_every_split_of_a_valid_start_without_losing_suffix() {
    let protocol = header(FrameType::RequestInit, 0);
    for split in 0..protocol.len() - 1 {
        let mut detector = Detector::new();
        let mut output = detector.feed(b"ordinary\r\n").unwrap();
        output.extend(detector.feed(&protocol[..split]).unwrap());
        let mut rest = protocol[split..].to_vec();
        rest.extend_from_slice(b"metadata-after-the-header");
        output.extend(detector.feed(&rest).unwrap());
        let mut terminal = Vec::new();
        let mut found = Vec::new();
        let mut started = false;
        for event in output {
            match event {
                DetectEvent::Terminal(bytes) => terminal.extend(bytes),
                DetectEvent::Start { role, bytes } => {
                    assert_eq!(role, Role::Receive);
                    assert!(!started);
                    started = true;
                    found.extend(bytes);
                }
            }
        }
        assert_eq!(terminal, b"ordinary\r\n");
        assert!(started);
        assert_eq!(
            found,
            [protocol.as_slice(), b"metadata-after-the-header"].concat()
        );
    }
}

#[test]
fn detector_fragmented_prefix_includes_original_prefix_and_entire_suffix() {
    let mut detector = Detector::new();
    let start = header(FrameType::RequestInit, 0);
    assert_eq!(
        detector.feed(&[b'x', start[0]]).unwrap(),
        vec![DetectEvent::Terminal(vec![b'x'])]
    );
    let mut suffix = start[1..].to_vec();
    suffix.extend_from_slice(b"first-file-metadata");
    let mut all = start.clone();
    all.extend_from_slice(b"first-file-metadata");
    assert_eq!(
        detector.feed(&suffix).unwrap(),
        vec![DetectEvent::Start {
            role: Role::Receive,
            bytes: all,
        }]
    );
}

#[test]
fn detector_invalid_crc_is_terminal_output_in_original_order() {
    let mut detector = Detector::new();
    let mut invalid = header(FrameType::RequestInit, 0);
    invalid[8] = b'1';
    let mut output = Vec::new();
    for chunk in invalid.chunks(3) {
        for event in detector.feed(chunk).unwrap() {
            match event {
                DetectEvent::Terminal(bytes) => output.extend(bytes),
                DetectEvent::Start { .. } => panic!("corrupt CRC triggered transfer"),
            }
        }
    }
    output.extend(detector.flush());
    assert_eq!(output, invalid);
}

#[test]
fn detector_flush_returns_a_fragment_exactly_and_reset_allows_reuse() {
    let mut detector = Detector::new();
    assert!(detector.feed(b"**\x18B00").unwrap().is_empty());
    assert_eq!(detector.flush(), b"**\x18B00");
    assert!(matches!(
        detector
            .feed(&header(FrameType::ReceiverInit, 0x2300_0000))
            .unwrap()
            .as_slice(),
        [DetectEvent::Start {
            role: Role::Send,
            ..
        }]
    ));
    assert!(matches!(detector.feed(b"x"), Err(Error::WrongState(_))));
    assert!(detector.reset().is_empty());
    assert_eq!(
        detector.feed(b"x").unwrap(),
        vec![DetectEvent::Terminal(vec![b'x'])]
    );
}

#[test]
fn detector_binary_headers_are_crc_validated_too() {
    for format in [HeaderFormat::Binary16, HeaderFormat::Binary32] {
        let mut detector = Detector::new();
        let mut bytes = encode_header(Header::new(FrameType::RequestInit, 0, format));
        bytes.extend_from_slice(b"suffix");
        assert_eq!(
            detector.feed(&bytes).unwrap(),
            vec![DetectEvent::Start {
                role: Role::Receive,
                bytes
            }]
        );
    }
}

#[test]
fn detector_noninit_header_is_preserved_then_a_later_start_is_detected() {
    let plain = header(FrameType::Ack, 0);
    let start = header(FrameType::RequestInit, 0);
    let mut detector = Detector::new();
    let mut bytes = plain.clone();
    bytes.extend_from_slice(&start);
    assert_eq!(
        detector.feed(&bytes).unwrap(),
        vec![
            DetectEvent::Terminal(plain),
            DetectEvent::Start {
                role: Role::Receive,
                bytes: start
            }
        ]
    );
}

#[test]
fn detector_retains_at_most_a_small_prefix_during_arbitrary_noise() {
    let mut detector = Detector::new();
    let input = [b'*'; MAX_FEED_BYTES];
    let mut terminal = Vec::new();
    for event in detector.feed(&input).unwrap() {
        if let DetectEvent::Terminal(bytes) = event {
            terminal.extend(bytes);
        } else {
            panic!("noise triggered transfer");
        }
    }
    let pending = detector.flush();
    assert!(pending.len() <= 96);
    terminal.extend(pending);
    assert_eq!(terminal, input);
}

#[test]
fn receiver_start_advertises_bounded_buffer_crc32_and_control_escaping() {
    let mut receiver = Receiver::new();
    assert_eq!(
        wire(receiver.start().unwrap()),
        header(FrameType::ReceiverInit, 0x6000_0400)
    );
    assert!(matches!(receiver.start(), Err(Error::WrongState(_))));
}

#[test]
fn receiver_offers_then_pauses_without_zrpos_until_explicit_accept() {
    let mut receiver = receiver();
    let events = receiver
        .feed(&offer_bytes("资料.bin", 3, HeaderFormat::Binary32))
        .unwrap();
    assert_eq!(
        events,
        vec![Event::Offer(FileOffer {
            name: "资料.bin".to_owned(),
            size: 3
        })]
    );
    assert!(receiver.is_waiting_for_accept());
    assert!(receiver.timeout().unwrap().is_empty());
    assert_eq!(
        wire(receiver.accept().unwrap()),
        header(FrameType::Position, 0)
    );
    assert!(!receiver.is_waiting_for_accept());
}

#[test]
fn receiver_rejects_data_before_approval_even_in_the_metadata_feed() {
    let mut receiver = receiver();
    let mut premature = offer_bytes("a", 1, HeaderFormat::Binary32);
    premature.extend(frame(0, b"x", FrameEnd::End, HeaderFormat::Binary32));
    assert_eq!(receiver.feed(&premature), Err(Error::ApprovalRequired));
    assert!(matches!(receiver.accept(), Err(Error::WrongState(_))));
    assert!(matches!(
        receiver.cancel().unwrap().as_slice(),
        [Event::Send(_), Event::Cancelled]
    ));
}

#[test]
fn receiver_crc_and_size_verified_bytes_pause_until_saved_acknowledgement() {
    let mut receiver = accepted("x", 3, HeaderFormat::Binary32);
    let events = complete(&mut receiver, b"\0\xffx", HeaderFormat::Binary32);
    assert!(events.iter().any(|event| matches!(
        event,
        Event::FileReceived { offer, bytes }
        if offer.name == "x" && offer.size == 3 && bytes == b"\0\xffx"
    )));
    assert!(wire(events).is_empty());
    assert!(receiver.is_waiting_for_saved());
    assert!(receiver.timeout().unwrap().is_empty());
    assert!(
        receiver
            .feed(&header(FrameType::EndOfFile, 3))
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        wire(receiver.acknowledge_saved().unwrap()),
        header(FrameType::ReceiverInit, 0x6000_0400)
    );
    assert!(!receiver.is_waiting_for_saved());
}

#[test]
fn receiver_new_file_before_save_acknowledgement_is_forbidden() {
    let mut receiver = accepted("first", 0, HeaderFormat::Binary16);
    receiver.feed(&header(FrameType::EndOfFile, 0)).unwrap();
    assert_eq!(
        receiver.feed(&offer_bytes("second", 1, HeaderFormat::Binary16)),
        Err(Error::ApprovalRequired)
    );
}

#[test]
fn receiver_repeated_identical_metadata_does_not_repeat_or_reset_approval() {
    let mut receiver = receiver();
    let metadata = offer_bytes("same", 2, HeaderFormat::Binary16);
    receiver.feed(&metadata).unwrap();
    assert!(receiver.feed(&metadata).unwrap().is_empty());
    receiver.accept().unwrap();
    assert_eq!(
        wire(receiver.feed(&metadata).unwrap()),
        header(FrameType::Position, 0)
    );
}

#[test]
fn receiver_changed_metadata_cannot_inherit_a_prior_review() {
    let mut receiver = receiver();
    receiver
        .feed(&offer_bytes("one", 2, HeaderFormat::Binary32))
        .unwrap();
    assert_eq!(
        receiver.feed(&offer_bytes("two", 2, HeaderFormat::Binary32)),
        Err(Error::ApprovalRequired)
    );
}

#[test]
fn receiver_reject_skips_only_the_current_file_and_next_offer_needs_review() {
    let mut receiver = receiver();
    receiver
        .feed(&offer_bytes("one", 0, HeaderFormat::Binary32))
        .unwrap();
    let events = receiver.reject().unwrap();
    assert!(matches!(events.first(), Some(Event::FileSkipped(_))));
    assert_eq!(wire(events), header(FrameType::Skip, 0));
    assert!(matches!(receiver.accept(), Err(Error::WrongState(_))));
    assert!(matches!(
        receiver
            .feed(&offer_bytes("two", 0, HeaderFormat::Binary32))
            .unwrap()
            .as_slice(),
        [Event::Offer(_)]
    ));
}

#[test]
fn receiver_repeats_a_lost_skip_without_reoffering_or_creating_a_name_alias() {
    let mut receiver = receiver();
    let offer = offer_bytes("skip", 1, HeaderFormat::Binary32);
    receiver.feed(&offer).unwrap();
    receiver.reject().unwrap();
    assert_eq!(
        receiver.feed(&offer).unwrap(),
        vec![Event::Send(header(FrameType::Skip, 0))]
    );
    assert!(!receiver.is_waiting_for_accept());
}

#[test]
fn receiver_supports_all_header_formats_in_single_byte_transport_fragments() {
    for format in [
        HeaderFormat::Hex,
        HeaderFormat::Binary16,
        HeaderFormat::Binary32,
    ] {
        let mut receiver = receiver();
        let mut offer_events = Vec::new();
        for byte in offer_bytes("café.txt", 3, format) {
            offer_events.extend(receiver.feed(&[byte]).unwrap());
        }
        assert!(matches!(offer_events.as_slice(), [Event::Offer(_)]));
        receiver.accept().unwrap();
        let mut content = frame(0, b"abc", FrameEnd::End, format);
        content.extend(header(FrameType::EndOfFile, 3));
        let mut events = Vec::new();
        for byte in content {
            events.extend(receiver.feed(&[byte]).unwrap());
        }
        assert!(events.iter().any(|event| matches!(
            event, Event::FileReceived { bytes, .. } if bytes == b"abc"
        )));
    }
}

#[test]
fn receiver_preserves_every_byte_value_through_control_and_rubout_escapes() {
    let bytes: Vec<_> = (0_u8..=255).cycle().take(2048).collect();
    for format in [HeaderFormat::Binary16, HeaderFormat::Binary32] {
        let mut receiver = accepted("binary", bytes.len() as u64, format);
        let events = complete(&mut receiver, &bytes, format);
        assert!(events.iter().any(|event| matches!(
            event, Event::FileReceived { bytes: actual, .. } if actual == &bytes
        )));
    }
}

#[test]
fn receiver_zcrcg_zcrcq_zcrcw_and_zcrce_have_distinct_ack_semantics() {
    let format = HeaderFormat::Binary32;
    let mut receiver = accepted("flow", 4, format);
    assert!(
        wire(
            receiver
                .feed(&frame(0, b"a", FrameEnd::Continue, format))
                .unwrap()
        )
        .is_empty()
    );
    assert_eq!(
        wire(
            receiver
                .feed(&encode_data(b"b", FrameEnd::ContinueAck, format).unwrap())
                .unwrap()
        ),
        header(FrameType::Ack, 2)
    );
    assert_eq!(
        wire(
            receiver
                .feed(&encode_data(b"c", FrameEnd::EndAck, format).unwrap())
                .unwrap()
        ),
        header(FrameType::Ack, 3)
    );
    assert!(
        wire(
            receiver
                .feed(&frame(3, b"d", FrameEnd::End, format))
                .unwrap()
        )
        .is_empty()
    );
    assert!(matches!(
        receiver
            .feed(&header(FrameType::EndOfFile, 4))
            .unwrap()
            .as_slice(),
        [Event::FileReceived { bytes, .. }] if bytes == b"abcd"
    ));
}

#[test]
fn receiver_corrupt_data_recovers_at_last_crc_verified_position() {
    let format = HeaderFormat::Binary32;
    let mut receiver = accepted("retry", 6, format);
    receiver
        .feed(&frame(0, b"abc", FrameEnd::EndAck, format))
        .unwrap();
    let mut bad = frame(3, b"def", FrameEnd::EndAck, format);
    let data_index = bad.iter().position(|byte| *byte == b'd').unwrap();
    bad[data_index] = b'x';
    let events = receiver.feed(&bad).unwrap();
    assert!(events.contains(&Event::Retry { position: Some(3) }));
    assert_eq!(wire(events), header(FrameType::Position, 3));
    receiver
        .feed(&frame(3, b"def", FrameEnd::End, format))
        .unwrap();
    assert!(matches!(
        receiver
            .feed(&header(FrameType::EndOfFile, 6))
            .unwrap()
            .as_slice(),
        [Event::FileReceived { bytes, .. }] if bytes == b"abcdef"
    ));
}

#[test]
fn receiver_corrupt_header_never_becomes_an_offer() {
    let mut receiver = receiver();
    let mut corrupt = header(FrameType::File, 0);
    corrupt[5] = b'5';
    let events = receiver.feed(&corrupt).unwrap();
    assert!(events.contains(&Event::Retry { position: None }));
    assert!(!events.iter().any(|event| matches!(event, Event::Offer(_))));
    assert!(receiver.pending_offer().is_none());
}

#[test]
fn receiver_wrong_data_position_requests_contiguous_offset_without_appending() {
    let format = HeaderFormat::Binary16;
    let mut receiver = accepted("position", 3, format);
    let events = receiver
        .feed(&frame(1, b"abc", FrameEnd::EndAck, format))
        .unwrap();
    assert_eq!(wire(events), header(FrameType::Position, 0));
    assert!(
        complete(&mut receiver, b"abc", format)
            .iter()
            .any(|event| matches!(
                event, Event::FileReceived { bytes, .. } if bytes == b"abc"
            ))
    );
}

#[test]
fn receiver_short_or_mismatched_eof_never_emits_complete_bytes() {
    let format = HeaderFormat::Binary32;
    let mut receiver = accepted("short", 5, format);
    receiver
        .feed(&frame(0, b"abc", FrameEnd::End, format))
        .unwrap();
    let events = receiver.feed(&header(FrameType::EndOfFile, 3)).unwrap();
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::FileReceived { .. }))
    );
    assert_eq!(wire(events), header(FrameType::Position, 3));
    receiver
        .feed(&frame(3, b"de", FrameEnd::End, format))
        .unwrap();
    assert!(matches!(
        receiver
            .feed(&header(FrameType::EndOfFile, 5))
            .unwrap()
            .as_slice(),
        [Event::FileReceived { bytes, .. }] if bytes == b"abcde"
    ));
}

#[test]
fn receiver_data_exceeding_declared_length_fails_closed() {
    let format = HeaderFormat::Binary16;
    let mut receiver = accepted("long", 1, format);
    assert_eq!(
        receiver.feed(&frame(0, b"ab", FrameEnd::End, format)),
        Err(Error::InvalidPosition)
    );
    assert!(matches!(
        receiver.feed(&header(FrameType::EndOfFile, 1)),
        Err(Error::WrongState(_))
    ));
}

#[test]
fn receiver_requires_declared_size_and_valid_regular_mode() {
    for bad in [
        b"x\0\0".as_slice(),
        b"x\0-1\0",
        b"x\0+1\0",
        b"x\x0018446744073709551616\0",
        b"x\x001 0 120777\0",
        b"x\x001 0 40755\0",
        b"x\x001 0 10644\0",
        b"x\x001 0 100000000100644\0",
        b"x\x001 0 100644\0hidden",
    ] {
        let mut receiver = receiver();
        assert_eq!(
            receiver.feed(&metadata_bytes(bad, HeaderFormat::Binary32)),
            Err(Error::InvalidMetadata),
            "{bad:?}"
        );
    }
}

#[test]
fn receiver_rejects_non_utf8_name_and_unbounded_unterminated_metadata() {
    let mut receiver = receiver();
    assert_eq!(
        receiver.feed(&metadata_bytes(
            b"\xff\0\x001 0 100644\0",
            HeaderFormat::Binary32
        )),
        Err(Error::InvalidMetadata)
    );
    let mut receiver = Receiver::new();
    receiver.start().unwrap();
    let mut incomplete = file_header(HeaderFormat::Binary32);
    incomplete.extend(vec![b'a'; MAX_SUBPACKET_BYTES + 1]);
    assert_eq!(
        receiver.feed(&incomplete),
        Err(Error::Limit("subpacket bytes"))
    );
}

#[test]
fn portable_basename_policy_rejects_paths_devices_controls_and_alias_primitives() {
    for bad in [
        "",
        ".",
        "..",
        "../x",
        "a/b",
        "a\\b",
        "C:x",
        "x:stream",
        "nul.txt",
        "COM1",
        "lpt¹.log",
        "CONIN$",
        "clock$",
        "x.",
        "x ",
        " x",
        "a\nb",
        "a\u{202e}b",
        "e\u{301}.txt",
        "a?b",
        "a*b",
        "a|b",
    ] {
        assert!(validate_basename(bad).is_err(), "{bad:?}");
    }
    for good in [
        "资料.bin",
        "café.txt",
        "한국어.txt",
        "résumé 2026.txt",
        ";$(id).txt",
    ] {
        validate_basename(good).unwrap();
    }
}

#[test]
fn receiver_batch_aliases_and_rejected_offers_still_count() {
    let mut receiver = receiver();
    receiver
        .feed(&offer_bytes("Straße.txt", 0, HeaderFormat::Binary16))
        .unwrap();
    receiver.reject().unwrap();
    assert_eq!(
        receiver.feed(&offer_bytes("STRASSE.TXT", 0, HeaderFormat::Binary16)),
        Err(Error::NameAlias)
    );
}

#[test]
fn receiver_enforces_32_offer_limit_before_approval() {
    let mut receiver = receiver();
    for index in 0..MAX_FILES {
        receiver
            .feed(&offer_bytes(
                &format!("file-{index}"),
                0,
                HeaderFormat::Binary32,
            ))
            .unwrap();
        receiver.reject().unwrap();
    }
    assert_eq!(
        receiver.feed(&offer_bytes("file-33", 0, HeaderFormat::Binary32)),
        Err(Error::Limit("file count"))
    );
}

#[test]
fn receiver_enforces_per_file_and_total_declared_budgets_without_allocating_them() {
    let mut receiver = receiver();
    for name in ["one", "two"] {
        receiver
            .feed(&offer_bytes(name, MAX_FILE_BYTES, HeaderFormat::Binary32))
            .unwrap();
        receiver.reject().unwrap();
    }
    assert_eq!(
        receiver.feed(&offer_bytes("three", 1, HeaderFormat::Binary32)),
        Err(Error::Limit("total file bytes"))
    );
    let mut receiver = Receiver::new();
    receiver.start().unwrap();
    assert_eq!(
        receiver.feed(&offer_bytes(
            "big",
            MAX_FILE_BYTES + 1,
            HeaderFormat::Binary32
        )),
        Err(Error::Limit("file bytes"))
    );
}

#[test]
fn receiver_and_sender_explicitly_reject_command_frames_and_command_invitations() {
    for command in [
        header(FrameType::Command, 0),
        header(FrameType::RequestInit, (FrameType::Command as u32) << 24),
    ] {
        let mut receiver = receiver();
        assert_eq!(receiver.feed(&command), Err(Error::RemoteCommand));
        let mut sender = sender(b"x");
        assert_eq!(sender.feed(&command), Err(Error::RemoteCommand));
    }
}

#[test]
fn receiver_rejects_conversion_compression_and_sparse_options() {
    for flags in [2 << 24, 3 << 24, 1 << 8, 1] {
        let mut receiver = receiver();
        assert!(matches!(
            receiver.feed(&encode_header(Header::new(
                FrameType::File,
                flags,
                HeaderFormat::Binary32
            ))),
            Err(Error::Unsupported(_))
        ));
    }
}

#[test]
fn receiver_sender_init_attention_string_is_inert_and_only_acknowledged() {
    let mut receiver = receiver();
    let mut bytes = header(FrameType::SenderInit, 0x4000_0000);
    bytes.extend(encode_data(b"\x03\xde\0", FrameEnd::EndAck, HeaderFormat::Hex).unwrap());
    assert_eq!(
        receiver.feed(&bytes).unwrap(),
        vec![Event::Send(header(FrameType::Ack, 0))]
    );
}

#[test]
fn receiver_finish_requires_fragmented_oo_and_returns_terminal_suffix() {
    let mut receiver = receiver();
    assert_eq!(
        wire(receiver.feed(&header(FrameType::Finish, 0)).unwrap()),
        header(FrameType::Finish, 0)
    );
    assert!(!receiver.is_finished());
    assert!(receiver.feed(b"O").unwrap().is_empty());
    assert_eq!(
        receiver.feed(b"O$ prompt").unwrap(),
        vec![Event::Finished, Event::Terminal(b"$ prompt".to_vec())]
    );
    assert!(receiver.is_finished());
}

#[test]
fn receiver_finish_nak_and_timeout_repeat_zfin_without_returning_to_init() {
    let mut receiver = receiver();
    receiver.feed(&header(FrameType::Finish, 0)).unwrap();
    assert_eq!(
        wire(receiver.feed(&header(FrameType::Nak, 0)).unwrap()),
        header(FrameType::Finish, 0)
    );
    assert_eq!(
        wire(receiver.timeout().unwrap()),
        header(FrameType::Finish, 0)
    );
}

#[test]
fn receiver_empty_file_is_still_approved_and_saved_before_finish() {
    let mut receiver = accepted("empty", 0, HeaderFormat::Binary32);
    assert!(matches!(
        receiver.feed(&header(FrameType::EndOfFile, 0)).unwrap().as_slice(),
        [Event::FileReceived { bytes, .. }] if bytes.is_empty()
    ));
    assert!(matches!(
        receiver.feed(&header(FrameType::Finish, 0)),
        Err(Error::UnexpectedFrame(FrameType::Finish))
    ));
}

#[test]
fn fragmented_five_can_cancels_while_parsing_a_data_packet() {
    let mut receiver = accepted("cancel", 10, HeaderFormat::Binary16);
    receiver
        .feed(&encode_header(Header::new(
            FrameType::Data,
            0,
            HeaderFormat::Binary16,
        )))
        .unwrap();
    assert!(receiver.feed(b"a\x18\x18").unwrap().is_empty());
    assert!(receiver.feed(b"\x18\x18").unwrap().is_empty());
    assert_eq!(receiver.feed(b"\x18").unwrap(), vec![Event::Cancelled]);
}

#[test]
fn local_cancel_is_bounded_idempotent_and_available_after_protocol_error() {
    let mut receiver = receiver();
    assert_eq!(
        receiver.feed(&header(FrameType::Command, 0)),
        Err(Error::RemoteCommand)
    );
    let events = receiver.cancel().unwrap();
    assert_eq!(wire(events), [vec![24; 8], vec![8; 10]].concat());
    assert!(receiver.cancel().unwrap().is_empty());
    let mut sender = sender(b"x");
    assert_eq!(
        wire(sender.cancel().unwrap()),
        [vec![24; 8], vec![8; 10]].concat()
    );
    assert!(sender.cancel().unwrap().is_empty());
}

#[test]
fn cumulative_timeout_retries_are_strictly_bounded() {
    let mut receiver = receiver();
    for _ in 0..MAX_RETRIES {
        receiver.timeout().unwrap();
    }
    assert_eq!(receiver.timeout(), Err(Error::RetryLimit));
    let mut sender = sender(b"x");
    for _ in 0..MAX_RETRIES {
        sender.timeout().unwrap();
    }
    assert_eq!(sender.timeout(), Err(Error::RetryLimit));
}

#[test]
fn feed_and_subpacket_budgets_reject_oversized_input() {
    let mut receiver = receiver();
    assert_eq!(
        receiver.feed(&vec![0; MAX_FEED_BYTES + 1]),
        Err(Error::Limit("feed bytes"))
    );
    assert_eq!(
        encode_data(
            &vec![0; MAX_SUBPACKET_BYTES + 1],
            FrameEnd::End,
            HeaderFormat::Binary32
        ),
        Err(Error::Limit("subpacket bytes"))
    );
}

#[test]
fn sender_rejects_empty_oversized_or_aliasing_batches_before_start() {
    assert!(Sender::new(Vec::new()).is_err());
    assert!(matches!(
        Sender::new(vec![
            SendFile::new("a", Vec::new()).unwrap(),
            SendFile::new("A", Vec::new()).unwrap()
        ]),
        Err(Error::NameAlias)
    ));
    let files = (0..MAX_FILES + 1)
        .map(|index| SendFile::new(format!("f{index}"), Vec::new()).unwrap())
        .collect();
    assert!(matches!(
        Sender::new(files),
        Err(Error::Limit("file count"))
    ));
}

#[test]
fn sender_start_does_not_execute_rz_or_emit_file_data() {
    let mut sender = Sender::new(vec![SendFile::new("a", vec![1]).unwrap()]).unwrap();
    assert_eq!(
        wire(sender.start().unwrap()),
        header(FrameType::RequestInit, 0)
    );
}

#[test]
fn sender_waits_for_exact_ack_and_does_not_advance_on_a_stale_ack() {
    let bytes = vec![b'x'; 2500];
    let mut sender = sender(&bytes);
    let initial = sender.feed(&header(FrameType::Position, 0)).unwrap();
    assert!(!wire(initial).is_empty());
    assert!(sender.feed(&header(FrameType::Ack, 12)).unwrap().is_empty());
    let events = sender.feed(&header(FrameType::Ack, 1024)).unwrap();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::Progress { bytes: 1024, .. }))
    );
    assert!(!wire(events).is_empty());
}

#[test]
fn sender_zrpos_retransmits_from_the_requested_byte_without_changing_source() {
    let mut sender = sender(b"abcdef");
    let first = wire(sender.feed(&header(FrameType::Position, 0)).unwrap());
    let repeated = sender.feed(&header(FrameType::Position, 0)).unwrap();
    assert!(repeated.contains(&Event::Retry { position: Some(0) }));
    assert_eq!(wire(repeated), first);
    let at_three = wire(sender.feed(&header(FrameType::Position, 3)).unwrap());
    assert_eq!(
        at_three,
        frame(3, b"def", FrameEnd::EndAck, HeaderFormat::Binary32)
    );
    assert_eq!(
        sender.feed(&header(FrameType::Position, 7)),
        Err(Error::InvalidPosition)
    );
}

#[test]
fn sender_acknowledged_forward_zrpos_pacing_does_not_exhaust_error_retries() {
    let bytes = vec![b'x'; 200 * 1024];
    let mut sender = sender(&bytes);
    sender.feed(&header(FrameType::Position, 0)).unwrap();
    for position in (1024..=bytes.len()).step_by(1024) {
        let next = wire(
            sender
                .feed(&header(FrameType::Ack, position as u32))
                .unwrap(),
        );
        let pacing = sender
            .feed(&header(FrameType::Position, position as u32))
            .unwrap();
        assert!(
            !pacing
                .iter()
                .any(|event| matches!(event, Event::Retry { .. }))
        );
        assert_eq!(wire(pacing), next);
        // An independent receiver can repeat its current position before the
        // next ACK reaches the sender. Bound stalled recovery per progress
        // point, rather than terminating an otherwise advancing large file.
        for _ in 0..3 {
            assert_eq!(
                wire(
                    sender
                        .feed(&header(FrameType::Position, position as u32))
                        .unwrap()
                ),
                next
            );
        }
        assert!(
            sender
                .feed(&header(FrameType::Ack, position as u32))
                .unwrap()
                .is_empty()
        );
    }
    assert!(
        !sender.is_finished(),
        "pacing cannot acknowledge remote publication"
    );
    let completed = sender
        .feed(&header(FrameType::ReceiverInit, 0x2300_0000))
        .unwrap();
    assert!(
        completed
            .iter()
            .any(|event| matches!(event, Event::FileSent(_)))
    );
}

#[test]
fn sender_repeated_zrpos_without_progress_still_exhausts_retry_budget() {
    let mut sender = sender(b"not yet acknowledged");
    sender.feed(&header(FrameType::Position, 0)).unwrap();
    for _ in 0..MAX_RETRIES {
        sender.feed(&header(FrameType::Position, 0)).unwrap();
    }
    assert_eq!(
        sender.feed(&header(FrameType::Position, 0)),
        Err(Error::RetryLimit)
    );
}

#[test]
fn sender_honors_a_smaller_receiver_buffer_and_crc16_only_peer() {
    let mut sender = Sender::new(vec![SendFile::new("a", b"abcdef".to_vec()).unwrap()]).unwrap();
    sender.start().unwrap();
    sender.feed(&header(FrameType::ReceiverInit, 2)).unwrap();
    assert_eq!(
        wire(sender.feed(&header(FrameType::Position, 0)).unwrap()),
        frame(0, b"ab", FrameEnd::EndAck, HeaderFormat::Binary16)
    );
    assert_eq!(
        wire(sender.feed(&header(FrameType::Ack, 2)).unwrap()),
        frame(2, b"cd", FrameEnd::EndAck, HeaderFormat::Binary16)
    );
}

#[test]
fn sender_accepts_fragmented_zrinit_and_zrpos_and_can_resume_an_existing_prefix() {
    let mut sender = Sender::new(vec![SendFile::new("a", b"abcdef".to_vec()).unwrap()]).unwrap();
    sender.start().unwrap();
    let mut negotiation = Vec::new();
    for byte in header(FrameType::ReceiverInit, 0x2300_0000) {
        negotiation.extend(sender.feed(&[byte]).unwrap());
    }
    assert!(!wire(negotiation).is_empty());
    let mut response = Vec::new();
    for byte in header(FrameType::Position, 3) {
        response.extend(sender.feed(&[byte]).unwrap());
    }
    assert_eq!(
        wire(response),
        frame(3, b"def", FrameEnd::EndAck, HeaderFormat::Binary32)
    );
}

#[test]
fn sender_fragmented_five_can_cancellation_cannot_be_a_command() {
    let mut sender = sender(b"x");
    assert!(sender.feed(b"\x18\x18\x18").unwrap().is_empty());
    assert_eq!(sender.feed(b"\x18\x18").unwrap(), vec![Event::Cancelled]);
    assert!(!sender.is_finished());
}

#[test]
fn sender_reports_file_sent_only_after_zrinit_not_after_data_or_eof() {
    let mut sender = sender(b"x");
    sender.feed(&header(FrameType::Position, 0)).unwrap();
    let events = sender.feed(&header(FrameType::Ack, 1)).unwrap();
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::FileSent(_)))
    );
    assert_eq!(wire(events), header(FrameType::EndOfFile, 1));
    let events = sender
        .feed(&header(FrameType::ReceiverInit, 0x2300_0000))
        .unwrap();
    assert!(matches!(events.first(), Some(Event::FileSent(_))));
    assert_eq!(wire(events), header(FrameType::Finish, 0));
}

#[test]
fn sender_finish_writes_oo_and_preserves_already_arrived_terminal_suffix() {
    let mut sender = sender(b"");
    assert_eq!(
        wire(sender.feed(&header(FrameType::Position, 0)).unwrap()),
        header(FrameType::EndOfFile, 0)
    );
    sender
        .feed(&header(FrameType::ReceiverInit, 0x2300_0000))
        .unwrap();
    let mut bytes = header(FrameType::Finish, 0);
    bytes.extend_from_slice(b"$ ");
    assert_eq!(
        sender.feed(&bytes).unwrap(),
        vec![
            Event::Send(b"OO".to_vec()),
            Event::Finished,
            Event::Terminal(b"$ ".to_vec())
        ]
    );
    assert!(sender.is_finished());
}

#[test]
fn sender_zcrc_answers_independent_ieee_check_value_for_the_full_file() {
    let mut sender = sender(b"123456789");
    assert_eq!(
        wire(sender.feed(&header(FrameType::Crc, 0)).unwrap()),
        header(FrameType::Crc, 0xcbf4_3926)
    );
    assert_eq!(
        sender.feed(&header(FrameType::Crc, 10)),
        Err(Error::InvalidPosition)
    );
}

#[test]
fn sender_peer_skip_advances_batch_but_does_not_claim_remote_publication() {
    let mut sender = sender(b"x");
    let events = sender.feed(&header(FrameType::Skip, 0)).unwrap();
    assert!(matches!(events.first(), Some(Event::FileSkipped(_))));
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::FileSent(_)))
    );
    assert_eq!(wire(events), header(FrameType::Finish, 0));
}

#[test]
fn sender_challenge_is_an_exact_numeric_echo_not_a_command() {
    let mut sender = sender(b"x");
    assert_eq!(
        wire(
            sender
                .feed(&header(FrameType::Challenge, 0x1234_5678))
                .unwrap()
        ),
        header(FrameType::Ack, 0x1234_5678)
    );
}

#[test]
fn sender_abort_completes_handshake_as_cancelled_without_file_success() {
    let mut sender = sender(b"x");
    assert_eq!(
        wire(sender.feed(&header(FrameType::Abort, 0)).unwrap()),
        header(FrameType::Finish, 0)
    );
    assert_eq!(
        sender.feed(&header(FrameType::Finish, 0)).unwrap(),
        vec![Event::Send(b"OO".to_vec()), Event::Cancelled]
    );
    assert!(!sender.is_finished());
}

#[test]
fn receiver_hex_headers_ignore_parity_and_injected_flow_control() {
    let mut receiver = receiver();
    let mut bytes = Vec::new();
    for byte in header(FrameType::RequestInit, 0) {
        bytes.push(0x13);
        bytes.push(byte | 0x80);
        bytes.push(0x91);
    }
    assert_eq!(
        wire(receiver.feed(&bytes).unwrap()),
        header(FrameType::ReceiverInit, 0x6000_0400)
    );
}

#[test]
fn receiver_lost_post_save_zrinit_can_be_repeated_without_a_second_file_event() {
    let mut receiver = accepted("x", 0, HeaderFormat::Binary32);
    receiver.feed(&header(FrameType::EndOfFile, 0)).unwrap();
    receiver.acknowledge_saved().unwrap();
    assert_eq!(
        receiver.feed(&header(FrameType::EndOfFile, 0)).unwrap(),
        vec![Event::Send(header(FrameType::ReceiverInit, 0x6000_0400))]
    );
}

fn drive_events(
    events: Vec<Event>,
    from_sender: bool,
    queue: &mut VecDeque<(bool, Vec<u8>)>,
    receiver: &mut Receiver,
    received: &mut Vec<(String, Vec<u8>)>,
) {
    for event in events {
        match event {
            Event::Send(bytes) => queue.push_back((from_sender, bytes)),
            Event::Offer(_) if !from_sender => {
                queue.push_back((false, wire(receiver.accept().unwrap())));
            }
            Event::FileReceived { offer, bytes } if !from_sender => {
                received.push((offer.name, bytes));
                queue.push_back((false, wire(receiver.acknowledge_saved().unwrap())));
            }
            _ => {}
        }
    }
}

#[test]
fn bounded_batch_state_machine_regression_uses_fragmentation_and_explicit_save_acks() {
    // This is a regression, not external rz/sz interoperability evidence.
    for chunk_bytes in [1, 7, 1024, MAX_FEED_BYTES] {
        let contents: Vec<_> = (0_u8..=255).cycle().take(3500).collect();
        let files = vec![
            SendFile::new("资料.bin", contents.clone()).unwrap(),
            SendFile::new("empty.txt", Vec::new()).unwrap(),
            SendFile::new("last.txt", b"last".to_vec()).unwrap(),
        ];
        let mut sender = Sender::new(files).unwrap();
        let mut receiver = Receiver::new();
        let mut queue = VecDeque::new();
        sender.start().unwrap();
        queue.push_back((false, wire(receiver.start().unwrap())));
        let mut received = Vec::new();
        let mut iterations = 0;
        while let Some((from_sender, bytes)) = queue.pop_front() {
            iterations += 1;
            assert!(iterations <= 1000, "state machines failed to terminate");
            for chunk in bytes.chunks(chunk_bytes) {
                let events = if from_sender {
                    receiver.feed(chunk).unwrap()
                } else {
                    sender.feed(chunk).unwrap()
                };
                drive_events(
                    events,
                    !from_sender,
                    &mut queue,
                    &mut receiver,
                    &mut received,
                );
            }
        }
        assert!(sender.is_finished());
        assert!(receiver.is_finished());
        assert_eq!(
            received,
            vec![
                ("资料.bin".to_owned(), contents),
                ("empty.txt".to_owned(), Vec::new()),
                ("last.txt".to_owned(), b"last".to_vec()),
            ]
        );
    }
}
