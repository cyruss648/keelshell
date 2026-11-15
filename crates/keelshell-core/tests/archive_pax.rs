#![expect(clippy::unwrap_used, reason = "test fixtures and assertions")]

use std::io::{Cursor, Write};

use flate2::Compression;
use keelshell_core::{
    ArchiveBundle, ArchiveError, ArchiveFormat, ArchiveMember, MAX_ARCHIVE_DEPTH,
    MAX_ARCHIVE_ENTRIES, MAX_ARCHIVE_EXPANDED_BYTES, MAX_ARCHIVE_MANIFEST_BYTES,
    MAX_ARCHIVE_PATH_BYTES,
};

#[test]
fn pax_python_tarfile_archive_decodes_files_directories_and_metadata() {
    let expected = ArchiveBundle::new(vec![
        ArchiveMember::new_directory("records").unwrap(),
        ArchiveMember::new_file(
            format!("records/long-{}.bin", "a".repeat(110)),
            b"\0\xffPAX interoperability\r\n".repeat(25),
        )
        .unwrap(),
        ArchiveMember::new_file("records/short.txt", b"plain text\n".to_vec()).unwrap(),
        ArchiveMember::new_directory("empty").unwrap(),
    ])
    .unwrap();
    assert_eq!(
        ArchiveBundle::decode(ArchiveFormat::TarGz, PYTHON_PAX_TAR_GZ).unwrap(),
        expected
    );
}

#[test]
fn pax_macos_system_tar_writer_archive_decodes_ordinary_metadata() {
    assert_eq!(
        ArchiveBundle::decode(ArchiveFormat::TarGz, MACOS_PAX_TAR_GZ).unwrap(),
        ArchiveBundle::new(vec![
            ArchiveMember::new_directory("posix").unwrap(),
            ArchiveMember::new_file("posix/data.bin", b"\0\xffnative writer\n".to_vec()).unwrap(),
        ])
        .unwrap()
    );
}

#[test]
fn pax_posix_metadata_accepts_utf8_paths_and_signed_fractional_times() {
    let records = [
        ("path", "资料/报告.txt"),
        ("size", "4"),
        ("mtime", "-0.000000001"),
        ("atime", "1735689601.123456789012"),
        ("ctime", "-9223372036854775808.5"),
        ("uid", "18446744073709551615"),
        ("gid", "000123"),
        ("uname", "归档所有者"),
        ("gname", "archive group"),
    ];
    let mut extension = pax(b'x', &records);
    extension[..100].fill(0);
    extension[..b"PaxHeaders.1234/report".len()].copy_from_slice(b"PaxHeaders.1234/report");
    fix_checksum(&mut extension);
    let bundle = decode(&[extension, entry(b"fallback.txt", b'0', b"\0\xff\r\n", None)]).unwrap();
    assert_eq!(
        bundle,
        ArchiveBundle::new(vec![
            ArchiveMember::new_file("资料/报告.txt", b"\0\xff\r\n".to_vec()).unwrap(),
        ])
        .unwrap()
    );
}

#[test]
fn pax_local_metadata_is_consumed_once_and_does_not_leak() {
    let bundle = decode(&[
        pax(b'x', &[("path", "renamed"), ("size", "3")]),
        entry(b"fallback", b'0', b"one", Some(0)),
        entry(b"second", b'0', b"two!", None),
    ])
    .unwrap();
    assert_eq!(
        bundle,
        ArchiveBundle::new(vec![
            ArchiveMember::new_file("renamed", b"one".to_vec()).unwrap(),
            ArchiveMember::new_file("second", b"two!".to_vec()).unwrap(),
        ])
        .unwrap()
    );
}

#[test]
fn pax_local_overrides_global_and_later_global_updates_retain_missing_defaults() {
    let bundle = decode(&[
        entry(b"before", b'0', b"before", None),
        pax(b'g', &[("path", "global"), ("size", "4"), ("uid", "1000")]),
        pax(b'x', &[("path", "local"), ("size", "3")]),
        entry(b"first", b'0', b"one", Some(0)),
        entry(b"second", b'0', b"two!", Some(0)),
        pax(b'g', &[("path", "updated")]),
        entry(b"third", b'0', b"next", Some(0)),
    ])
    .unwrap();
    assert_eq!(
        bundle,
        ArchiveBundle::new(vec![
            ArchiveMember::new_file("before", b"before".to_vec()).unwrap(),
            ArchiveMember::new_file("local", b"one".to_vec()).unwrap(),
            ArchiveMember::new_file("global", b"two!".to_vec()).unwrap(),
            ArchiveMember::new_file("updated", b"next".to_vec()).unwrap(),
        ])
        .unwrap()
    );
}

#[test]
fn pax_size_override_controls_block_framing_in_both_directions() {
    for (declared, effective) in [(1, 513), (1025, 1)] {
        let payload = vec![0xab; effective];
        let bundle = decode(&[
            pax(b'x', &[("size", &effective.to_string())]),
            entry(b"resized", b'0', &payload, Some(declared)),
            entry(b"following", b'0', b"unaffected", None),
        ])
        .unwrap();
        assert_eq!(
            bundle,
            ArchiveBundle::new(vec![
                ArchiveMember::new_file("resized", payload).unwrap(),
                ArchiveMember::new_file("following", b"unaffected".to_vec()).unwrap(),
            ])
            .unwrap()
        );
    }
}

#[test]
fn pax_directory_paths_allow_one_suffix_and_zero_size_overrides() {
    let bundle = decode(&[
        pax(b'x', &[("path", "资料/"), ("size", "0")]),
        entry(b"fallback/", b'5', b"", None),
        pax(b'x', &[("path", "资料/empty")]),
        entry(b"empty/", b'5', b"", None),
    ])
    .unwrap();
    assert_eq!(
        bundle,
        ArchiveBundle::new(vec![
            ArchiveMember::new_directory("资料").unwrap(),
            ArchiveMember::new_directory("资料/empty").unwrap(),
        ])
        .unwrap()
    );
}

