//! Bounded Darwin resource collection, without installing an agent.
//!
//! Schema 1 carries explicit units and the integer Mach clock conversion.
//! Free pages are not a kernel available-memory estimate. Disk I/O, process
//! actions and socket actions are deliberately outside this collector.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    CapabilityReason, CapabilityState, CpuCounters, DiskIoError, FilesystemSnapshot,
    MemorySnapshot, MonitorError, MonitorPlatform, MonitorResult, NetworkCounters, Snapshot,
};

const MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_INTERFACES: usize = 256;
const MAX_FILESYSTEMS: usize = 512;
const START: &str = "@@KS:darwin@@\n";
const DF_MARKER: &str = "\n@@KS:df@@\n";
const END: &str = "\n@@KS:end@@\n";
const UNITS: &str = "bytes seconds ticks pages nanoseconds";
const COLLECTOR: &str = include_str!("darwin.py");
const KEYS: &[&str] = &[
    "schema",
    "platform",
    "units",
    "boot_time",
    "boot_time_usec",
    "mach_ticks",
    "timebase_numer",
    "timebase_denom",
    "uptime_ns",
    "page_size",
    "sysconf_page_size",
    "memory_total_bytes",
    "memory_free_pages",
    "memory_free_bytes",
    "cpu_user",
    "cpu_nice",
    "cpu_system",
    "cpu_idle",
    "load_1",
    "load_5",
    "load_15",
    "network_count",
    "df_status",
    "df_bytes",
];

/// Build a fixed stdin-only Python 3 command with isolated interpreter startup.
///
/// A missing interpreter exits with status 66, the parent's explicit
/// `CapabilityReason::CommandMissing` contract. `skip_capacity` bypasses df
/// entirely and returns an unobserved capacity state.
pub(super) fn script(skip_capacity: bool) -> String {
    format!(
        "LC_ALL=C; export LC_ALL\n\
         ks_python=''\n\
         for ks_candidate in /usr/bin/python3 /opt/homebrew/bin/python3 /usr/local/bin/python3; do\n\
           if [ -f \"$ks_candidate\" ] && [ -x \"$ks_candidate\" ]; then ks_python=\"$ks_candidate\"; break; fi\n\
         done\n\
         [ -n \"$ks_python\" ] || exit 66\n\
         \"$ks_python\" -I -S -B - {} 2>/dev/null <<'KS_DARWIN_PY'\n\
         {COLLECTOR}\nKS_DARWIN_PY\n",
        u8::from(skip_capacity),
    )
}

/// Validate a complete schema-1 Darwin frame and its optional capacity result.
///
/// Errors contain only fixed descriptions, never remote output or stderr.
/// Invalid df rows leave core resources usable; invalid framing, core fields,
/// duplicates, units or clocks reject the whole frame.
pub(super) fn parse(input: &str, stderr: &[u8]) -> MonitorResult<(Snapshot, CapabilityState)> {
    if input.len().saturating_add(stderr.len()) > MAX_BYTES {
        return invalid("Darwin output exceeds 2 MiB");
    }
    if input.bytes().any(|byte| byte == 0 || byte == b'\r') {
        return invalid("invalid Darwin output character");
    }
    let body = input
        .strip_prefix(START)
        .ok_or(MonitorError::InvalidData("missing Darwin frame"))?;
    let (header, tail) = body
        .split_once(DF_MARKER)
        .ok_or(MonitorError::InvalidData("missing Darwin capacity frame"))?;
    let (fields, networks) = parse_header(header)?;
    if field(&fields, "schema")? != "1" {
        return invalid("unsupported Darwin schema");
    }
    if field(&fields, "platform")? != "Darwin" {
        return invalid("invalid Darwin platform");
    }
    if field(&fields, "units")? != UNITS {
        return invalid("invalid Darwin units");
    }
    let df_bytes = decimal(field(&fields, "df_bytes")?)?;
    let df_len = usize::try_from(df_bytes)
        .map_err(|_| MonitorError::InvalidData("Darwin capacity length overflow"))?;
    let df = tail
        .get(..df_len)
        .ok_or(MonitorError::InvalidData("truncated Darwin capacity"))?;
    if tail.get(df_len..) != Some(END) {
        return invalid("incomplete or trailing Darwin output");
    }
    if df
        .lines()
        .any(|line| matches!(line, "@@KS:darwin@@" | "@@KS:df@@" | "@@KS:end@@"))
    {
        return invalid("repeated Darwin frame marker");
    }
    let count = decimal(field(&fields, "network_count")?)?;
    if count == 0 || count > MAX_INTERFACES as u64 || count != networks.len() as u64 {
        return invalid("invalid Darwin interface count");
    }
    let boot_time = decimal(field(&fields, "boot_time")?)?;
    if boot_time == 0
        || boot_time > i64::MAX as u64
        || decimal(field(&fields, "boot_time_usec")?)? >= 1_000_000
    {
        return invalid("invalid Darwin boot timestamp");
    }
    let uptime_seconds = parse_clock(&fields)?;
    let memory = parse_memory(&fields)?;
    // Mach's native order is user/system/idle/nice. The wire names prevent
    // that ABI order from being mistaken for the public user/nice/system/idle.
    let mut ticks = [0; 8];
    for (slot, name) in ["cpu_user", "cpu_nice", "cpu_system", "cpu_idle"]
        .into_iter()
        .enumerate()
    {
        ticks[slot] = word(field(&fields, name)?)?;
    }
    let load_average = [
        load(field(&fields, "load_1")?)?,
        load(field(&fields, "load_5")?)?,
        load(field(&fields, "load_15")?)?,
    ];
    let (filesystems, capacity) = capacity(field(&fields, "df_status")?, df)?;
    Ok((
        Snapshot {
            platform: MonitorPlatform::Darwin,
            boot_time,
            uptime_seconds,
            load_average,
            cpu: CpuCounters { ticks },
            memory,
            networks,
            filesystems,
            disk_io: Err(DiskIoError::MissingData),
        },
        capacity,
    ))
}

