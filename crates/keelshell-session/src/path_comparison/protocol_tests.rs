#![cfg(test)]
//! Control-message rejection at the decoded-message boundary. In particular,
//! russh's public server API suppresses duplicate ACKs using wants_reply, so
//! calling channel_success twice cannot provide encrypted wire evidence.

use super::*;

fn valid_messages(kind: PathProbeKind) -> Vec<ChannelMsg> {
    let output = match kind {
        PathProbeKind::Exec => EXEC_REPLY.to_vec(),
        PathProbeKind::Upload => UPLOAD_REPLY.to_vec(),
        PathProbeKind::Download => vec![PAYLOAD_BYTE; PAYLOAD_BYTES],
    };
    vec![
        ChannelMsg::Success,
        ChannelMsg::Data {
            data: output.into(),
        },
        ChannelMsg::Eof,
        ChannelMsg::ExitStatus { exit_status: 0 },
        ChannelMsg::Close,
    ]
}

#[test]
fn path_comparison_duplicate_controls_cannot_be_repaired_by_a_valid_tail() {
    for kind in [
        PathProbeKind::Exec,
        PathProbeKind::Upload,
        PathProbeKind::Download,
    ] {
        let mut valid = Protocol::new(kind);
        for message in valid_messages(kind) {
            valid.observe(Some(message));
        }
        assert!(valid.finished());
        assert_eq!(valid.outcome(), PathSampleOutcome::Passed);

        for duplicate_index in [0, 2, 3] {
            let mut protocol = Protocol::new(kind);
            for (index, message) in valid_messages(kind).into_iter().enumerate() {
                protocol.observe(Some(message));
                if index == duplicate_index {
                    protocol.observe(Some(match index {
                        0 => ChannelMsg::Success,
                        2 => ChannelMsg::Eof,
                        3 => ChannelMsg::ExitStatus { exit_status: 0 },
                        _ => unreachable!("only known control-message indexes are tested"),
                    }));
                }
            }
            assert!(protocol.finished(), "{kind:?}, control {duplicate_index}");
            assert_eq!(
                protocol.outcome(),
                PathSampleOutcome::Failed(Failure::InvalidSequence),
                "{kind:?}, control {duplicate_index}"
            );
        }
    }
}
