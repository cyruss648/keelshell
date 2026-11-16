#![expect(clippy::unwrap_used, reason = "test fixtures and assertions")]

use std::io::{Cursor, Write};

use flate2::{Compression, Crc};
use keelshell_core::{
    ArchiveBundle, ArchiveEntryKind, ArchiveError, ArchiveFormat, ArchiveMember, MAX_ARCHIVE_BYTES,
    MAX_ARCHIVE_DEPTH, MAX_ARCHIVE_ENTRIES, MAX_ARCHIVE_EXPANDED_BYTES, MAX_ARCHIVE_MANIFEST_BYTES,
    MAX_ARCHIVE_PATH_BYTES,
};

const FORMATS: [ArchiveFormat; 2] = [ArchiveFormat::Zip, ArchiveFormat::TarGz];

#[test]
fn public_bounds_match_the_archive_contract() {
    assert_eq!(MAX_ARCHIVE_BYTES, 64 * 1024 * 1024);
    assert_eq!(MAX_ARCHIVE_EXPANDED_BYTES, 128 * 1024 * 1024);
    assert_eq!(MAX_ARCHIVE_ENTRIES, 2048);
    assert_eq!(MAX_ARCHIVE_DEPTH, 32);
    assert_eq!(MAX_ARCHIVE_PATH_BYTES, 4096);
    assert_eq!(MAX_ARCHIVE_MANIFEST_BYTES, 48 * 1024);
}

#[test]
fn both_formats_roundtrip_binary_unicode_and_explicit_empty_directories() {
    let bundle = ArchiveBundle::new(vec![
        ArchiveMember::new_directory("资料").unwrap(),
        ArchiveMember::new_file("资料/二进制.dat", vec![0, 255, 128, 13, 10, 0]).unwrap(),
        ArchiveMember::new_file("empty", Vec::new()).unwrap(),
        ArchiveMember::new_directory("empty-directory/nested").unwrap(),
    ])
    .unwrap();
    assert_eq!(bundle.expanded_bytes(), 6);
    assert_eq!(bundle.members()[0].kind(), ArchiveEntryKind::Directory);
    assert!(bundle.members()[0].bytes().is_empty());
    for format in FORMATS {
        assert_eq!(
            ArchiveBundle::decode(format, &bundle.encode(format).unwrap()).unwrap(),
            bundle,
            "{format:?}"
        );
    }
}

#[test]
fn both_formats_roundtrip_an_empty_manifest() {
    let bundle = ArchiveBundle::new(Vec::new()).unwrap();
    for format in FORMATS {
        assert_eq!(
            ArchiveBundle::decode(format, &bundle.encode(format).unwrap()).unwrap(),
            bundle
        );
    }
}

#[test]
fn both_formats_roundtrip_maximum_paths_and_directory_suffixes() {
    let path = format!(
        "{}/{}",
        "a".repeat(99),
        "b".repeat(MAX_ARCHIVE_PATH_BYTES - 100)
    );
    let bundle = ArchiveBundle::new(vec![
        ArchiveMember::new_file(&path, b"binary\0\xff".to_vec()).unwrap(),
        ArchiveMember::new_directory(format!("{path}d")[1..].to_owned()).unwrap(),
    ])
    .unwrap();
    for format in FORMATS {
        assert_eq!(
            ArchiveBundle::decode(format, &bundle.encode(format).unwrap()).unwrap(),
            bundle
        );
    }
}

#[test]
fn constructors_reject_nonportable_or_escaping_paths() {
    for path in [
        "",
        "/",
        "/root",
        "//server/share",
        "C:/file",
        "c:file",
        r"\\server\share",
        r"a\b",
        ".",
        "..",
        "a/./b",
        "a/../b",
        "a//b",
        "a/",
        "a\0b",
        "a\nb",
        "a\u{0085}b",
        "a:b",
        "a<b",
        "a>b",
        "a\"b",
        "a|b",
        "a?b",
        "a*b",
        "a.",
        "a ",
        "a./b",
        "a /b",
        "CON",
        "con.txt",
        "CON .txt",
        "aux",
        "NUL.bin",
        "PRN",
        "COM1",
        "com9.bin",
        "LPT1",
        "lpt9.ext",
        "COM¹",
        "LPT².txt",
        "CONIN$",
        "CONOUT$",
        "CLOCK$",
    ] {
        assert!(
            ArchiveMember::new_file(path, Vec::new()).is_err(),
            "{path:?}"
        );
        assert!(ArchiveMember::new_directory(path).is_err(), "{path:?}");
    }
}