fn invalid<T>(message: &'static str) -> MonitorResult<T> {
    Err(MonitorError::InvalidData(message))
}

fn parse_header(header: &str) -> MonitorResult<(BTreeMap<&str, &str>, Vec<NetworkCounters>)> {
    let mut fields = BTreeMap::new();
    let mut networks = Vec::new();
    let mut names = BTreeSet::new();
    for line in header.split('\n') {
        let (key, value) = line
            .split_once('\t')
            .ok_or(MonitorError::InvalidData("invalid Darwin field"))?;
        if key == "network" {
            if networks.len() == MAX_INTERFACES {
                return invalid("too many Darwin interfaces");
            }
            let mut parts = value.split('\t');
            let name = parts
                .next()
                .ok_or(MonitorError::InvalidData("missing Darwin interface"))?;
            if !interface_name(name) || !names.insert(name) {
                return invalid("invalid or duplicate Darwin interface");
            }
            let received_bytes = decimal(
                parts
                    .next()
                    .ok_or(MonitorError::InvalidData("missing Darwin received bytes"))?,
            )?;
            let transmitted_bytes = decimal(parts.next().ok_or(MonitorError::InvalidData(
                "missing Darwin transmitted bytes",
            ))?)?;
            if parts.next().is_some() {
                return invalid("extra Darwin interface field");
            }
            networks.push(NetworkCounters {
                name: name.to_owned(),
                received_bytes,
                transmitted_bytes,
            });
        } else if !KEYS.contains(&key)
            || value.is_empty()
            || value.contains('\t')
            || fields.insert(key, value).is_some()
        {
            return invalid("unknown or repeated Darwin field");
        }
    }
    if fields.len() != KEYS.len() {
        return invalid("missing Darwin field");
    }
    Ok((fields, networks))
}

fn field<'a>(fields: &BTreeMap<&str, &'a str>, name: &str) -> MonitorResult<&'a str> {
    fields
        .get(name)
        .copied()
        .ok_or(MonitorError::InvalidData("missing Darwin field"))
}

fn decimal(value: &str) -> MonitorResult<u64> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return invalid("invalid Darwin unsigned integer");
    }
    value
        .parse()
        .map_err(|_| MonitorError::InvalidData("Darwin integer overflow"))
}

fn word(value: &str) -> MonitorResult<u64> {
    let value = decimal(value)?;
    if value > u32::MAX as u64 {
        return invalid("Darwin Mach word overflow");
    }
    Ok(value)
}

fn load(value: &str) -> MonitorResult<f64> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b'.')
    {
        return invalid("invalid Darwin load average");
    }
    let number: f64 = value
        .parse()
        .map_err(|_| MonitorError::InvalidData("invalid Darwin load average"))?;
    if !number.is_finite() || number < 0.0 {
        return invalid("invalid Darwin load average");
    }
    Ok(number)
}

fn parse_clock(fields: &BTreeMap<&str, &str>) -> MonitorResult<f64> {
    let ticks = decimal(field(fields, "mach_ticks")?)?;
    let numer = word(field(fields, "timebase_numer")?)?;
    let denom = word(field(fields, "timebase_denom")?)?;
    let ns = decimal(field(fields, "uptime_ns")?)?;
    if ticks == 0 || numer == 0 || denom == 0 || ns == 0 {
        return invalid("invalid Darwin Mach clock");
    }
    // A u64 tick count times a u32 numerator fits u128; convert to seconds
    // only after checking the collector's exact integer nanosecond result.
    let expected = u128::from(ticks) * u128::from(numer) / u128::from(denom);
    if expected != u128::from(ns) {
        return invalid("inconsistent Darwin Mach clock");
    }
    Ok(ns as f64 / 1_000_000_000.0)
}

