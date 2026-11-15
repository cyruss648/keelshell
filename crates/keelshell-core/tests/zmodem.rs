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

fn waiting_for_farewell() -> Receiver {
    let mut receiver = receiver();
    assert_eq!(
        receiver.feed(&header(FrameType::Finish, 0)).unwrap(),
        vec![Event::Send(header(FrameType::Finish, 0))]
    );
    assert!(receiver.is_waiting_for_farewell());
    assert!(receiver.pending_offer().is_none());
    assert!(!receiver.is_finished());
    receiver
}

#[test]
fn receiver_missing_farewell_rejects_every_other_state_without_advancing() {
    let mut reviewing = receiver();
    reviewing
        .feed(&offer_bytes("review", 0, HeaderFormat::Binary32))
        .unwrap();
    let receiving = accepted("receive", 1, HeaderFormat::Binary32);
    let mut saving = accepted("save", 0, HeaderFormat::Binary32);
    saving.feed(&header(FrameType::EndOfFile, 0)).unwrap();
    let mut done = waiting_for_farewell();
    assert_eq!(done.feed(b"OO").unwrap(), vec![Event::Finished]);
    let mut cancelled = waiting_for_farewell();
    cancelled.cancel().unwrap();
    let mut failed = waiting_for_farewell();
    assert_eq!(
        failed.feed(&header(FrameType::Command, 0)),
        Err(Error::RemoteCommand)
    );
    let cases = [
        ("new", Receiver::new()),
        ("ready", receiver()),
        ("review", reviewing),
        ("receiving", receiving),
        ("saving", saving),
        ("done", done),
        ("cancelled", cancelled),
        ("failed", failed),
    ];
    for (state, mut receiver) in cases {
        let offer = receiver.pending_offer().cloned();
        let waiting_for_accept = receiver.is_waiting_for_accept();
        let waiting_for_saved = receiver.is_waiting_for_saved();
        let finished = receiver.is_finished();
        assert!(!receiver.is_waiting_for_farewell(), "{state}");
        for _ in 0..2 {
            assert_eq!(
                receiver.finish_without_farewell(),
                Err(Error::WrongState("receiver not waiting for farewell")),
                "{state}"
            );
        }
        assert_eq!(receiver.pending_offer(), offer.as_ref(), "{state}");
        assert_eq!(
            receiver.is_waiting_for_accept(),
            waiting_for_accept,
            "{state}"
        );
        assert_eq!(
            receiver.is_waiting_for_saved(),
            waiting_for_saved,
            "{state}"
        );
        assert_eq!(receiver.is_finished(), finished, "{state}");
        if state == "new" {
            receiver.start().unwrap();
        } else if state == "review" {
            receiver.accept().unwrap();
        } else if state == "saving" {
            receiver.acknowledge_saved().unwrap();
        }
    }
}

#[test]
fn receiver_missing_farewell_requires_a_complete_valid_initial_zfin() {
    let finish = header(FrameType::Finish, 0);
    for split in 0..finish.len() {
        let mut receiver = receiver();
        assert!(receiver.feed(&finish[..split]).unwrap().is_empty());
        assert!(!receiver.is_waiting_for_farewell());
        assert!(matches!(
            receiver.finish_without_farewell(),
            Err(Error::WrongState(_))
        ));
        assert_eq!(
            receiver.feed(&finish[split..]).unwrap(),
            vec![Event::Send(finish.clone())]
        );
        assert!(receiver.is_waiting_for_farewell());
        assert_eq!(
            receiver.finish_without_farewell().unwrap(),
            vec![Event::FarewellMissing]
        );
    }
}

#[test]
fn receiver_missing_farewell_cannot_bypass_review_or_unacknowledged_save() {
    for save_pause in [false, true] {
        let mut receiver = receiver();
        receiver
            .feed(&offer_bytes("pending", 0, HeaderFormat::Binary32))
            .unwrap();
        if save_pause {
            receiver.accept().unwrap();
            receiver.feed(&header(FrameType::EndOfFile, 0)).unwrap();
        }
        assert!(receiver.pending_offer().is_some());
        assert!(receiver.timeout().unwrap().is_empty());
        assert!(!receiver.is_waiting_for_farewell());
        assert!(matches!(
            receiver.finish_without_farewell(),
            Err(Error::WrongState(_))
        ));
        assert_eq!(
            receiver.feed(&header(FrameType::Finish, 0)),
            Err(Error::UnexpectedFrame(FrameType::Finish))
        );
        assert!(!receiver.is_waiting_for_farewell());
        assert!(matches!(
            receiver.finish_without_farewell(),
            Err(Error::WrongState(_))
        ));
    }
}

