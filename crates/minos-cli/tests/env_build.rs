//! `minos env build` against a fake builder: the lock rules (D-10, D-11), the build context's hygiene (KRUN-09),
//! the guest check (05-RESEARCH Pitfall 8), and the packed image (D-09). The Docker reproducibility test (D-12) is
//! ignored here and runs in plan 05-18 with Docker started.

use std::cell::{Cell, RefCell};
use std::error::Error;
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::process::Command;

use minos_cli::builder::{BuilderError, DockerBuildx, RootfsBuilder, RunningBuild};
use minos_cli::env_build::{EnvBuildError, EnvBuildOptions, EnvBuildReport, env_build};
use minos_core::id::BlobId;
use minos_core::ledger::BackendKind;
use minos_core::lock::{BuilderId, BuilderText, Lock, LockedImage};
use minos_core::manifest::Arch;
use minos_core::path::RelPath;
use minos_source::SnapshotError;
use minos_store::{Blobs, FileStore, STORE_DIR};
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn Error>>;

/// The Phase 2 fixture's base image, pinned by digest without the `:3` tag, which `ImageRef` refuses.
const ALPINE: &str = "docker.io/library/alpine@sha256:294b683cb724975bec92580e1e685676bd4b50bda910ddb8c51d4cabeaec77e6";

/// The `minos.toml` example in CLAUDE.md, section "Projects": `judge/` is under `[source] exclude`.
const MANIFEST: &str = r#"[project]
name = "logparse"
kind = "optimize"
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
kind = "greedy"
width = 4

[[proposer]]
kind = "single_shot"
model = "mid"
batch = 4
"#;

/// The fixture Dockerfile: one digest-pinned base, and a `RUN` that stamps a file with the wall-clock time, which
/// the packer must zero (02-SPIKE-EVIDENCE.md RI-04).
fn dockerfile() -> String {
    format!(
        "FROM {ALPINE}\nCOPY program.md /usr/local/share/minos-fixture/program.md\nRUN date > /usr/local/share/minos-fixture/stamp\n"
    )
}

/// A builder that records every context it is given and returns a fixed rootfs tar.
struct FakeBuilder {
    rootfs: RefCell<Vec<u8>>,
    contexts: RefCell<Vec<Vec<u8>>>,
    started: Cell<u32>,
}

impl FakeBuilder {
    fn new(rootfs: Vec<u8>) -> Self {
        Self {
            rootfs: RefCell::new(rootfs),
            contexts: RefCell::new(Vec::new()),
            started: Cell::new(0),
        }
    }

    fn set_rootfs(&self, rootfs: Vec<u8>) {
        self.rootfs.replace(rootfs);
    }
}

impl RootfsBuilder for FakeBuilder {
    type Rootfs = Cursor<Vec<u8>>;

    fn identity(&self) -> Result<BuilderId, BuilderError> {
        let text = |text: &str| BuilderText::try_from(text.to_owned());
        match (text("fake-builder"), text("v1")) {
            (Ok(name), Ok(version)) => Ok(BuilderId::new(name, version)),
            _ => Err(BuilderError::Identity),
        }
    }

    fn start(
        &self,
        context: Vec<u8>,
        _dockerfile: &RelPath,
    ) -> Result<RunningBuild<Cursor<Vec<u8>>>, BuilderError> {
        self.started.set(self.started.get().saturating_add(1));
        self.contexts.borrow_mut().push(context);
        Ok(RunningBuild::completed(
            Cursor::new(self.rootfs.borrow().clone()),
            Ok(()),
        ))
    }
}

/// A builder that records every context it is given and delegates to another builder.
struct Recording<B> {
    inner: B,
    contexts: RefCell<Vec<Vec<u8>>>,
}

impl<B: RootfsBuilder> RootfsBuilder for Recording<B> {
    type Rootfs = B::Rootfs;

    fn identity(&self) -> Result<BuilderId, BuilderError> {
        self.inner.identity()
    }

