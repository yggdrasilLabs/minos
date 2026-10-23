//! The deterministic snapshot: byte identity, the externally computed golden `TreeId`, zeroed ustar headers, a
//! listing by the system `tar`, the four exclusion sources, and the denylist report.

use std::collections::BTreeSet;
use std::error::Error;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};

use minos_core::id::BlobId;
use minos_core::manifest::Manifest;
use minos_source::{CleanPolicy, DenyRule, Snapshot, snapshot};
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn Error>>;

/// The `minos.toml` example in CLAUDE.md, section "Projects", copied byte for byte from `CLAUDE_EXAMPLE` in
/// crates/minos-core/tests/manifest.rs.
const CLAUDE_EXAMPLE: &str = r#"[project]
name = "logparse"
kind = "optimize"            # tune | optimize | explore
program = "program.md"

[surface]
writable = ["src/parse.rs"]
readable = ["src/lib.rs"]

[source]
exclude = ["judge/", "docs/", "target/"]

[environment]
dockerfile = "minos.Dockerfile"
requires = { arch = "arm64" }

[commands]
build = ["cargo", "build", "--release"]
run = ["./target/release/bench", "--out", "/out/result.json"]
check = ["cargo", "check"]

[budget]
build = "2m"
run = "5m"
spend_usd = 40

[judge]
version = 1
command = ["./judge"]

[[judge.gate]]
metric = "identical_output"

[[judge.objective]]
metric = "throughput_mb_s"
goal = "maximize"
min_improvement = "5%"

[policy]
kind = "greedy"              # greedy | pareto | review
width = 4

[[proposer]]
kind = "single_shot"
model = "mid"
batch = 4
"#;

/// One file of the golden project tree.
struct GoldenFile {
    path: &'static str,
    bytes: &'static [u8],
    executable: bool,
    /// The SHA-256 of `bytes`, computed outside the code with `shasum -a 256`.
    sha256: &'static str,
}

/// The five-file golden project tree (04-07-PLAN.md interfaces block), in path byte order.
fn golden_files() -> [GoldenFile; 5] {
    [
        GoldenFile {
            path: "minos.Dockerfile",
            bytes: b"FROM scratch\n",
            executable: false,
            sha256: "bb57c7da220a8753d7bdabac0d3afdb6efa742e4c736c5bc93ab40dfd5e23b9b",
        },
        GoldenFile {
            path: "minos.toml",
            bytes: CLAUDE_EXAMPLE.as_bytes(),
            executable: false,
            sha256: "031035c76f41cd8e1bba0f948845756e373debf3940fee4e0550f5c9adfe0540",
        },
        GoldenFile {
            path: "program.md",
            bytes: b"Make src/parse.rs faster without changing its output.\n",
            executable: false,
            sha256: "82b6adbe4a55bd7adc155a7d2e9457c0a995f50668c5e5b7ec7bc7a8ee47032f",
        },
        GoldenFile {
            path: "src/lib.rs",
            bytes: b"pub mod parse;\n",
            executable: false,
            sha256: "1c7fe1aee3e1c37fa78b0b120b2a125ffd9a84a39afd8f7982bc9138343981e7",
        },
        GoldenFile {
            path: "src/parse.rs",
            bytes: b"pub fn parse(input: &str) -> usize {\n    input.len()\n}\n",
            executable: true,
            sha256: "575559c10c24271808ede734922ae8bbcaa7ae1a1f2f201cad5e762da37ac26b",
        },
    ]
}

/// Writes `bytes` to `dir/rel`, creating parent directories, with mode 0755 when `executable` and 0644 otherwise.
fn write_file(dir: &Path, rel: &str, bytes: &[u8], executable: bool) -> TestResult {
    let path = dir.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, bytes)?;
    let mode = if executable { 0o755 } else { 0o644 };
    fs::set_permissions(&path, fs::Permissions::from_mode(mode))?;
    Ok(())
}

/// Writes the golden project files into `dir`, in the order given.
fn write_project<'a>(dir: &Path, files: impl IntoIterator<Item = &'a GoldenFile>) -> TestResult {
    for file in files {
        write_file(dir, file.path, file.bytes, file.executable)?;
    }
    Ok(())
}

/// A temporary directory holding the golden project.
fn project() -> Result<TempDir, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    write_project(dir.path(), &golden_files())?;
    Ok(dir)
}

/// The example manifest.
fn manifest() -> Result<Manifest, Box<dyn Error>> {
    Ok(Manifest::parse_toml(CLAUDE_EXAMPLE)?)
}

/// Snapshots `dir` with the example manifest. Temporary directories are not repositories, so the clean check is
/// skipped; the denylist, the caps, and the scan still run.
fn pack(dir: &Path) -> Result<Snapshot, Box<dyn Error>> {
    Ok(snapshot(
        dir,
        &manifest()?,
        CleanPolicy::AllowDirty,
        &BTreeSet::new(),
    )?)
}