#[test]
fn constructor_bounds_apply_to_utf8_bytes_and_path_depth() {
    let at_depth = ["d"; MAX_ARCHIVE_DEPTH].join("/");
    assert!(ArchiveMember::new_directory(&at_depth).is_ok());
    assert!(ArchiveMember::new_directory(format!("{at_depth}/d")).is_err());
    assert!(ArchiveMember::new_file("x".repeat(MAX_ARCHIVE_PATH_BYTES), Vec::new()).is_ok());
    assert!(ArchiveMember::new_file("x".repeat(MAX_ARCHIVE_PATH_BYTES + 1), Vec::new()).is_err());
    assert!(
        ArchiveMember::new_file("界".repeat(MAX_ARCHIVE_PATH_BYTES / 3 + 1), Vec::new()).is_err()
    );
}

#[test]
fn manifest_rejects_duplicates_aliases_and_file_ancestors_in_either_order() {
    for paths in [
        ["same", "same"],
        ["Case", "case"],
        ["Straße", "STRASSE"],
        ["Σ", "ς"],
        ["K", "K"],
        ["parent", "parent/child"],
        ["parent/child", "parent"],
        ["Parent/one", "parent/two"],
        ["a/Parent/one", "a/parent/two"],
    ] {
        assert!(
            ArchiveBundle::new(
                paths
                    .map(|path| ArchiveMember::new_file(path, Vec::new()).unwrap())
                    .to_vec()
            )
            .is_err(),
            "{paths:?}"
        );
    }
    assert!(
        ArchiveBundle::new(vec![
            ArchiveMember::new_directory("dir").unwrap(),
            ArchiveMember::new_file("dir", Vec::new()).unwrap(),
        ])
        .is_err()
    );
    assert!(
        ArchiveBundle::new(vec![
            ArchiveMember::new_directory("dir").unwrap(),
            ArchiveMember::new_directory("dir").unwrap(),
        ])
        .is_err()
    );
}

#[test]
fn exact_explicit_parent_is_allowed_after_an_implicit_parent() {
    let bundle = ArchiveBundle::new(vec![
        ArchiveMember::new_file("parent/child", Vec::new()).unwrap(),
        ArchiveMember::new_directory("parent").unwrap(),
    ])
    .unwrap();
    for format in FORMATS {
        assert_eq!(
            ArchiveBundle::decode(format, &bundle.encode(format).unwrap()).unwrap(),
            bundle
        );
    }
}

#[test]
fn manifest_counts_full_paths_instead_of_only_leaf_names() {
    let prefix = "p".repeat(4090);
    let members = (0..12)
        .map(|index| ArchiveMember::new_file(format!("{prefix}/{index:03}"), Vec::new()).unwrap())
        .collect::<Vec<_>>();
    assert!(ArchiveBundle::new(members.clone()).is_ok());
    let mut oversized = members;
    oversized.push(ArchiveMember::new_file(format!("{prefix}/012"), Vec::new()).unwrap());
    assert!(matches!(
        ArchiveBundle::new(oversized),
        Err(ArchiveError::ManifestTooLarge)
    ));
}

#[test]
fn manifest_counts_directory_suffixes_at_the_exact_manifest_bound() {
    let mut members = (0..12)
        .map(|index| {
            ArchiveMember::new_directory(format!("{index:04}{}", "d".repeat(4091))).unwrap()
        })
        .collect::<Vec<_>>();
    let bundle = ArchiveBundle::new(members.clone()).unwrap();
    for format in FORMATS {
        assert_eq!(
            ArchiveBundle::decode(format, &bundle.encode(format).unwrap()).unwrap(),
            bundle
        );
    }
    members.push(ArchiveMember::new_directory("one-more").unwrap());
    assert!(matches!(
        ArchiveBundle::new(members),
        Err(ArchiveError::ManifestTooLarge)
    ));
}