#[test]
fn pax_reencoding_discards_ownership_permissions_and_timestamps() {
    let bundle = ArchiveBundle::decode(ArchiveFormat::TarGz, PYTHON_PAX_TAR_GZ).unwrap();
    let encoded = bundle.encode(ArchiveFormat::TarGz).unwrap();
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(encoded.as_slice()));
    for member in archive.entries().unwrap() {
        let member = member.unwrap();
        let header = member.header();
        assert_eq!(header.uid().unwrap(), 0);
        assert_eq!(header.gid().unwrap(), 0);
        assert_eq!(header.mtime().unwrap(), 0);
        assert_eq!(
            header.mode().unwrap(),
            if header.entry_type().is_dir() {
                0o700
            } else {
                0o600
            }
        );
    }
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bundle.encode(ArchiveFormat::Zip).unwrap())).unwrap();
    for index in 0..archive.len() {
        let member = archive.by_index(index).unwrap();
        assert_eq!(
            member.unix_mode().unwrap() & 0o7777,
            if member.is_dir() { 0o700 } else { 0o600 }
        );
    }
}

#[test]
fn pax_rejects_full_width_base256_sizes_in_every_admitted_header_kind() {
    for (kind, name, payload) in [
        (
            b'x',
            b"././@PaxHeader".as_slice(),
            b"13 path=safe\n".as_slice(),
        ),
        (
            b'g',
            b"././@PaxHeader".as_slice(),
            b"13 path=safe\n".as_slice(),
        ),
        (b'0', b"ordinary".as_slice(), b"data".as_slice()),
        (b'5', b"directory".as_slice(), b"".as_slice()),
        (b'L', b"././@LongLink".as_slice(), b"safe\0".as_slice()),
    ] {
        for high in [
            [0x80, 0, 0, 1],
            [0x80, 0, 1, 0],
            [0x80, 1, 0, 0],
            [0x81, 0, 0, 0],
            [0xc0, 0, 0, 0],
            [0xff, 0xff, 0xff, 0xff],
        ] {
            let mut member = entry(name, kind, payload, None);
            member[124..128].copy_from_slice(&high);
            member[128..136].copy_from_slice(&(payload.len() as u64).to_be_bytes());
            fix_checksum(&mut member);
            let mut entries = vec![member];
            if matches!(kind, b'x' | b'g' | b'L') {
                entries.push(entry(b"fallback", b'0', b"", None));
            }
            let result = decode(&entries);
            assert!(
                matches!(
                    &result,
                    Err(ArchiveError::Corrupt("invalid TAR size encoding"))
                ),
                "type={}, high={high:x?}, result={result:?}",
                char::from(kind)
            );
        }
    }
}

#[test]
fn pax_size_cannot_hide_a_full_width_invalid_ordinary_size() {
    for kind in *b"xg" {
        for high in [[0x80, 0, 0, 1], [0xff, 0xff, 0xff, 0xff]] {
            let mut member = entry(b"fallback", b'0', b"abc", None);
            member[124..128].copy_from_slice(&high);
            member[128..136].fill(0);
            fix_checksum(&mut member);
            let result = decode(&[pax(kind, &[("size", "3")]), member]);
            assert!(
                matches!(
                    &result,
                    Err(ArchiveError::Corrupt("invalid TAR size encoding"))
                ),
                "PAX kind={}, high={high:x?}, result={result:?}",
                char::from(kind)
            );
        }
    }
}

#[test]
fn pax_accepts_small_nonnegative_base256_sizes_for_all_admitted_headers() {
    for (kind, name, payload) in [
        (
            b'x',
            b"././@PaxHeader".as_slice(),
            b"13 path=safe\n".as_slice(),
        ),
        (
            b'g',
            b"././@PaxHeader".as_slice(),
            b"13 path=safe\n".as_slice(),
        ),
        (b'0', b"ordinary".as_slice(), b"data".as_slice()),
        (b'5', b"directory".as_slice(), b"".as_slice()),
        (b'L', b"././@LongLink".as_slice(), b"safe\0".as_slice()),
    ] {
        let mut entries = vec![entry(name, kind, payload, None)];
        if matches!(kind, b'x' | b'g' | b'L') {
            entries.push(entry(b"fallback", b'0', b"", None));
        }
        let expected = decode(&entries).unwrap();
        entries[0][124..128].copy_from_slice(&[0x80, 0, 0, 0]);
        entries[0][128..136].copy_from_slice(&(payload.len() as u64).to_be_bytes());
        fix_checksum(&mut entries[0]);
        assert_eq!(
            decode(&entries).unwrap(),
            expected,
            "type={}",
            char::from(kind)
        );
    }
    for size in [0, 1, 511, 512, 513] {
        let payload = vec![0xab; size];
        let mut member = entry(b"boundary", b'0', &payload, None);
        member[124..128].copy_from_slice(&[0x80, 0, 0, 0]);
        member[128..136].copy_from_slice(&(size as u64).to_be_bytes());
        fix_checksum(&mut member);
        assert_eq!(
            decode(&[member]).unwrap(),
            ArchiveBundle::new(vec![ArchiveMember::new_file("boundary", payload).unwrap()])
                .unwrap()
        );
    }
}

#[test]
fn pax_base256_sizes_preserve_full_u64_and_existing_budget_limits() {
    for size in [MAX_ARCHIVE_EXPANDED_BYTES + 1, u64::MAX] {
        let mut member = entry(b"fallback", b'0', b"", None);
        member[124..128].copy_from_slice(&[0x80, 0, 0, 0]);
        member[128..136].copy_from_slice(&size.to_be_bytes());
        fix_checksum(&mut member);
        assert!(matches!(
            decode(&[pax(b'x', &[("size", "0")]), member]),
            Err(ArchiveError::ExpandedTooLarge)
        ));
    }
    for kind in *b"xg" {
        for size in [8193u64, u64::MAX] {
            let mut metadata = pax(kind, &[("path", "safe")]);
            metadata[124..128].copy_from_slice(&[0x80, 0, 0, 0]);
            metadata[128..136].copy_from_slice(&size.to_be_bytes());
            fix_checksum(&mut metadata);
            assert!(matches!(
                decode(&[metadata, entry(b"fallback", b'0', b"", None)]),
                Err(ArchiveError::Corrupt("invalid PAX metadata size"))
            ));
        }
    }
}