#[test]
fn receiver_missing_farewell_follows_explicit_save_acknowledgement() {
    let mut receiver = accepted("verified", 3, HeaderFormat::Binary32);
    assert!(
        complete(&mut receiver, b"abc", HeaderFormat::Binary32)
            .iter()
            .any(|event| matches!(event, Event::FileReceived { bytes, .. } if bytes == b"abc"))
    );
    assert!(matches!(
        receiver.finish_without_farewell(),
        Err(Error::WrongState(_))
    ));
    receiver.acknowledge_saved().unwrap();
    assert!(receiver.pending_offer().is_none());
    assert!(!receiver.is_waiting_for_farewell());
    assert_eq!(
        receiver.feed(&header(FrameType::Finish, 0)).unwrap(),
        vec![Event::Send(header(FrameType::Finish, 0))]
    );
    assert_eq!(
        receiver.finish_without_farewell().unwrap(),
        vec![Event::FarewellMissing]
    );
    assert!(!receiver.is_finished());
}

#[test]
fn receiver_missing_farewell_is_distinct_idempotent_and_terminal() {
    let mut receiver = waiting_for_farewell();
    assert_eq!(
        receiver.finish_without_farewell().unwrap(),
        vec![Event::FarewellMissing]
    );
    assert!(!receiver.is_waiting_for_farewell());
    assert!(!receiver.is_finished());
    assert!(receiver.finish_without_farewell().unwrap().is_empty());
    assert!(receiver.timeout().unwrap().is_empty());
    assert!(receiver.cancel().unwrap().is_empty());
    let mut terminal = b"OO\r\n\x1b[0m".to_vec();
    terminal.extend(header(FrameType::Command, 0));
    terminal.extend_from_slice(&[24; 5]);
    assert_eq!(
        receiver.feed(&terminal).unwrap(),
        vec![Event::Terminal(terminal)]
    );
    assert!(receiver.feed(b"").unwrap().is_empty());
    assert_eq!(
        receiver.feed(&vec![0; MAX_FEED_BYTES + 1]),
        Err(Error::Limit("feed bytes"))
    );
    assert!(receiver.finish_without_farewell().unwrap().is_empty());
    assert!(!receiver.is_finished());
}

#[test]
fn receiver_missing_farewell_preserves_suffix_arriving_with_initial_zfin() {
    let suffix = "\r\n\x1b[?2004h用户@host:~/资料$ \x1b[0m".as_bytes();
    let mut bytes = header(FrameType::Finish, 0);
    bytes.extend_from_slice(suffix);
    let mut receiver = receiver();
    assert_eq!(
        receiver.feed(&bytes).unwrap(),
        vec![Event::Send(header(FrameType::Finish, 0))]
    );
    assert_eq!(
        receiver.finish_without_farewell().unwrap(),
        vec![Event::FarewellMissing, Event::Terminal(suffix.to_vec())]
    );
    assert!(receiver.finish_without_farewell().unwrap().is_empty());
}

#[test]
fn receiver_missing_farewell_preserves_fragmented_raw_and_incomplete_lookalikes() {
    let finish = header(FrameType::Finish, 0);
    let mut suffixes = vec![
        b"O".to_vec(),
        b"prompt O".to_vec(),
        b"**\x18Qnot-a-header".to_vec(),
        b"*\x18C\x18H".to_vec(),
        b"*********literal".to_vec(),
        b"\x1b[?2004".to_vec(),
        b"\xe4\xb8".to_vec(),
        b"\x00\x11\x13\x91\x93\x7f\xff\x18".to_vec(),
    ];
    suffixes.extend((1..finish.len()).map(|length| finish[..length].to_vec()));
    for suffix in suffixes {
        for chunk_bytes in [1, 2, 7, MAX_FEED_BYTES] {
            let mut receiver = waiting_for_farewell();
            for chunk in suffix.chunks(chunk_bytes) {
                assert!(receiver.feed(chunk).unwrap().is_empty());
            }
            assert!(receiver.is_waiting_for_farewell());
            assert_eq!(
                receiver.finish_without_farewell().unwrap(),
                vec![Event::FarewellMissing, Event::Terminal(suffix.clone())]
            );
        }
    }
}

