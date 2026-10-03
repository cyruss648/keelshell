use super::{ProxyCredentials, ProxyError};
use crate::Result;
use base64::{Engine, engine::general_purpose::STANDARD};
use std::net::IpAddr;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use zeroize::Zeroizing;

const MAX_HEADERS: usize = 16 * 1024;
const MAX_FIELDS: usize = 100;
const MAX_INFORMATIONAL: usize = 4;

pub(super) async fn connect<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
    credentials: Option<&ProxyCredentials>,
    host: &str,
    port: u16,
) -> Result<Vec<u8>> {
    let authority = match host.parse::<IpAddr>() {
        Ok(IpAddr::V6(_)) => format!("[{host}]:{port}"),
        _ => format!("{host}:{port}"),
    };
    let encoded_len = credentials.map_or(0, |credentials| {
        (credentials.username.len() + 1 + credentials.password.len()).div_ceil(3) * 4
    });
    let mut request = Zeroizing::new(String::with_capacity(
        authority.len() * 2 + 128 + encoded_len,
    ));
    request.push_str(&format!(
        "CONNECT {authority} HTTP/1.1\r\nHost: {authority}\r\n"
    ));
    if let Some(credentials) = credentials {
        let mut plain = Zeroizing::new(Vec::with_capacity(
            credentials.username.len() + 1 + credentials.password.len(),
        ));
        plain.extend_from_slice(credentials.username.as_bytes());
        plain.push(b':');
        plain.extend_from_slice(credentials.password.as_bytes());
        let mut encoded = Zeroizing::new(String::with_capacity(encoded_len));
        STANDARD.encode_string(&plain, &mut encoded);
        request.push_str("Proxy-Authorization: Basic ");
        request.push_str(&encoded);
        request.push_str("\r\n");
    }
    request.push_str("\r\n");
    stream.write_all(request.as_bytes()).await?;
    stream.flush().await?;
    drop(request);
    // Zeroizing also erases malicious responses echoing an Authorization value.
    let mut received = Zeroizing::new(Vec::with_capacity(MAX_HEADERS + 2048));
    let mut consumed = 0;
    let mut informational = 0;
    let mut chunk = Zeroizing::new(vec![0; 2048]);
    loop {
        if let Some(start) = received.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
            let end = start + 4;
            consumed += end;
            if consumed > MAX_HEADERS {
                return Err(ProxyError::ResponseTooLarge.into());
            }
            let status = parse_headers(&received[..end])?;
            if (200..300).contains(&status) {
                // RFC 9110 CONNECT ignores Content-Length / Transfer-Encoding.
                // Anything after the terminating CRLF is already tunnel data.
                return Ok(received[end..].to_vec());
            }
            if (100..200).contains(&status) && status != 101 {
                informational += 1;
                if informational > MAX_INFORMATIONAL {
                    return Err(ProxyError::ResponseTooLarge.into());
                }
                received.drain(..end);
                continue;
            }
            return Err(if status == 407 {
                if credentials.is_some() {
                    ProxyError::AuthenticationRejected
                } else {
                    ProxyError::AuthenticationRequired
                }
            } else {
                ProxyError::HttpRejected(status)
            }
            .into());
        }
        if consumed + received.len() >= MAX_HEADERS {
            return Err(ProxyError::ResponseTooLarge.into());
        }
        let count = stream.read(&mut chunk).await?;
        if count == 0 {
            return Err(ProxyError::InvalidResponse("incomplete HTTP headers").into());
        }
        received.extend_from_slice(&chunk[..count]);
    }
}

fn parse_headers(headers: &[u8]) -> std::result::Result<u16, ProxyError> {
    let invalid = || ProxyError::InvalidResponse("HTTP header syntax");
    let mut lines = headers.split(|byte| *byte == b'\n');
    let status_line = lines
        .next()
        .and_then(|line| line.strip_suffix(b"\r"))
        .ok_or_else(invalid)?;
    if status_line.len() < 13
        || !(status_line.starts_with(b"HTTP/1.1 ") || status_line.starts_with(b"HTTP/1.0 "))
        || !status_line[9..12].iter().all(u8::is_ascii_digit)
        || status_line[12] != b' '
        || status_line[13..]
            .iter()
            .any(|byte| (*byte < 32 && *byte != b'\t') || *byte == 127)
    {
        return Err(invalid());
    }
    let status = u16::from(status_line[9] - b'0') * 100
        + u16::from(status_line[10] - b'0') * 10
        + u16::from(status_line[11] - b'0');
    if !(100..600).contains(&status) {
        return Err(invalid());
    }
    for (index, line) in lines.enumerate() {
        let line = line.strip_suffix(b"\r").ok_or_else(invalid)?;
        if line.is_empty() {
            return Ok(status);
        }
        if index >= MAX_FIELDS {
            return Err(ProxyError::ResponseTooLarge);
        }
        let colon = line
            .iter()
            .position(|byte| *byte == b':')
            .ok_or_else(invalid)?;
        if colon == 0
            || !line[..colon]
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(byte))
            || line[colon + 1..]
                .iter()
                .any(|byte| (*byte < 32 && *byte != b'\t') || *byte == 127)
        {
            return Err(invalid());
        }
    }
    Err(invalid())
}