/// The packed paths, in archive order.
fn listed(tarball: &[u8]) -> Result<Vec<String>, Box<dyn Error>> {
    let mut archive = tar::Archive::new(tarball);
    let mut paths = Vec::new();
    for entry in archive.entries()? {
        let entry = entry?;
        paths.push(entry.path()?.to_string_lossy().into_owned());
    }
    Ok(paths)
}

#[test]
fn same_tree_twice_is_byte_identical() -> TestResult {
    let dir = project()?;
    let first = pack(dir.path())?;
    let second = pack(dir.path())?;
    assert_eq!(first.tarball(), second.tarball());
    assert_eq!(first.tree(), second.tree());
    Ok(())
}

#[test]
fn creation_order_does_not_matter() -> TestResult {
    let files = golden_files();
    let forward = tempfile::tempdir()?;
    write_project(forward.path(), &files)?;
    let backward = tempfile::tempdir()?;
    write_project(backward.path(), files.iter().rev())?;
    let a = pack(forward.path())?;
    let b = pack(backward.path())?;
    assert_eq!(a.tarball(), b.tarball());
    assert_eq!(a.tree(), b.tree());
    Ok(())
}

#[test]
fn golden_project_tree() -> TestResult {
    let files = golden_files();
    // Each file's content first, so a typo in a file fails here and not as an opaque tarball mismatch.
    for file in &files {
        assert_eq!(
            BlobId::of(file.bytes).to_hex(),
            file.sha256,
            "{}",
            file.path
        );
    }
    let dir = project()?;
    let snap = pack(dir.path())?;

    // Five 512-byte headers, each file's bytes rounded up to 512, and the 1024-byte end marker.
    let expected_len: usize = files
        .iter()
        .map(|file| 512 + file.bytes.len().div_ceil(512) * 512)
        .sum::<usize>()
        + 1024;
    assert_eq!(expected_len, 6656);
    assert_eq!(snap.tarball().len(), 6656);

    // Computed outside the code on 2026-10-14 by a Python writer built from the POSIX ustar layout, and by a
    // scratch tar 0.4.46 program using the packer's header recipe; the two agree byte for byte. Recipe: for each
    // file in byte order of its path, a 512-byte header that is zero except for the path at offset 0; mode at
    // 100, uid at 108, and gid at 116 as seven zero-padded octal digits and a NUL; size at 124 and mtime 0 at 136
    // as eleven zero-padded octal digits and a NUL; typeflag `0` at 156; `ustar` and a NUL at 257; `00` at 263;
    // and the checksum at 148 as seven zero-padded octal digits and a NUL, the sum of all 512 header bytes with
    // the checksum field counted as eight spaces (tar 0.4.46 `octal_into`; GNU tar writes six digits, a NUL, and
    // a space, so a system tar cannot reproduce these bytes). Each header is followed by the file bytes
    // zero-padded to a multiple of 512, the archive ends with 1024 zero bytes, and the whole is hashed with
    // `shasum -a 256`. Never replace this literal with the packer's own output.
    assert_eq!(
        snap.tree().to_hex(),
        "bb2319a34a6a8a85974b4e7ce637428492c8f9b33f8b189747fca11462d9c0f0"
    );
    assert_eq!(snap.report().files(), 5);
    let content: u64 = files.iter().map(|file| file.bytes.len() as u64).sum();
    assert_eq!(snap.report().content_bytes(), content);
    assert!(snap.report().denied().is_empty());
    Ok(())
}

#[test]
fn headers_are_zeroed() -> TestResult {
    let dir = project()?;
    let snap = pack(dir.path())?;
    let mut archive = tar::Archive::new(snap.tarball());
    let mut seen = 0_usize;
    for entry in archive.entries()? {
        let entry = entry?;
        let header = entry.header();
        let path = entry.path()?.to_string_lossy().into_owned();
        assert_eq!(header.mtime()?, 0, "{path}");
        assert_eq!(header.uid()?, 0, "{path}");
        assert_eq!(header.gid()?, 0, "{path}");
        assert_eq!(header.username_bytes(), Some(&b""[..]), "{path}");
        assert_eq!(header.groupname_bytes(), Some(&b""[..]), "{path}");
        let bytes = header.as_bytes();
        assert_eq!(bytes.get(257..263), Some(&b"ustar\0"[..]), "{path}");
        assert_eq!(bytes.get(263..265), Some(&b"00"[..]), "{path}");
        assert_eq!(bytes.get(156), Some(&b'0'), "{path}");
        let mode = if path == "src/parse.rs" { 0o755 } else { 0o644 };
        assert_eq!(header.mode()?, mode, "{path}");
        seen += 1;
    }
    assert_eq!(seen, 5);
    Ok(())
}