#[test]
fn receiver_missing_farewell_strips_fragmented_repeated_zfin_in_all_formats() {
    for format in [
        HeaderFormat::Hex,
        HeaderFormat::Binary16,
        HeaderFormat::Binary32,
    ] {
        let prefix = b"\r\n\x1b[0mO";
        let middle = "用户@host:~$ ".as_bytes();
        let suffix = b"\x1b[?2004hO**\x18B0800";
        let repeated = encode_header(Header::new(FrameType::Finish, 0, format));
        let bytes = [
            prefix.as_slice(),
            repeated.as_slice(),
            middle,
            repeated.as_slice(),
            suffix.as_slice(),
        ]
        .concat();
        let terminal = [prefix.as_slice(), middle, suffix.as_slice()].concat();
        for split in 0..=bytes.len() {
            let mut receiver = waiting_for_farewell();
            let mut events = receiver.feed(&bytes[..split]).unwrap();
            events.extend(receiver.feed(&bytes[split..]).unwrap());
            assert_eq!(
                events,
                vec![
                    Event::Send(header(FrameType::Finish, 0)),
                    Event::Send(header(FrameType::Finish, 0))
                ]
            );
            assert_eq!(
                receiver.finish_without_farewell().unwrap(),
                vec![Event::FarewellMissing, Event::Terminal(terminal.clone())]
            );
        }
        let mut receiver = waiting_for_farewell();
        for byte in &bytes {
            receiver.feed(&[*byte]).unwrap();
        }
        assert_eq!(
            receiver.finish_without_farewell().unwrap(),
            vec![Event::FarewellMissing, Event::Terminal(terminal)]
        );
    }
}

#[test]
fn receiver_missing_farewell_preserves_controls_outside_validated_zfin() {
    let mut receiver = waiting_for_farewell();
    let prefix = b"\x11\x93\r\n";
    let suffix = b"\x91\x13\x1b[0m$ ";
    let mut bytes = prefix.to_vec();
    for (index, byte) in header(FrameType::Finish, 0).into_iter().enumerate() {
        if index > 0 {
            bytes.push(0x13);
        }
        bytes.push(byte | 0x80);
    }
    bytes.extend_from_slice(suffix);
    for byte in bytes {
        receiver.feed(&[byte]).unwrap();
    }
    assert_eq!(
        receiver.finish_without_farewell().unwrap(),
        vec![
            Event::FarewellMissing,
            Event::Terminal([prefix.as_slice(), suffix.as_slice()].concat())
        ]
    );
}

#[test]
fn receiver_missing_farewell_retains_lookalikes_across_recovery_and_timeout() {
    let mut receiver = waiting_for_farewell();
    let mut corrupt = header(FrameType::Finish, 0);
    corrupt[8] = b'1';
    assert_eq!(
        receiver.feed(&corrupt).unwrap(),
        vec![
            Event::Retry { position: None },
            Event::Send(header(FrameType::Finish, 0))
        ]
    );
    let partial = b"\x1b[0m$ **\x18B08";
    assert!(receiver.feed(partial).unwrap().is_empty());
    assert_eq!(
        receiver.timeout().unwrap(),
        vec![
            Event::Retry { position: None },
            Event::Send(header(FrameType::Finish, 0))
        ]
    );
    assert_eq!(
        receiver.finish_without_farewell().unwrap(),
        vec![
            Event::FarewellMissing,
            Event::Terminal([corrupt.as_slice(), partial.as_slice()].concat())
        ]
    );
}

#[test]
fn receiver_actual_oo_stays_strict_and_cannot_be_reclassified_as_missing() {
    let mut receiver = waiting_for_farewell();
    assert!(receiver.feed(b"O").unwrap().is_empty());
    assert!(receiver.is_waiting_for_farewell());
    assert!(!receiver.is_finished());
    assert_eq!(
        receiver.feed(b"O$ prompt").unwrap(),
        vec![Event::Finished, Event::Terminal(b"$ prompt".to_vec())]
    );
    assert!(receiver.is_finished());
    assert!(!receiver.is_waiting_for_farewell());
    assert!(matches!(
        receiver.finish_without_farewell(),
        Err(Error::WrongState(_))
    ));
}