    fn start(
        &self,
        context: Vec<u8>,
        dockerfile: &RelPath,
    ) -> Result<RunningBuild<B::Rootfs>, BuilderError> {
        self.contexts.borrow_mut().push(context.clone());
        self.inner.start(context, dockerfile)
    }
}

/// A ustar header for `kind` with `mode` and `size`, owned by root, mtime 0.
fn header(kind: tar::EntryType, mode: u32, size: u64) -> tar::Header {
    let mut header = tar::Header::new_ustar();
    header.set_entry_type(kind);
    header.set_mode(mode);
    header.set_size(size);
    header.set_uid(0);
    header.set_gid(0);
    header.set_mtime(0);
    header
}

/// A rootfs tar holding `etc/` and `etc/hostname` with `hostname`.
fn rootfs_tar(hostname: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut builder = tar::Builder::new(Vec::new());
    let mut dir = header(tar::EntryType::Directory, 0o755, 0);
    builder.append_data(&mut dir, "etc/", std::io::empty())?;
    let mut file = header(
        tar::EntryType::Regular,
        0o644,
        u64::try_from(hostname.len())?,
    );
    builder.append_data(&mut file, "etc/hostname", hostname)?;
    Ok(builder.into_inner()?)
}

/// A minimal static `AArch64` ELF: the 64-byte header (ELFCLASS64, little-endian, `EM_AARCH64`, no program
/// headers) followed by padding.
fn elf_guest() -> Vec<u8> {
    let mut bytes = vec![0_u8; 64];
    let header: [(usize, u8); 10] = [
        (0, 0x7f),
        (1, b'E'),
        (2, b'L'),
        (3, b'F'),
        (4, 2),
        (5, 1),
        (6, 1),
        (16, 2),
        (18, 183),
        (20, 1),
    ];
    for (at, byte) in header {
        if let Some(slot) = bytes.get_mut(at) {
            *slot = byte;
        }
    }
    bytes.extend(std::iter::repeat_n(0xa5, 1024));
    bytes
}

/// A Mach-O header, as the macOS build of `minos-guest` starts.
fn macho_guest() -> Vec<u8> {
    let mut bytes = vec![0xcf, 0xfa, 0xed, 0xfe, 0x0c, 0x00, 0x00, 0x01];
    bytes.extend(std::iter::repeat_n(0, 1024));
    bytes
}

/// A fixture project and its guest binary, in temporary directories.
struct Fixture {
    _dir: TempDir,
    project: PathBuf,
    guest: PathBuf,
    guest_bytes: Vec<u8>,
}

/// Writes `bytes` to `dir/rel`, creating parent directories.
fn write(dir: &Path, rel: &str, bytes: &[u8]) -> TestResult {
    let path = dir.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes)?;
    Ok(())
}

/// The fixture project, with judge data and a `.env` file the build context must leave out, and `guest` as the
/// guest binary.
fn fixture(guest: Vec<u8>) -> Result<Fixture, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let root = fs::canonicalize(dir.path())?;
    let project = root.join("project");
    fs::create_dir(&project)?;
    write(&project, "minos.toml", MANIFEST.as_bytes())?;
    write(
        &project,
        "program.md",
        b"Make src/parse.rs faster without changing its output.\n",
    )?;
    write(&project, "src/lib.rs", b"pub mod parse;\n")?;
    write(
        &project,
        "src/parse.rs",
        b"pub fn parse(input: &str) -> usize {\n    input.len()\n}\n",
    )?;
    write(&project, "minos.Dockerfile", dockerfile().as_bytes())?;
    write(
        &project,
        "judge/judge",
        b"#!/bin/sh\necho judge-private-data\n",
    )?;
    write(&project, "judge/expected.txt", b"judge-private-data\n")?;
    write(&project, ".env", b"MINOS_FIXTURE=1\n")?;
    let guest_path = root.join("minos-guest");
    fs::write(&guest_path, &guest)?;
    Ok(Fixture {
        _dir: dir,
        project,
        guest: guest_path,
        guest_bytes: guest,
    })
}