#[test]
fn manifest_accepts_entry_limit_and_rejects_one_more() {
    let mut members = (0..MAX_ARCHIVE_ENTRIES)
        .map(|index| ArchiveMember::new_file(format!("f{index}"), Vec::new()).unwrap())
        .collect::<Vec<_>>();
    assert!(ArchiveBundle::new(members.clone()).is_ok());
    members.push(ArchiveMember::new_file("overflow", Vec::new()).unwrap());
    assert!(matches!(
        ArchiveBundle::new(members),
        Err(ArchiveError::TooManyEntries)
    ));
}

#[test]
fn both_decoders_reject_oversized_encoded_input_before_parsing() {
    let bytes = vec![0; MAX_ARCHIVE_BYTES as usize + 1];
    for format in FORMATS {
        assert!(matches!(
            ArchiveBundle::decode(format, &bytes),
            Err(ArchiveError::ArchiveTooLarge)
        ));
    }
}

#[test]
fn file_constructor_rejects_actual_payload_over_expanded_limit() {
    assert!(matches!(
        ArchiveMember::new_file("large", vec![0; MAX_ARCHIVE_EXPANDED_BYTES as usize + 1]),
        Err(ArchiveError::ExpandedTooLarge)
    ));
}

#[test]
fn both_decoders_reject_empty_and_unrelated_data() {
    for format in FORMATS {
        for bytes in [&b""[..], &b"not an archive"[..], &b"PK\x05\x06"[..]] {
            assert!(ArchiveBundle::decode(format, bytes).is_err());
        }
    }
}

#[test]
fn zip_accepts_stored_deflated_and_data_descriptor_members() {
    let mut deflated = ZipFixture::file(b"deflated", &[1; 40_000]);
    deflated.method = 8;
    deflated.flags = 8;
    let archive = raw_zip(&[ZipFixture::file(b"stored", b"\0\xff"), deflated]);
    let bundle = ArchiveBundle::decode(ArchiveFormat::Zip, &archive).unwrap();
    assert_eq!(bundle.members()[0].bytes(), b"\0\xff");
    assert_eq!(bundle.members()[1].bytes(), &[1; 40_000]);
}

#[test]
fn zip_rejects_raw_invalid_utf8_and_dangerous_names() {
    for name in [
        &b"\xff"[..],
        &b"../escape"[..],
        &b"/absolute"[..],
        &b"a\\b"[..],
        &b"NUL.txt"[..],
    ] {
        assert!(
            ArchiveBundle::decode(
                ArchiveFormat::Zip,
                &raw_zip(&[ZipFixture::file(name, b"x")])
            )
            .is_err()
        );
    }
}

#[test]
fn zip_rejects_duplicate_raw_names_and_case_aliased_parents() {
    for names in [
        [b"same".as_slice(), b"same"],
        [b"Dir/a".as_slice(), b"dir/b"],
    ] {
        assert!(
            ArchiveBundle::decode(
                ArchiveFormat::Zip,
                &raw_zip(&names.map(|name| ZipFixture::file(name, b"")))
            )
            .is_err()
        );
    }
}

#[test]
fn zip_rejects_links_special_modes_and_directory_payloads() {
    for mode in [0o120777, 0o020600, 0o060600, 0o010600, 0o140600] {
        let mut member = ZipFixture::file(b"special", b"target");
        member.mode = mode;
        assert!(ArchiveBundle::decode(ArchiveFormat::Zip, &raw_zip(&[member])).is_err());
    }
    let mut directory = ZipFixture::file(b"directory/", b"hidden");
    directory.mode = 0o040700;
    assert!(ArchiveBundle::decode(ArchiveFormat::Zip, &raw_zip(&[directory])).is_err());
}

#[test]
fn zip_rejects_regular_mode_on_a_directory_name() {
    assert!(
        ArchiveBundle::decode(
            ArchiveFormat::Zip,
            &raw_zip(&[ZipFixture::file(b"directory/", b"")])
        )
        .is_err()
    );
}