#[test]
fn receiver_actual_oo_preserves_earlier_ordinary_bytes_and_suffix_order() {
    let mut receiver = waiting_for_farewell();
    let prefix = "\r\n\x1b[0m终端 O text ".as_bytes();
    for byte in prefix {
        assert!(receiver.feed(&[*byte]).unwrap().is_empty());
    }
    assert!(receiver.feed(b"O\x13").unwrap().is_empty());
    assert_eq!(
        receiver.feed(b"O$ ").unwrap(),
        vec![
            Event::Finished,
            Event::Terminal([prefix, b"\x13$ "].concat())
        ]
    );
    assert!(receiver.is_finished());
}

#[test]
fn receiver_missing_farewell_cannot_convert_finishing_input_errors() {
    for (bytes, error) in [
        (header(FrameType::Command, 0), Error::RemoteCommand),
        (
            header(FrameType::RequestInit, (FrameType::Command as u32) << 24),
            Error::RemoteCommand,
        ),
        (
            header(FrameType::File, 1 << 24),
            Error::UnexpectedFrame(FrameType::File),
        ),
        (
            header(FrameType::Data, 0),
            Error::UnexpectedFrame(FrameType::Data),
        ),
        (header(FrameType::EndOfFile, 0), Error::ApprovalRequired),
        (
            header(FrameType::SenderInit, 0),
            Error::UnexpectedFrame(FrameType::SenderInit),
        ),
    ] {
        let mut receiver = waiting_for_farewell();
        assert_eq!(receiver.feed(&bytes), Err(error));
        assert!(!receiver.is_waiting_for_farewell());
        assert!(!receiver.is_finished());
        assert!(matches!(
            receiver.finish_without_farewell(),
            Err(Error::WrongState(_))
        ));
        assert!(matches!(
            receiver.feed(&header(FrameType::Finish, 0)),
            Err(Error::WrongState(_))
        ));
        assert!(matches!(
            receiver.cancel().unwrap().as_slice(),
            [Event::Send(_), Event::Cancelled]
        ));
    }
}

#[test]
fn receiver_missing_farewell_cannot_convert_peer_or_local_cancellation() {
    for (bytes, trailing_xon) in [
        (header(FrameType::Cancel, 0), true),
        (header(FrameType::Abort, 0), true),
        (header(FrameType::FileError, 0), true),
        (vec![24; 5], false),
    ] {
        let mut receiver = waiting_for_farewell();
        let mut events = Vec::new();
        for byte in bytes {
            events.extend(receiver.feed(&[byte]).unwrap());
        }
        let mut expected = vec![Event::Cancelled];
        if trailing_xon {
            expected.push(Event::Terminal(vec![0x11]));
        }
        assert_eq!(events, expected);
        assert!(!receiver.is_waiting_for_farewell());
        assert!(!receiver.is_finished());
        assert!(matches!(
            receiver.finish_without_farewell(),
            Err(Error::WrongState(_))
        ));
    }
    let mut receiver = waiting_for_farewell();
    receiver.cancel().unwrap();
    assert!(matches!(
        receiver.finish_without_farewell(),
        Err(Error::WrongState(_))
    ));
}

#[test]
fn receiver_missing_farewell_enforces_feed_and_noise_bounds() {
    let mut oversized = waiting_for_farewell();
    assert_eq!(
        oversized.feed(&vec![b'x'; MAX_FEED_BYTES + 1]),
        Err(Error::Limit("feed bytes"))
    );
    assert!(matches!(
        oversized.finish_without_farewell(),
        Err(Error::WrongState(_))
    ));
    let mut receiver = waiting_for_farewell();
    let bytes = vec![b'x'; MAX_FEED_BYTES];
    assert!(receiver.feed(&bytes).unwrap().is_empty());
    assert_eq!(
        receiver.finish_without_farewell().unwrap(),
        vec![Event::FarewellMissing, Event::Terminal(bytes.clone())]
    );
    let mut excessive_noise = waiting_for_farewell();
    excessive_noise.feed(&bytes).unwrap();
    assert_eq!(
        excessive_noise.feed(b"x"),
        Err(Error::Limit("out-of-frame noise"))
    );
    assert!(!excessive_noise.is_waiting_for_farewell());
    assert!(matches!(
        excessive_noise.finish_without_farewell(),
        Err(Error::WrongState(_))
    ));
}

