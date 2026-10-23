//! Every refusal of the snapshot: offenders listed together, a sanitized hostile name, the per-file, file-count,
//! and total caps, the `.minosignore` negation, a missing surface path, a planted secret, and the clean check.
//!
//! No complete secret-shaped value appears in this file. The AWS-shaped key is joined at run time from pieces that
//! do not match on their own (04-RESEARCH Pitfall 5), so the self-scan in tests/scan.rs still passes.

use std::collections::BTreeSet;
use std::error::Error;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};

use minos_core::manifest::Manifest;
use minos_core::path::RelPath;
use minos_source::{CleanPolicy, Dirty, Offender, OffenderKind, Snapshot, SnapshotError, snapshot};
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

/// The five project files: path, bytes, and whether the file is executable.
const PROJECT: [(&str, &[u8], bool); 5] = [
    ("minos.Dockerfile", b"FROM scratch\n", false),
    ("minos.toml", CLAUDE_EXAMPLE.as_bytes(), false),
    (
        "program.md",
        b"Make src/parse.rs faster without changing its output.\n",
        false,
    ),
    ("src/lib.rs", b"pub mod parse;\n", false),
    (
        "src/parse.rs",
        b"pub fn parse(input: &str) -> usize {\n    input.len()\n}\n",
        true,
    ),
];

/// The per-file cap, 16 MiB.
const FILE_CAP: u64 = 16_777_216;

/// The total content cap, 64 MiB.
const TOTAL_CAP: u64 = 67_108_864;

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

/// A temporary directory holding the five-file project.
fn project() -> Result<TempDir, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    for (rel, bytes, executable) in PROJECT {
        write_file(dir.path(), rel, bytes, executable)?;
    }
    Ok(dir)
}

/// Snapshots `dir` with the example manifest under `policy`.
fn pack(
    dir: &Path,
    policy: CleanPolicy,
) -> Result<Result<Snapshot, SnapshotError>, Box<dyn Error>> {
    let manifest = Manifest::parse_toml(CLAUDE_EXAMPLE)?;
    Ok(snapshot(dir, &manifest, policy, &BTreeSet::new()))
}

/// `n` characters cycling through `[A-Z]`.
fn upper(n: usize) -> String {
    ('A'..='Z').cycle().take(n).collect()
}

/// An AWS-shaped access key ID, joined at run time.
fn aws_key() -> String {
    format!("{}{}{}", "AK", "IA", upper(16))
}

/// Runs the system git in `dir` with a cleared environment and a fixed test identity.
fn git(dir: &Path, args: &[&str]) -> TestResult {
    let status = Command::new("/usr/bin/git")
        .args([
            "-c",
            "user.name=minos-test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .current_dir(dir)
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("git {args:?} failed: {status}").into())
    }
}

#[test]
fn offenders_are_listed_together() -> TestResult {
    let dir = project()?;
    let root = dir.path();
    write_file(root, "bad name.txt", b"a space\n", false)?;
    std::os::unix::fs::symlink("program.md", root.join("link"))?;
    let status = Command::new("/usr/bin/mkfifo")
        .arg(root.join("pipe"))
        .env_clear()
        .status()?;
    assert!(status.success(), "mkfifo failed: {status}");
    write_file(root, "a.txt", b"linked\n", false)?;
    fs::hard_link(root.join("a.txt"), root.join("b.txt"))?;
    let long = format!("{}.txt", "x".repeat(97));
    assert_eq!(long.len(), 101);
    write_file(root, &long, b"long\n", false)?;

    let Err(SnapshotError::Offenders { shown, more }) = pack(root, CleanPolicy::AllowDirty)? else {
        return Err("expected Offenders".into());
    };
    assert_eq!(more, 0);
    let listed: Vec<(&str, OffenderKind)> = shown
        .iter()
        .map(|offender| (offender.path().as_str(), offender.kind()))
        .collect();
    assert_eq!(
        listed,
        [
            ("a.txt", OffenderKind::Hardlink),
            ("b.txt", OffenderKind::Hardlink),
            ("bad name.txt", OffenderKind::NotInGrammar),
            ("link", OffenderKind::Symlink),
            ("pipe", OffenderKind::Special),
            (long.as_str(), OffenderKind::UstarPath),
        ]
    );
    let kinds: BTreeSet<OffenderKind> = shown.iter().map(Offender::kind).collect();
    assert_eq!(kinds.len(), 5);
    Ok(())
}

