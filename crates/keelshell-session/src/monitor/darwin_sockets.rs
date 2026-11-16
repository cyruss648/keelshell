//! Strict, account-visible Darwin TCP LISTEN collection using system lsof.
//!
//! The wrapper retains lsof's status independently of SSH completion. A quiet
//! status 1 with no matches is distinct from failed, partial or warning-bearing
//! output. NUL fields and newline set boundaries must both remain intact.

use std::{
    collections::{BTreeMap, BTreeSet},
    net::{Ipv4Addr, Ipv6Addr},
};

use super::{
    Capability, CapabilityReason, ExecOutput, MAX_OUTPUT, MonitorError, MonitorResult, SocketInfo,
    failure_reason,
};

pub(super) const START: &str = "@@KS:darwin-sockets:1@@\n";
const STATUS: &str = "@@KS:lsof-status@@\n";
const END: &str = "\n@@KS:end@@\n";
const MAX_SOCKETS: usize = 10_000;
const MAX_COMMAND: usize = 256;
const MAX_SET: usize = 1024;
const MAX_ENDPOINT: usize = 128;

pub(super) fn parse(output: ExecOutput) -> MonitorResult<Vec<SocketInfo>> {
    if output.stdout.len().saturating_add(output.stderr.len()) > MAX_OUTPUT {
        return invalid("Darwin socket output exceeds 2 MiB");
    }
    if output.exit_status != Some(0) {
        return Err(MonitorError::RemoteFailure {
            operation: "Darwin socket collection",
            exit_status: output.exit_status,
        });
    }
    let input = std::str::from_utf8(&output.stdout)
        .map_err(|_| MonitorError::InvalidData("non-UTF-8 Darwin socket output"))?;
    let input = input
        .strip_prefix(START)
        .ok_or(MonitorError::InvalidData("missing Darwin socket frame"))?;
    let (body, status) = input
        .split_once(STATUS)
        .ok_or(MonitorError::InvalidData("missing Darwin lsof status"))?;
    let status = status
        .strip_suffix(END)
        .ok_or(MonitorError::InvalidData("incomplete Darwin socket frame"))?;
    let status =
        decimal(status, u32::MAX).ok_or(MonitorError::InvalidData("invalid Darwin lsof status"))?;
    // Warnings can describe hidden or uninspectable sockets even on exit 0.
    // Never suppress stderr or publish such a partial inventory as Available.
    if !output.stderr.is_empty() {
        return unavailable(failure_reason(status, &output.stderr));
    }
    match (status, body.is_empty()) {
        // With this fixed selection, lsof documents no matching files as 1.
        // Both wrapper boundaries and a quiet, empty body are required.
        (1, true) => Ok(Vec::new()),
        (0, false) => parse_sets(body),
        (0, true) => invalid("empty successful Darwin socket output"),
        _ => unavailable(failure_reason(status, &[])),
    }
}

fn parse_sets(body: &str) -> MonitorResult<Vec<SocketInfo>> {
    let body = body
        .strip_suffix('\n')
        .ok_or(MonitorError::InvalidData("incomplete Darwin lsof set"))?;
    let mut sockets = Vec::new();
    let mut process: Option<(u32, &str)> = None;
    let mut process_has_files = false;
    let mut pids = BTreeSet::new();
    let mut descriptors = BTreeSet::new();
    for record in body.split('\n') {
        if record.len() > MAX_SET {
            return invalid("Darwin lsof set exceeds its bound");
        }
        let fields = fields(record)?;
        match record.as_bytes().first() {
            Some(b'p') => {
                if process.is_some() && !process_has_files {
                    return invalid("Darwin lsof process has no files");
                }
                if fields.len() != 2 {
                    return invalid("invalid Darwin lsof process fields");
                }
                let pid = required(&fields, b'p')?;
                let pid = decimal(pid, i32::MAX as u32)
                    .filter(|pid| *pid != 0)
                    .ok_or(MonitorError::InvalidData("invalid Darwin socket PID"))?;
                let command = required(&fields, b'c')?;
                if command.len() > MAX_COMMAND || command.chars().any(char::is_control) {
                    return invalid("invalid Darwin socket command");
                }
                if pids.len() == MAX_SOCKETS || !pids.insert(pid) {
                    return invalid("duplicate or excessive Darwin socket processes");
                }
                process = Some((pid, command));
                process_has_files = false;
                descriptors.clear();
            }
            Some(b'f') => {
                let (pid, command) =
                    process.ok_or(MonitorError::InvalidData("Darwin socket has no process"))?;
                if fields.len() != 5 {
                    return invalid("invalid Darwin lsof socket fields");
                }
                let fd = decimal(required(&fields, b'f')?, i32::MAX as u32).ok_or(
                    MonitorError::InvalidData("invalid Darwin socket descriptor"),
                )?;
                if !descriptors.insert(fd) {
                    return invalid("duplicate Darwin socket descriptor");
                }
                if required(&fields, b'P')? != "TCP" || required(&fields, b'T')? != "ST=LISTEN" {
                    return invalid("Darwin socket is not TCP LISTEN");
                }
                let local = endpoint(required(&fields, b'n')?, required(&fields, b't')?)?;
                if sockets.len() == MAX_SOCKETS {
                    return invalid("Darwin socket count exceeds 10000");
                }
                sockets.push(SocketInfo {
                    protocol: "tcp".into(),
                    state: "LISTEN".into(),
                    local,
                    peer: "*:*".into(),
                    process: Some(format!("{command} (pid={pid})")),
                });
                process_has_files = true;
            }
            _ => return invalid("invalid Darwin lsof set boundary"),
        }
    }
    if !process_has_files {
        return invalid("Darwin lsof process has no files");
    }
    Ok(sockets)
}