#[test]
fn pax_accepts_unambiguous_octal_size_padding() {
    for field in [
        *b"00000000003\0",
        *b"000000000003",
        *b" 0000000003 ",
        *b"3\0          ",
    ] {
        let mut member = entry(b"fallback", b'0', b"abc", None);
        member[124..136].copy_from_slice(&field);
        fix_checksum(&mut member);
        assert_eq!(
            decode(&[member]).unwrap(),
            ArchiveBundle::new(vec![
                ArchiveMember::new_file("fallback", b"abc".to_vec()).unwrap(),
            ])
            .unwrap()
        );
    }
}

#[test]
fn pax_rejects_hidden_or_malformed_octal_sizes_even_with_overrides() {
    for field in [
        *b"0003\x000000007",
        *b"00000000003x",
        *b"+00000000003",
        *b"-00000000003",
        *b"00000000003\n",
        *b"00000000003\t",
        *b"\x0000000000003",
        *b"            ",
        [0; 12],
    ] {
        let mut member = entry(b"fallback", b'0', b"abc", None);
        member[124..136].copy_from_slice(&field);
        fix_checksum(&mut member);
        assert!(matches!(
            decode(&[pax(b'x', &[("size", "3")]), member]),
            Err(ArchiveError::Corrupt("invalid TAR size encoding"))
        ));
    }
}

#[test]
fn pax_rejects_duplicate_keys_even_when_values_match() {
    for kind in *b"xg" {
        for (key, value) in [
            ("path", "safe"),
            ("size", "0"),
            ("mtime", "1.25"),
            ("atime", "1"),
            ("ctime", "-1"),
            ("uid", "1000"),
            ("gid", "1000"),
            ("uname", "owner"),
            ("gname", "group"),
        ] {
            for duplicate in [value, "different"] {
                assert!(
                    matches!(
                        decode(&[
                            pax(kind, &[(key, value), (key, duplicate)]),
                            entry(b"fallback", b'0', b"", None),
                        ]),
                        Err(ArchiveError::Corrupt("duplicate PAX keyword"))
                    ),
                    "kind={kind}, key={key}, duplicate={duplicate}"
                );
            }
        }
    }
}

#[test]
fn pax_rejects_malformed_record_lengths_and_framing() {
    for payload in [
        b"0 path=safe\n".as_slice(),
        b"013 path=safe\n",
        b"+13 path=safe\n",
        b"-13 path=safe\n",
        b"13\tpath=safe\n",
        b"13  path=safe\n",
        b"12 path=safe\n",
        b"14 path=safe\n",
        b"999 path=safe\n",
        b"18446744073709551616 path=safe\n",
        b"4 x\n",
        b"7 path\n",
        b"5 =x\n",
        b"13 path=safeX",
        b"13 path=safe\n\0",
        b"13 path=safe\njunk",
    ] {
        assert!(
            decode(&[pax_raw(b'x', payload), entry(b"fallback", b'0', b"", None),]).is_err(),
            "{payload:?}"
        );
    }
}

#[test]
fn pax_record_lengths_count_utf8_bytes_and_require_valid_utf8_values() {
    assert!(
        decode(&[
            pax(b'x', &[("path", "资料")]),
            entry(b"fallback", b'0', b"", None),
        ])
        .is_ok()
    );
    assert!(
        decode(&[
            pax_raw(b'x', "13 path=资料\n".as_bytes()),
            entry(b"fallback", b'0', b"", None),
        ])
        .is_err()
    );
    for key in [b"path".as_slice(), b"uname", b"gname"] {
        assert!(
            matches!(
                decode(&[
                    pax_raw(b'x', &record(key, b"invalid\xff")),
                    entry(b"fallback", b'0', b"", None),
                ]),
                Err(ArchiveError::InvalidUtf8)
            ),
            "{key:?}"
        );
    }
}

#[test]
fn pax_rejects_invalid_unsigned_sizes_and_owner_ids() {
    for key in ["size", "uid", "gid"] {
        for value in [
            "-1",
            "+1",
            "1.0",
            "1e3",
            "0x10",
            " 1",
            "1 ",
            "1\n",
            "1\0",
            "18446744073709551616",
        ] {
            assert!(
                decode(&[
                    pax(b'x', &[(key, value)]),
                    entry(b"fallback", b'0', b"", None),
                ])
                .is_err(),
                "{key}={value:?}"
            );
        }
    }
}

#[test]
fn pax_rejects_invalid_or_overflowing_timestamps() {
    for key in ["mtime", "atime", "ctime"] {
        for value in [
            "+1",
            "1e9",
            "NaN",
            "inf",
            "1.",
            ".5",
            "-.5",
            "1.2.3",
            "1.-2",
            " 1",
            "1\n",
            "1.2\0",
            "9223372036854775808",
            "-9223372036854775809",
        ] {
            assert!(
                decode(&[
                    pax(b'x', &[(key, value)]),
                    entry(b"fallback", b'0', b"", None),
                ])
                .is_err(),
                "{key}={value:?}"
            );
        }
    }
}

#[test]
fn pax_rejects_deletions_and_control_characters_in_discarded_names() {
    for kind in *b"xg" {
        for key in [
            "path", "size", "mtime", "atime", "ctime", "uid", "gid", "uname", "gname",
        ] {
            assert!(
                matches!(
                    decode(&[pax(kind, &[(key, "")]), entry(b"fallback", b'0', b"", None),]),
                    Err(ArchiveError::UnsupportedEntry)
                ),
                "kind={kind}, key={key}"
            );
        }
    }
    for key in ["uname", "gname"] {
        for value in ["owner\0hidden", "owner\nhidden", "owner\u{0085}hidden"] {
            assert!(
                decode(&[
                    pax(b'x', &[(key, value)]),
                    entry(b"fallback", b'0', b"", None),
                ])
                .is_err()
            );
        }
    }
}