#[test]
fn hostile_name_is_sanitized() -> TestResult {
    let dir = project()?;
    write_file(dir.path(), "evil\u{1b}[31mred.txt", b"x\n", false)?;
    let result = pack(dir.path(), CleanPolicy::AllowDirty)?;
    let Err(error @ SnapshotError::Offenders { .. }) = result else {
        return Err("expected Offenders".into());
    };
    if let SnapshotError::Offenders { shown, .. } = &error {
        assert_eq!(shown.len(), 1);
        assert!(
            shown
                .iter()
                .all(|offender| offender.kind() == OffenderKind::NotInGrammar)
        );
    }
    let text = error.to_string();
    assert!(!text.contains('\u{1b}'), "{text:?}");
    assert!(text.contains("evil"), "{text:?}");
    Ok(())
}

#[test]
fn oversize_file_is_named() -> TestResult {
    let dir = project()?;
    let size = usize::try_from(FILE_CAP + 1)?;
    fs::write(dir.path().join("big.bin"), vec![b'a'; size])?;
    let Err(SnapshotError::Offenders { shown, more }) = pack(dir.path(), CleanPolicy::AllowDirty)?
    else {
        return Err("expected Offenders".into());
    };
    assert_eq!(more, 0);
    let listed: Vec<(&str, OffenderKind)> = shown
        .iter()
        .map(|offender| (offender.path().as_str(), offender.kind()))
        .collect();
    assert_eq!(
        listed,
        [("big.bin", OffenderKind::Oversize { bytes: 16_777_217 })]
    );
    Ok(())
}

#[test]
fn too_many_files() -> TestResult {
    let dir = project()?;
    let many = dir.path().join("many");
    fs::create_dir(&many)?;
    for index in 0..10_001 {
        fs::write(many.join(format!("f{index}.txt")), b"a")?;
    }
    let result = pack(dir.path(), CleanPolicy::AllowDirty)?;
    assert!(
        matches!(result, Err(SnapshotError::TooManyFiles { max: 10_000 })),
        "{result:?}"
    );
    Ok(())
}

#[test]
fn total_over_cap_names_largest() -> TestResult {
    let dir = project()?;
    let size: u64 = 15 * 1024 * 1024;
    let data = vec![b'a'; usize::try_from(size)?];
    for index in 0..5 {
        write_file(dir.path(), &format!("data/d{index}.bin"), &data, false)?;
    }
    let Err(SnapshotError::TooLarge {
        bytes,
        max,
        largest,
    }) = pack(dir.path(), CleanPolicy::AllowDirty)?
    else {
        return Err("expected TooLarge".into());
    };
    assert_eq!(max, TOTAL_CAP);
    assert!(bytes > TOTAL_CAP, "{bytes}");
    assert!(bytes >= 5 * size, "{bytes}");
    // Every packable file is listed (ten here, under the 20 shown), largest first.
    assert_eq!(largest.len(), 10);
    assert!(largest.windows(2).all(|pair| match pair {
        [a, b] => a.1 >= b.1,
        _ => false,
    }));
    let top: Vec<(&str, u64)> = largest
        .iter()
        .take(5)
        .map(|(path, len)| (path.as_str(), *len))
        .collect();
    assert_eq!(
        top,
        [
            ("data/d0.bin", size),
            ("data/d1.bin", size),
            ("data/d2.bin", size),
            ("data/d3.bin", size),
            ("data/d4.bin", size),
        ]
    );
    Ok(())
}

#[test]
fn minosignore_negation_is_refused() -> TestResult {
    let dir = project()?;
    write_file(
        dir.path(),
        ".minosignore",
        b"a.log\nb.log\n!keep.log\n",
        false,
    )?;
    let Err(SnapshotError::MinosignoreNegation { file, line }) =
        pack(dir.path(), CleanPolicy::AllowDirty)?
    else {
        return Err("expected MinosignoreNegation".into());
    };
    assert_eq!(file.as_str(), ".minosignore");
    assert_eq!(line, 3);
    Ok(())
}

#[test]
fn nested_minosignore_negation_after_a_bom_is_refused() -> TestResult {
    let dir = project()?;
    // The matcher strips a UTF-8 byte order mark from the first line, so `!` after one is still a negation.
    write_file(
        dir.path(),
        "sub/.minosignore",
        b"\xEF\xBB\xBF!keep.log\n",
        false,
    )?;
    let Err(SnapshotError::MinosignoreNegation { file, line }) =
        pack(dir.path(), CleanPolicy::AllowDirty)?
    else {
        return Err("expected MinosignoreNegation".into());
    };
    assert_eq!(file.as_str(), "sub/.minosignore");
    assert_eq!(line, 1);
    Ok(())
}