fn fields(record: &str) -> MonitorResult<BTreeMap<u8, &str>> {
    let record = record.strip_suffix('\0').ok_or(MonitorError::InvalidData(
        "missing Darwin lsof NUL terminator",
    ))?;
    let mut fields = BTreeMap::new();
    for field in record.split('\0') {
        let Some((&tag, value)) = field.as_bytes().split_first() else {
            return invalid("empty Darwin lsof field");
        };
        if !matches!(tag, b'p' | b'c' | b'f' | b't' | b'P' | b'n' | b'T') {
            return invalid("unexpected Darwin lsof field");
        }
        // All accepted identifiers are one-byte ASCII; slicing cannot split
        // an encoded character after validating the tag.
        let value = std::str::from_utf8(value)
            .map_err(|_| MonitorError::InvalidData("invalid Darwin lsof field"))?;
        if value.is_empty() || fields.insert(tag, value).is_some() {
            return invalid("empty or duplicate Darwin lsof field");
        }
    }
    Ok(fields)
}

fn required<'a>(fields: &BTreeMap<u8, &'a str>, tag: u8) -> MonitorResult<&'a str> {
    fields
        .get(&tag)
        .copied()
        .ok_or(MonitorError::InvalidData("missing Darwin lsof field"))
}

fn endpoint(input: &str, family: &str) -> MonitorResult<String> {
    if input.len() > MAX_ENDPOINT || !input.is_ascii() {
        return invalid("invalid Darwin socket endpoint");
    }
    let (host, port) = input
        .rsplit_once(':')
        .ok_or(MonitorError::InvalidData("missing Darwin socket port"))?;
    let port = decimal(port, u16::MAX as u32)
        .filter(|port| *port != 0)
        .ok_or(MonitorError::InvalidData("invalid Darwin socket port"))?;
    match family {
        "IPv4" if host == "*" => Ok(format!("0.0.0.0:{port}")),
        "IPv6" if host == "*" => Ok(format!("[::]:{port}")),
        "IPv4" => {
            let address: Ipv4Addr = host
                .parse()
                .map_err(|_| MonitorError::InvalidData("invalid Darwin IPv4 address"))?;
            Ok(format!("{address}:{port}"))
        }
        "IPv6" => {
            // Darwin lsof prints an IPv4-mapped IPv6 socket as an unbracketed
            // IPv4 endpoint. Preserve that observed address, without inventing
            // an IPv6 address from the descriptor's family.
            if let Ok(address) = host.parse::<Ipv4Addr>() {
                return Ok(format!("{address}:{port}"));
            }
            let address = host
                .strip_prefix('[')
                .and_then(|host| host.strip_suffix(']'))
                .ok_or(MonitorError::InvalidData("invalid Darwin IPv6 address"))?;
            let (address, scope) = match address.split_once('%') {
                Some((address, scope)) => {
                    if scope.is_empty()
                        || scope.len() > 64
                        || !scope
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
                    {
                        return invalid("invalid Darwin IPv6 scope");
                    }
                    (address, Some(scope))
                }
                None => (address, None),
            };
            let address: Ipv6Addr = address
                .parse()
                .map_err(|_| MonitorError::InvalidData("invalid Darwin IPv6 address"))?;
            Ok(match scope {
                Some(scope) => format!("[{address}%{scope}]:{port}"),
                None => format!("[{address}]:{port}"),
            })
        }
        _ => invalid("invalid Darwin socket address family"),
    }
}

fn decimal(input: &str, max: u32) -> Option<u32> {
    if input.is_empty() || input.len() > 10 || !input.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    input.parse::<u32>().ok().filter(|value| *value <= max)
}

fn unavailable<T>(reason: CapabilityReason) -> MonitorResult<T> {
    Err(MonitorError::CapabilityUnavailable {
        capability: Capability::Sockets,
        reason,
    })
}

fn invalid<T>(message: &'static str) -> MonitorResult<T> {
    Err(MonitorError::InvalidData(message))
}

#[cfg(test)]
mod tests;
