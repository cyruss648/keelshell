//! Optional Linux block counters, validated independently of resource sampling.
//!
//! Fields follow <https://docs.kernel.org/admin-guide/iostats.html> and
//! <https://docs.kernel.org/block/stat.html>. Sectors are always 512 bytes;
//! time counters are milliseconds. `io_ticks` is not exact device utilization,
//! especially with concurrent I/O, so no capacity or saturation is inferred.
use std::collections::BTreeSet;

const MAX_DEVICES: usize = 256;
const MAX_INPUT_BYTES: usize = 256 * 1024;

/// Why the optional collector cannot provide trustworthy block counters.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DiskIoError {
    /// The old collector, kernel or permission scope supplied no disk data.
    #[error("disk counters are unavailable")]
    MissingData,
    /// The kernel boot UUID is missing or malformed.
    #[error("disk boot identity is unavailable or invalid")]
    BootIdentity,
    /// Only the documented 11-, 15- or 17-counter layouts are supported.
    #[error("unsupported disk counter layout")]
    UnsupportedLayout,
    /// The optional device response exceeded its explicit resource budget.
    #[error("disk data exceeds 256 KiB or 256 devices")]
    Limit,
    /// Names, device identities or numeric fields are invalid or repeated.
    #[error("invalid disk counters")]
    InvalidData,
}

/// Kernel device identity. Includes disks, partitions and stacked logical devices.
/// This identifies a row, not a physical serial number or hotplug generation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DiskDevice {
    /// Kernel major device number.
    pub major: u32,
    /// Kernel minor device number.
    pub minor: u32,
    /// Exact bounded ASCII device name reported by `/proc/diskstats`.
    pub name: String,
}

/// One device's checked cumulative fields. No physical-device totals are inferred.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskCounters {
    /// Row identity, compared across samples along with the boot UUID.
    pub device: DiskDevice,
    fields: Vec<u64>,
}

impl DiskCounters {
    /// Requests issued to the driver but not yet completed; this is a gauge.
    pub fn in_flight(&self) -> u64 {
        self.fields[8]
    }
}

/// Optional Linux counter sample, tied to the kernel's unchanged boot UUID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskIoSnapshot {
    boot_id: uuid::Uuid,
    /// Per-device counters. Parent disks and partitions remain separate rows.
    pub devices: Vec<DiskCounters>,
}

/// Why a device cannot provide a trustworthy interval rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskRateUnavailable {
    /// There is no valid previous disk sample.
    FirstSample,
    /// Boot identity changed between samples.
    BootChanged,
    /// Remote uptime was equal, backward or non-finite.
    InvalidInterval,
    /// This row has no previous matching major/minor/name identity.
    NewDevice,
    /// The previous device no longer appears in the current sample.
    Disappeared,
    /// Documented cumulative counters decreased or wrapped.
    CounterReset,
    /// Counter layout changed between samples.
    LayoutChanged,
    /// Byte conversion or derived rates overflowed their numeric budget.
    Overflow,
}

/// Valid interval observations, measured using remote uptime seconds.
#[derive(Debug, Clone, PartialEq)]
pub struct DiskIoRates {
    /// Read bytes per second; sectors are converted with checked ×512.
    pub read_bytes_per_second: f64,
    /// Written bytes per second; sectors are converted with checked ×512.
    pub written_bytes_per_second: f64,
    /// Completed read requests per second, after kernel merging.
    pub reads_per_second: f64,
    /// Completed write requests per second, after kernel merging.
    pub writes_per_second: f64,
    /// Mean duration of completed read requests; absent when none completed.
    pub read_milliseconds_per_request: Option<f64>,
    /// Mean duration of completed write requests; absent when none completed.
    pub write_milliseconds_per_request: Option<f64>,
    /// Kernel I/O activity milliseconds in the interval, not a utilization ratio.
    pub activity_milliseconds: u64,
    /// Remote uptime interval used for these rates.
    pub interval_seconds: f64,
}