#[test]
fn zip_rejects_encryption_and_unsupported_compression() {
    for flag in [1, 0x40, 0x2000] {
        let mut entry = ZipFixture::file(b"secret", b"data");
        entry.flags = flag;
        assert!(matches!(
            ArchiveBundle::decode(ArchiveFormat::Zip, &raw_zip(&[entry])),
            Err(ArchiveError::EncryptedZip)
        ));
    }
    let mut entry = ZipFixture::file(b"unsupported", b"data");
    entry.method = 12;
    assert!(ArchiveBundle::decode(ArchiveFormat::Zip, &raw_zip(&[entry])).is_err());
}

#[test]
fn zip_rejects_declared_expansion_over_limit_before_inflating() {
    let mut entry = ZipFixture::file(b"oversized", b"");
    entry.declared = Some(MAX_ARCHIVE_EXPANDED_BYTES as u32 + 1);
    assert!(matches!(
        ArchiveBundle::decode(ArchiveFormat::Zip, &raw_zip(&[entry])),
        Err(ArchiveError::ExpandedTooLarge)
    ));
}

#[test]
fn zip_rejects_actual_expansion_beyond_the_declared_size() {
    let mut entry = ZipFixture::file(b"bomb", &[0; 64 * 1024]);
    entry.method = 8;
    entry.declared = Some(1);
    assert!(ArchiveBundle::decode(ArchiveFormat::Zip, &raw_zip(&[entry])).is_err());
}

#[test]
fn zip_rejects_crc_corruption_and_unconsumed_deflate_bytes() {
    for method in [0, 8] {
        let mut entry = ZipFixture::file(b"checksum", b"data");
        entry.method = method;
        entry.bad_crc = true;
        assert!(ArchiveBundle::decode(ArchiveFormat::Zip, &raw_zip(&[entry])).is_err());
    }
    let mut entry = ZipFixture::file(b"trailing", b"data");
    entry.method = 8;
    entry.compressed_suffix = vec![0, 1, 2, 3];
    assert!(ArchiveBundle::decode(ArchiveFormat::Zip, &raw_zip(&[entry])).is_err());
}

#[test]
fn zip_rejects_truncated_archives_and_mismatched_local_names() {
    let archive = raw_zip(&[ZipFixture::file(b"member", b"payload")]);
    for length in [0, 4, 30, archive.len() / 2, archive.len() - 1] {
        assert!(ArchiveBundle::decode(ArchiveFormat::Zip, &archive[..length]).is_err());
    }
    let mut mismatched = archive.clone();
    mismatched[30] = b'x';
    assert!(ArchiveBundle::decode(ArchiveFormat::Zip, &mismatched).is_err());
    let mut trailing = archive;
    trailing.push(0);
    assert!(ArchiveBundle::decode(ArchiveFormat::Zip, &trailing).is_err());
}

#[test]
fn zip_rejects_unlisted_local_headers_and_overlapping_offsets() {
    let mut archive = raw_zip(&[
        ZipFixture::file(b"first", b"one"),
        ZipFixture::file(b"second", b"two"),
    ]);
    let central = archive
        .windows(4)
        .position(|bytes| bytes == b"PK\x01\x02")
        .unwrap();
    let second = central + 46 + b"first".len();
    archive[second + 42..second + 46].copy_from_slice(&0u32.to_le_bytes());
    assert!(ArchiveBundle::decode(ArchiveFormat::Zip, &archive).is_err());

    let mut archive = raw_zip(&[
        ZipFixture::file(b"first", b"one"),
        ZipFixture::file(b"second", b"two"),
    ]);
    let second_length = 46 + b"second".len();
    archive.drain(second..second + second_length);
    let end = archive.len() - 22;
    archive[end + 8..end + 10].copy_from_slice(&1u16.to_le_bytes());
    archive[end + 10..end + 12].copy_from_slice(&1u16.to_le_bytes());
    archive[end + 12..end + 16].copy_from_slice(&(46u32 + 5).to_le_bytes());
    assert!(ArchiveBundle::decode(ArchiveFormat::Zip, &archive).is_err());
}