fn parse_memory(fields: &BTreeMap<&str, &str>) -> MonitorResult<MemorySnapshot> {
    let page = decimal(field(fields, "page_size")?)?;
    let total = decimal(field(fields, "memory_total_bytes")?)?;
    let pages = word(field(fields, "memory_free_pages")?)?;
    let free = decimal(field(fields, "memory_free_bytes")?)?;
    if !(4096..=65536).contains(&page)
        || !page.is_power_of_two()
        || page != decimal(field(fields, "sysconf_page_size")?)?
        || total == 0
        || total % page != 0
        // HOST_VM_INFO pegs overflowing counters to UINT32_MAX. That sentinel
        // cannot establish the actual number of free pages.
        || pages == u32::MAX as u64
        || pages.checked_mul(page) != Some(free)
        || free > total
    {
        return invalid("inconsistent Darwin memory counters");
    }
    Ok(MemorySnapshot {
        total_bytes: total,
        available_bytes: None,
        free_bytes: Some(free),
        swap_total_bytes: None,
        swap_free_bytes: None,
    })
}

fn interface_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() < 16
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

fn capacity(status: &str, df: &str) -> MonitorResult<(Vec<FilesystemSnapshot>, CapabilityState)> {
    let reason = match status {
        "ok" => {
            return Ok(match filesystems(df) {
                Ok(rows) => (rows, CapabilityState::Available),
                Err(_) => (
                    Vec::new(),
                    CapabilityState::Unavailable(CapabilityReason::InvalidOutput),
                ),
            });
        }
        "skipped" if df.is_empty() => return Ok((Vec::new(), CapabilityState::Unknown)),
        "missing" => CapabilityReason::CommandMissing,
        "permission_denied" => CapabilityReason::PermissionDenied,
        "failed" | "timeout" => CapabilityReason::CommandFailed,
        "invalid_output" => CapabilityReason::InvalidOutput,
        _ => return invalid("invalid Darwin capacity status"),
    };
    if !df.is_empty() {
        return invalid("unexpected Darwin failed capacity payload");
    }
    Ok((Vec::new(), CapabilityState::Unavailable(reason)))
}

fn filesystems(df: &str) -> MonitorResult<Vec<FilesystemSnapshot>> {
    let mut lines = df.lines();
    if lines
        .next()
        .map(|header| header.split_whitespace().collect::<Vec<_>>())
        .as_deref()
        != Some(
            &[
                "Filesystem",
                "1024-blocks",
                "Used",
                "Available",
                "Capacity",
                "Mounted",
                "on",
            ][..],
        )
    {
        return invalid("invalid Darwin df units or header");
    }
    let mut result = Vec::new();
    let mut mounts = BTreeSet::new();
    for line in lines {
        if result.len() == MAX_FILESYSTEMS || line.is_empty() {
            return invalid("invalid Darwin filesystem count");
        }
        let (source, cols, mount) = filesystem_columns(line)?;
        if !mount.starts_with('/')
            || mount.len() > 4096
            || mount.chars().any(char::is_control)
            || !mounts.insert(mount)
            || source.len() > 1024
            || source.chars().any(char::is_control)
        {
            return invalid("invalid Darwin filesystem row");
        }
        let bytes = |value| {
            decimal(value)?
                .checked_mul(1024)
                .ok_or(MonitorError::InvalidData("Darwin capacity overflow"))
        };
        let percent = cols[3]
            .strip_suffix('%')
            .ok_or(MonitorError::InvalidData("invalid Darwin capacity percent"))?;
        result.push(FilesystemSnapshot {
            source: source.to_owned(),
            mount: mount.to_owned(),
            total_bytes: bytes(cols[0])?,
            used_bytes: bytes(cols[1])?,
            available_bytes: bytes(cols[2])?,
            used_percent: u32::try_from(decimal(percent)?)
                .map_err(|_| MonitorError::InvalidData("Darwin capacity percent overflow"))?,
        });
    }
    if result.is_empty() {
        return invalid("missing Darwin filesystem rows");
    }
    Ok(result)
}

fn filesystem_columns(line: &str) -> MonitorResult<(&str, [&str; 4], &str)> {
    if line.len() > 8192 {
        return invalid("Darwin filesystem row exceeds limit");
    }
    let mut words = Vec::new();
    let mut start = None;
    for (index, ch) in line
        .char_indices()
        .chain(std::iter::once((line.len(), ' ')))
    {
        if ch.is_whitespace() {
            if let Some(begin) = start.take() {
                words.push((begin, index));
            }
        } else {
            start.get_or_insert(index);
        }
    }
    let mut result = None;
    // Darwin's automounter reports sources such as "map auto_home". Numeric
    // capacity columns anchor the boundary; ambiguous unescaped names fail closed.
    for index in 1..words.len().saturating_sub(4) {
        let columns: [&str; 4] = std::array::from_fn(|offset| {
            let (begin, end) = words[index + offset];
            &line[begin..end]
        });
        if columns[..3].iter().any(|value| decimal(value).is_err())
            || columns[3]
                .strip_suffix('%')
                .is_none_or(|value| decimal(value).is_err())
        {
            continue;
        }
        let mount = &line[words[index + 4].0..];
        if !mount.starts_with('/') {
            continue;
        }
        if result.is_some() {
            return invalid("ambiguous Darwin filesystem columns");
        }
        result = Some((line[..words[index].0].trim(), columns, mount));
    }
    result.ok_or(MonitorError::InvalidData(
        "invalid Darwin filesystem columns",
    ))
}

#[cfg(test)]
#[path = "darwin/tests.rs"]
mod tests;
