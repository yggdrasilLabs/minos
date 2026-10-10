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
//!
//! `boot_from_dockerfile` (02-06-PLAN.md, D-15) boots the produced image as the root disk and runs a command
//! from it. It also needs a signed `MINOS_VMM` and `MINOS_KRUNFW` (`BOOT_SETUP`). Its launch helper repeats the
//! one in `spike_boot.rs` on purpose: each integration-test file is its own crate.

use std::error::Error;
use std::ffi::OsString;
use std::io::{self, Cursor, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use backhand::{FilesystemReader, InnerNode};
use minos_backend_krun::input_disk::input_disk_image;
use minos_backend_krun::launch_config::{
    Boot, Console, Disks, Kernel, LaunchConfig, Machine, Vsock,
};
use minos_backend_krun::rootfs::{PackLimits, PackReport, PackedImage, pack_root_image};
use minos_backend_krun::supervise::{self, JobFiles, Launch, MAX_FRAME_BYTES, VmRun};
use serde::Serialize;
use serde_json::Value;
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

/// 02-06-PLAN.md E-1 and E-4, for `boot_from_dockerfile`.
const BOOT_SETUP: &str = "\
build and sign first:
  cargo build -p minos-guest --release --target aarch64-unknown-linux-musl
  cargo build -p minos-vmm
  codesign --force --sign - --options runtime --entitlements crates/minos-vmm/minos-vmm.entitlements target/debug/minos-vmm
  codesign --display --entitlements - target/debug/minos-vmm
then set:
  MINOS_VMM=$(pwd)/target/debug/minos-vmm
  MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest
  MINOS_KRUNFW=$(realpath /opt/homebrew/lib/libkrunfw.5.dylib)
  MINOS_DOCKER=$(command -v docker)";

/// The guest's vsock port for the status frame; the job below names the same port.
const VSOCK_PORT: u32 = 1024;
/// Scratch disk size.
const SCRATCH_CAP_BYTES: u64 = 64 * 1024 * 1024;
/// Console bytes the launcher allows and the supervisor reads back.
const CONSOLE_CAP_BYTES: u64 = 1024 * 1024;
/// One vCPU and 256 MiB.
const MACHINE: Machine = Machine::new(1, 256);
/// Most characters of any one excerpt printed (at least the console cap, so the console prints whole).
const EXCERPT_CHARS: usize = 1_048_576;

/// `report_boot`, then `cat` the file the fixture's `RUN` step wrote, then power off.
const DOCKERFILE_JOB: &str = r#"{"v":1,"vsock_port":1024,"steps":[{"report_boot":{}},{"run":{"argv":["/bin/cat","/etc/minos-spike"],"stdout_cap":4096}}],"finish":"poweroff"}"#;

/// An absolute path from a variable `boot_from_dockerfile` needs. Missing is an error with the boot setup text.
fn boot_path(name: &str) -> TestResult<PathBuf> {
    let value = std::env::var_os(name).ok_or_else(|| format!("{name} is not set; {BOOT_SETUP}"))?;
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(format!("{name} must be an absolute path; {BOOT_SETUP}").into());
    }
    Ok(path)
}

/// Launches `vmm` once in root-disk mode for `job_json` in a fresh job directory.
fn launch(
    vmm: &Path,
    root_image: &[u8],
    job_json: &str,
    max_duration_ms: u64,
    host_deadline: Duration,
) -> TestResult<VmRun> {
    let dir = supervise::short_job_dir()?;
    let files = JobFiles::in_dir(dir.path())?;
    let input = input_disk_image(job_json.as_bytes())?;
    supervise::write_disks(&files, root_image, &input, SCRATCH_CAP_BYTES)?;
    let listener = supervise::bind_output(&files)?;
    let krunfw = std::fs::canonicalize(boot_path("MINOS_KRUNFW")?)?;
    let krunfw_sha256 = supervise::sha256_file(&krunfw)?;
    let config = LaunchConfig::new(
        Kernel::new(krunfw, krunfw_sha256),
        Boot::RootDisk,
        Disks::new(
            files.root(),
            files.input(),
            files.scratch(),
            SCRATCH_CAP_BYTES,
        ),
        Vsock::new(VSOCK_PORT, files.socket()),
        Console::new(files.console(), CONSOLE_CAP_BYTES),
        max_duration_ms,
        MACHINE,
    );
    supervise::write_config(&files, &config)?;
    let argv = [vmm, files.config()];
    let run = supervise::run(Launch::new(argv, &files, listener).host_deadline(host_deadline))?;
    Ok(run)
}