#[test]
fn tar_rejects_raw_invalid_utf8_and_unsafe_paths() {
    for name in [
        &b"\xff"[..],
        &b"../escape"[..],
        &b"/absolute"[..],
        &b"a\\b"[..],
        &b"COM1"[..],
    ] {
        let archive = tar_gz(&tar_stream(&[tar_member(name, b'0', b"x", None)]));
        assert!(ArchiveBundle::decode(ArchiveFormat::TarGz, &archive).is_err());
    }
}

#[test]
fn tar_rejects_links_devices_sparse_and_unhandled_extensions() {
    for &kind in b"123467SKxg" {
        let archive = tar_gz(&tar_stream(&[tar_member(b"member", kind, b"", None)]));
        assert!(
            ArchiveBundle::decode(ArchiveFormat::TarGz, &archive).is_err(),
            "{kind}"
        );
    }
}

#[test]
fn tar_rejects_directory_payload_and_nonempty_link_fields_on_regular_files() {
    let archive = tar_gz(&tar_stream(&[tar_member(b"dir/", b'5', b"x", None)]));
    assert!(ArchiveBundle::decode(ArchiveFormat::TarGz, &archive).is_err());
    let mut member = tar_member(b"regular", b'0', b"", None);
    member[157..163].copy_from_slice(b"target");
    fix_tar_checksum(&mut member);
    assert!(ArchiveBundle::decode(ArchiveFormat::TarGz, &tar_gz(&tar_stream(&[member]))).is_err());
}

#[test]
fn tar_rejects_header_corruption_and_nul_hidden_path_suffixes() {
    let mut member = tar_member(b"member", b'0', b"", None);
    member[0] ^= 1;
    assert!(ArchiveBundle::decode(ArchiveFormat::TarGz, &tar_gz(&tar_stream(&[member]))).is_err());
    let member = tar_member(b"safe\0../hidden", b'0', b"", None);
    assert!(ArchiveBundle::decode(ArchiveFormat::TarGz, &tar_gz(&tar_stream(&[member]))).is_err());
}

#[test]
fn tar_rejects_declared_expansion_and_truncated_payload_or_padding() {
    let member = tar_member(b"large", b'0', b"", Some(MAX_ARCHIVE_EXPANDED_BYTES + 1));
    assert!(matches!(
        ArchiveBundle::decode(ArchiveFormat::TarGz, &tar_gz(&tar_stream(&[member]))),
        Err(ArchiveError::ExpandedTooLarge)
    ));
    let mut member = tar_member(b"short", b'0', b"x", Some(8192));
    assert!(
        ArchiveBundle::decode(
            ArchiveFormat::TarGz,
            &tar_gz(&tar_stream(&[member.clone()]))
        )
        .is_err()
    );
    member.truncate(513);
    assert!(ArchiveBundle::decode(ArchiveFormat::TarGz, &tar_gz(&member)).is_err());
}

#[test]
fn tar_rejects_unsafe_ustar_prefixes() {
    for prefix in [&b".."[..], &b"/root"[..], &b"a\\b"[..], &b"\xff"[..]] {
        let mut header = tar::Header::new_ustar();
        header.set_entry_type(tar::EntryType::Regular);
        header.set_size(0);
        header.set_mode(0o600);
        header.set_path("file").unwrap();
        header.as_mut_bytes()[345..345 + prefix.len()].copy_from_slice(prefix);
        header.set_cksum();
        let archive = tar_gz(&tar_stream(&[header.as_bytes().to_vec()]));
        assert!(ArchiveBundle::decode(ArchiveFormat::TarGz, &archive).is_err());
    }
}

#[test]
fn tar_rejects_duplicate_names_and_aliased_implicit_parents() {
    for paths in [
        [b"same".as_slice(), b"same"],
        [b"Dir/a".as_slice(), b"dir/b"],
    ] {
        let members = paths.map(|name| tar_member(name, b'0', b"", None));
        assert!(
            ArchiveBundle::decode(ArchiveFormat::TarGz, &tar_gz(&tar_stream(&members))).is_err()
        );
    }
}