#[test]
fn receiver_missing_farewell_bounds_retained_flow_control_bytes() {
    let mut receiver = waiting_for_farewell();
    receiver.feed(&vec![0x11; MAX_FEED_BYTES]).unwrap();
    receiver.feed(&[0x13; 96]).unwrap();
    assert_eq!(receiver.feed(&[0x11]), Err(Error::Limit("farewell bytes")));
    assert!(!receiver.is_waiting_for_farewell());
    assert!(matches!(
        receiver.finish_without_farewell(),
        Err(Error::WrongState(_))
    ));
}

#[test]
fn receiver_missing_farewell_retains_header_candidate_bound() {
    let mut receiver = waiting_for_farewell();
    let mut bytes = b"**\x18B".to_vec();
    bytes.extend_from_slice(&[0x11; 92]);
    assert!(receiver.feed(&bytes).unwrap().is_empty());
    assert_eq!(receiver.feed(&[0x11]), Err(Error::Limit("header bytes")));
    assert!(matches!(
        receiver.finish_without_farewell(),
        Err(Error::WrongState(_))
    ));
}

#[test]
fn receiver_missing_farewell_retry_exhaustion_never_finishes_automatically() {
    let mut receiver = waiting_for_farewell();
    for _ in 0..MAX_RETRIES {
        assert_eq!(
            receiver.timeout().unwrap(),
            vec![
                Event::Retry { position: None },
                Event::Send(header(FrameType::Finish, 0))
            ]
        );
        assert!(receiver.is_waiting_for_farewell());
        assert!(!receiver.is_finished());
    }
    assert_eq!(receiver.timeout(), Err(Error::RetryLimit));
    assert!(!receiver.is_waiting_for_farewell());
    assert!(matches!(
        receiver.finish_without_farewell(),
        Err(Error::WrongState(_))
    ));
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

fn repeated_hex_finish_tail(prefix: &[u8], finish: &[u8], strict: bool) -> Vec<u8> {
    let mut bytes = [prefix, finish, b"$ "].concat();
    if strict {
        bytes.extend_from_slice(b"OO");
    }
    bytes
}

fn assert_repeated_hex_finish_tail<'a>(
    chunks: impl IntoIterator<Item = &'a [u8]>,
    strict: bool,
    terminal: &[u8],
) {
    let mut receiver = waiting_for_farewell();
    let mut events = Vec::new();
    for chunk in chunks {
        events.extend(receiver.feed(chunk).unwrap());
    }
    if strict {
        assert!(receiver.is_finished());
        assert!(!receiver.is_waiting_for_farewell());
        assert_eq!(
            receiver.finish_without_farewell(),
            Err(Error::WrongState("receiver not waiting for farewell"))
        );
    } else {
        assert!(receiver.is_waiting_for_farewell());
        assert!(!receiver.is_finished());
        events.extend(receiver.finish_without_farewell().unwrap());
        assert!(!receiver.is_waiting_for_farewell());
        assert!(!receiver.is_finished());
        assert!(receiver.finish_without_farewell().unwrap().is_empty());
    }
    assert_eq!(
        events,
        vec![
            Event::Send(header(FrameType::Finish, 0)),
            if strict {
                Event::Finished
            } else {
                Event::FarewellMissing
            },
            Event::Terminal(terminal.to_vec())
        ],
        "strict farewell: {strict}"
    );
}

#[test]
fn receiver_missing_farewell_preserves_utf8_before_repeated_hex_zfin() {
    // CORE-R1-01: the final byte of 个 is 0xaa, also a parity-masked pad.
    let bytes = repeated_hex_finish_tail("个".as_bytes(), &header(FrameType::Finish, 0), false);
    assert_repeated_hex_finish_tail([bytes.as_slice()], false, "个$ ".as_bytes());
}

#[test]
fn receiver_missing_farewell_preserves_utf8_before_repeated_hex_zfin_bytewise() {
    let bytes = repeated_hex_finish_tail("个".as_bytes(), &header(FrameType::Finish, 0), false);
    assert_repeated_hex_finish_tail(bytes.chunks(1), false, "个$ ".as_bytes());
}

