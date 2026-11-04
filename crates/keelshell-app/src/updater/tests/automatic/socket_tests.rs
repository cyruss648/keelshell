//! Bounded request IO for the owned nonblocking updater fixture.
use super::{FIXTURE_LIFE, HttpFixture, REQUEST_BOUND, Reply, poll_bounded, read_bounded};
use std::{
    io::{Read, Write},
    net::TcpStream,
    time::{Duration, Instant},
};

fn connect(endpoint: &str) -> TcpStream {
    let address = endpoint
        .strip_prefix("http://")
        .and_then(|value| value.split_once('/').map(|(address, _)| address))
        .expect("owned fixture endpoint");
    let stream = TcpStream::connect_timeout(
        &address.parse().expect("fixture address"),
        Duration::from_secs(2),
    )
    .expect("connect to owned fixture");
    stream
        .set_nonblocking(true)
        .expect("exercise partial fixture reads");
    stream
}

fn write_exact_bounded(stream: &mut TcpStream, mut bytes: &[u8]) {
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while !bytes.is_empty() {
        assert!(
            std::time::Instant::now() < deadline,
            "partial request write expired"
        );
        match stream.write(bytes) {
            Ok(0) => panic!("fixture peer accepted no request bytes"),
            Ok(written) => bytes = &bytes[written..],
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(2));
            }
            Err(error) => panic!("partial request write: {error}"),
        }
    }
}

#[test]
fn partial_headers_resume_without_resetting_the_request_deadline() {
    let fixture = HttpFixture::new(vec![Reply {
        path: "/latest",
        status: 204,
        body: Vec::new(),
        held: false,
    }]);
    let mut stream = connect(&fixture.endpoint("/latest"));
    let request = b"GET /latest HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";
    write_exact_bounded(&mut stream, &request[..8]);
    std::thread::sleep(Duration::from_millis(20));
    stream
        .set_nonblocking(false)
        .expect("deliver the remaining header");
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .expect("request write bound");
    stream
        .write_all(&request[8..])
        .expect("complete fragmented request");
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("response read bound");
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .expect("complete fixture response");
    assert!(response.starts_with("HTTP/1.1 204 fixture\r\n"));
}

#[test]
fn idle_partial_request_expires_without_aborting_cleanup() {
    let started = std::time::Instant::now();
    let fixture = HttpFixture::new(vec![Reply {
        path: "/latest",
        status: 204,
        body: Vec::new(),
        held: false,
    }]);
    let mut stream = connect(&fixture.endpoint("/latest"));
    write_exact_bounded(&mut stream, b"GET /late");
    drop(stream);
    drop(fixture);
    assert!(started.elapsed() < Duration::from_secs(4));
}

#[test]
fn peer_closes_before_request_completion() {
    let fixture = HttpFixture::new(vec![Reply {
        path: "/latest",
        status: 204,
        body: Vec::new(),
        held: false,
    }]);
    let mut stream = connect(&fixture.endpoint("/latest"));
    write_exact_bounded(&mut stream, b"GET /latest HTTP/1.1\r\n");
    stream
        .shutdown(std::net::Shutdown::Both)
        .expect("close partial request");
    drop(stream);
    drop(fixture);
}

#[test]
fn cancelled_held_response_does_not_abort_the_following_scripted_reply() {
    const CANCELLED_BODY_LEN: usize = 32 * 1024 * 1024;
    let following_body = b"following reply: complete\r\nwith distinct bytes\x00\xff";
    let started = Instant::now();
    let mut fixture = HttpFixture::new(vec![
        Reply {
            path: "/cancelled",
            status: 200,
            body: vec![0xa5; CANCELLED_BODY_LEN],
            held: true,
        },
        Reply {
            path: "/following",
            status: 201,
            body: following_body.to_vec(),
            held: false,
        },
    ]);
    let mut cancelled = connect(&fixture.endpoint("/cancelled"));
    write_exact_bounded(
        &mut cancelled,
        b"GET /cancelled HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
    );
    assert_eq!(
        fixture
            .observed
            .recv_timeout(REQUEST_BOUND)
            .expect("held request reached the fixture"),
        "/cancelled"
    );

    // Queue the next request before releasing the held response so a stopped
    // server is detected while reading its reply, not by a later connect race.
    let mut following = connect(&fixture.endpoint("/following"));
    write_exact_bounded(
        &mut following,
        b"GET /following HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
    );
    fixture.release();
    let cancelled_deadline = Instant::now() + REQUEST_BOUND;
    let expected_header = format!(
        "HTTP/1.1 200 fixture\r\nContent-Length: {CANCELLED_BODY_LEN}\r\nConnection: close\r\n\r\n"
    );
    let mut header = vec![0; expected_header.len()];
    read_bounded(
        &mut cancelled,
        &mut header,
        cancelled_deadline,
        &fixture.stopped,
    )
    .expect("held response header");
    let mut prefix = [0; 1024];
    read_bounded(
        &mut cancelled,
        &mut prefix,
        cancelled_deadline,
        &fixture.stopped,
    )
    .expect("held response began its large body");
    let mut unread = [0];
    let peeked = poll_bounded(cancelled_deadline, &fixture.stopped, || {
        cancelled.peek(&mut unread)
    })
    .expect("unread response bytes before cancellation");
    // A tiny response can fit in socket buffers even after its client closes.
    // Read only a prefix of this large body, leave actual bytes unread, and
    // close while the server still has the rest of the body to write.
    drop(cancelled);
    assert_eq!(header, expected_header.as_bytes());
    assert_eq!(prefix, [0xa5; 1024]);
    assert_eq!(peeked, 1);
    assert_eq!(unread, [0xa5]);

    let mut expected_response = format!(
        "HTTP/1.1 201 fixture\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        following_body.len()
    )
    .into_bytes();
    expected_response.extend_from_slice(following_body);
    let mut response = vec![0; expected_response.len()];
    let following_deadline = Instant::now() + REQUEST_BOUND;
    let received = read_bounded(
        &mut following,
        &mut response,
        following_deadline,
        &fixture.stopped,
    )
    .and_then(|()| {
        poll_bounded(following_deadline, &fixture.stopped, || {
            following.read(&mut [0])
        })
    });
    drop(following);

    // Reap the owner before asserting delivery, including on the old fixture's
    // failure path; Drop remains the fallback for earlier assertion failures.
    fixture.finish();
    assert!(fixture.owner.is_none(), "fixture owner was joined");
    assert!(started.elapsed() < FIXTURE_LIFE, "bounded fixture cleanup");
    assert_eq!(
        received.expect("following scripted reply must survive the cancelled response"),
        0,
        "following response ends exactly at its declared length"
    );
    assert_eq!(response, expected_response);
    assert_eq!(
        fixture.observed.try_iter().collect::<Vec<_>>(),
        ["/following"]
    );
}