#[test]
fn tar_rejects_unsafe_or_orphaned_gnu_long_names() {
    for name in [
        &b"../escape\0"[..],
        &b"bad\xff\0"[..],
        &b"missing-terminator"[..],
    ] {
        let members = [
            tar_member(b"././@LongLink", b'L', name, None),
            tar_member(b"safe", b'0', b"", None),
        ];
        assert!(
            ArchiveBundle::decode(ArchiveFormat::TarGz, &tar_gz(&tar_stream(&members))).is_err()
        );
    }
    let extension = tar_member(b"././@LongLink", b'L', b"orphan\0", None);
    assert!(
        ArchiveBundle::decode(ArchiveFormat::TarGz, &tar_gz(&tar_stream(&[extension]))).is_err()
    );
}

#[test]
fn tar_requires_two_end_blocks_and_zero_aligned_trailing_padding() {
    let member = tar_member(b"file", b'0', b"x", None);
    assert!(ArchiveBundle::decode(ArchiveFormat::TarGz, &tar_gz(&member)).is_err());
    let mut one_end = member.clone();
    one_end.extend_from_slice(&[0; 512]);
    assert!(ArchiveBundle::decode(ArchiveFormat::TarGz, &tar_gz(&one_end)).is_err());
    let valid = tar_stream(&[member]);
    for tail in [
        vec![1; 512],
        vec![0; 1],
        tar_member(b"hidden", b'0', b"x", None),
    ] {
        let mut stream = valid.clone();
        stream.extend(tail);
        assert!(ArchiveBundle::decode(ArchiveFormat::TarGz, &tar_gz(&stream)).is_err());
    }
    let mut stream = valid;
    stream.extend_from_slice(&[0; 4096]);
    assert!(ArchiveBundle::decode(ArchiveFormat::TarGz, &tar_gz(&stream)).is_ok());
}

#[test]
fn gzip_trailer_is_verified_after_tar_iteration_would_have_stopped() {
    let valid = tar_gz(&tar_stream(&[tar_member(b"file", b'0', b"data", None)]));
    for distance in [8, 4, 1] {
        let mut corrupt = valid.clone();
        let index = corrupt.len() - distance;
        corrupt[index] ^= 1;
        assert!(ArchiveBundle::decode(ArchiveFormat::TarGz, &corrupt).is_err());
    }
    for missing in 1..=8 {
        assert!(
            ArchiveBundle::decode(ArchiveFormat::TarGz, &valid[..valid.len() - missing]).is_err()
        );
    }
}

#[test]
fn gzip_rejects_concatenated_members_and_trailing_compressed_bytes() {
    let valid = tar_gz(&tar_stream(&[]));
    for tail in [vec![0], tar_gz(b""), valid.clone()] {
        let mut archive = valid.clone();
        archive.extend(tail);
        assert!(ArchiveBundle::decode(ArchiveFormat::TarGz, &archive).is_err());
    }
}

#[test]
fn tar_streaming_budget_also_bounds_zero_padding_bombs() {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), Compression::fast());
    let zeros = [0; 64 * 1024];
    for _ in 0..(MAX_ARCHIVE_EXPANDED_BYTES / zeros.len() as u64 + 128) {
        encoder.write_all(&zeros).unwrap();
    }
    assert!(ArchiveBundle::decode(ArchiveFormat::TarGz, &encoder.finish().unwrap()).is_err());
}