/// One current or disappeared device's validated interval result.
#[derive(Debug, Clone, PartialEq)]
pub struct DiskRate {
    /// Exact row identity; never aggregated with parent/partition rows.
    pub device: DiskDevice,
    /// Unavailable observations remain explicit rather than becoming zero.
    pub observation: Result<DiskIoRates, DiskRateUnavailable>,
}

impl DiskIoSnapshot {
    /// Parse bounded, optional UTF-8 collector sections. Invalid optional data
    /// can be retained as an error without rejecting the other resource fields.
    pub fn parse(boot_id: &str, input: &str) -> Result<Self, DiskIoError> {
        if input.len() > MAX_INPUT_BYTES {
            return Err(DiskIoError::Limit);
        }
        let boot = boot_id.trim();
        if boot.len() != 36
            || !boot.bytes().enumerate().all(|(index, byte)| {
                if [8, 13, 18, 23].contains(&index) {
                    byte == b'-'
                } else {
                    byte.is_ascii_hexdigit()
                }
            })
        {
            return Err(DiskIoError::BootIdentity);
        }
        let boot_id = uuid::Uuid::parse_str(boot).map_err(|_| DiskIoError::BootIdentity)?;
        if boot_id.is_nil() {
            return Err(DiskIoError::BootIdentity);
        }
        let mut devices = Vec::new();
        let mut identities = BTreeSet::new();
        let mut names = BTreeSet::new();
        for line in input.lines().filter(|line| !line.trim().is_empty()) {
            if devices.len() == MAX_DEVICES {
                return Err(DiskIoError::Limit);
            }
            // At most 3 identity fields plus the 17 documented counters.
            let parts: Vec<_> = line.split_whitespace().take(21).collect();
            if parts.len() < 3 {
                return Err(DiskIoError::InvalidData);
            }
            if ![14, 18, 20].contains(&parts.len()) {
                return Err(DiskIoError::UnsupportedLayout);
            }
            let major = decimal(parts[0])?
                .try_into()
                .map_err(|_| DiskIoError::InvalidData)?;
            let minor = decimal(parts[1])?
                .try_into()
                .map_err(|_| DiskIoError::InvalidData)?;
            let name = parts[2];
            if name.is_empty()
                || name.len() > 128
                || !name.bytes().all(|byte| byte.is_ascii_graphic())
                || !identities.insert((major, minor))
                || !names.insert(name)
            {
                return Err(DiskIoError::InvalidData);
            }
            devices.push(DiskCounters {
                device: DiskDevice {
                    major,
                    minor,
                    name: name.to_owned(),
                },
                fields: parts[3..]
                    .iter()
                    .map(|part| decimal(part))
                    .collect::<Result<_, _>>()?,
            });
        }
        if devices.is_empty() {
            return Err(DiskIoError::MissingData);
        }
        Ok(Self { boot_id, devices })
    }

    /// Derive individual observations, refusing boot/time/layout/reset/overflow
    /// boundaries. A decreasing in-flight gauge is valid, not a counter reset.
    /// No physical or logical device total is calculated.
    pub fn rates_since(
        &self,
        previous: Option<&Self>,
        elapsed_seconds: f64,
        same_boot_timestamp: bool,
    ) -> Vec<DiskRate> {
        let boundary = if !same_boot_timestamp
            || previous.is_some_and(|previous| previous.boot_id != self.boot_id)
        {
            Some(DiskRateUnavailable::BootChanged)
        } else if !elapsed_seconds.is_finite() || elapsed_seconds <= 0.0 {
            Some(DiskRateUnavailable::InvalidInterval)
        } else if previous.is_none() {
            Some(DiskRateUnavailable::FirstSample)
        } else {
            None
        };
        let mut result: Vec<_> = self
            .devices
            .iter()
            .map(|current| {
                let observation = if let Some(reason) = boundary {
                    Err(reason)
                } else {
                    previous
                        .and_then(|previous| {
                            previous
                                .devices
                                .iter()
                                .find(|before| before.device == current.device)
                        })
                        .ok_or(DiskRateUnavailable::NewDevice)
                        .and_then(|before| delta(current, before, elapsed_seconds))
                };
                DiskRate {
                    device: current.device.clone(),
                    observation,
                }
            })
            .collect();
        if let Some(previous) = previous {
            result.extend(
                previous
                    .devices
                    .iter()
                    .filter(|before| !self.devices.iter().any(|now| now.device == before.device))
                    .map(|before| DiskRate {
                        device: before.device.clone(),
                        observation: Err(DiskRateUnavailable::Disappeared),
                    }),
            );
        }
        result
    }
}