#[test]
fn pax_rejects_vendor_sparse_link_encoding_and_unknown_keywords() {
    for kind in *b"xg" {
        for (key, value) in [
            ("linkpath", "target"),
            ("GNU.sparse.map", "0,4"),
            ("GNU.sparse.size", "4"),
            ("GNU.sparse.realsize", "4"),
            ("GNU.sparse.major", "1"),
            ("GNU.sparse.minor", "0"),
            ("GNU.sparse.name", "safe"),
            ("SCHILY.filetype", "sparse"),
            ("SCHILY.realsize", "4"),
            ("SCHILY.xattr.user.test", "value"),
            ("LIBARCHIVE.xattr.user.test", "dmFsdWU="),
            ("hdrcharset", "BINARY"),
            ("hdrcharset", "ISO-IR 10646 2000 UTF-8"),
            ("charset", "UTF-8"),
            ("comment", "unimplemented"),
            ("mode", "07777"),
            ("unknown", "value"),
        ] {
            assert!(
                matches!(
                    decode(&[
                        pax(kind, &[(key, value)]),
                        pax(b'x', &[("path", "safe"), ("size", "4")]),
                        entry(b"fallback", b'0', b"data", None),
                    ]),
                    Err(ArchiveError::UnsupportedEntry)
                ),
                "kind={kind}, {key}={value}"
            );
        }
    }
}

#[test]
fn pax_rejects_unsupported_effective_entry_types() {
    for &kind in b"123467SKVD" {
        assert!(
            matches!(
                decode(&[
                    pax(b'x', &[("path", "safe"), ("size", "0")]),
                    entry(b"fallback", kind, b"", None),
                ]),
                Err(ArchiveError::UnsupportedEntry)
            ),
            "{kind}"
        );
    }
}

#[test]
fn pax_rejects_unsafe_override_paths_even_when_globals_are_shadowed() {
    for path in [
        "../escape",
        "/absolute",
        "a/../escape",
        "a/./file",
        "a//file",
        r"a\b",
        "C:/file",
        "a\0hidden",
        "a\nhidden",
        "a\u{0085}hidden",
        "CON.txt",
        "a?",
        "a.",
        "a ",
        "directory//",
    ] {
        for kind in *b"xg" {
            assert!(
                decode(&[
                    pax(kind, &[("path", path)]),
                    entry(b"fallback", b'0', b"", None),
                ])
                .is_err(),
                "kind={kind}, path={path:?}"
            );
        }
        assert!(
            decode(&[
                pax(b'g', &[("path", path)]),
                pax(b'x', &[("path", "safe")]),
                entry(b"fallback", b'0', b"", None),
            ])
            .is_err()
        );
    }
    assert!(
        decode(&[
            pax(b'x', &[("path", "directory/")]),
            entry(b"regular", b'0', b"", None),
        ])
        .is_err()
    );
}

#[test]
fn pax_rejects_unsafe_fallback_paths_and_hidden_ustar_prefixes() {
    for path in [
        b"../escape".as_slice(),
        b"/absolute",
        b"COM1",
        b"a\\b",
        b"a?",
        b"invalid\xff",
        b"safe\0../hidden",
    ] {
        assert!(
            decode(&[pax(b'x', &[("path", "safe")]), entry(path, b'0', b"", None),]).is_err(),
            "{path:?}"
        );
    }
    for prefix in [
        b"..".as_slice(),
        b"/absolute",
        b"invalid\xff",
        b"safe\0../hidden",
    ] {
        let mut member = entry(b"fallback", b'0', b"", None);
        member[345..345 + prefix.len()].copy_from_slice(prefix);
        fix_checksum(&mut member);
        assert!(decode(&[pax(b'x', &[("path", "safe")]), member]).is_err());
    }
}

#[test]
fn pax_rejects_nonempty_link_fields_on_metadata_or_effective_entries() {
    for metadata in [false, true] {
        let mut extension = pax(b'x', &[("path", "safe")]);
        let mut member = entry(b"fallback", b'0', b"", None);
        let changed = if metadata {
            &mut extension
        } else {
            &mut member
        };
        changed[157..163].copy_from_slice(b"target");
        fix_checksum(changed);
        assert!(matches!(
            decode(&[extension, member]),
            Err(ArchiveError::UnsupportedEntry)
        ));
    }
}

#[test]
fn pax_effective_paths_preserve_duplicate_case_and_ancestor_conflict_checks() {
    for paths in [
        ["same", "same"],
        ["Case", "case"],
        ["Straße", "STRASSE"],
        ["Σ", "ς"],
        ["K", "K"],
        ["Dir/a", "dir/b"],
        ["parent", "parent/child"],
        ["parent/child", "parent"],
    ] {
        assert!(
            matches!(
                decode(&[
                    pax(b'x', &[("path", paths[0])]),
                    entry(b"first", b'0', b"", None),
                    pax(b'x', &[("path", paths[1])]),
                    entry(b"second", b'0', b"", None),
                ]),
                Err(ArchiveError::ConflictingPath(_))
            ),
            "{paths:?}"
        );
    }
    assert!(matches!(
        decode(&[
            pax(b'g', &[("path", "inherited")]),
            entry(b"first", b'0', b"", None),
            entry(b"second", b'0', b"", None),
        ]),
        Err(ArchiveError::ConflictingPath(_))
    ));
}

