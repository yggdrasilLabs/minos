//! SPIKE-06 image production (02-04-PLAN.md Task 3, 02-CONTEXT.md D-14 and D-16).
//!
//! A digest-pinned arm64 Dockerfile goes through `docker buildx build --output type=tar` and the Rust
//! packer, twice. Whether the two builds match is recorded, not asserted (D-16). Whether the image
//! boots is 02-06's question (D-15).
//!
//! Setup: `cargo build -p minos-guest --release --target aarch64-unknown-linux-musl`, then
//! `MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest
//! MINOS_DOCKER=$(command -v docker) cargo test -p minos-backend-krun --test spike_rootimg --
//! --ignored --nocapture --test-threads=1`.

use std::error::Error;
use std::ffi::OsString;
use std::io::{self, Cursor, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use backhand::{FilesystemReader, InnerNode};
use minos_backend_krun::rootfs::{PackLimits, PackReport, PackedImage, pack_root_image};
use serde::Serialize;
use sha2::{Digest, Sha256};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

/// What to set when an environment variable is missing.
const SETUP: &str = "set MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest \
                     after cargo build -p minos-guest --release --target aarch64-unknown-linux-musl, \
                     and MINOS_DOCKER=$(command -v docker)";

/// System directories on the child's `PATH`, after the docker CLI's own directories.
const SYSTEM_PATH: [&str; 5] = ["/usr/bin", "/bin", "/usr/sbin", "/sbin", "/usr/local/bin"];

/// Bytes of the build's stderr kept for an error message.
const STDERR_CAP: u64 = 64 * 1024;

/// The file the fixture's `RUN` step writes, and what it holds.
const SPIKE_FILE: &str = "/etc/minos-spike";
const SPIKE_TEXT: &[u8] = b"minos-spike-06\n";

/// An absolute path from a required environment variable. Missing is an error, never a skip.
fn required_path(name: &str) -> TestResult<PathBuf> {
    let value = std::env::var_os(name).ok_or_else(|| format!("{name} is not set: {SETUP}"))?;
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(format!("{name} must be an absolute path: {SETUP}").into());
    }
    Ok(path)
}

/// `PATH` for the docker CLI: its directory, then the directory of its canonical path (Docker
/// Desktop installs the credential helper `credsStore` names beside the real binary), then the
/// system directories. Nothing from the caller's `PATH`.
fn child_path(docker: &Path) -> TestResult<OsString> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    let canonical = std::fs::canonicalize(docker)?;
    for dir in [docker.parent(), canonical.parent()]
        .into_iter()
        .flatten()
        .map(Path::to_path_buf)
        .chain(SYSTEM_PATH.iter().map(PathBuf::from))
    {
        if !dirs.contains(&dir) {
            dirs.push(dir);
        }
    }
    Ok(std::env::join_paths(dirs)?)
}

/// Builds the fixture with buildx into `<out_dir>/<name>.tar` and packs that tar with `guest` as
/// `/init`. Returns the tar bytes and the packed image.
fn produce(
    guest: &[u8],
    docker: &Path,
    out_dir: &Path,
    name: &str,
    no_cache: bool,
) -> TestResult<(Vec<u8>, PackedImage)> {
    let tar_path = out_dir.join(format!("{name}.tar"));
    match std::fs::remove_file(&tar_path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error.into()),
        _ => {}
    }
    let mut output = OsString::from("type=tar,dest=");
    output.push(&tar_path);
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rootimg");
    let home = std::env::var_os("HOME").ok_or("HOME is not set")?;

    let mut command = Command::new(docker);
    command
        .env_clear()
        .env("HOME", home)
        .env("PATH", child_path(docker)?)
        .args(["buildx", "build", "--platform", "linux/arm64", "--output"])
        .arg(&output);
    if no_cache {
        command.arg("--no-cache");
    }
    command
        .arg(&fixture)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = command.spawn()?;
    let mut stderr = Vec::new();
    if let Some(mut pipe) = child.stderr.take() {
        pipe.by_ref().take(STDERR_CAP).read_to_end(&mut stderr)?;
        // Drain the rest so the build never blocks on a full pipe.
        io::copy(&mut pipe, &mut io::sink())?;
    }
    let status = child.wait()?;
    if !status.success() {
        return Err(format!(
            "docker buildx build for {name} exited with {status}:\n{}",
            String::from_utf8_lossy(&stderr)
        )
        .into());
    }

    let tar = std::fs::read(&tar_path)?;
    let image = pack_root_image(tar.as_slice(), guest, &PackLimits::default())?;
    Ok((tar, image))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    base16ct::lower::encode_string(&digest)
}

/// The bytes of the regular file at `path` in a packed image.
fn image_file(image: &PackedImage, path: &str) -> TestResult<Vec<u8>> {
    let fs = FilesystemReader::from_reader(Cursor::new(image.bytes().to_vec()))?;
    let node = fs
        .files()
        .find(|node| node.fullpath == Path::new(path))
        .ok_or_else(|| format!("{path} is not in the image"))?;
    match &node.inner {
        InnerNode::File(file) => {
            let mut out = Vec::new();
            fs.file(file).reader().read_to_end(&mut out)?;
            Ok(out)
        }
        _ => Err(format!("{path} is not a regular file").into()),
    }
}

#[derive(Serialize)]
struct Artifact {
    bytes: usize,
    sha256: String,
}

impl Artifact {
    fn of(bytes: &[u8]) -> Self {
        Self {
            bytes: bytes.len(),
            sha256: sha256_hex(bytes),
        }
    }
}

#[derive(Serialize)]
struct Summary<'a> {
    guest_sha256: String,
    tar_1: Artifact,
    tar_2: Artifact,
    image_1: Artifact,
    image_2: Artifact,
    image_1_repacked_sha256: String,
    tars_match: bool,
    images_match: bool,
    report_1: &'a PackReport,
    report_2: &'a PackReport,
}

#[test]
#[ignore = "needs the Docker CLI and daemon; see 02-04-PLAN.md"]
fn produce_twice() -> TestResult {
    let guest = std::fs::read(required_path("MINOS_GUEST")?)?;
    let docker = required_path("MINOS_DOCKER")?;
    let out_dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("spike_rootimg");
    std::fs::create_dir_all(&out_dir)?;

    let (tar_1, image_1) = produce(&guest, &docker, &out_dir, "rootfs-1", false)?;
    let (tar_2, image_2) = produce(&guest, &docker, &out_dir, "rootfs-2", true)?;
    let repacked = pack_root_image(tar_1.as_slice(), &guest, &PackLimits::default())?;

    let summary = Summary {
        guest_sha256: sha256_hex(&guest),
        tar_1: Artifact::of(&tar_1),
        tar_2: Artifact::of(&tar_2),
        image_1: Artifact::of(image_1.bytes()),
        image_2: Artifact::of(image_2.bytes()),
        image_1_repacked_sha256: sha256_hex(repacked.bytes()),
        tars_match: tar_1 == tar_2,
        images_match: image_1.bytes() == image_2.bytes(),
        report_1: image_1.report(),
        report_2: image_2.report(),
    };
    println!("{}", serde_json::to_string(&summary)?);

    assert_eq!(repacked.bytes(), image_1.bytes(), "same tar packed twice");
    assert_eq!(image_file(&image_1, SPIKE_FILE)?, SPIKE_TEXT);
    assert_eq!(image_file(&image_1, "/init")?, guest);
    Ok(())
}
