use std::io;

/// Maximum UTF-8 byte length of a reviewed local Windows path.
pub const MAX_WINDOWS_PATH_BYTES: usize = 4096;

/// Return the conservative Unicode case alias used for local names and claims.
///
/// Uppercasing before lowercasing also folds expanding and final-sigma forms.
/// This is a portable exclusion policy, not a filesystem lookup, Unicode
/// normalization, or a claim about a volume's native case-comparison algorithm.
pub fn filename_alias(name: &str) -> String {
    name.chars()
        .flat_map(char::to_uppercase)
        .flat_map(char::to_lowercase)
        .collect()
}

/// A checked absolute drive path whose components cannot escape a held directory.
///
/// UNC, device namespaces and drive-relative paths are rejected. Remote or
/// unsupported filesystems must not silently inherit local publication claims.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowsPath {
    drive: u8,
    names: Vec<String>,
}

impl WindowsPath {
    /// Parse a normalized drive path, including the extended drive spelling.
    ///
    /// Both `C:\directory` and `\\?\C:\directory` refer to a drive root and
    /// individual literal names. Forward slashes, dot components, reserved DOS
    /// aliases, controls and trailing spaces/dots are rejected before native I/O.
    pub fn parse(text: &str) -> io::Result<Self> {
        if text.len() > MAX_WINDOWS_PATH_BYTES {
            return Err(invalid("local path exceeds its review bound"));
        }
        let text = text.strip_prefix(r"\\?\").unwrap_or(text);
        if text.starts_with(r"\\") || text.starts_with("UNC\\") {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "local publication requires a local drive filesystem",
            ));
        }
        let prefix = text.as_bytes();
        if prefix.len() < 3
            || !prefix[0].is_ascii_alphabetic()
            || prefix[1] != b':'
            || prefix[2] != b'\\'
            || text.contains('/')
        {
            return Err(invalid("local path must be an absolute drive path"));
        }
        let mut names = Vec::new();
        if !text[3..].is_empty() {
            for name in text[3..].split('\\') {
                validate_windows_component(name)?;
                names.push(name.to_owned());
            }
        }
        Ok(Self {
            drive: prefix[0].to_ascii_uppercase(),
            names,
        })
    }

    /// Return the normalized ASCII drive letter without a colon.
    pub fn drive(&self) -> u8 {
        self.drive
    }

    /// Iterate over the checked names after the drive root.
    pub fn components(&self) -> impl ExactSizeIterator<Item = &str> {
        self.names.iter().map(String::as_str)
    }
}

/// Validate a single literal Windows filesystem name before a relative syscall.
///
/// The checked name cannot contain a separator, alternate stream, control,
/// hidden direction marker, dot component or a normalized DOS device alias.
pub fn validate_windows_component(name: &str) -> io::Result<()> {
    if name.is_empty()
        || matches!(name, "." | "..")
        || name.encode_utf16().count() > 255
        || name.ends_with([' ', '.'])
        || name.chars().any(|c| {
            c.is_control()
                || matches!(c, '/' | '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*')
                || matches!(c as u32, 0x200b..=0x200f | 0x202a..=0x202e | 0x2066..=0x2069)
        })
    {
        return Err(invalid("local name is not a portable literal component"));
    }
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(' ')
        .to_uppercase();
    if matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$" | "CONIN$" | "CONOUT$"
    ) || ["COM", "LPT"].iter().any(|prefix| {
        stem.strip_prefix(prefix).is_some_and(|suffix| {
            matches!(
                suffix,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
    }) {
        return Err(invalid("local name aliases a DOS device"));
    }
    Ok(())
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_aliases_fold_expanding_and_final_sigma_case_forms() {
        for (first, second) in [
            ("New", "new"),
            ("STRASSE.bin", "straße.bin"),
            ("σ.bin", "ς.bin"),
        ] {
            assert_eq!(filename_alias(first), filename_alias(second));
        }
        assert_ne!(filename_alias("first"), filename_alias("second"));
    }

    #[test]
    fn absolute_drive_and_extended_drive_have_identical_literal_components() -> io::Result<()> {
        let ordinary = WindowsPath::parse(r"c:\用户\日志.txt")?;
        let extended = WindowsPath::parse(r"\\?\C:\用户\日志.txt")?;
        assert_eq!(ordinary, extended);
        assert_eq!(ordinary.drive(), b'C');
        assert_eq!(
            ordinary.components().collect::<Vec<_>>(),
            ["用户", "日志.txt"]
        );
        Ok(())
    }

    #[test]
    fn drive_root_is_a_directory_capability_without_a_fabricated_name() -> io::Result<()> {
        assert_eq!(WindowsPath::parse("C:\\")?.components().count(), 0);
        Ok(())
    }

    #[test]
    fn drive_relative_and_normalizing_path_spellings_are_rejected() {
        for path in [
            r"C:relative",
            r"\rooted",
            r"C:\a\..\b",
            r"C:\a\.\b",
            r"C:\a\\b",
            r"C:\a\",
            "C:/a",
            r"1:\a",
            r"C:\stream:data",
        ] {
            assert!(WindowsPath::parse(path).is_err(), "{path:?}");
        }
    }

    #[test]
    fn unc_and_device_namespaces_do_not_fall_back_to_drive_access() {
        for path in [
            r"\\server\share\file",
            r"\\?\UNC\server\share\file",
            r"\\.\PhysicalDrive0",
            r"\\?\GLOBALROOT\Device\HarddiskVolume1\file",
        ] {
            assert!(WindowsPath::parse(path).is_err(), "{path:?}");
        }
    }

    #[test]
    fn reserved_dos_names_are_rejected_with_extensions_and_superscript_aliases() {
        for name in [
            "CON",
            "nul.txt",
            "Lpt1.log",
            "COM¹.txt",
            "COM²",
            "LPT³",
            "conin$",
            "CONOUT$.log",
            "NUL .txt",
            "COM1  .log",
        ] {
            assert!(validate_windows_component(name).is_err(), "{name:?}");
        }
    }

    #[test]
    fn controls_streams_and_visual_path_disguises_are_rejected() {
        for name in [
            "report.",
            "report ",
            "line\nname",
            "a\0b",
            "a\u{202e}txt",
            "a/b",
            "a\\b",
            "a:b",
            "..",
            "",
        ] {
            assert!(validate_windows_component(name).is_err(), "{name:?}");
        }
    }

    #[test]
    fn filename_bound_counts_utf16_units_instead_of_utf8_bytes() {
        assert!(validate_windows_component(&"界".repeat(255)).is_ok());
        assert!(validate_windows_component(&"界".repeat(256)).is_err());
        assert!(validate_windows_component(&"🦀".repeat(127)).is_ok());
        assert!(validate_windows_component(&"🦀".repeat(128)).is_err());
    }

    #[test]
    fn full_path_review_bound_is_enforced_before_native_access() {
        let path = format!("C:\\{}", "a".repeat(MAX_WINDOWS_PATH_BYTES));
        assert!(WindowsPath::parse(&path).is_err());
    }
}