#[test]
fn pax_preserves_utf8_path_byte_depth_and_directory_suffix_bounds() {
    for path in [
        "x".repeat(MAX_ARCHIVE_PATH_BYTES),
        "界".repeat(MAX_ARCHIVE_PATH_BYTES / 3),
        ["d"; MAX_ARCHIVE_DEPTH].join("/"),
    ] {
        for kind in *b"05" {
            let effective = format!("{path}{}", if kind == b'5' { "/" } else { "" });
            let bundle = decode(&[
                pax(b'x', &[("path", &effective)]),
                entry(b"fallback", kind, b"", None),
            ])
            .unwrap();
            assert_eq!(bundle.members()[0].path(), path);
        }
    }
    for path in [
        "x".repeat(MAX_ARCHIVE_PATH_BYTES + 1),
        "界".repeat(MAX_ARCHIVE_PATH_BYTES / 3 + 1),
        ["d"; MAX_ARCHIVE_DEPTH + 1].join("/"),
    ] {
        assert!(
            decode(&[
                pax(b'x', &[("path", &path)]),
                entry(b"fallback", b'0', b"", None),
            ])
            .is_err()
        );
    }
}

#[test]
fn pax_preserves_explicit_entry_limit_without_counting_metadata_as_members() {
    let mut entries = Vec::new();
    for index in 0..MAX_ARCHIVE_ENTRIES {
        entries.push(pax(b'x', &[("path", &format!("file{index}"))]));
        entries.push(entry(b"fallback", b'0', b"", None));
    }
    assert_eq!(
        decode(&entries).unwrap().members().len(),
        MAX_ARCHIVE_ENTRIES
    );
    entries.push(pax(b'x', &[("path", "overflow")]));
    entries.push(entry(b"fallback", b'0', b"", None));
    assert!(matches!(
        decode(&entries),
        Err(ArchiveError::TooManyEntries)
    ));
}

#[test]
fn pax_effective_directory_paths_preserve_exact_manifest_byte_limit() {
    let mut entries = Vec::new();
    let count = MAX_ARCHIVE_MANIFEST_BYTES / MAX_ARCHIVE_PATH_BYTES;
    for index in 0..count {
        let path = format!("{index:04}{}/", "d".repeat(MAX_ARCHIVE_PATH_BYTES - 5));
        entries.push(pax(b'x', &[("path", &path)]));
        entries.push(entry(b"fallback/", b'5', b"", None));
    }
    assert_eq!(decode(&entries).unwrap().members().len(), count);
    entries.push(pax(b'x', &[("path", "overflow/")]));
    entries.push(entry(b"fallback/", b'5', b"", None));
    assert!(matches!(
        decode(&entries),
        Err(ArchiveError::ManifestTooLarge)
    ));
}

#[test]
fn pax_preserves_raw_and_effective_declared_expansion_limits_before_reading() {
    for kind in *b"xg" {
        assert!(matches!(
            decode(&[
                pax(
                    kind,
                    &[("size", &(MAX_ARCHIVE_EXPANDED_BYTES + 1).to_string())],
                ),
                entry(b"fallback", b'0', b"", None),
            ]),
            Err(ArchiveError::ExpandedTooLarge)
        ));
    }
    assert!(matches!(
        decode(&[
            pax(b'x', &[("size", "0")]),
            entry(b"fallback", b'0', b"", Some(MAX_ARCHIVE_EXPANDED_BYTES + 1),),
        ]),
        Err(ArchiveError::ExpandedTooLarge)
    ));
}

#[test]
fn pax_preserves_cumulative_raw_and_effective_expansion_limits() {
    assert!(matches!(
        decode(&[
            pax(b'x', &[("size", "0")]),
            entry(b"first", b'0', b"", Some(MAX_ARCHIVE_EXPANDED_BYTES)),
            pax(b'x', &[("size", "0")]),
            entry(b"second", b'0', b"", Some(1)),
        ]),
        Err(ArchiveError::ExpandedTooLarge)
    ));
    assert!(matches!(
        decode(&[
            entry(b"first", b'0', b"x", None),
            pax(b'x', &[("size", &MAX_ARCHIVE_EXPANDED_BYTES.to_string())]),
            entry(b"second", b'0', b"", Some(0)),
        ]),
        Err(ArchiveError::ExpandedTooLarge)
    ));
}

#[test]
fn pax_rejects_nonzero_directory_sizes_in_either_header() {
    for (pax_size, raw_size) in [("1", 0), ("0", 1), ("1", 1)] {
        assert!(
            decode(&[
                pax(b'x', &[("size", pax_size)]),
                entry(b"directory/", b'5', b"", Some(raw_size)),
            ])
            .is_err()
        );
    }
    assert!(
        decode(&[
            pax(b'g', &[("size", "1")]),
            entry(b"directory/", b'5', b"", None),
        ])
        .is_err()
    );
}

#[test]
fn pax_rejects_empty_or_orphaned_metadata_and_stacked_local_headers() {
    for kind in *b"xg" {
        assert!(decode(&[pax_raw(kind, b"")]).is_err());
        assert!(decode(&[pax(kind, &[("mtime", "1.5")])]).is_err());
    }
    for next in *b"xg" {
        assert!(
            decode(&[
                pax(b'x', &[("mtime", "1.5")]),
                pax(next, &[("path", "safe")]),
                entry(b"fallback", b'0', b"", None),
            ])
            .is_err()
        );
    }
}

#[test]
fn pax_rejects_ambiguous_gnu_long_name_extension_chains() {
    for entries in [
        vec![
            pax(b'x', &[("mtime", "1.5")]),
            long_name(b"gnu-name\0"),
            entry(b"fallback", b'0', b"", None),
        ],
        vec![
            long_name(b"gnu-name\0"),
            pax(b'x', &[("path", "pax-name")]),
            entry(b"fallback", b'0', b"", None),
        ],
        vec![
            long_name(b"gnu-name\0"),
            pax(b'g', &[("mtime", "1.5")]),
            entry(b"fallback", b'0', b"", None),
        ],
        vec![
            pax(b'g', &[("path", "global-name")]),
            long_name(b"gnu-name\0"),
            entry(b"fallback", b'0', b"", None),
        ],
    ] {
        assert!(decode(&entries).is_err());
    }
}

