//! The read-back of a snapshot tarball: `Snapshot::surface` returns the surface and program bytes that were
//! packed, `read_files` returns exactly the requested files, and every malformed shape (a link, a repeated path, a
//! name outside the path grammar, a missing end marker, an over-cap entry, too many entries) is refused with fixed
//! text and no file bytes.

use std::collections::BTreeSet;
use std::error::Error;
use std::fs;
use std::path::Path;

use minos_core::manifest::Manifest;
use minos_core::path::RelPath;
use minos_source::{CleanPolicy, ReadError, Snapshot, read_files, snapshot};
use tar::{Builder, EntryType, Header};
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn Error>>;

/// The `minos.toml` example in CLAUDE.md, section "Projects", as in tests/pack.rs.
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

const PROGRAM: &[u8] = b"Make src/parse.rs faster without changing its output.\n";
const LIB: &[u8] = b"pub mod parse;\n";
const PARSE: &[u8] = b"pub fn parse(input: &str) -> usize {\n    input.len()\n}\n";

/// The per-file cap of a snapshot (16 MiB), mirrored from the crate's private constant.
const FILE_CAP: u64 = 16_777_216;

/// The entry-count cap of a snapshot, mirrored from the crate's private constant.
const MAX_FILES: usize = 10_000;

/// Writes `bytes` to `dir/rel`, creating parent directories.
fn write_file(dir: &Path, rel: &str, bytes: &[u8]) -> TestResult {
    let path = dir.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, bytes)?;
    Ok(())
}

/// A temporary project with the example manifest, its program, its surface, and one file outside the surface.
fn project() -> Result<TempDir, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    write_file(dir.path(), "minos.toml", CLAUDE_EXAMPLE.as_bytes())?;
    write_file(dir.path(), "minos.Dockerfile", b"FROM scratch\n")?;
    write_file(dir.path(), "program.md", PROGRAM)?;
    write_file(dir.path(), "src/lib.rs", LIB)?;
    write_file(dir.path(), "src/parse.rs", PARSE)?;
    write_file(dir.path(), "src/other.rs", b"pub fn other() {}\n")?;
    Ok(dir)
}

/// The example manifest.
fn manifest() -> Result<Manifest, Box<dyn Error>> {
    Ok(Manifest::parse_toml(CLAUDE_EXAMPLE)?)
}

/// Snapshots `dir` with the example manifest and no clean check.
fn pack(dir: &Path) -> Result<Snapshot, Box<dyn Error>> {
    Ok(snapshot(
        dir,
        &manifest()?,
        CleanPolicy::AllowDirty,
        &BTreeSet::new(),
    )?)
}

/// A set of project paths.
fn paths(items: &[&str]) -> Result<BTreeSet<RelPath>, Box<dyn Error>> {
    let mut set = BTreeSet::new();
    for item in items {
        set.insert(RelPath::try_from(*item)?);
    }
    Ok(set)
}

/// A ustar header for a regular file of `size` bytes at `path`, built the way the packer builds its own.
fn regular(path: &str, size: u64) -> Result<Header, Box<dyn Error>> {
    let mut header = Header::new_ustar();
    header.set_path(path)?;
    header.set_size(size);
    header.set_mode(0o644);
    header.set_uid(0);
    header.set_gid(0);
    header.set_mtime(0);
    header.set_entry_type(EntryType::Regular);
    header.set_cksum();
    Ok(header)
}

/// A tarball of `entries`, each appended with its header exactly as given, ending with the two zero blocks.
fn archive(entries: &[(Header, &[u8])]) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut builder = Builder::new(Vec::new());
    for (header, bytes) in entries {
        builder.append(header, *bytes)?;
    }
    Ok(builder.into_inner()?)
}

/// Checks `result` is a `Malformed` refusal with exactly `what`.
fn assert_malformed<T>(result: Result<T, ReadError>, what: &str) -> TestResult {
    match result {
        Err(ReadError::Malformed { what: got }) if got == what => Ok(()),
        Err(other) => Err(format!("expected Malformed({what}), got {other}").into()),
        Ok(_) => Err(format!("expected Malformed({what}), got a tree").into()),
    }
}

#[test]
fn surface_returns_the_packed_surface_and_program() -> TestResult {
    let dir = project()?;
    let snap = pack(dir.path())?;
    let (surface, program) = snap.surface(&manifest()?)?;
    let listed: Vec<&str> = surface.iter().map(|(path, _)| path.as_str()).collect();
    assert_eq!(listed, ["src/lib.rs", "src/parse.rs"]);
    assert_eq!(surface.get(&RelPath::try_from("src/lib.rs")?), Some(LIB));
    assert_eq!(
        surface.get(&RelPath::try_from("src/parse.rs")?),
        Some(PARSE)
    );
    assert_eq!(program, fs::read(dir.path().join("program.md"))?);
    Ok(())
}

#[test]
fn read_files_returns_exactly_the_requested_files() -> TestResult {
    let dir = project()?;
    let snap = pack(dir.path())?;
    let tree = read_files(snap.tarball(), &paths(&["src/parse.rs"])?)?;
    assert_eq!(tree.len(), 1);
    assert_eq!(tree.get(&RelPath::try_from("src/parse.rs")?), Some(PARSE));
    Ok(())
}

#[test]
fn read_files_with_nothing_requested_still_checks_the_archive() -> TestResult {
    let dir = project()?;
    let snap = pack(dir.path())?;
    let tree = read_files(snap.tarball(), &BTreeSet::new())?;
    assert!(tree.is_empty());
    assert_malformed(
        read_files(&[], &BTreeSet::new()),
        "the archive has no end marker",
    )?;
    Ok(())
}