#[test]
fn decoding_enforces_entry_and_full_manifest_limits_in_both_formats() {
    let zip_members = (0..=MAX_ARCHIVE_ENTRIES)
        .map(|index| ZipFixture::file(format!("f{index}").as_bytes(), b""))
        .collect::<Vec<_>>();
    assert!(matches!(
        ArchiveBundle::decode(ArchiveFormat::Zip, &raw_zip(&zip_members)),
        Err(ArchiveError::TooManyEntries)
    ));
    let tar_members = (0..=MAX_ARCHIVE_ENTRIES)
        .map(|index| tar_member(format!("f{index}").as_bytes(), b'0', b"", None))
        .collect::<Vec<_>>();
    assert!(matches!(
        ArchiveBundle::decode(ArchiveFormat::TarGz, &tar_gz(&tar_stream(&tar_members))),
        Err(ArchiveError::TooManyEntries)
    ));
    let names = (0..600)
        .map(|index| format!("{index:04}{}", "n".repeat(90)))
        .collect::<Vec<_>>();
    let zip_members = names
        .iter()
        .map(|name| ZipFixture::file(name.as_bytes(), b""))
        .collect::<Vec<_>>();
    assert!(matches!(
        ArchiveBundle::decode(ArchiveFormat::Zip, &raw_zip(&zip_members)),
        Err(ArchiveError::ManifestTooLarge)
    ));
    let tar_members = names
        .iter()
        .map(|name| tar_member(name.as_bytes(), b'0', b"", None))
        .collect::<Vec<_>>();
    assert!(matches!(
        ArchiveBundle::decode(ArchiveFormat::TarGz, &tar_gz(&tar_stream(&tar_members))),
        Err(ArchiveError::ManifestTooLarge)
    ));
}

#[test]
fn zip_records_private_file_and_directory_modes_in_unix_metadata() {
    let bundle = ArchiveBundle::new(vec![
        ArchiveMember::new_directory("private").unwrap(),
        ArchiveMember::new_file("private/data.bin", b"\0\xffdata".to_vec()).unwrap(),
        ArchiveMember::new_directory("private/empty").unwrap(),
    ])
    .unwrap();
    let encoded = bundle.encode(ArchiveFormat::Zip).unwrap();
    let mut archive = zip::ZipArchive::new(Cursor::new(&encoded)).unwrap();
    assert_eq!(archive.len(), bundle.members().len());
    for index in 0..archive.len() {
        let entry = archive.by_index(index).unwrap();
        let expected = if entry.is_dir() { 0o40700 } else { 0o100600 };
        assert_eq!(
            entry.system(),
            zip::System::Unix,
            "{}",
            entry.name().unwrap()
        );
        assert_eq!(entry.unix_mode(), Some(expected));
        let header = usize::try_from(entry.central_header_start()).unwrap();
        assert_eq!(&encoded[header..header + 4], b"PK\x01\x02");
        assert_eq!(encoded[header + 5], 3, "central-directory creator OS");
        let attributes = u32::from_le_bytes(encoded[header + 38..header + 42].try_into().unwrap());
        assert_eq!(attributes >> 16, expected, "raw Unix mode and file type");
    }
    assert_eq!(
        ArchiveBundle::decode(ArchiveFormat::Zip, &encoded).unwrap(),
        bundle
    );
}

#[test]
fn reencoding_discards_dangerous_incoming_permissions_and_ownership() {
    let mut member = ZipFixture::file(b"file", b"data");
    member.mode = 0o107777;
    let bundle = ArchiveBundle::decode(ArchiveFormat::Zip, &raw_zip(&[member])).unwrap();
    let zip_bytes = bundle.encode(ArchiveFormat::Zip).unwrap();
    let mut archive = zip::ZipArchive::new(Cursor::new(zip_bytes)).unwrap();
    assert_eq!(
        archive.by_index(0).unwrap().unix_mode().unwrap() & 0o7777,
        0o600
    );
    let tar_bytes = bundle.encode(ArchiveFormat::TarGz).unwrap();
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(tar_bytes.as_slice()));
    let member = archive.entries().unwrap().next().unwrap().unwrap();
    assert_eq!(member.header().mode().unwrap(), 0o600);
    assert_eq!(member.header().uid().unwrap(), 0);
    assert_eq!(member.header().gid().unwrap(), 0);
}

#[test]
fn both_encoders_bound_incompressible_output_during_compression() {
    let mut state = 0x1234_5678_90ab_cdefu64;
    let mut bytes = Vec::with_capacity(MAX_ARCHIVE_BYTES as usize + 128 * 1024);
    for _ in 0..bytes.capacity() / 8 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        bytes.extend_from_slice(&state.to_le_bytes());
    }
    let bundle =
        ArchiveBundle::new(vec![ArchiveMember::new_file("noise", bytes).unwrap()]).unwrap();
    for format in FORMATS {
        assert!(bundle.encode(format).is_err(), "{format:?}");
    }
}