#[test]
fn surface_missing_is_refused() -> TestResult {
    let dir = project()?;
    write_file(dir.path(), ".gitignore", b"src/parse.rs\n", false)?;
    let Err(SnapshotError::SurfaceMissing { paths }) = pack(dir.path(), CleanPolicy::AllowDirty)?
    else {
        return Err("expected SurfaceMissing".into());
    };
    let names: Vec<&str> = paths.iter().map(RelPath::as_str).collect();
    assert_eq!(names, ["src/parse.rs"]);
    Ok(())
}

/// The project with the key planted on line 4 of src/lib.rs.
fn planted() -> Result<TempDir, Box<dyn Error>> {
    let dir = project()?;
    let lib = format!(
        "pub mod parse;\n// one\n// two\nconst KEY: &str = \"{}\";\n",
        aws_key()
    );
    write_file(dir.path(), "src/lib.rs", lib.as_bytes(), false)?;
    Ok(dir)
}

#[test]
fn planted_secret_blocks() -> TestResult {
    let dir = planted()?;
    let result = pack(dir.path(), CleanPolicy::AllowDirty)?;
    let Err(error @ SnapshotError::Secrets { .. }) = result else {
        return Err("expected Secrets".into());
    };
    if let SnapshotError::Secrets { shown, total } = &error {
        assert_eq!(*total, 1);
        let hits: Vec<(&str, &str, u64)> = shown
            .iter()
            .map(|hit| (hit.path().as_str(), hit.pattern(), hit.line()))
            .collect();
        assert_eq!(hits, [("src/lib.rs", "aws-access-key-id", 4)]);
    }
    let text = error.to_string();
    assert!(!text.contains(&aws_key()), "{text:?}");
    assert!(
        text.contains("src/lib.rs:4: matches aws-access-key-id"),
        "{text:?}"
    );

    // The same key in a gitignored file is never packed, so it never blocks.
    let clean = project()?;
    write_file(clean.path(), ".gitignore", b"secret.txt\n", false)?;
    write_file(clean.path(), "secret.txt", aws_key().as_bytes(), false)?;
    let snap = pack(clean.path(), CleanPolicy::AllowDirty)??;
    assert_eq!(snap.report().files(), 6);
    Ok(())
}

#[test]
fn allow_dirty_never_skips_the_scan() -> TestResult {
    let dir = planted()?;
    let result = pack(dir.path(), CleanPolicy::AllowDirty)?;
    assert!(
        matches!(result, Err(SnapshotError::Secrets { total: 1, .. })),
        "{result:?}"
    );
    Ok(())
}

#[test]
fn require_clean_outside_a_repository_is_dirty() -> TestResult {
    let dir = project()?;
    let result = pack(dir.path(), CleanPolicy::RequireClean)?;
    assert!(
        matches!(result, Err(SnapshotError::Dirty(Dirty::GitFailed { .. }))),
        "{result:?}"
    );
    let snap = pack(dir.path(), CleanPolicy::AllowDirty)??;
    assert_eq!(snap.report().files(), 5);
    Ok(())
}

#[test]
fn require_clean_in_a_committed_repository_succeeds() -> TestResult {
    let dir = project()?;
    git(dir.path(), &["init", "-q"])?;
    git(dir.path(), &["add", "-A"])?;
    git(dir.path(), &["commit", "-q", "-m", "project"])?;
    let snap = pack(dir.path(), CleanPolicy::RequireClean)??;
    assert_eq!(snap.report().files(), 5);
    let denied: Vec<&str> = snap
        .report()
        .denied()
        .iter()
        .map(|drop| drop.path().as_str())
        .collect();
    assert_eq!(denied, [".git"]);
    // The committed tree packs to the same bytes as the uncommitted one.
    let plain = project()?;
    let same = pack(plain.path(), CleanPolicy::AllowDirty)??;
    assert_eq!(snap.tree(), same.tree());

    write_file(dir.path(), "new.txt", b"untracked\n", false)?;
    let result = pack(dir.path(), CleanPolicy::RequireClean)?;
    assert!(
        matches!(result, Err(SnapshotError::Dirty(Dirty::Changes))),
        "{result:?}"
    );
    Ok(())
}