#[test]
fn pax_rejects_unsafe_metadata_container_names() {
    for name in [
        b"../escape".as_slice(),
        b"./../escape",
        b"/absolute",
        b"././../escape",
        b"invalid\xff",
        b"safe\0hidden",
    ] {
        assert!(
            decode(&[
                entry(name, b'x', &record(b"mtime", b"1.5"), None),
                entry(b"fallback", b'0', b"", None),
            ])
            .is_err(),
            "{name:?}"
        );
    }
}

#[test]
fn pax_bounds_each_metadata_header_before_reading_or_allocating() {
    for size in [MAX_ARCHIVE_PATH_BYTES as u64 * 2 + 1, u64::MAX] {
        assert!(matches!(
            decode(&[
                entry(b"PaxHeader", b'x', b"", Some(size)),
                entry(b"fallback", b'0', b"", None),
            ]),
            Err(ArchiveError::Corrupt("invalid PAX metadata size"))
        ));
    }
}

#[test]
fn pax_bounds_cumulative_metadata_payloads_even_when_values_are_discarded() {
    let extension = pax(b'g', &[("uname", &"u".repeat(4096))]);
    let mut entries = vec![extension; 700];
    entries.push(entry(b"file", b'0', b"", None));
    assert!(matches!(
        decode(&entries),
        Err(ArchiveError::Corrupt("PAX metadata exceeds its allowance"))
    ));
}

#[test]
fn pax_bounds_metadata_header_count_independently_of_payload_and_entry_counts() {
    let extension = pax(b'g', &[("mtime", "1")]);
    let mut entries = vec![extension.clone(); MAX_ARCHIVE_ENTRIES * 2];
    entries.push(entry(b"file", b'0', b"", None));
    assert_eq!(decode(&entries).unwrap().members().len(), 1);
    entries.insert(0, extension);
    assert!(matches!(
        decode(&entries),
        Err(ArchiveError::Corrupt("PAX metadata exceeds its allowance"))
    ));
}

#[test]
fn pax_rejects_shadowed_malformed_global_values() {
    for pair in [("size", "-1"), ("mtime", "NaN"), ("uid", "+1")] {
        assert!(
            decode(&[
                pax(b'g', &[pair]),
                pax(
                    b'x',
                    &[
                        ("path", "safe"),
                        ("size", "0"),
                        ("mtime", "1"),
                        ("uid", "1")
                    ]
                ),
                entry(b"fallback", b'0', b"", None),
            ])
            .is_err()
        );
    }
}

#[test]
fn pax_metadata_and_effective_payloads_retain_padding_and_truncation_checks() {
    let mut extension = pax(b'x', &[("path", "safe")]);
    extension[512 + record(b"path", b"safe").len()] = 1;
    assert!(decode(&[extension, entry(b"fallback", b'0', b"", None)]).is_err());

    let mut member = entry(b"fallback", b'0', b"data", Some(0));
    member[516] = 1;
    assert!(decode(&[pax(b'x', &[("size", "4")]), member]).is_err());

    let extension = pax(b'x', &[("path", "safe")]);
    assert!(ArchiveBundle::decode(ArchiveFormat::TarGz, &gzip(&extension[..513]),).is_err());
    let entries = [
        pax(b'x', &[("size", "8192")]),
        entry(b"fallback", b'0', b"x", Some(0)),
    ];
    assert!(decode(&entries).is_err());
}

fn record(key: &[u8], value: &[u8]) -> Vec<u8> {
    let body_length = key.len() + value.len() + 3;
    let mut length = body_length + 1;
    loop {
        let next = body_length + length.to_string().len();
        if next == length {
            break;
        }
        length = next;
    }
    let mut bytes = format!("{length} ").into_bytes();
    bytes.extend_from_slice(key);
    bytes.push(b'=');
    bytes.extend_from_slice(value);
    bytes.push(b'\n');
    bytes
}

fn pax(kind: u8, pairs: &[(&str, &str)]) -> Vec<u8> {
    let payload: Vec<u8> = pairs
        .iter()
        .flat_map(|(key, value)| record(key.as_bytes(), value.as_bytes()))
        .collect();
    pax_raw(kind, &payload)
}

fn pax_raw(kind: u8, payload: &[u8]) -> Vec<u8> {
    entry(b"././@PaxHeader", kind, payload, None)
}

fn entry(name: &[u8], kind: u8, payload: &[u8], declared: Option<u64>) -> Vec<u8> {
    assert!(name.len() <= 100);
    let mut header = tar::Header::new_ustar();
    header.set_entry_type(tar::EntryType::new(kind));
    header.set_mode(0o7777);
    header.set_uid(1234);
    header.set_gid(5678);
    header.set_mtime(1735689600);
    header.set_size(declared.unwrap_or(payload.len() as u64));
    header.as_mut_bytes()[..name.len()].copy_from_slice(name);
    header.set_cksum();
    let mut bytes = header.as_bytes().to_vec();
    bytes.extend_from_slice(payload);
    bytes.resize(bytes.len().next_multiple_of(512), 0);
    bytes
}

fn long_name(path: &[u8]) -> Vec<u8> {
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(tar::EntryType::GNULongName);
    header.set_size(path.len() as u64);
    header.as_mut_bytes()[..b"././@LongLink".len()].copy_from_slice(b"././@LongLink");
    header.set_cksum();
    let mut bytes = header.as_bytes().to_vec();
    bytes.extend_from_slice(path);
    bytes.resize(bytes.len().next_multiple_of(512), 0);
    bytes
}

fn fix_checksum(member: &mut [u8]) {
    let mut header = tar::Header::new_old();
    header.as_mut_bytes().copy_from_slice(&member[..512]);
    header.set_cksum();
    member[..512].copy_from_slice(header.as_bytes());
}