/// `env build` of the fixture with `--allow-dirty` (the temporary project is not a git repository).
fn build<B: RootfsBuilder>(
    fixture: &Fixture,
    builder: &B,
    update: bool,
) -> Result<EnvBuildReport, EnvBuildError> {
    env_build(
        &fixture.project,
        &fixture.guest,
        builder,
        EnvBuildOptions::new(update, true),
    )
}

/// The fixture's lock entry for krun on arm64.
fn lock_entry(fixture: &Fixture) -> Result<LockedImage, Box<dyn Error>> {
    let text = fs::read_to_string(fixture.project.join("minos.lock"))?;
    let lock = Lock::parse_toml(&text)?;
    Ok(lock
        .image(BackendKind::Krun, Arch::Arm64)
        .cloned()
        .ok_or("no krun/arm64 entry")?)
}

/// The entry paths of a tarball.
fn context_paths(tarball: &[u8]) -> Result<Vec<String>, Box<dyn Error>> {
    let mut archive = tar::Archive::new(tarball);
    let mut paths = Vec::new();
    for entry in archive.entries()? {
        let entry = entry?;
        paths.push(String::from_utf8(entry.path_bytes().into_owned())?);
    }
    Ok(paths)
}

/// Asserts that `context` holds the project's files and no judge data, `.env`, store, or lock.
fn assert_clean_context(context: &[u8]) -> TestResult {
    let paths = context_paths(context)?;
    for wanted in [
        "minos.Dockerfile",
        "minos.toml",
        "program.md",
        "src/parse.rs",
    ] {
        assert!(
            paths.iter().any(|path| path == wanted),
            "{wanted} missing: {paths:?}"
        );
    }
    for path in &paths {
        assert!(
            !path.starts_with("judge/"),
            "judge data in the context: {path}"
        );
        assert!(!path.starts_with(".env"), ".env in the context: {path}");
        assert!(
            !path.starts_with(".minos/"),
            "the store in the context: {path}"
        );
        assert!(path != "minos.lock", "minos.lock in the context");
    }
    Ok(())
}

#[test]
fn first_build_writes_lock() -> TestResult {
    let fixture = fixture(elf_guest())?;
    let builder = FakeBuilder::new(rootfs_tar(b"one\n")?);

    let report = build(&fixture, &builder, false)?;

    assert!(report.lock_written());
    let entry = lock_entry(&fixture)?;
    assert_eq!(entry.root_image(), report.image());
    let store = FileStore::open(&fixture.project.join(STORE_DIR))?;
    assert!(store.contains(&report.image().to_blob_id())?);
    assert_eq!(
        *entry.dockerfile_sha256(),
        BlobId::of(dockerfile().as_bytes())
    );
    let bases: Vec<String> = entry
        .base_images()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(bases, [ALPINE]);
    assert_eq!(*entry.guest_sha256(), BlobId::of(&fixture.guest_bytes));
    assert_eq!(entry.builder().name().as_str(), "fake-builder");
    assert_eq!(entry.builder().version().as_str(), "v1");
    Ok(())
}

#[test]
fn same_build_keeps_lock() -> TestResult {
    let fixture = fixture(elf_guest())?;
    let builder = FakeBuilder::new(rootfs_tar(b"one\n")?);
    let first = build(&fixture, &builder, false)?;
    let before = fs::read(fixture.project.join("minos.lock"))?;

    let second = build(&fixture, &builder, false)?;

    assert_eq!(first.image(), second.image());
    assert!(!second.lock_written());
    assert_eq!(fs::read(fixture.project.join("minos.lock"))?, before);
    Ok(())
}