#[test]
fn receiver_missing_farewell_preserves_utf8_before_repeated_hex_zfin_at_every_split() {
    let bytes = repeated_hex_finish_tail("个".as_bytes(), &header(FrameType::Finish, 0), false);
    for split in 0..=bytes.len() {
        assert_repeated_hex_finish_tail(
            [&bytes[..split], &bytes[split..]],
            false,
            "个$ ".as_bytes(),
        );
    }
}

#[test]
fn receiver_actual_oo_preserves_utf8_before_repeated_hex_zfin() {
    let bytes = repeated_hex_finish_tail("个".as_bytes(), &header(FrameType::Finish, 0), true);
    assert_repeated_hex_finish_tail([bytes.as_slice()], true, "个$ ".as_bytes());
}

#[test]
fn receiver_actual_oo_preserves_utf8_before_repeated_hex_zfin_bytewise() {
    let bytes = repeated_hex_finish_tail("个".as_bytes(), &header(FrameType::Finish, 0), true);
    assert_repeated_hex_finish_tail(bytes.chunks(1), true, "个$ ".as_bytes());
}

#[test]
fn receiver_actual_oo_preserves_utf8_before_repeated_hex_zfin_at_every_split() {
    let bytes = repeated_hex_finish_tail("个".as_bytes(), &header(FrameType::Finish, 0), true);
    for split in 0..=bytes.len() {
        assert_repeated_hex_finish_tail(
            [&bytes[..split], &bytes[split..]],
            true,
            "个$ ".as_bytes(),
        );
    }
}

fn parity_padded_hex_finishes() -> Vec<Vec<u8>> {
    let finish = header(FrameType::Finish, 0);
    [
        b"\xaa".as_slice(),
        b"\xaa\xaa".as_slice(),
        b"*\xaa".as_slice(),
        b"\xaa*".as_slice(),
        b"\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa".as_slice(),
    ]
    .into_iter()
    .map(|pads| [pads, &finish[2..]].concat())
    .collect()
}

#[test]
fn receiver_finish_accepts_parity_tolerant_hex_protocol_pads() {
    for finish in parity_padded_hex_finishes() {
        for split in 0..=finish.len() {
            let mut receiver = receiver();
            let mut events = receiver.feed(&finish[..split]).unwrap();
            events.extend(receiver.feed(&finish[split..]).unwrap());
            assert_eq!(events, vec![Event::Send(header(FrameType::Finish, 0))]);
            assert!(receiver.is_waiting_for_farewell());
            assert_eq!(receiver.feed(b"OO").unwrap(), vec![Event::Finished]);
            assert!(receiver.is_finished());
        }
    }
}

#[test]
fn receiver_missing_farewell_strips_repeated_hex_zfin_with_parity_pads() {
    for finish in parity_padded_hex_finishes() {
        let bytes = repeated_hex_finish_tail(b"tail ", &finish, false);
        assert_repeated_hex_finish_tail(bytes.chunks(1), false, b"tail $ ");
        for split in 0..=bytes.len() {
            assert_repeated_hex_finish_tail([&bytes[..split], &bytes[split..]], false, b"tail $ ");
        }
    }
}

#[test]
fn receiver_actual_oo_strips_repeated_hex_zfin_with_parity_pads() {
    for finish in parity_padded_hex_finishes() {
        let bytes = repeated_hex_finish_tail(b"tail ", &finish, true);
        assert_repeated_hex_finish_tail(bytes.chunks(1), true, b"tail $ ");
        for split in 0..=bytes.len() {
            assert_repeated_hex_finish_tail([&bytes[..split], &bytes[split..]], true, b"tail $ ");
        }
    }
}

#[test]
fn receiver_finishing_cancel_preserves_utf8_tail_after_repeated_zfin() {
    let bytes = repeated_hex_finish_tail("个".as_bytes(), &header(FrameType::Finish, 0), false);
    for chunk_bytes in [1, 2, bytes.len()] {
        let mut receiver = waiting_for_farewell();
        let mut events = Vec::new();
        for chunk in bytes.chunks(chunk_bytes) {
            events.extend(receiver.feed(chunk).unwrap());
        }
        assert_eq!(events, vec![Event::Send(header(FrameType::Finish, 0))]);
        assert!(receiver.is_waiting_for_farewell());
        assert_eq!(
            receiver.cancel().unwrap(),
            vec![
                Event::Send([vec![24; 8], vec![8; 10]].concat()),
                Event::Cancelled,
                Event::Terminal("个$ ".as_bytes().to_vec())
            ]
        );
        assert!(!receiver.is_waiting_for_farewell());
        assert!(!receiver.is_finished());
        assert!(matches!(
            receiver.finish_without_farewell(),
            Err(Error::WrongState(_))
        ));
        assert!(receiver.cancel().unwrap().is_empty());
        assert_eq!(
            receiver.feed(b"OO$ next").unwrap(),
            vec![Event::Terminal(b"OO$ next".to_vec())]
        );
    }
}

