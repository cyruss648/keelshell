//! Bounded request IO for the owned nonblocking updater fixture.
use super::{HttpFixture, Reply};
use std::{
    io::{Read, Write},
    net::TcpStream,
    time::Duration,
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