fn decode(entries: &[Vec<u8>]) -> Result<ArchiveBundle, ArchiveError> {
    let mut bytes = entries.concat();
    bytes.extend_from_slice(&[0; 1024]);
    ArchiveBundle::decode(ArchiveFormat::TarGz, &gzip(&bytes))
}

fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

// Generated in memory with Python 3.15.2's stdlib tarfile PAX_FORMAT (also its
// default format), then gzip.compress(..., mtime=0). No Python runtime is needed
// to run the regression. The writer emitted one global and four local headers:
// fractional mtime/atime/ctime, large uid/gid, Unicode uname, a long ASCII path,
// and an explicit size. The binary file crosses a 512-byte payload boundary.
const PYTHON_PAX_TAR_GZ: &[u8] = &[
    0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0xed, 0x98, 0xb1, 0x6e, 0xdb, 0x30,
    0x10, 0x86, 0x3d, 0x0b, 0xe8, 0x3b, 0xf0, 0x05, 0x42, 0xf3, 0x8e, 0x94, 0x58, 0x0d, 0x86, 0xdb,
    0xad, 0x63, 0xc6, 0xae, 0x8c, 0x4d, 0xc8, 0x02, 0x6c, 0x49, 0xa0, 0xe9, 0xd6, 0xe9, 0x94, 0xa5,
    0x43, 0xde, 0xa1, 0xcf, 0x17, 0xf4, 0x2d, 0x5a, 0x2a, 0x85, 0x1a, 0xc8, 0x8e, 0xd1, 0xa0, 0x85,
    0x58, 0x05, 0xba, 0x6f, 0x91, 0xf0, 0x6b, 0x38, 0x0a, 0xe2, 0x7f, 0xf7, 0x8b, 0x7c, 0xce, 0xe7,
    0xef, 0xae, 0xcd, 0xf1, 0x83, 0x35, 0x6b, 0xeb, 0x66, 0x83, 0x20, 0x7e, 0x71, 0xe9, 0x2a, 0x40,
    0x3e, 0xdd, 0x3f, 0xea, 0x20, 0x20, 0xcb, 0x66, 0xac, 0x18, 0x66, 0x39, 0x7d, 0x0e, 0x7b, 0x6f,
    0x5c, 0x28, 0x1f, 0xa3, 0xd6, 0x08, 0x41, 0x60, 0x87, 0x72, 0xbd, 0x00, 0x91, 0xe7, 0x29, 0x40,
    0x86, 0x5a, 0xeb, 0x2c, 0x09, 0x62, 0x11, 0x44, 0x84, 0x3c, 0x17, 0x28, 0x31, 0x0d, 0x60, 0x82,
    0x92, 0x1d, 0x2a, 0xb3, 0xb3, 0x0b, 0xe3, 0x56, 0x9b, 0xf2, 0x93, 0xbd, 0xaa, 0x3f, 0x57, 0xd6,
    0xb5, 0x72, 0xd1, 0x93, 0x0b, 0x57, 0x1f, 0x9a, 0xe4, 0x7f, 0xbf, 0x17, 0xf1, 0x32, 0xf8, 0x18,
    0xfc, 0x0f, 0xea, 0xd4, 0xff, 0x08, 0x30, 0x63, 0xc7, 0x61, 0x96, 0xd3, 0x67, 0xe2, 0xfe, 0x97,
    0x82, 0x19, 0x5f, 0x06, 0xfb, 0x82, 0x96, 0x69, 0xf6, 0x36, 0xcf, 0x04, 0xf0, 0xdf, 0xdf, 0x25,
    0x41, 0x64, 0xab, 0xfe, 0x53, 0xe4, 0x69, 0x82, 0x8a, 0xed, 0xfa, 0xaa, 0xe0, 0x80, 0x29, 0x79,
    0xfe, 0x15, 0xe2, 0xec, 0xaa, 0x76, 0xeb, 0xfd, 0x7c, 0xc8, 0x1a, 0x61, 0x2b, 0x85, 0xb1, 0xa2,
    0x2f, 0xfa, 0xbf, 0xbd, 0x07, 0x15, 0xf6, 0x52, 0xf0, 0x7d, 0xf6, 0xa8, 0x6b, 0x95, 0x86, 0xf9,
    0x9f, 0x0e, 0xb9, 0xa8, 0x8e, 0x89, 0xfb, 0x7f, 0x0c, 0xfd, 0x5f, 0x0a, 0x7d, 0xde, 0xff, 0x53,
    0xea, 0xff, 0x31, 0x00, 0x64, 0xfb, 0xf2, 0x8b, 0x5d, 0x04, 0xdf, 0x25, 0x20, 0x35, 0x6b, 0x8c,
    0xdf, 0x2c, 0xba, 0xa6, 0xb0, 0xad, 0xab, 0xe2, 0xca, 0x44, 0x85, 0xdf, 0x94, 0x55, 0x82, 0xfa,
    0x24, 0x6a, 0x3e, 0xdc, 0xdf, 0x3d, 0x7c, 0xbb, 0xff, 0x7e, 0xf7, 0xb5, 0x8d, 0x9b, 0x27, 0xb3,
    0x47, 0x72, 0x4d, 0xa3, 0xe7, 0xaf, 0x89, 0xf1, 0xa9, 0xff, 0xd0, 0xff, 0xa1, 0xfd, 0xff, 0x7b,
    0xea, 0xff, 0x21, 0x0b, 0x4a, 0xd4, 0x88, 0x33, 0x16, 0xc5, 0x93, 0x9d, 0xff, 0xbb, 0xad, 0xb6,
    0x5c, 0x2e, 0x63, 0x94, 0x1d, 0x0d, 0x3f, 0xae, 0xdf, 0x7f, 0x64, 0x65, 0xe5, 0xad, 0xab, 0x1b,
    0xeb, 0xcc, 0x4d, 0xb9, 0x2d, 0xfd, 0xed, 0x9b, 0x84, 0x74, 0xd2, 0xff, 0x49, 0x27, 0x5e, 0x05,
    0x63, 0xc8, 0x7f, 0x02, 0x9f, 0xc9, 0x7f, 0x8a, 0xf2, 0x5f, 0x0c, 0xce, 0xf3, 0x94, 0xe2, 0xf4,
    0x2b, 0x3f, 0x1d, 0xba, 0xfc, 0xb7, 0xdf, 0xd4, 0xce, 0x73, 0x7f, 0xf4, 0x03, 0xd4, 0x68, 0x4d,
    0x9d, 0x29, 0x75, 0xd9, 0xff, 0x20, 0xfb, 0xf9, 0x2f, 0x24, 0x42, 0x05, 0x91, 0xf3, 0x5f, 0x8c,
    0x5a, 0x23, 0xa4, 0xd9, 0x9a, 0xb2, 0x62, 0xde, 0x1e, 0x3d, 0x79, 0x7e, 0x8a, 0x8c, 0x63, 0xfe,
    0x67, 0xe7, 0xf3, 0x5f, 0xd2, 0xfc, 0x8f, 0x01, 0xe2, 0xe9, 0xfc, 0x4f, 0x39, 0x8d, 0xff, 0xe9,
    0x60, 0x77, 0x8d, 0xbf, 0x1d, 0xf4, 0xf4, 0xff, 0x05, 0xf3, 0xbf, 0x77, 0xfe, 0xdf, 0xf6, 0x02,
    0x0d, 0x4a, 0xd2, 0xf9, 0x3f, 0x41, 0x10, 0x04, 0x41, 0x0c, 0xc5, 0x4f, 0xbe, 0x01, 0xde, 0x5c,
    0x00, 0x28, 0x00, 0x00,
];