#[test]
fn receiver_cancel_after_actual_oo_does_not_repeat_utf8_tail() {
    let bytes = repeated_hex_finish_tail("个".as_bytes(), &header(FrameType::Finish, 0), true);
    for chunk_bytes in [1, 2, bytes.len()] {
        let mut receiver = waiting_for_farewell();
        let mut events = Vec::new();
        for chunk in bytes.chunks(chunk_bytes) {
            events.extend(receiver.feed(chunk).unwrap());
        }
        assert_eq!(
            events,
            vec![
                Event::Send(header(FrameType::Finish, 0)),
                Event::Finished,
                Event::Terminal("个$ ".as_bytes().to_vec())
            ]
        );
        assert!(receiver.is_finished());
        assert!(!receiver.is_waiting_for_farewell());
        assert!(receiver.cancel().unwrap().is_empty());
        assert!(receiver.cancel().unwrap().is_empty());
        assert_eq!(
            receiver.feed(b"$ next").unwrap(),
            vec![Event::Terminal(b"$ next".to_vec())]
        );
        assert!(receiver.is_finished());
    }
}

fn assert_peer_cancellation_preserves_farewell_tail(wire: &[u8], split: Option<usize>) {
    let mut receiver = waiting_for_farewell();
    assert!(receiver.feed("个$ ".as_bytes()).unwrap().is_empty());
    let mut events = Vec::new();
    match split {
        Some(split) => {
            events.extend(receiver.feed(&wire[..split]).unwrap());
            events.extend(receiver.feed(&wire[split..]).unwrap());
        }
        None => {
            for byte in wire {
                events.extend(receiver.feed(&[*byte]).unwrap());
            }
        }
    }
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, Event::Cancelled))
            .count(),
        1
    );
    let terminal: Vec<u8> = events
        .iter()
        .filter_map(|event| match event {
            Event::Terminal(bytes) => Some(bytes.as_slice()),
            _ => None,
        })
        .flatten()
        .copied()
        .collect();
    let expected = if wire.starts_with(&[24; 5]) {
        "个$ next".as_bytes().to_vec()
    } else {
        ["个$ ".as_bytes(), b"\x11next"].concat()
    };
    assert_eq!(terminal, expected);
    assert!(
        events
            .iter()
            .all(|event| matches!(event, Event::Cancelled | Event::Terminal(_)))
    );
    assert!(receiver.cancel().unwrap().is_empty());
    assert!(receiver.cancel().unwrap().is_empty());
    assert!(!receiver.is_finished());
    assert!(!receiver.is_waiting_for_farewell());
    assert!(receiver.finish_without_farewell().is_err());
    assert_eq!(
        receiver.feed(b"later").unwrap(),
        vec![Event::Terminal(b"later".to_vec())]
    );
}

fn peer_cancel_frames() -> Vec<Vec<u8>> {
    [
        header(FrameType::Cancel, 0),
        header(FrameType::Abort, 0),
        header(FrameType::FileError, 0),
        vec![24; 5],
    ]
    .into_iter()
    .map(|frame| [frame.as_slice(), b"next"].concat())
    .collect()
}

#[test]
fn receiver_peer_cancel_preserves_farewell_tail_whole() {
    for wire in peer_cancel_frames() {
        assert_peer_cancellation_preserves_farewell_tail(&wire, Some(0));
    }
}

#[test]
fn receiver_peer_cancel_preserves_farewell_tail_bytewise() {
    for wire in peer_cancel_frames() {
        assert_peer_cancellation_preserves_farewell_tail(&wire, None);
    }
}

#[test]
fn receiver_peer_cancel_preserves_farewell_tail_at_every_split() {
    for wire in peer_cancel_frames() {
        for split in 0..=wire.len() {
            assert_peer_cancellation_preserves_farewell_tail(&wire, Some(split));
        }
    }
}