#[test]
fn changed_image_refused() -> TestResult {
    let fixture = fixture(elf_guest())?;
    let builder = FakeBuilder::new(rootfs_tar(b"one\n")?);
    let first = build(&fixture, &builder, false)?;
    let before = fs::read(fixture.project.join("minos.lock"))?;

    builder.set_rootfs(rootfs_tar(b"two\n")?);
    let refused = build(&fixture, &builder, false);
    let Err(error @ EnvBuildError::LockMismatch { locked, built }) = refused else {
        return Err(format!("expected LockMismatch, got {refused:?}").into());
    };
    assert_eq!(locked, *first.image());
    assert_ne!(built, locked);
    let message = error.to_string();
    assert!(message.contains(&locked.to_string()), "{message}");
    assert!(message.contains(&built.to_string()), "{message}");
    assert!(message.contains("--update"), "{message}");
    assert_eq!(fs::read(fixture.project.join("minos.lock"))?, before);

    let updated = build(&fixture, &builder, true)?;
    assert!(updated.lock_written());
    assert_eq!(*updated.image(), built);
    assert_eq!(*lock_entry(&fixture)?.root_image(), built);
    Ok(())
}

#[test]
fn context_has_no_judge_or_secrets() -> TestResult {
    let fixture = fixture(elf_guest())?;
    let builder = FakeBuilder::new(rootfs_tar(b"one\n")?);
    build(&fixture, &builder, false)?;
    // The first build created .minos/ and minos.lock, so the second context is checked with all four present.
    assert!(fixture.project.join(".minos").is_dir());
    assert!(fixture.project.join("minos.lock").is_file());
    assert!(fixture.project.join("judge/judge").is_file());
    assert!(fixture.project.join(".env").is_file());

    build(&fixture, &builder, false)?;

    assert_eq!(builder.started.get(), 2);
    for context in builder.contexts.borrow().iter() {
        assert_clean_context(context)?;
    }
    Ok(())
}

#[test]
fn secret_blocks_build() -> TestResult {
    let fixture = fixture(elf_guest())?;
    // An AWS-style access key ID, assembled at run time so no key literal sits in the repository.
    let key = format!("AKIA{}", "Q".repeat(16));
    write(
        &fixture.project,
        "src/lib.rs",
        format!("pub mod parse;\n// {key}\n").as_bytes(),
    )?;
    let builder = FakeBuilder::new(rootfs_tar(b"one\n")?);

    let result = build(&fixture, &builder, false);

    assert!(
        matches!(
            result,
            Err(EnvBuildError::Snapshot(SnapshotError::Secrets { .. }))
        ),
        "{result:?}"
    );
    assert_eq!(builder.started.get(), 0);
    assert!(!fixture.project.join("minos.lock").exists());
    Ok(())
}

#[test]
fn dirty_tree_blocks_build_without_allow_dirty() -> TestResult {
    let fixture = fixture(elf_guest())?;
    let builder = FakeBuilder::new(rootfs_tar(b"one\n")?);

    // The temporary project is not a git repository, so the clean check refuses it.
    let result = env_build(
        &fixture.project,
        &fixture.guest,
        &builder,
        EnvBuildOptions::new(false, false),
    );

    assert!(
        matches!(
            result,
            Err(EnvBuildError::Snapshot(SnapshotError::Dirty(_)))
        ),
        "{result:?}"
    );
    assert_eq!(builder.started.get(), 0);
    Ok(())
}

#[test]
fn wrong_guest_refused() -> TestResult {
    let fixture = fixture(macho_guest())?;
    let builder = FakeBuilder::new(rootfs_tar(b"one\n")?);

    let result = build(&fixture, &builder, false);

    assert!(
        matches!(result, Err(EnvBuildError::Guest { .. })),
        "{result:?}"
    );
    assert_eq!(builder.started.get(), 0);
    Ok(())
}

#[test]
fn tagged_base_image_refused() -> TestResult {
    let fixture = fixture(elf_guest())?;
    write(&fixture.project, "minos.Dockerfile", b"FROM alpine:3.20\n")?;
    let builder = FakeBuilder::new(rootfs_tar(b"one\n")?);

    let result = build(&fixture, &builder, false);

    assert!(
        matches!(result, Err(EnvBuildError::BaseImages(_))),
        "{result:?}"
    );
    assert_eq!(builder.started.get(), 0);
    Ok(())
}