#[test]
fn system_tar_lists_the_tarball() -> TestResult {
    let dir = project()?;
    let snap = pack(dir.path())?;
    let out = tempfile::tempdir()?;
    let file = out.path().join("tree.tar");
    fs::write(&file, snap.tarball())?;
    let output = Command::new("/usr/bin/tar")
        .arg("-tf")
        .arg(&file)
        .env_clear()
        .stdin(Stdio::null())
        .output()?;
    assert!(output.status.success(), "tar -tf failed: {}", output.status);
    let text = String::from_utf8(output.stdout)?;
    let names: Vec<&str> = text.lines().collect();
    assert_eq!(
        names,
        [
            "minos.Dockerfile",
            "minos.toml",
            "program.md",
            "src/lib.rs",
            "src/parse.rs"
        ]
    );
    Ok(())
}

#[test]
fn tree_id_is_the_blob_address() -> TestResult {
    let dir = project()?;
    let snap = pack(dir.path())?;
    assert_eq!(snap.tree().to_blob_id(), BlobId::of(snap.tarball()));
    let tree = snap.tree();
    let tarball = snap.into_tarball();
    assert_eq!(tree.to_blob_id(), BlobId::of(&tarball));
    Ok(())
}

#[test]
fn exclusions() -> TestResult {
    let parent = tempfile::tempdir()?;
    // A .gitignore above the project root must have no effect.
    write_file(parent.path(), ".gitignore", b"notes.txt\n", false)?;
    let dir = parent.path().join("project");
    write_project(&dir, &golden_files())?;
    write_file(
        &dir,
        "notes.txt",
        b"kept: only a parent .gitignore names it\n",
        false,
    )?;
    // A project .ignore file must have no effect.
    write_file(&dir, ".ignore", b"extra.txt\n", false)?;
    write_file(&dir, "extra.txt", b"kept: only .ignore names it\n", false)?;
    write_file(&dir, ".gitignore", b"build.log\n", false)?;
    write_file(&dir, "build.log", b"dropped by .gitignore\n", false)?;
    write_file(&dir, "sub/.gitignore", b"local.tmp\n", false)?;
    write_file(
        &dir,
        "sub/local.tmp",
        b"dropped by a nested .gitignore\n",
        false,
    )?;
    write_file(&dir, "sub/kept.txt", b"kept\n", false)?;
    write_file(&dir, ".minosignore", b"scratch.txt\n", false)?;
    write_file(&dir, "scratch.txt", b"dropped by .minosignore\n", false)?;
    write_file(
        &dir,
        "docs/guide.md",
        b"dropped by [source] exclude\n",
        false,
    )?;

    let snap = pack(&dir)?;
    let paths = listed(snap.tarball())?;
    assert_eq!(
        paths,
        [
            ".gitignore",
            ".ignore",
            ".minosignore",
            "extra.txt",
            "minos.Dockerfile",
            "minos.toml",
            "notes.txt",
            "program.md",
            "src/lib.rs",
            "src/parse.rs",
            "sub/.gitignore",
            "sub/kept.txt",
        ]
    );
    // An exclude the user wrote is not a denylist drop.
    assert!(snap.report().denied().is_empty());
    Ok(())
}

#[test]
fn denylist_drops_are_reported() -> TestResult {
    let dir = project()?;
    for (rel, bytes) in [
        (".env", &b"MODE=dev\n"[..]),
        (".env.local", b"MODE=local\n"),
        ("cert.pem", b"not a certificate\n"),
        ("id_ed25519", b"not a key\n"),
        ("minos.lock", b"lock\n"),
        (".git/config", b"[core]\n"),
        (".minos/ledger.jsonl", b"\n"),
        (".claude/settings.json", b"{}\n"),
        (".aws/credentials", b"[default]\n"),
        (".ssh/config", b"Host *\n"),
        ("judge", b"#!/bin/sh\n"),
    ] {
        write_file(dir.path(), rel, bytes, false)?;
    }
    let snap = pack(dir.path())?;
    assert_eq!(
        listed(snap.tarball())?,
        [
            "minos.Dockerfile",
            "minos.toml",
            "program.md",
            "src/lib.rs",
            "src/parse.rs"
        ]
    );
    let denied: Vec<(&str, DenyRule)> = snap
        .report()
        .denied()
        .iter()
        .map(|drop| (drop.path().as_str(), drop.rule()))
        .collect();
    assert_eq!(
        denied,
        [
            (".aws", DenyRule::AwsDir),
            (".claude", DenyRule::ClaudeDir),
            (".env", DenyRule::EnvFile),
            (".env.local", DenyRule::EnvFile),
            (".git", DenyRule::GitDir),
            (".minos", DenyRule::MinosDir),
            (".ssh", DenyRule::SshDir),
            ("cert.pem", DenyRule::PemFile),
            ("id_ed25519", DenyRule::SshKey),
            ("judge", DenyRule::JudgePath),
            ("minos.lock", DenyRule::MinosLock),
        ]
    );
    Ok(())
}