fn assert_failed_input_preserves_farewell_tail(chunks: Vec<&[u8]>, expected: Error) {
    let mut receiver = waiting_for_farewell();
    assert!(receiver.feed("个$ ".as_bytes()).unwrap().is_empty());
    let mut failed = false;
    for chunk in chunks {
        match receiver.feed(chunk) {
            Ok(events) => assert!(events.is_empty()),
            Err(error) => {
                assert_eq!(error, expected);
                failed = true;
                break;
            }
        }
    }
    assert!(failed);
    assert!(!receiver.is_finished());
    assert!(!receiver.is_waiting_for_farewell());
    assert!(receiver.finish_without_farewell().is_err());
    assert_eq!(
        receiver.cancel().unwrap(),
        vec![
            Event::Send([vec![24; 8], vec![8; 10]].concat()),
            Event::Cancelled,
            Event::Terminal("个$ ".as_bytes().to_vec()),
        ]
    );
    assert!(receiver.cancel().unwrap().is_empty());
    assert_eq!(
        receiver.feed(b"later").unwrap(),
        vec![Event::Terminal(b"later".to_vec())]
    );
}

fn invalid_finishing_frames() -> Vec<(Vec<u8>, Error)> {
    vec![
        (header(FrameType::Command, 0), Error::RemoteCommand),
        (
            header(FrameType::RequestInit, (FrameType::Command as u32) << 24),
            Error::RemoteCommand,
        ),
        (
            header(FrameType::File, 1 << 24),
            Error::UnexpectedFrame(FrameType::File),
        ),
        (
            header(FrameType::Data, 0),
            Error::UnexpectedFrame(FrameType::Data),
        ),
        (header(FrameType::EndOfFile, 0), Error::ApprovalRequired),
        (
            header(FrameType::SenderInit, 0),
            Error::UnexpectedFrame(FrameType::SenderInit),
        ),
    ]
}

#[test]
fn receiver_failed_input_preserves_farewell_tail_whole() {
    for (wire, error) in invalid_finishing_frames() {
        assert_failed_input_preserves_farewell_tail(vec![&wire], error);
    }
}

#[test]
fn receiver_failed_input_preserves_farewell_tail_bytewise() {
    for (wire, error) in invalid_finishing_frames() {
        assert_failed_input_preserves_farewell_tail(wire.chunks(1).collect(), error);
    }
}

#[test]
fn receiver_failed_input_preserves_farewell_tail_at_every_split() {
    for (wire, error) in invalid_finishing_frames() {
        for split in 0..=wire.len() {
            assert_failed_input_preserves_farewell_tail(
                vec![&wire[..split], &wire[split..]],
                error.clone(),
            );
        }
    }
}

#[test]
fn receiver_retry_failure_preserves_farewell_tail_during_cancel_cleanup() {
    let mut receiver = waiting_for_farewell();
    assert!(receiver.feed("个$ ".as_bytes()).unwrap().is_empty());
    let mut failed = false;
    for _ in 0..128 {
        if receiver.timeout().is_err() {
            failed = true;
            break;
        }
    }
    assert!(failed, "the bounded retry budget must fail");
    let events = receiver.cancel().unwrap();
    assert_eq!(
        events.last(),
        Some(&Event::Terminal("个$ ".as_bytes().to_vec()))
    );
    assert!(events.iter().all(|event| matches!(
        event,
        Event::Send(_) | Event::Cancelled | Event::Terminal(_)
    )));
    assert!(receiver.cancel().unwrap().is_empty());
    assert!(!receiver.is_finished());
}

#[test]
fn receiver_input_limit_failure_preserves_only_previously_retained_farewell_tail() {
    let mut receiver = waiting_for_farewell();
    assert!(receiver.feed("个$ ".as_bytes()).unwrap().is_empty());
    assert_eq!(
        receiver.feed(&vec![b'x'; MAX_FEED_BYTES + 1]),
        Err(Error::Limit("feed bytes"))
    );
    let events = receiver.cancel().unwrap();
    assert_eq!(
        events.last(),
        Some(&Event::Terminal("个$ ".as_bytes().to_vec()))
    );
    assert!(receiver.cancel().unwrap().is_empty());
    assert!(!receiver.is_finished());
}