#[test]
fn image_has_init_and_mount_points() -> TestResult {
    let fixture = fixture(elf_guest())?;
    // The rootfs supplies its own init, which the trusted guest replaces.
    let mut tar = tar::Builder::new(Vec::new());
    let mut init = header(tar::EntryType::Regular, 0o755, 9);
    tar.append_data(&mut init, "init", &b"#!/bin/sh"[..])?;
    let builder = FakeBuilder::new(tar.into_inner()?);

    let report = build(&fixture, &builder, false)?;

    let pack = report.pack();
    assert_eq!(
        pack.init_sha256(),
        BlobId::of(&fixture.guest_bytes).to_string()
    );
    assert!(pack.init_overridden());
    // /init is the one file; the seven mount points are the directories.
    assert_eq!(pack.files(), 1);
    assert_eq!(pack.dirs(), 7);
    let store = FileStore::open(&fixture.project.join(STORE_DIR))?;
    let image = store.get(&report.image().to_blob_id(), 1 << 30)?;
    assert_eq!(BlobId::of(&image).to_string(), pack.image_sha256());
    assert_eq!(image.get(..4), Some(&b"hsqs"[..]), "not a SquashFS image");
    Ok(())
}

#[test]
fn cli_wiring() -> TestResult {
    let output = Command::new(env!("CARGO_BIN_EXE_minos"))
        .args(["env", "build", "--help"])
        .output()?;
    assert!(output.status.success(), "minos env build --help failed");
    let help = String::from_utf8(output.stdout)?;
    for wanted in ["--update", "--allow-dirty", "/usr/local", "64000"] {
        assert!(help.contains(wanted), "{wanted} missing in:\n{help}");
    }
    Ok(())
}

/// D-12 with Docker: the same Dockerfile built twice gives the same image digest; the lock holds every D-11
/// field; the context holds no judge data and no secret-pattern file. The second build reuses `BuildKit`'s layer
/// cache, so its `RUN` step's wall-clock stamp is the first build's; a `--no-cache` rebuild writes a new stamp and so
/// a new image (05-EVIDENCE.md 05-18 env build). The packer's mtime zeroing is proven by `minos-backend-krun`'s
/// `rootfs_tar_mtimes_do_not_change_the_image`.
#[test]
#[ignore = "needs Docker Desktop running and MINOS_GUEST; run in 05-18"]
fn docker_env_build_reproduces() -> TestResult {
    let guest_path =
        std::env::var_os("MINOS_GUEST").ok_or("set MINOS_GUEST to the musl minos-guest")?;
    let mut guest = Vec::new();
    fs::File::open(&guest_path)?.read_to_end(&mut guest)?;
    let fixture = fixture(guest)?;
    let builder = Recording {
        inner: DockerBuildx::find()?,
        contexts: RefCell::new(Vec::new()),
    };

    let first = build(&fixture, &builder, false)?;
    let entry = lock_entry(&fixture)?;
    let second = build(&fixture, &builder, false)?;
    println!("first image:  {}", first.image());
    println!("second image: {}", second.image());
    println!(
        "minos.lock:\n{}",
        fs::read_to_string(fixture.project.join("minos.lock"))?
    );

    assert_eq!(first.image(), second.image(), "the image did not reproduce");
    assert!(first.lock_written());
    assert!(!second.lock_written());
    assert_eq!(entry.root_image(), first.image());
    assert_eq!(
        *entry.dockerfile_sha256(),
        BlobId::of(dockerfile().as_bytes())
    );
    let bases: Vec<String> = entry
        .base_images()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(bases, [ALPINE]);
    assert_eq!(*entry.guest_sha256(), BlobId::of(&fixture.guest_bytes));
    assert_eq!(entry.builder().name().as_str(), "docker-buildx");
    assert!(entry.builder().version().as_str().starts_with('v'));
    for context in builder.contexts.borrow().iter() {
        assert_clean_context(context)?;
    }
    Ok(())
}