/// `text` with every control character except newline, and every bidi override, written as `\u{..}`.
fn printable(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        let bidi = matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}');
        if (c.is_control() && c != '\n') || bidi {
            out.extend(c.escape_unicode());
        } else {
            out.push(c);
        }
    }
    out
}

/// Prints everything a run left behind, so the test output is the evidence.
fn print_run(label: &str, run: &VmRun) -> TestResult {
    println!(
        "== {label}: exit_code {:?}, signal {:?}, killed_at_deadline {}, elapsed {:?}, connections {}",
        run.exit_code(),
        run.signal(),
        run.killed_at_deadline(),
        run.elapsed(),
        run.connections().len()
    );
    println!(
        "-- console ({} bytes, overflowed {}):\n{}",
        run.console().len(),
        run.console().overflowed(),
        run.console().excerpt(EXCERPT_CHARS)
    );
    println!(
        "-- minos-vmm stderr ({} bytes):\n{}",
        run.vmm_stderr().len(),
        run.vmm_stderr().excerpt(EXCERPT_CHARS)
    );
    println!(
        "-- minos-vmm stdout ({} bytes):\n{}",
        run.vmm_stdout().len(),
        run.vmm_stdout().excerpt(EXCERPT_CHARS)
    );
    for (index, connection) in run.connections().iter().enumerate() {
        match connection.status_frame(MAX_FRAME_BYTES) {
            Ok(frame) => println!(
                "-- connection {index} ({} bytes), frame:\n{}",
                connection.len(),
                printable(&serde_json::to_string_pretty(&frame)?)
            ),
            Err(error) => println!(
                "-- connection {index} ({} bytes, overflowed {}): not a frame: {error}",
                connection.len(),
                connection.overflowed()
            ),
        }
    }
    println!("== end {label}");
    Ok(())
}

/// The result object of the first step named `key` in a frame.
fn step<'a>(frame: &'a Value, key: &str) -> TestResult<&'a Value> {
    frame
        .get("steps")
        .and_then(Value::as_array)
        .and_then(|steps| steps.iter().find_map(|step| step.get(key)))
        .ok_or_else(|| format!("the frame has no {key} result").into())
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST, MINOS_KRUNFW, and MINOS_DOCKER with a running daemon; see 02-06-PLAN.md E-1"]
fn boot_from_dockerfile() -> TestResult {
    let vmm = boot_path("MINOS_VMM")?;
    if !supervise::has_hypervisor_entitlement(&vmm)? {
        return Err(format!(
            "{} lacks com.apple.security.hypervisor (cargo build relinks it without one); {BOOT_SETUP}",
            vmm.display()
        )
        .into());
    }
    let guest = std::fs::read(boot_path("MINOS_GUEST")?)?;
    let docker = boot_path("MINOS_DOCKER")?;
    let out_dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("spike_rootimg");
    std::fs::create_dir_all(&out_dir)?;
    let (_tar, image) = produce(&guest, &docker, &out_dir, "rootfs-boot", false)?;
    println!(
        "image: {} bytes, sha256 {}",
        image.bytes().len(),
        image.report().image_sha256()
    );

    let run = launch(
        &vmm,
        image.bytes(),
        DOCKERFILE_JOB,
        60_000,
        Duration::from_mins(2),
    )?;
    print_run("boot_from_dockerfile", &run)?;

    assert_eq!(run.exit_code(), Some(0));
    let frame = match run.connections() {
        [connection] => connection.status_frame(MAX_FRAME_BYTES)?,
        connections => {
            return Err(
                format!("expected exactly one connection, got {}", connections.len()).into(),
            );
        }
    };
    assert_eq!(
        step(&frame, "report_boot")?
            .get("marker")
            .and_then(Value::as_str),
        Some("MINOS-GUEST-BOOT-OK")
    );
    let cat = step(&frame, "run")?;
    assert_eq!(cat.get("status"), Some(&Value::from(0)));
    assert_eq!(
        cat.get("stdout").and_then(Value::as_str).map(str::as_bytes),
        Some(SPIKE_TEXT)
    );
    Ok(())
}
