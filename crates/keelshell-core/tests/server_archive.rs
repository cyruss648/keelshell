#![expect(clippy::unwrap_used, reason = "test fixtures and assertions")]

use keelshell_core::{
    ArchiveBundle, ArchiveEntryKind, ArchiveFormat, ArchiveMember, MAX_ARCHIVE_EXPANDED_BYTES,
    MAX_SERVER_ARCHIVE_INPUT_BYTES, MAX_SERVER_ARCHIVE_RESPONSE_BYTES, ServerArchiveError,
    ServerArchiveManifest, ServerArchiveReceipt, ServerArchiveRequest,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn digest_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn request(format: ArchiveFormat) -> ServerArchiveRequest {
    ServerArchiveRequest::new(
        vec!["/sandbox/资料's;$(false)".to_owned()],
        match format {
            ArchiveFormat::Zip => "/sandbox/result.zip",
            ArchiveFormat::TarGz => "/sandbox/result.tar.gz",
        }
        .to_owned(),
        format,
    )
    .unwrap()
}

fn snapshot(directory: bool, size: u64, inode: u64) -> Value {
    json!({
        "dev": 1, "ino": inode,
        "mode": if directory { 0o040700 } else { 0o100600 },
        "nlink": if directory { 2 } else { 1 },
        "size": size, "mtime_ns": -1, "ctime_ns": 1,
        "uid": 501, "gid": 20, "flags": 0,
    })
}

fn report(request: &ServerArchiveRequest) -> Value {
    let root = request.sources()[0].rsplit('/').next().unwrap();
    json!({
        "version": 1,
        "request": {
            "version": 1,
            "sources": request.sources(),
            "destination": request.destination(),
            "format": match request.format() {
                ArchiveFormat::Zip => "zip",
                ArchiveFormat::TarGz => "tar.gz",
            },
        },
        "root": snapshot(true, 128, 1),
        "members": [
            {"path": root, "kind": "directory", "bytes": 0,
             "sha256": null, "stat": snapshot(true, 128, 2)},
            {"path": format!("{root}/数据"), "kind": "file", "bytes": 3,
             "sha256": digest_hex(b"a\0b"),
             "stat": snapshot(false, 3, 3)},
            {"path": format!("{root}/empty"), "kind": "directory", "bytes": 0,
             "sha256": null, "stat": snapshot(true, 0, 4)},
        ],
        "expanded_bytes": 3,
    })
}

fn parse(
    request: &ServerArchiveRequest,
    report: &Value,
) -> Result<ServerArchiveManifest, ServerArchiveError> {
    ServerArchiveManifest::parse(request, &serde_json::to_vec(report).unwrap())
}

#[test]
fn protocol_limits_are_at_most_one_mib() {
    assert_eq!(MAX_SERVER_ARCHIVE_INPUT_BYTES, 1024 * 1024);
    assert_eq!(MAX_SERVER_ARCHIVE_RESPONSE_BYTES, 1024 * 1024);
}

#[test]
fn request_keeps_exact_unicode_quotes_and_metacharacters_inside_quoted_arguments() {
    let request = request(ArchiveFormat::Zip);
    assert_eq!(request.sources(), &["/sandbox/资料's;$(false)"]);
    assert_eq!(request.destination(), "/sandbox/result.zip");
    assert_eq!(request.format(), ArchiveFormat::Zip);
    let command = request.inspection_command();
    assert!(command.starts_with("python3 -c '"));
    assert!(command.contains(" inspect '"));
    assert!(command.contains("'\"'\"'"));
    // User paths occur only in the base64 argument, never as shell syntax.
    assert!(!command.contains("/sandbox/"));
}

#[test]
fn request_rejects_unnormalized_nonportable_and_format_control_paths() {
    for path in [
        "",
        "/",
        "relative",
        "//sandbox/x",
        "/sandbox/./x",
        "/sandbox/../x",
        "/sandbox/x/",
        "/sandbox/a\\b",
        "/sandbox/CON.txt",
        "/sandbox/a.",
        "/sandbox/a ",
        "/sandbox/a\nb",
        "/sandbox/a\u{200b}b",
        "/sandbox/a\u{202e}b",
        "/sandbox/a\u{00ad}b",
        "/sandbox/a\"b",
    ] {
        assert!(
            ServerArchiveRequest::new(
                vec![path.to_owned()],
                "/sandbox/out.zip".to_owned(),
                ArchiveFormat::Zip,
            )
            .is_err(),
            "{path:?}"
        );
    }
}

#[test]
fn request_rejects_differing_parents_aliases_collisions_and_wrong_suffix() {
    for sources in [
        vec![],
        vec!["/sandbox/a", "/elsewhere/b"],
        vec!["/sandbox/a", "/sandbox/a"],
        vec!["/sandbox/Σ", "/sandbox/ς"],
        vec!["/sandbox/Straße", "/sandbox/STRASSE"],
        vec!["/sandbox/OUT.ZIP"],
    ] {
        assert!(
            ServerArchiveRequest::new(
                sources.into_iter().map(str::to_owned).collect(),
                "/sandbox/out.zip".to_owned(),
                ArchiveFormat::Zip,
            )
            .is_err()
        );
    }
    for (destination, format) in [
        ("/elsewhere/out.zip", ArchiveFormat::Zip),
        ("/sandbox/out.tar.gz", ArchiveFormat::Zip),
        ("/sandbox/out.zip", ArchiveFormat::TarGz),
        ("/sandbox/out", ArchiveFormat::Zip),
    ] {
        assert!(
            ServerArchiveRequest::new(
                vec!["/sandbox/a".to_owned()],
                destination.to_owned(),
                format,
            )
            .is_err()
        );
    }
}

#[test]
fn request_bounds_sources_full_path_bytes_and_path_depth() {
    assert!(
        ServerArchiveRequest::new(
            (0..33).map(|index| format!("/sandbox/{index}")).collect(),
            "/sandbox/out.zip".to_owned(),
            ArchiveFormat::Zip,
        )
        .is_err()
    );
    assert!(
        ServerArchiveRequest::new(
            (0..13)
                .map(|index| format!("/sandbox/{index}{}", "x".repeat(3900)))
                .collect(),
            "/sandbox/out.zip".to_owned(),
            ArchiveFormat::Zip,
        )
        .is_err()
    );
    let deep = format!("/{}/x", ["d"; 32].join("/"));
    assert!(
        ServerArchiveRequest::new(vec![deep], "/out.zip".to_owned(), ArchiveFormat::Zip).is_err()
    );
}

#[test]
fn packing_command_requires_a_canonical_sibling_stage_directory_uuid() {
    let request = request(ArchiveFormat::Zip);
    let stage = "/sandbox/.keelshell-server-archive-00000000-0000-0000-0000-000000000001";
    assert!(
        request
            .packing_command(stage)
            .unwrap()
            .ends_with(&format!(" '{stage}'"))
    );
    for stage in [
        "/elsewhere/.keelshell-server-archive-00000000-0000-0000-0000-000000000001",
        "/sandbox/staging",
        "/sandbox/.keelshell-server-archive-00000000000000000000000000000001",
        "/sandbox/.keelshell-server-archive-AAAAAAAA-0000-0000-0000-000000000001",
        "/sandbox/.keelshell-server-archive-00000000-0000-0000-0000-000000000001/",
    ] {
        assert!(request.packing_command(stage).is_err(), "{stage}");
    }
    let collision = ServerArchiveRequest::new(
        vec![stage.to_owned()],
        "/sandbox/result.zip".to_owned(),
        ArchiveFormat::Zip,
    )
    .unwrap();
    assert!(collision.packing_command(stage).is_err());
}

#[test]
fn complete_manifest_roundtrips_canonical_utf8_without_placeholder_file_allocation() {
    let request = request(ArchiveFormat::Zip);
    let manifest = parse(&request, &report(&request)).unwrap();
    assert_eq!(manifest.expanded_bytes(), 3);
    assert_eq!(manifest.members().len(), 3);
    assert_eq!(manifest.members()[1].kind(), ArchiveEntryKind::File);
    assert_eq!(manifest.members()[1].bytes(), 3);
    assert_eq!(manifest.members()[0].sha256(), None);
    let canonical = manifest.packing_input();
    let text = std::str::from_utf8(&canonical).unwrap();
    assert!(text.contains("资料's;$(false)"));
    assert_eq!(
        ServerArchiveManifest::parse(&request, &canonical).unwrap(),
        manifest
    );

    let mut large = report(&request);
    large["members"][1]["bytes"] = json!(MAX_ARCHIVE_EXPANDED_BYTES);
    large["members"][1]["stat"]["size"] = json!(MAX_ARCHIVE_EXPANDED_BYTES);
    large["expanded_bytes"] = json!(MAX_ARCHIVE_EXPANDED_BYTES);
    assert_eq!(
        parse(&request, &large).unwrap().packing_input().len(),
        serde_json::to_vec(&large).unwrap().len()
    );
}

#[test]
fn manifest_rejects_wrong_request_versions_unknown_duplicate_and_missing_fields() {
    let request = request(ArchiveFormat::Zip);
    let base = report(&request);
    for altered in [
        {
            let mut value = base.clone();
            value["version"] = json!(2);
            value
        },
        {
            let mut value = base.clone();
            value["request"]["destination"] = json!("/sandbox/other.zip");
            value
        },
        {
            let mut value = base.clone();
            value["unknown"] = json!(0);
            value
        },
        {
            let mut value = base.clone();
            value["root"]["unknown"] = json!(0);
            value
        },
        {
            let mut value = base.clone();
            value["members"][1]["unknown"] = json!(0);
            value
        },
        {
            let mut value = base.clone();
            value["members"][1]
                .as_object_mut()
                .unwrap()
                .remove("sha256");
            value
        },
        {
            let mut value = base.clone();
            value["request"]["version"] = json!(true);
            value
        },
    ] {
        assert!(parse(&request, &altered).is_err(), "{altered}");
    }
    let raw = serde_json::to_string(&base).unwrap();
    let duplicate = raw.replacen("\"version\":1", "\"version\":1,\"version\":1", 1);
    assert!(ServerArchiveManifest::parse(&request, duplicate.as_bytes()).is_err());
    assert!(
        ServerArchiveManifest::parse(&request, &vec![b' '; MAX_SERVER_ARCHIVE_RESPONSE_BYTES + 1])
            .is_err()
    );
    assert!(ServerArchiveManifest::parse(&request, b"{\"version\":1}\0").is_err());
    assert!(ServerArchiveManifest::parse(&request, &[0xff]).is_err());
}

#[test]
fn manifest_rejects_missing_roots_ancestors_aliases_and_escaping_members() {
    let request = request(ArchiveFormat::Zip);
    let root = request.sources()[0].rsplit('/').next().unwrap();
    let base = report(&request);
    for path in [
        "../escape".to_owned(),
        "/absolute".to_owned(),
        "other/file".to_owned(),
        format!("{root}/../escape"),
        format!("{root}/missing/child"),
        format!("{root}/a\u{202e}b"),
        format!("{root}/DATA"),
    ] {
        let mut value = base.clone();
        value["members"][1]["path"] = json!(path);
        if path.ends_with("/DATA") {
            let mut alias = value["members"][1].clone();
            alias["path"] = json!(format!("{root}/data"));
            value["members"].as_array_mut().unwrap().push(alias);
            value["expanded_bytes"] = json!(6);
        }
        assert!(parse(&request, &value).is_err(), "{path}");
    }
    let mut value = base.clone();
    value["members"].as_array_mut().unwrap().remove(0);
    assert!(parse(&request, &value).is_err());
    let mut value = base;
    let duplicate = value["members"][1].clone();
    value["members"].as_array_mut().unwrap().push(duplicate);
    value["expanded_bytes"] = json!(6);
    assert!(parse(&request, &value).is_err());
}

#[test]
fn manifest_rejects_stat_type_hardlinks_digests_and_aggregate_corruptions() {
    let request = request(ArchiveFormat::Zip);
    let base = report(&request);
    for (pointer, replacement) in [
        ("/members/1/stat/nlink", json!(2)),
        ("/members/1/stat/mode", json!(0o120600)),
        ("/members/1/stat/ino", json!(0)),
        ("/members/1/stat/size", json!(4)),
        ("/members/1/stat/mtime_ns", json!(1.5)),
        ("/members/1/stat/size", json!(-1)),
        ("/members/1/sha256", json!("A".repeat(64))),
        ("/members/1/sha256", Value::Null),
        ("/members/0/sha256", json!("0".repeat(64))),
        ("/members/0/bytes", json!(1)),
        ("/members/0/kind", json!("file")),
        ("/expanded_bytes", json!(4)),
        ("/expanded_bytes", json!(MAX_ARCHIVE_EXPANDED_BYTES + 1)),
        ("/root/mode", json!(0o100600)),
    ] {
        let mut value = base.clone();
        *value.pointer_mut(pointer).unwrap() = replacement;
        assert!(parse(&request, &value).is_err(), "{pointer}");
    }
}

#[test]
fn archive_verification_checks_exact_paths_kinds_sizes_and_full_digests() {
    for format in [ArchiveFormat::Zip, ArchiveFormat::TarGz] {
        let request = request(format);
        let manifest = parse(&request, &report(&request)).unwrap();
        let root = manifest.members()[0].path();
        let directory = || ArchiveMember::new_directory(root).unwrap();
        let file = |name: &str, bytes: &[u8]| {
            ArchiveMember::new_file(format!("{root}/{name}"), bytes.to_vec()).unwrap()
        };
        let empty = || ArchiveMember::new_directory(format!("{root}/empty")).unwrap();
        let encode = |members| ArchiveBundle::new(members).unwrap().encode(format).unwrap();
        let encoded = encode(vec![directory(), file("数据", b"a\0b"), empty()]);
        manifest.verify_archive(&encoded).unwrap();
        for members in [
            vec![directory(), file("数据", b"a\0c"), empty()],
            vec![directory(), file("数据", b"a"), empty()],
            vec![directory(), file("changed", b"a\0b"), empty()],
            vec![directory(), file("数据", b"a\0b")],
            vec![directory(), file("数据", b"a\0b"), file("empty", b"")],
            vec![
                directory(),
                file("数据", b"a\0b"),
                empty(),
                file("extra", b""),
            ],
        ] {
            assert!(manifest.verify_archive(&encode(members)).is_err());
        }
        assert!(
            manifest
                .verify_archive(&encoded[..encoded.len() - 1])
                .is_err()
        );
        let mut corrupt = encoded;
        corrupt[0] ^= 0xff;
        assert!(manifest.verify_archive(&corrupt).is_err());
    }
}

#[test]
fn receipt_rejects_ambiguous_malformed_and_out_of_bound_results() {
    let digest = "a".repeat(64);
    let value = json!({"version":1, "archive_bytes":123, "sha256":digest});
    let receipt = ServerArchiveReceipt::parse(&serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(receipt.archive_bytes(), 123);
    assert_eq!(receipt.sha256(), digest);
    for bad in [
        json!({"version":2, "archive_bytes":123, "sha256":digest}),
        json!({"version":1, "archive_bytes":0, "sha256":digest}),
        json!({"version":1, "archive_bytes":64 * 1024 * 1024 + 1, "sha256":digest}),
        json!({"version":1, "archive_bytes":1, "sha256":"F".repeat(64)}),
        json!({"version":1, "archive_bytes":1, "sha256":digest, "unknown":0}),
        json!({"version":1, "archive_bytes":true, "sha256":digest}),
    ] {
        assert!(ServerArchiveReceipt::parse(&serde_json::to_vec(&bad).unwrap()).is_err());
    }
    assert!(
        ServerArchiveReceipt::parse(
            b"{\"version\":1,\"version\":1,\"archive_bytes\":1,\"sha256\":\"a\"}"
        )
        .is_err()
    );
    assert!(
        ServerArchiveReceipt::parse(&vec![b' '; MAX_SERVER_ARCHIVE_RESPONSE_BYTES + 1]).is_err()
    );
}

#[cfg(unix)]
#[test]
fn embedded_commands_pack_real_sources_and_existing_codec_verifies_both_formats() {
    use std::io::{Read, Seek, Write};
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    fn execute(command: &str, input: &[u8]) -> Vec<u8> {
        // Regular temporary streams avoid pipe deadlocks and are isolated from
        // customer data. Each helper and the parent process have finite waits.
        let mut stdin = tempfile::tempfile().unwrap();
        stdin.write_all(input).unwrap();
        stdin.rewind().unwrap();
        let mut stdout = tempfile::tempfile().unwrap();
        let mut stderr = tempfile::tempfile().unwrap();
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(format!("exec {command}"))
            .stdin(Stdio::from(stdin))
            .stdout(Stdio::from(stdout.try_clone().unwrap()))
            .stderr(Stdio::from(stderr.try_clone().unwrap()))
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("server helper exceeded bounded subprocess deadline");
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        let mut output = Vec::new();
        stdout.rewind().unwrap();
        stdout
            .take((MAX_SERVER_ARCHIVE_RESPONSE_BYTES + 1) as u64)
            .read_to_end(&mut output)
            .unwrap();
        let mut errors = String::new();
        stderr.rewind().unwrap();
        stderr.take(4096).read_to_string(&mut errors).unwrap();
        assert!(status.success(), "{errors}");
        assert!(errors.is_empty(), "{errors}");
        output
    }

    for format in [ArchiveFormat::Zip, ArchiveFormat::TarGz] {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().canonicalize().unwrap();
        let source = root.join("资料's;$(false)");
        std::fs::create_dir(&source).unwrap();
        std::fs::create_dir(source.join("empty")).unwrap();
        std::fs::write(source.join("数据"), b"a\0b").unwrap();
        // The GNU long-name fallback must not truncate a multibyte scalar or
        // emit a PAX question-mark placeholder in its ordinary header.
        std::fs::write(source.join("界".repeat(65)), b"long").unwrap();
        let destination = root.join(match format {
            ArchiveFormat::Zip => "result.zip",
            ArchiveFormat::TarGz => "result.tar.gz",
        });
        let request = ServerArchiveRequest::new(
            vec![source.to_str().unwrap().to_owned()],
            destination.to_str().unwrap().to_owned(),
            format,
        )
        .unwrap();
        let manifest =
            ServerArchiveManifest::parse(&request, &execute(&request.inspection_command(), b""))
                .unwrap();
        let stage = root.join(format!(
            ".keelshell-server-archive-{}",
            uuid::Uuid::new_v4()
        ));
        let command = request.packing_command(stage.to_str().unwrap()).unwrap();
        let receipt =
            ServerArchiveReceipt::parse(&execute(&command, &manifest.packing_input())).unwrap();
        let encoded = std::fs::read(&destination).unwrap();
        assert_eq!(receipt.archive_bytes(), encoded.len() as u64);
        assert_eq!(receipt.sha256(), digest_hex(&encoded));
        manifest.verify_archive(&encoded).unwrap();
        assert!(!stage.exists());
    }
}

// This standalone suite can also be extracted and run by the implementation
// worker without using Cargo or touching the parent's shared target directory.
#[cfg(unix)]
const PYTHON_SUITE: &str = r####"
import base64
import contextlib
import errno
import hashlib
import io
import json
import os
import pathlib
import stat
import subprocess
import sys
import tarfile
import tempfile
import unittest
import uuid
import zipfile

HELPER = sys.argv[1]
sys.argv = [sys.argv[0]]
ERROR = b"KeelShell server archive rejected\n"
PREFIX = ".keelshell-server-archive-"


class HelperTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="keelshell-server-")
        self.addCleanup(self.temporary.cleanup)
        self.root = pathlib.Path(self.temporary.name).resolve()
        self.source = self.root / "资料's;$(false)"
        self.source.mkdir()
        (self.source / "empty").mkdir()
        (self.source / "binary").write_bytes(b"alpha\0\xff")
        self.stage = self.root / (PREFIX + str(uuid.uuid4()))

    def request(self, fmt="zip"):
        return {"version": 1, "sources": [str(self.source)],
                "destination": str(self.root / ("result." + fmt)), "format": fmt}

    def run_helper(self, mode="inspect", request=None, data=b"", stage=None,
                   helper=HELPER, args=None):
        request = request or self.request()
        encoded = base64.b64encode(json.dumps(request, ensure_ascii=False).encode()).decode()
        arguments = [sys.executable, "-c", helper, mode, encoded]
        if mode == "pack":
            arguments.append(str(stage or self.stage))
        if args is not None:
            arguments = [sys.executable, "-c", helper] + args
        return subprocess.run(arguments, input=data, capture_output=True, timeout=10)

    def inspect(self, request=None):
        result = self.run_helper(request=request)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stderr, b"")
        self.assertLessEqual(len(result.stdout), 1024 * 1024)
        return result.stdout

    def rejected(self, result):
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stdout, b"")
        self.assertEqual(result.stderr, ERROR)

    def test_zip_and_gnu_tar_pack_binary_unicode_quotes_and_empty_directories(self):
        long_name = "界" * 65
        (self.source / long_name).write_bytes(b"long name")
        for fmt in ("zip", "tar.gz"):
            with self.subTest(fmt=fmt):
                request = self.request(fmt)
                inspected = self.inspect(request)
                expected = json.loads(inspected)
                before = {p.name: p.read_bytes() for p in self.source.iterdir() if p.is_file()}
                result = self.run_helper("pack", request, inspected)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stderr, b"")
                receipt = json.loads(result.stdout)
                archive = pathlib.Path(request["destination"])
                encoded = archive.read_bytes()
                self.assertEqual(receipt, {"version": 1, "archive_bytes": len(encoded),
                                           "sha256": hashlib.sha256(encoded).hexdigest()})
                self.assertEqual(archive.stat().st_mode & 0o7777, 0o600)
                self.assertEqual(archive.stat().st_nlink, 1)
                self.assertFalse(self.stage.exists())
                if fmt == "zip":
                    with zipfile.ZipFile(io.BytesIO(encoded)) as reader:
                        actual = {}
                        for info in reader.infolist():
                            self.assertEqual(info.create_system, 3)
                            self.assertEqual(info.extra, b"")
                            self.assertEqual(info.date_time, (1980, 1, 1, 0, 0, 0))
                            self.assertLess(info.extract_version, 45)
                            self.assertIn(info.compress_type, (zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED))
                            actual[info.filename.rstrip("/")] = (info.is_dir(), reader.read(info))
                else:
                    with tarfile.open(fileobj=io.BytesIO(encoded), mode="r:gz") as reader:
                        actual = {}
                        for info in reader:
                            self.assertEqual(info.pax_headers, {})
                            self.assertEqual((info.uid, info.gid, info.mtime), (0, 0, 0))
                            self.assertEqual(info.mode, 0o700 if info.isdir() else 0o600)
                            actual[info.name.rstrip("/")] = (info.isdir(), b"" if info.isdir()
                                                            else reader.extractfile(info).read())
                self.assertEqual(set(actual), {m["path"] for m in expected["members"]})
                for member in expected["members"]:
                    directory, payload = actual[member["path"]]
                    self.assertEqual(directory, member["kind"] == "directory")
                    self.assertEqual(len(payload), member["bytes"])
                    self.assertEqual(None if directory else hashlib.sha256(payload).hexdigest(),
                                     member["sha256"])
                self.assertEqual(before, {p.name: p.read_bytes() for p in self.source.iterdir() if p.is_file()})
                archive.unlink()

    def test_inspect_has_no_writes_and_import_has_no_execution(self):
        before = {str(p): (p.stat().st_mode, p.stat().st_mtime_ns, p.stat().st_ctime_ns)
                  for p in [self.root, self.source] + list(self.source.iterdir())}
        self.inspect()
        after = {str(p): (p.stat().st_mode, p.stat().st_mtime_ns, p.stat().st_ctime_ns)
                 for p in [self.root, self.source] + list(self.source.iterdir())}
        self.assertEqual(before, after)
        result = subprocess.run([sys.executable, "-c",
                                 "exec(compile(" + repr(HELPER) + ",'helper','exec'),{'__name__':'library'})"],
                                capture_output=True, timeout=10)
        self.assertEqual((result.returncode, result.stdout, result.stderr), (0, b"", b""))

    @unittest.skipUnless(sys.platform == "darwin", "requires real Darwin inherited ACLs")
    def test_darwin_inherited_acl_is_removed_before_source_bytes_and_publication(self):
        subprocess.run(["/bin/chmod", "+a",
                        "everyone allow read,execute,file_inherit,directory_inherit",
                        str(self.root)], check=True, capture_output=True, timeout=5)
        listing = lambda path: subprocess.run(
            ["/bin/ls", "-led", str(path)], check=True, capture_output=True, timeout=5).stdout
        parent_acl = listing(self.root).splitlines()[1:]
        source_acl = listing(self.source).splitlines()[1:]
        self.assertTrue(parent_acl, "the fixture must actually carry an inherited ACL")
        audit = self.root / "before-source-write.json"
        observation = (
            'import subprocess\n'
            '                audit = subprocess.run(["/bin/ls", "-led", sys.argv[3], '
            'os.path.join(sys.argv[3], PAYLOAD)], capture_output=True, check=True, timeout=5)\n'
            '                with open(' + repr(str(audit)) + ', "w") as stream:\n'
            '                    json.dump({"listing": audit.stdout.decode(), '
            '"payload_bytes": os.fstat(raw.fileno()).st_size}, stream)\n'
            '                output = BoundedWriter(raw)')
        helper = HELPER.replace("output = BoundedWriter(raw)", observation)
        self.assertNotEqual(helper, HELPER, "the before-write observation must be installed")
        for fmt in ("zip", "tar.gz"):
            with self.subTest(fmt=fmt):
                request = self.request(fmt)
                result = self.run_helper("pack", request, self.inspect(request), helper=helper)
                self.assertEqual(result.returncode, 0, result.stderr)
                observed = json.loads(audit.read_text())
                self.assertEqual(observed["payload_bytes"], 0)
                self.assertFalse(any(line.lstrip().startswith("0:")
                                     for line in observed["listing"].splitlines()),
                                 observed["listing"])
                archive = pathlib.Path(request["destination"])
                self.assertEqual(listing(archive).splitlines()[1:], [])
                self.assertEqual(listing(self.root).splitlines()[1:], parent_acl)
                self.assertEqual(listing(self.source).splitlines()[1:], source_acl)
                self.assertFalse(self.stage.exists())
                archive.unlink()

    @unittest.skipUnless(sys.platform == "darwin", "requires actual Darwin ACL API errors")
    def test_darwin_acl_set_or_inspection_failure_retains_empty_stage_and_no_archive(self):
        for operation in ("set", "get"):
            with self.subTest(operation=operation):
                if operation == "set":
                    old = "library.acl_set_fd_np(descriptor, empty, 0x100)"
                    new = "library.acl_set_fd_np(-1, empty, 0x100)"
                else:
                    old = "library.acl_get_fd_np(descriptor, 0x100)"
                    new = "library.acl_get_fd_np(-1, 0x100)"
                helper = HELPER.replace(old, new)
                self.assertNotEqual(helper, HELPER, "must inject the actual fd API failure")
                self.rejected(self.run_helper("pack", data=self.inspect(), helper=helper))
                self.assertFalse((self.root / "result.zip").exists())
                self.assertTrue(self.stage.is_dir())
                self.assertEqual(list(self.stage.iterdir()), [])
                self.stage.rmdir()

    @unittest.skipUnless(sys.platform == "darwin", "requires actual Darwin ACL changes")
    def test_darwin_acl_added_during_packing_blocks_receipt_and_keeps_unknown_objects(self):
        for target in ("stage", "payload"):
            with self.subTest(target=target):
                path = self.stage if target == "stage" else self.stage / "payload"
                injection = ('codec.close()\n'
                             '                    import subprocess\n'
                             '                    subprocess.run(["/bin/chmod", "+a", '
                             '"everyone allow read,execute", ' + repr(str(path)) +
                             '], check=True, capture_output=True, timeout=5)')
                helper = HELPER.replace("codec.close()", injection)
                self.assertNotEqual(helper, HELPER)
                self.rejected(self.run_helper("pack", data=self.inspect(), helper=helper))
                self.assertFalse((self.root / "result.zip").exists())
                self.assertTrue((self.stage / "payload").is_file())
                self.assertGreater((self.stage / "payload").stat().st_size, 0)
                (self.stage / "payload").unlink()
                self.stage.rmdir()

    def test_multiple_sources_and_empty_regular_file(self):
        extra = self.root / "extra"
        extra.write_bytes(b"")
        request = self.request()
        request["sources"].append(str(extra))
        inspected = self.inspect(request)
        result = self.run_helper("pack", request, inspected)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_empty_directory_as_only_source(self):
        request = self.request()
        request["sources"] = [str(self.source / "empty")]
        request["destination"] = str(self.source / "only.zip")
        inspected = self.inspect(request)
        stage = self.source / (PREFIX + str(uuid.uuid4()))
        result = self.run_helper("pack", request, inspected, stage)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_request_path_attacks_aliases_parent_and_suffix(self):
        valid = self.request()
        attacks = [
            {"sources": []}, {"sources": valid["sources"] * 33},
            {"sources": [str(self.root / "missing") + "/../x"]},
            {"sources": [str(self.root) + "//x"]},
            {"sources": [str(self.root / "CON.txt")]},
            {"sources": [str(self.root / "a\u202eb")]},
            {"sources": [str(self.source), "/different/parent/x"]},
            {"sources": [str(self.source), str(self.source)]},
            {"sources": [str(self.root / "Straße"), str(self.root / "STRASSE")]},
            {"sources": [str(self.root / "RESULT.ZIP")]},
            {"destination": str(self.root / "wrong.tar.gz")},
            {"destination": str(self.root / "x") + "\n.zip"},
            {"version": True}, {"format": "rar"}, {"unknown": 1},
        ]
        for fields in attacks:
            with self.subTest(fields=fields):
                self.rejected(self.run_helper(request=dict(valid, **fields)))
                self.assertFalse(self.stage.exists())

    def test_member_aliases_and_nonportable_names(self):
        for first, second in [("Straße", "STRASSE"), ("Σ", "ς"), ("A", "a")]:
            with self.subTest(first=first):
                a, b = self.source / first, self.source / second
                a.write_bytes(b"one")
                # Case-insensitive filesystems cannot represent this attack.
                if not b.exists():
                    b.write_bytes(b"two")
                    self.rejected(self.run_helper())
                    b.unlink()
                a.unlink()
        for name in ("CON.txt", "control\nname", "format\u200bname", "trailing.", "a\\b"):
            with self.subTest(name=name):
                path = self.source / name
                path.write_bytes(b"x")
                self.rejected(self.run_helper())
                path.unlink()

    def test_symlink_member_root_and_ancestor_rejected(self):
        target = self.root / "target"
        target.write_bytes(b"outside source")
        link = self.source / "link"
        link.symlink_to(target)
        self.rejected(self.run_helper())
        link.unlink()
        request = self.request()
        request["sources"] = [str(link)]
        link.symlink_to(self.source, target_is_directory=True)
        self.rejected(self.run_helper(request=request))
        link.unlink()
        alias = self.root / "alias"
        alias.symlink_to(self.source, target_is_directory=True)
        request["sources"] = [str(alias / "binary")]
        request["destination"] = str(alias / "result.zip")
        self.rejected(self.run_helper(request=request))
        self.assertEqual(target.read_bytes(), b"outside source")

    def test_hardlinks_and_special_files_rejected(self):
        link = self.source / "linked"
        os.link(self.source / "binary", link)
        self.rejected(self.run_helper())
        link.unlink()
        fifo = self.source / "fifo"
        os.mkfifo(fifo, 0o600)
        self.rejected(self.run_helper())

    def test_non_utf8_directory_entry_rejected(self):
        name = os.fsencode(self.source) + b"/invalid-\xff"
        try:
            descriptor = os.open(name, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        except OSError as error:
            if error.errno == errno.EILSEQ:
                self.skipTest("host filesystem itself rejects non-UTF8 names")
            raise
        os.close(descriptor)
        self.rejected(self.run_helper())

    def test_source_content_drift_even_at_same_size_and_restored_mtime(self):
        inspected = self.inspect()
        path = self.source / "binary"
        before = path.stat()
        path.write_bytes(b"bravo\0\xff")
        os.utime(path, ns=(before.st_atime_ns, before.st_mtime_ns))
        self.rejected(self.run_helper("pack", data=inspected))
        self.assertFalse(self.stage.exists())
        self.assertFalse((self.root / "result.zip").exists())

    def test_source_inventory_add_remove_and_replacement_drift(self):
        for change in ("add", "remove", "replace", "mode"):
            with self.subTest(change=change):
                inspected = self.inspect()
                path = self.source / "binary"
                data = path.read_bytes()
                if change == "add":
                    (self.source / "new").write_bytes(b"new")
                elif change == "remove":
                    path.unlink()
                elif change == "replace":
                    path.unlink()
                    path.write_bytes(data)
                else:
                    path.chmod(0o400)
                self.rejected(self.run_helper("pack", data=inspected))
                self.assertFalse(self.stage.exists())
                if change == "add":
                    (self.source / "new").unlink()
                if path.exists():
                    path.chmod(0o600)
                path.write_bytes(data)

    def test_existing_final_and_stage_are_preserved(self):
        inspected = self.inspect()
        final = self.root / "result.zip"
        final.write_bytes(b"existing final")
        self.rejected(self.run_helper("pack", data=inspected))
        self.assertEqual(final.read_bytes(), b"existing final")
        self.rejected(self.run_helper())
        final.unlink()
        for kind in ("directory", "file", "symlink"):
            with self.subTest(kind=kind):
                inspected = self.inspect()
                if kind == "directory":
                    self.stage.mkdir()
                    (self.stage / "unknown").write_bytes(b"preserve")
                elif kind == "file":
                    self.stage.write_bytes(b"preserve")
                else:
                    self.stage.symlink_to(self.source, target_is_directory=True)
                self.rejected(self.run_helper("pack", data=inspected))
                self.assertTrue(self.stage.exists())
                if kind == "directory":
                    self.assertEqual((self.stage / "unknown").read_bytes(), b"preserve")
                    (self.stage / "unknown").unlink()
                    self.stage.rmdir()
                else:
                    self.stage.unlink()

    def test_malicious_stdin_schema_binding_digests_and_path_attacks(self):
        inspected = self.inspect()
        original = json.loads(inspected)
        mutations = [
            lambda m: m.update(version=2),
            lambda m: m.update(unknown=0),
            lambda m: m["request"].update(version=True),
            lambda m: m["request"].update(destination=str(self.root / "elsewhere.zip")),
            lambda m: m["root"].update(unknown=1),
            lambda m: m["members"][0].update(bytes=1),
            lambda m: m["members"][0].update(sha256="0" * 64),
            lambda m: m["members"][1].update(path="../escape"),
            lambda m: m["members"][1].update(sha256="A" * 64),
            lambda m: m["members"][1].update(bytes=True),
            lambda m: m["members"][1]["stat"].update(nlink=2),
            lambda m: m["members"].pop(0),
            lambda m: m["members"].append(m["members"][0]),
            lambda m: m.update(expanded_bytes=128 * 1024 * 1024 + 1),
        ]
        for mutation in mutations:
            manifest = json.loads(inspected)
            mutation(manifest)
            self.rejected(self.run_helper("pack", data=json.dumps(manifest).encode()))
            self.assertFalse(self.stage.exists())
        raw = inspected.decode()
        for data in [
            b"", b"\xff", b"{}", b"[]", b"null", b" " * (1024 * 1024 + 1),
            raw.replace('"version":1', '"version":1,"version":1', 1).encode(),
            inspected + b"\0", b'{"version":NaN}',
        ]:
            self.rejected(self.run_helper("pack", data=data))
        self.assertEqual(original, json.loads(inspected))

    def test_stage_path_attacks_fail_before_creation(self):
        inspected = self.inspect()
        for name in [
            "unknown", PREFIX + "not-a-uuid", PREFIX + "A" * 32,
            PREFIX + str(uuid.uuid4()) + "/child",
        ]:
            self.rejected(self.run_helper("pack", data=inspected, stage=self.root / name))
        other = self.source / (PREFIX + str(uuid.uuid4()))
        self.rejected(self.run_helper("pack", data=inspected, stage=other))
        self.assertFalse(other.exists())

    def test_member_depth_count_and_expanded_budgets(self):
        too_big = self.source / "too-big"
        with too_big.open("wb") as output:
            output.truncate(128 * 1024 * 1024 + 1)
        self.rejected(self.run_helper())
        too_big.unlink()
        for index in range(2046):
            (self.source / ("n%04d" % index)).touch()
        self.rejected(self.run_helper())
        for path in self.source.glob("n*"):
            path.unlink()
        path = self.source
        for _ in range(32):
            path = path / "d"
            path.mkdir()
        self.rejected(self.run_helper())

    def test_full_member_name_and_selected_path_budgets(self):
        path = self.source
        for _ in range(3):
            path = path / ("p" * 200)
            path.mkdir()
        for index in range(100):
            (path / str(index)).touch()
        self.rejected(self.run_helper())
        request = self.request()
        request["sources"] = [str(self.root / (str(i) + "x" * 3900)) for i in range(13)]
        self.rejected(self.run_helper(request=request))

    def test_missing_capabilities_and_unknown_modes_fail_closed(self):
        helper = HELPER.replace('if __name__ == "__main__":',
                                'os.supports_dir_fd = set()\nif __name__ == "__main__":')
        self.rejected(self.run_helper(helper=helper))
        self.rejected(self.run_helper(args=[]))
        self.rejected(self.run_helper(args=["unknown", "e30="]))
        self.rejected(self.run_helper(args=["inspect", "not!base64"]))

    def test_encoded_limit_is_checked_during_writes_and_retains_only_private_stage(self):
        (self.source / "binary").write_bytes(bytes(range(256)) * 128)
        for fmt in ("zip", "tar.gz"):
            with self.subTest(fmt=fmt):
                request = self.request(fmt)
                inspected = self.inspect(request)
                helper = HELPER.replace("MAX_ENCODED = 64 * 1024 * 1024",
                                        "MAX_ENCODED = 100")
                self.rejected(self.run_helper("pack", request, inspected, helper=helper))
                self.assertFalse(pathlib.Path(request["destination"]).exists())
                self.assertTrue(self.stage.is_dir())
                self.assertEqual(self.stage.stat().st_mode & 0o7777, 0o700)
                payload = self.stage / "payload"
                self.assertLessEqual(payload.stat().st_size, 100)
                self.assertEqual(payload.stat().st_mode & 0o7777, 0o600)
                payload.unlink()
                self.stage.rmdir()

    def test_inventory_and_digest_rechecked_while_streaming_and_before_publication(self):
        for fmt in ("zip", "tar.gz"):
            for when in ("streaming", "final"):
                with self.subTest(fmt=fmt, when=when):
                    request = self.request(fmt)
                    inspected = self.inspect(request)
                    if when == "streaming":
                        old = 'chunk = os.read(descriptor, min(CHUNK, remaining))'
                        new = ('chunk = os.read(descriptor, min(CHUNK, remaining))\n'
                               '                        chunk = b"X" * len(chunk) if self.writer is not None else chunk')
                        helper = HELPER.replace(old, new, 1)
                    else:
                        old = 'scan(request, parent_fd, expected["members"])\n'
                        new = ('changed = os.open(request["sources"][0].rsplit("/", 1)[1], nofollow_flags(True), dir_fd=parent_fd)\n'
                               '                os.mkdir("unexpected", dir_fd=changed)\n'
                               '                os.close(changed)\n'
                               '                scan(request, parent_fd, expected["members"])\n')
                        helper = HELPER.replace(old, new, 1)
                    self.rejected(self.run_helper("pack", request, inspected, helper=helper))
                    self.assertFalse(pathlib.Path(request["destination"]).exists())
                    self.assertTrue(self.stage.is_dir())
                    for path in self.stage.iterdir():
                        path.unlink()
                    self.stage.rmdir()
                    if (self.source / "unexpected").exists():
                        (self.source / "unexpected").rmdir()

    def test_publication_race_preserves_winner_and_no_success_receipt(self):
        inspected = self.inspect()
        old = 'os.link(PAYLOAD, destination, src_dir_fd=stage_fd,'
        new = ('racer = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600, dir_fd=parent_fd)\n'
               '                os.write(racer, b"race winner")\n'
               '                os.close(racer)\n'
               '                os.link(PAYLOAD, destination, src_dir_fd=stage_fd,')
        helper = HELPER.replace(old, new)
        self.rejected(self.run_helper("pack", data=inspected, helper=helper))
        self.assertEqual((self.root / "result.zip").read_bytes(), b"race winner")
        self.assertTrue((self.stage / "payload").exists())

    def test_readback_corruption_and_unknown_stage_objects_block_receipt_and_cleanup(self):
        for failure in ("corrupt", "unknown"):
            with self.subTest(failure=failure):
                inspected = self.inspect()
                if failure == "corrupt":
                    old = 'os.fsync(parent_fd)\n                require(readback(parent_fd, destination, inode, size, 2) == digest)'
                    new = ('os.fsync(parent_fd)\n'
                           '                os.pwrite(raw.fileno(), b"BROKEN", 0)\n'
                           '                require(readback(parent_fd, destination, inode, size, 2) == digest)')
                else:
                    old = 'require(readback(parent_fd, destination, inode, size, 2) == digest)\n'
                    new = ('require(readback(parent_fd, destination, inode, size, 2) == digest)\n'
                           '                extra = os.open("unknown", os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600, dir_fd=stage_fd)\n'
                           '                os.close(extra)\n')
                self.rejected(self.run_helper("pack", data=inspected, helper=HELPER.replace(old, new)))
                self.assertTrue(self.stage.exists())
                self.assertTrue((self.root / "result.zip").exists())
                for path in self.stage.iterdir():
                    path.unlink()
                self.stage.rmdir()
                (self.root / "result.zip").unlink()

    def test_response_budget_rejects_instead_of_truncating_manifest(self):
        helper = HELPER.replace("MAX_RESPONSE = 1024 * 1024", "MAX_RESPONSE = 100")
        self.rejected(self.run_helper(helper=helper))


unittest.main(verbosity=2)
"####;

#[cfg(unix)]
#[test]
fn real_python_helper_adversarial_suite() {
    use std::io::{Read, Seek};
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    let mut stdout = tempfile::tempfile().unwrap();
    let mut stderr = tempfile::tempfile().unwrap();
    let mut child = Command::new("python3")
        .arg("-c")
        .arg(PYTHON_SUITE)
        .arg(include_str!("../src/server_archive.py"))
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout.try_clone().unwrap()))
        .stderr(Stdio::from(stderr.try_clone().unwrap()))
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(90);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("Python archive suite exceeded bounded subprocess deadline");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let mut output = String::new();
    stdout.rewind().unwrap();
    stdout.take(32 * 1024).read_to_string(&mut output).unwrap();
    stderr.rewind().unwrap();
    stderr.take(32 * 1024).read_to_string(&mut output).unwrap();
    assert!(status.success(), "{output}");
}