fn decimal(input: &str) -> Result<u64, DiskIoError> {
    if input.is_empty() || !input.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(DiskIoError::InvalidData);
    }
    input.parse().map_err(|_| DiskIoError::InvalidData)
}

fn delta(
    current: &DiskCounters,
    previous: &DiskCounters,
    elapsed: f64,
) -> Result<DiskIoRates, DiskRateUnavailable> {
    use DiskRateUnavailable as E;
    if current.fields.len() != previous.fields.len() {
        return Err(E::LayoutChanged);
    }
    let mut change = [0_u64; 17];
    for (index, (current, previous)) in current.fields.iter().zip(&previous.fields).enumerate() {
        if index != 8 {
            change[index] = current.checked_sub(*previous).ok_or(E::CounterReset)?;
        }
    }
    let per_second = |value: u64| {
        let rate = value as f64 / elapsed;
        rate.is_finite().then_some(rate).ok_or(E::Overflow)
    };
    let mean = |ticks: u64, requests: u64| (requests > 0).then(|| ticks as f64 / requests as f64);
    Ok(DiskIoRates {
        read_bytes_per_second: per_second(change[2].checked_mul(512).ok_or(E::Overflow)?)?,
        written_bytes_per_second: per_second(change[6].checked_mul(512).ok_or(E::Overflow)?)?,
        reads_per_second: per_second(change[0])?,
        writes_per_second: per_second(change[4])?,
        read_milliseconds_per_request: mean(change[3], change[0]),
        write_milliseconds_per_request: mean(change[7], change[4]),
        activity_milliseconds: change[9],
        interval_seconds: elapsed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const BOOT: &str = "11ac8d57-72c6-4ee6-93bf-68724d27e715";
    fn row(name: &str, minor: u32, fields: &[u64]) -> String {
        format!(
            "8 {minor} {name} {}\n",
            fields
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(" ")
        )
    }
    fn sample(fields: &[u64]) -> Result<DiskIoSnapshot, DiskIoError> {
        DiskIoSnapshot::parse(BOOT, &row("sda", 0, fields))
    }
    fn observation(rates: &[DiskRate]) -> &Result<DiskIoRates, DiskRateUnavailable> {
        &rates[0].observation
    }

    #[test]
    fn documented_layouts_and_exact_sector_request_and_time_units() -> Result<(), DiskIoError> {
        for count in [11, 15, 17] {
            let mut fields = vec![100; count];
            fields[8] = 9;
            let before = sample(&fields)?;
            fields[0] += 4;
            fields[2] += 20;
            fields[3] += 12;
            fields[4] += 8;
            fields[6] += 40;
            fields[7] += 32;
            fields[9] += 150;
            fields[8] = 0; // This gauge is allowed to decrease independently.
            let after = sample(&fields)?;
            let rates = after.rates_since(Some(&before), 2., true);
            let Ok(value) = observation(&rates) else {
                panic!("valid deltas")
            };
            assert_eq!(value.read_bytes_per_second, 5120.);
            assert_eq!(value.written_bytes_per_second, 10240.);
            assert_eq!(value.reads_per_second, 2.);
            assert_eq!(value.writes_per_second, 4.);
            assert_eq!(value.read_milliseconds_per_request, Some(3.));
            assert_eq!(value.write_milliseconds_per_request, Some(4.));
            assert_eq!(value.activity_milliseconds, 150);
            assert_eq!(after.devices[0].in_flight(), 0);
        }
        Ok(())
    }

    #[test]
    fn every_cumulative_field_reset_refuses_the_entire_device_observation()
    -> Result<(), DiskIoError> {
        let before = sample(&[10; 17])?;
        for index in 0..17 {
            if index == 8 {
                continue;
            }
            let mut fields = [20; 17];
            fields[index] = 9;
            let after = sample(&fields)?;
            assert_eq!(
                observation(&after.rates_since(Some(&before), 5., true)),
                &Err(DiskRateUnavailable::CounterReset)
            );
        }
        Ok(())
    }

    #[test]
    fn boot_and_remote_time_and_first_sample_are_explicitly_unavailable() -> Result<(), DiskIoError>
    {
        let before = sample(&[10; 11])?;
        let after = sample(&[20; 11])?;
        for elapsed in [0., -1., f64::NAN, f64::INFINITY] {
            assert_eq!(
                observation(&after.rates_since(Some(&before), elapsed, true)),
                &Err(DiskRateUnavailable::InvalidInterval)
            );
        }
        assert_eq!(
            observation(&after.rates_since(None, 5., true)),
            &Err(DiskRateUnavailable::FirstSample)
        );
        assert_eq!(
            observation(&after.rates_since(Some(&before), 5., false)),
            &Err(DiskRateUnavailable::BootChanged)
        );
        let reboot = DiskIoSnapshot::parse(
            "a1ac8d57-72c6-4ee6-93bf-68724d27e715",
            &row("sda", 0, &[20; 11]),
        )?;
        assert_eq!(
            observation(&reboot.rates_since(Some(&before), 5., true)),
            &Err(DiskRateUnavailable::BootChanged)
        );
        Ok(())
    }

    #[test]
    fn identity_change_new_device_and_disappearance_cannot_reuse_old_counters()
    -> Result<(), DiskIoError> {
        let before = sample(&[10; 11])?;
        for changed in [
            row("sdb", 0, &[20; 11]),
            row("sda", 1, &[20; 11]),
            row("sda", 0, &[20; 11]).replacen("8 ", "9 ", 1),
        ] {
            let after = DiskIoSnapshot::parse(BOOT, &changed)?;
            let rates = after.rates_since(Some(&before), 5., true);
            assert_eq!(rates.len(), 2);
            assert_eq!(rates[0].observation, Err(DiskRateUnavailable::NewDevice));
            assert_eq!(rates[1].observation, Err(DiskRateUnavailable::Disappeared));
        }
        Ok(())
    }

    #[test]
    fn parent_and_partition_remain_distinct_without_double_counted_totals()
    -> Result<(), DiskIoError> {
        let before = DiskIoSnapshot::parse(
            BOOT,
            &(row("sda", 0, &[10; 11]) + &row("sda1", 1, &[10; 11])),
        )?;
        let after = DiskIoSnapshot::parse(
            BOOT,
            &(row("sda", 0, &[20; 11]) + &row("sda1", 1, &[15; 11])),
        )?;
        let rates = after.rates_since(Some(&before), 5., true);
        assert_eq!(rates.len(), 2);
        assert_eq!(rates[0].device.name, "sda");
        assert_eq!(rates[1].device.name, "sda1");
        assert_eq!(
            rates[0]
                .observation
                .as_ref()
                .map(|rate| rate.read_bytes_per_second),
            Ok(1024.)
        );
        assert_eq!(
            rates[1]
                .observation
                .as_ref()
                .map(|rate| rate.read_bytes_per_second),
            Ok(512.)
        );
        Ok(())
    }

    #[test]
    fn layout_changes_and_byte_or_float_overflow_remain_unavailable() -> Result<(), DiskIoError> {
        let before = sample(&[0; 11])?;
        let layout = sample(&[0; 17])?;
        assert_eq!(
            observation(&layout.rates_since(Some(&before), 5., true)),
            &Err(DiskRateUnavailable::LayoutChanged)
        );
        let mut overflow = [0; 11];
        overflow[2] = u64::MAX;
        let after = sample(&overflow)?;
        assert_eq!(
            observation(&after.rates_since(Some(&before), 5., true)),
            &Err(DiskRateUnavailable::Overflow)
        );
        let after = sample(&[1000; 11])?;
        assert_eq!(
            observation(&after.rates_since(Some(&before), f64::from_bits(1), true)),
            &Err(DiskRateUnavailable::Overflow)
        );
        let idle = before.rates_since(Some(&before), 5., true);
        let Ok(idle) = observation(&idle) else {
            panic!("valid idle sample")
        };
        assert_eq!(idle.read_bytes_per_second, 0.);
        assert_eq!(idle.read_milliseconds_per_request, None);
        assert_eq!(idle.write_milliseconds_per_request, None);
        Ok(())
    }

    #[test]
    fn malformed_duplicate_unicode_controls_and_unsupported_layouts_are_refused() {
        let good = row("sda", 0, &[1; 11]);
        for bad in [
            good.clone() + &good,
            good.clone() + &row("sdb", 0, &[1; 11]),
            good.clone() + &row("sda", 1, &[1; 11]),
            row("磁盘", 0, &[1; 11]),
            row("sd\u{202e}a", 0, &[1; 11]),
            row("sd\0a", 0, &[1; 11]),
            good.replacen("8 0 ", "-8 0 ", 1),
            good.replacen("8 0 ", "4294967296 0 ", 1),
            good.replacen("sda 1 ", "sda +1 ", 1),
            good.replacen("sda 1 ", "sda 18446744073709551616 ", 1),
        ] {
            assert_eq!(
                DiskIoSnapshot::parse(BOOT, &bad),
                Err(DiskIoError::InvalidData)
            );
        }
        for count in [4, 10, 12, 13, 14, 16, 18, 24] {
            assert_eq!(sample(&vec![0; count]), Err(DiskIoError::UnsupportedLayout));
        }
        assert_eq!(
            DiskIoSnapshot::parse(BOOT, ""),
            Err(DiskIoError::MissingData)
        );
        for boot in [
            "",
            "not-a-uuid",
            "00000000-0000-0000-0000-000000000000",
            "11ac8d57-72c6-4ee6-93bf-68724d27e715\n!unavailable",
        ] {
            assert_eq!(
                DiskIoSnapshot::parse(boot, &good),
                Err(DiskIoError::BootIdentity)
            );
        }
    }

    #[test]
    fn exact_device_name_and_input_budgets_are_checked() -> Result<(), DiskIoError> {
        let rows = (0..256)
            .map(|minor| row(&format!("dev{minor}"), minor, &[0; 11]))
            .collect::<String>();
        assert_eq!(DiskIoSnapshot::parse(BOOT, &rows)?.devices.len(), 256);
        assert_eq!(
            DiskIoSnapshot::parse(BOOT, &(rows + &row("extra", 256, &[0; 11]))),
            Err(DiskIoError::Limit)
        );
        assert!(DiskIoSnapshot::parse(BOOT, &row(&"x".repeat(128), 0, &[0; 11])).is_ok());
        assert_eq!(
            DiskIoSnapshot::parse(BOOT, &row(&"x".repeat(129), 0, &[0; 11])),
            Err(DiskIoError::InvalidData)
        );
        assert_eq!(
            DiskIoSnapshot::parse(BOOT, &" ".repeat(MAX_INPUT_BYTES + 1)),
            Err(DiskIoError::Limit)
        );
        Ok(())
    }
}