// Generated by macOS /usr/bin/tar --format=pax -cf - @- from an in-memory
// ordinary directory/file archive with fractional mtime/atime/ctime, uid 501,
// gid 20, ASCII owner/group names, and mode 07777; gzip mtime is zero.
// This is a writer interoperability fixture, not native extraction acceptance.
const MACOS_PAX_TAR_GZ: &[u8] = &[
    0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0xed, 0xd5, 0x4b, 0x6e, 0x83, 0x30,
    0x10, 0x06, 0x60, 0xd6, 0x9c, 0xc2, 0x17, 0x28, 0x99, 0x31, 0xd8, 0x4e, 0x16, 0xd9, 0x77, 0x99,
    0x2b, 0xb8, 0x89, 0xd5, 0xb2, 0x08, 0x44, 0x0e, 0x79, 0xdc, 0xbe, 0x75, 0xb0, 0x4a, 0x45, 0x14,
    0xf5, 0x21, 0xd5, 0xad, 0x22, 0xfe, 0x6f, 0x33, 0x42, 0x83, 0x64, 0x1b, 0xeb, 0x1f, 0x56, 0xf6,
    0xfc, 0xe8, 0xec, 0xc6, 0xf9, 0xd9, 0xae, 0xdd, 0xd7, 0xe7, 0x2c, 0x05, 0x22, 0x32, 0xc6, 0x88,
    0xbe, 0x6a, 0xd5, 0x57, 0x92, 0x55, 0xac, 0x17, 0x1c, 0x1e, 0xb8, 0x32, 0xa5, 0x62, 0x92, 0x9a,
    0x48, 0x10, 0x1b, 0x45, 0x32, 0x13, 0x69, 0x76, 0x73, 0xe5, 0xb0, 0xef, 0xac, 0x0f, 0x5b, 0xb1,
    0x7e, 0xfd, 0x52, 0x1f, 0xdd, 0x43, 0x7b, 0x6a, 0x9c, 0xbf, 0xf1, 0xde, 0x7b, 0xff, 0xd9, 0xb7,
    0x87, 0xdd, 0x8d, 0x7e, 0x3c, 0x8a, 0x18, 0xea, 0x9d, 0x28, 0x49, 0xac, 0xbb, 0x7a, 0xeb, 0x96,
    0x1c, 0xbe, 0xbf, 0x9e, 0x2f, 0x34, 0xc9, 0x62, 0xb8, 0x97, 0x3c, 0x74, 0xed, 0xb8, 0xcb, 0xc5,
    0x62, 0x1e, 0x2e, 0xb1, 0x2a, 0x25, 0xe7, 0xe1, 0xda, 0xb6, 0xe3, 0x2e, 0x15, 0x2c, 0x55, 0xfe,
    0xdf, 0x67, 0x82, 0xef, 0xeb, 0x53, 0x3f, 0x4b, 0xbb, 0x46, 0x1f, 0xff, 0x4f, 0xf2, 0x7f, 0xc9,
    0xcb, 0x38, 0xff, 0x4a, 0xb3, 0xca, 0x84, 0x4a, 0xbb, 0xad, 0x68, 0xe2, 0xf9, 0x8f, 0xf7, 0xbf,
    0x1a, 0xfe, 0x02, 0x1b, 0xdb, 0xd9, 0xe2, 0xa9, 0x6e, 0x7e, 0x73, 0x8d, 0x1f, 0xcf, 0x7f, 0xc9,
    0xac, 0x31, 0xff, 0xff, 0x04, 0xe6, 0xff, 0xb4, 0xc5, 0xfc, 0xa7, 0x48, 0xfd, 0x87, 0x2f, 0xe7,
    0xbf, 0xbc, 0x9e, 0xff, 0x46, 0x72, 0x99, 0x09, 0x4a, 0xb3, 0x9d, 0xb1, 0x89, 0xe7, 0x3f, 0x7b,
    0x6d, 0x42, 0xc2, 0x8f, 0x4e, 0x9c, 0x7c, 0xdd, 0x39, 0x8f, 0xec, 0x02, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xdc, 0xb1, 0x37, 0x92, 0x8d, 0x94, 0xff, 0x00,
    0x28, 0x00, 0x00,
];