struct ZipFixture {
    name: Vec<u8>,
    bytes: Vec<u8>,
    method: u16,
    flags: u16,
    mode: u32,
    declared: Option<u32>,
    bad_crc: bool,
    compressed_suffix: Vec<u8>,
}

impl ZipFixture {
    fn file(name: &[u8], bytes: &[u8]) -> Self {
        Self {
            name: name.to_vec(),
            bytes: bytes.to_vec(),
            method: 0,
            flags: 0,
            mode: 0o100600,
            declared: None,
            bad_crc: false,
            compressed_suffix: Vec::new(),
        }
    }
}

fn raw_zip(entries: &[ZipFixture]) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut central = Vec::new();
    for entry in entries {
        let offset = bytes.len() as u32;
        let mut crc = Crc::new();
        crc.update(&entry.bytes);
        let crc = crc.sum() ^ u32::from(entry.bad_crc);
        let mut compressed = if entry.method == 8 {
            let mut encoder =
                flate2::write::DeflateEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(&entry.bytes).unwrap();
            encoder.finish().unwrap()
        } else {
            entry.bytes.clone()
        };
        compressed.extend_from_slice(&entry.compressed_suffix);
        let size = entry.declared.unwrap_or(entry.bytes.len() as u32);
        let descriptor = entry.flags & 8 != 0;
        bytes.extend_from_slice(b"PK\x03\x04");
        for value in [20, entry.flags, entry.method, 0, 0] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        for value in [crc, compressed.len() as u32, size] {
            bytes.extend_from_slice(&if descriptor { 0u32 } else { value }.to_le_bytes());
        }
        bytes.extend_from_slice(&(entry.name.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&entry.name);
        bytes.extend_from_slice(&compressed);
        if descriptor {
            bytes.extend_from_slice(b"PK\x07\x08");
            for value in [crc, compressed.len() as u32, size] {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        central.extend_from_slice(b"PK\x01\x02");
        for value in [0x0314u16, 20, entry.flags, entry.method, 0, 0] {
            central.extend_from_slice(&value.to_le_bytes());
        }
        for value in [crc, compressed.len() as u32, size] {
            central.extend_from_slice(&value.to_le_bytes());
        }
        for value in [entry.name.len() as u16, 0, 0, 0, 0] {
            central.extend_from_slice(&value.to_le_bytes());
        }
        central.extend_from_slice(&(entry.mode << 16).to_le_bytes());
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(&entry.name);
    }
    let central_start = bytes.len() as u32;
    let central_length = central.len() as u32;
    bytes.extend(central);
    bytes.extend_from_slice(b"PK\x05\x06");
    for value in [0u16, 0, entries.len() as u16, entries.len() as u16] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&central_length.to_le_bytes());
    bytes.extend_from_slice(&central_start.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes
}

fn tar_member(name: &[u8], kind: u8, bytes: &[u8], declared: Option<u64>) -> Vec<u8> {
    assert!(name.len() <= 100);
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(tar::EntryType::new(kind));
    header.set_mode(0o600);
    header.set_uid(1234);
    header.set_gid(5678);
    header.set_size(declared.unwrap_or(bytes.len() as u64));
    header.as_mut_bytes()[..name.len()].copy_from_slice(name);
    header.set_cksum();
    let mut member = header.as_bytes().to_vec();
    member.extend_from_slice(bytes);
    member.resize(member.len().next_multiple_of(512), 0);
    member
}

fn fix_tar_checksum(member: &mut [u8]) {
    let mut header = tar::Header::new_old();
    header.as_mut_bytes().copy_from_slice(&member[..512]);
    header.set_cksum();
    member[..512].copy_from_slice(header.as_bytes());
}

fn tar_stream(members: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = members.concat();
    bytes.extend_from_slice(&[0; 1024]);
    bytes
}

fn tar_gz(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}
