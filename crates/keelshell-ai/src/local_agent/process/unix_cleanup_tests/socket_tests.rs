//! Real socket regressions for the bounded, authenticated fixture protocol.

use std::{net::Shutdown, os::unix::net::UnixStream, sync::mpsc};

use super::*;

fn identity() -> MemberIdentity {
    MemberIdentity {
        pid: 42,
        group: 17,
        nonce: Uuid::new_v4(),
    }
}

#[test]
fn member_socket_identity_probe_returns_complete_matching_identity() {
    let identity = identity();
    let (mut client, mut server) = UnixStream::pair().unwrap();
    client.write_all(b"I").unwrap();
    assert!(
        !answer_member_request(
            &mut server,
            &identity,
            StdInstant::now() + Duration::from_secs(1),
        )
        .unwrap()
    );
    drop(server);
    let mut response = Vec::new();
    client.read_to_end(&mut response).unwrap();
    assert_eq!(
        serde_json::from_slice::<MemberIdentity>(&response).unwrap(),
        identity
    );
}

#[test]
fn member_socket_fragmented_nonce_is_required_before_stop() {
    let identity = identity();
    let (mut client, mut server) = UnixStream::pair().unwrap();
    let (result_tx, result_rx) = mpsc::sync_channel(1);
    client.write_all(b"Q").unwrap();
    client.write_all(&identity.nonce.as_bytes()[..8]).unwrap();
    std::thread::scope(|scope| {
        scope.spawn(|| {
            let result = answer_member_request(
                &mut server,
                &identity,
                StdInstant::now() + Duration::from_secs(1),
            );
            result_tx.send(result).unwrap();
        });
        assert!(matches!(
            result_rx.recv_timeout(Duration::from_millis(10)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        client.write_all(&identity.nonce.as_bytes()[8..]).unwrap();
        assert!(
            result_rx
                .recv_timeout(Duration::from_secs(1))
                .unwrap()
                .unwrap()
        );
    });
    drop(server);
    let mut response = Vec::new();
    client.read_to_end(&mut response).unwrap();
    assert_eq!(
        serde_json::from_slice::<MemberIdentity>(&response).unwrap(),
        identity
    );
}

#[test]
fn member_socket_partial_nonce_eof_and_wrong_nonce_do_not_authorize_stop() {
    let identity = identity();
    for partial in [true, false] {
        let (mut client, mut server) = UnixStream::pair().unwrap();
        client.write_all(b"Q").unwrap();
        if partial {
            client.write_all(&identity.nonce.as_bytes()[..8]).unwrap();
        } else {
            client.write_all(Uuid::new_v4().as_bytes()).unwrap();
        }
        client.shutdown(Shutdown::Write).unwrap();
        let result = answer_member_request(
            &mut server,
            &identity,
            StdInstant::now() + Duration::from_secs(1),
        );
        if partial {
            assert_eq!(result.unwrap_err().kind(), ErrorKind::UnexpectedEof);
        } else {
            assert!(!result.unwrap());
        }
        drop(server);
        let mut response = Vec::new();
        client.read_to_end(&mut response).unwrap();
        assert!(response.is_empty());
    }
}

#[test]
fn member_socket_silent_and_partial_requests_expire_at_the_parent_deadline() {
    for bytes in [&b""[..], &b"Qpartial"[..]] {
        let identity = identity();
        let (mut client, mut server) = UnixStream::pair().unwrap();
        client.write_all(bytes).unwrap();
        let deadline = StdInstant::now() + Duration::from_millis(10);
        let result = answer_member_request(&mut server, &identity, deadline);
        assert_eq!(result.unwrap_err().kind(), ErrorKind::TimedOut);
        assert!(StdInstant::now() >= deadline);
    }
}

#[test]
fn member_socket_expired_deadline_cannot_accept_an_already_buffered_stop() {
    let identity = identity();
    let (mut client, mut server) = UnixStream::pair().unwrap();
    client.write_all(b"Q").unwrap();
    client.write_all(identity.nonce.as_bytes()).unwrap();
    let result = answer_member_request(&mut server, &identity, StdInstant::now());
    assert_eq!(result.unwrap_err().kind(), ErrorKind::TimedOut);
    let mut unread = [0; 17];
    server.read_exact(&mut unread).unwrap();
    assert_eq!(unread[0], b'Q');
    assert_eq!(&unread[1..], identity.nonce.as_bytes());
}

#[test]
fn member_socket_backpressure_preserves_the_write_deadline_after_partial_progress() {
    let (mut client, mut server) = UnixStream::pair().unwrap();
    client.set_nonblocking(true).unwrap();
    server.set_nonblocking(true).unwrap();
    let payload = vec![b'x'; 8 * 1024 * 1024];
    let deadline = StdInstant::now() + Duration::from_millis(100);
    assert_eq!(
        write_fixture_all(&mut server, &payload, deadline)
            .unwrap_err()
            .kind(),
        ErrorKind::TimedOut
    );
    assert!(StdInstant::now() >= deadline);
    let mut received = Vec::new();
    assert_eq!(
        client.read_to_end(&mut received).unwrap_err().kind(),
        ErrorKind::WouldBlock
    );
    assert!(
        !received.is_empty(),
        "the socket must make partial progress"
    );
    assert!(received.len() < payload.len());
    assert!(received.iter().all(|byte| *byte == b'x'));
}

#[test]
fn member_socket_closed_peer_returns_write_error_but_authenticated_stop_still_exits() {
    let (client, mut server) = UnixStream::pair().unwrap();
    server.set_nonblocking(true).unwrap();
    drop(client);
    assert_eq!(
        write_fixture_all(
            &mut server,
            b"reply",
            StdInstant::now() + Duration::from_secs(1)
        )
        .unwrap_err()
        .kind(),
        ErrorKind::BrokenPipe,
    );

    let identity = identity();
    let (mut client, mut server) = UnixStream::pair().unwrap();
    client.write_all(b"Q").unwrap();
    client.write_all(identity.nonce.as_bytes()).unwrap();
    drop(client);
    assert!(
        answer_member_request(
            &mut server,
            &identity,
            StdInstant::now() + Duration::from_secs(1),
        )
        .unwrap()
    );
}