#[test]
fn every_absent_path_is_named() -> TestResult {
    let dir = project()?;
    let snap = pack(dir.path())?;
    let result = read_files(
        snap.tarball(),
        &paths(&["src/parse.rs", "src/absent.rs", "a/missing.rs"])?,
    );
    match result {
        Err(ReadError::Missing { paths: missing }) => {
            let names: Vec<&str> = missing.iter().map(RelPath::as_str).collect();
            assert_eq!(names, ["a/missing.rs", "src/absent.rs"]);
        }
        Err(other) => return Err(format!("expected Missing, got {other}").into()),
        Ok(_) => return Err("expected Missing, got a tree".into()),
    }
    Ok(())
}

#[test]
fn an_empty_slice_is_malformed() -> TestResult {
    assert_malformed(
        read_files(&[], &paths(&["src/parse.rs"])?),
        "the archive has no end marker",
    )?;
    Ok(())
}

#[test]
fn a_symlink_entry_is_malformed() -> TestResult {
    let mut link = Header::new_ustar();
    link.set_path("src/parse.rs")?;
    link.set_size(0);
    link.set_mode(0o644);
    link.set_entry_type(EntryType::Symlink);
    link.set_link_name("/etc/passwd")?;
    link.set_cksum();
    let tarball = archive(&[(link, &[])])?;
    assert_malformed(
        read_files(&tarball, &paths(&["src/parse.rs"])?),
        "an entry is not a regular file",
    )?;
    Ok(())
}

#[test]
fn a_directory_entry_is_malformed() -> TestResult {
    let mut dir = Header::new_ustar();
    dir.set_path("src/")?;
    dir.set_size(0);
    dir.set_mode(0o755);
    dir.set_entry_type(EntryType::Directory);
    dir.set_cksum();
    let tarball = archive(&[(dir, &[])])?;
    assert_malformed(
        read_files(&tarball, &BTreeSet::new()),
        "an entry is not a regular file",
    )?;
    Ok(())
}

#[test]
fn a_repeated_path_is_malformed() -> TestResult {
    let tarball = archive(&[
        (regular("src/parse.rs", 2)?, b"a\n"),
        (regular("src/parse.rs", 2)?, b"b\n"),
    ])?;
    assert_malformed(
        read_files(&tarball, &paths(&["src/parse.rs"])?),
        "a path appears twice",
    )?;
    Ok(())
}

#[test]
fn a_name_outside_the_grammar_is_malformed() -> TestResult {
    // `set_path` refuses `..`, so the name bytes are written into the header directly.
    for name in [&b"../x"[..], b"/etc/passwd", b"a/./b", b"\xff"] {
        let mut header = regular("placeholder", 1)?;
        let ustar = header
            .as_ustar_mut()
            .ok_or("a new ustar header has ustar fields")?;
        ustar.name = [0; 100];
        for (slot, byte) in ustar.name.iter_mut().zip(name) {
            *slot = *byte;
        }
        header.set_cksum();
        let tarball = archive(&[(header, b"x")])?;
        assert_malformed(
            read_files(&tarball, &BTreeSet::new()),
            "an entry name is not a project path",
        )?;
    }
    Ok(())
}

#[test]
fn an_entry_over_the_file_cap_is_refused_before_it_is_read() -> TestResult {
    // The header claims one byte over the cap and no data follows: the size is refused from the header alone.
    let mut tarball = regular("big.bin", FILE_CAP + 1)?.as_bytes().to_vec();
    tarball.extend_from_slice(&[0; 1024]);
    assert_malformed(
        read_files(&tarball, &BTreeSet::new()),
        "an entry is over its cap",
    )?;
    Ok(())
}

#[test]
fn too_many_entries_is_refused() -> TestResult {
    let mut builder = Builder::new(Vec::new());
    for index in 0..=MAX_FILES {
        let header = regular(&format!("f/{index}"), 0)?;
        builder.append(&header, &[][..])?;
    }
    let tarball = builder.into_inner()?;
    assert_malformed(read_files(&tarball, &BTreeSet::new()), "too many entries")?;
    Ok(())
}

#[test]
fn a_truncated_entry_is_malformed() -> TestResult {
    let whole = archive(&[(regular("src/parse.rs", 600)?, &[b'a'; 600])])?;
    // The header and the first data block only: the entry's second block and the end marker are gone.
    let cut = whole.get(..1024).ok_or("the archive holds two blocks")?;
    let result = read_files(cut, &paths(&["src/parse.rs"])?);
    assert!(
        matches!(result, Err(ReadError::Malformed { .. })),
        "a truncated archive is refused"
    );
    Ok(())
}

#[test]
fn data_after_the_end_marker_is_malformed() -> TestResult {
    let mut tarball = archive(&[(regular("src/parse.rs", 2)?, b"a\n")])?;
    tarball.extend_from_slice(&[b'x'; 512]);
    assert_malformed(
        read_files(&tarball, &paths(&["src/parse.rs"])?),
        "data follows the end marker",
    )?;
    Ok(())
}

#[test]
fn errors_carry_no_file_bytes() -> TestResult {
    let secret = b"hunter2-top-secret";
    let tarball = archive(&[
        (regular("src/a.rs", 18)?, secret),
        (regular("src/a.rs", 18)?, secret),
    ])?;
    let message = match read_files(&tarball, &paths(&["src/a.rs"])?) {
        Err(error) => error.to_string(),
        Ok(_) => return Err("expected a refusal".into()),
    };
    assert!(!message.contains("hunter2"), "{message}");
    Ok(())
}
