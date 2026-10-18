//! Wave 0 proof (05-03-PLAN.md, 05-CONTEXT D-14): a root image read from the project's `.minos/blobs` store path
//! boots under the deny-default Seatbelt profile, and the profile's `ROOT_IMG` ancestor-metadata rule is what lets
//! it: the same layout under the profile with that rule removed is refused by preflight (05-RESEARCH Pitfall 2).
//!
//! The root image is written to `<job>/proj/.minos/blobs/sha256/<2 hex>/<62 hex>`, the layout of `FileStore`
//! (`minos-store` is kept out of backend crates by `deny.toml`, so the test writes the layout itself). The helper
//! runs as `/usr/bin/sandbox-exec -f <profile> -D NAME=value ... <minos-vmm> <config>`, an argv array with every
//! value canonical. Every test is ignored; a missing variable or an unentitled helper is an error carrying the
//! setup commands, never a skip. Guest-derived text is printed only with control and format characters escaped.
//!
//! Run: `cargo test -p minos-backend-krun --test wave0_store_root -- --ignored --nocapture --test-threads=1`.

use std::collections::BTreeMap;
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File, Permissions};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use minos_backend_krun::input_disk::input_disk_image;
use minos_backend_krun::launch_config::{
    Boot, Console, Disks, Kernel, LaunchConfig, Machine, Vsock,
};
use minos_backend_krun::rootfs::minimal_root_image;
use minos_backend_krun::supervise::{self, JobFiles, Launch, MAX_FRAME_BYTES, VmRun};
use serde_json::Value;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

/// What to run and set when a prerequisite is missing (CLAUDE.md Commands).
const SETUP: &str = "\
build and sign first:
  cargo build -p minos-guest --release --target aarch64-unknown-linux-musl
  cargo build -p minos-vmm
  codesign --force --sign - --options runtime --entitlements crates/minos-vmm/minos-vmm.entitlements target/debug/minos-vmm
  codesign --display --entitlements - target/debug/minos-vmm
then set:
  MINOS_VMM=$(pwd)/target/debug/minos-vmm
  MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest
  MINOS_KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib";

/// The Seatbelt launcher.
const SANDBOX_EXEC: &str = "/usr/bin/sandbox-exec";
/// The Homebrew symlink to libkrunfw. The profile lists it beside its Cellar target (`MINOS_KRUNFW`).
const KRUNFW_LINK: &str = "/opt/homebrew/lib/libkrunfw.5.dylib";
/// The profile, relative to this crate's manifest directory.
const PROFILE: &str = "../minos-vmm/minos-vmm.sb";
/// The profile rule under test, exactly as the profile writes it.
const ANCESTOR_RULE: &str = r#"(allow file-read-metadata (path-ancestors (param "ROOT_IMG")))"#;

/// The guest's vsock port for the status frame; the job below names the same port.
const VSOCK_PORT: u32 = 1024;
/// Scratch disk size.
const SCRATCH_CAP_BYTES: u64 = 64 * 1024 * 1024;
/// Console bytes the launcher allows and the supervisor reads back.
const CONSOLE_CAP_BYTES: u64 = 1024 * 1024;
/// One vCPU and 256 MiB.
const MACHINE: Machine = Machine::new(1, 256);
/// The helper's own deadline.
const MAX_DURATION_MS: u64 = 60_000;
/// Most characters of any one excerpt printed.
const EXCERPT_CHARS: usize = 1_048_576;

/// `report_boot` only, then power off.
const REPORT_BOOT_JOB: &str =
    r#"{"v":1,"vsock_port":1024,"steps":[{"report_boot":{}}],"finish":"poweroff"}"#;

/// An absolute path from a required environment variable. Missing or relative is an error with the setup text.
fn env_path(name: &str) -> TestResult<PathBuf> {
    let value = std::env::var_os(name).ok_or_else(|| format!("{name} is not set; {SETUP}"))?;
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(format!("{name} must be an absolute path; {SETUP}").into());
    }
    Ok(path)
}

/// The canonical libkrunfw path from `MINOS_KRUNFW` and its SHA-256.
fn krunfw() -> TestResult<(PathBuf, String)> {
    let path = fs::canonicalize(env_path("MINOS_KRUNFW")?)?;
    let sha256 = supervise::sha256_file(&path)?;
    Ok((path, sha256))
}

/// The canonical `MINOS_VMM`, which must carry the hypervisor entitlement.
fn signed_vmm() -> TestResult<PathBuf> {
    let vmm = fs::canonicalize(env_path("MINOS_VMM")?)?;
    if !supervise::has_hypervisor_entitlement(&vmm)? {
        return Err(format!(
            "{} lacks com.apple.security.hypervisor (cargo build relinks it without one); {SETUP}",
            vmm.display()
        )
        .into());
    }
    Ok(vmm)
}

/// The guest binary named by `MINOS_GUEST`.
fn guest() -> TestResult<Vec<u8>> {
    Ok(fs::read(env_path("MINOS_GUEST")?)?)
}

/// The canonical profile path.
fn profile() -> TestResult<PathBuf> {
    Ok(fs::canonicalize(
        Path::new(env!("CARGO_MANIFEST_DIR")).join(PROFILE),
    )?)
}

/// `path` with its parent directory canonicalized and its last component kept as written.
fn canonical_entry(path: &Path) -> TestResult<PathBuf> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent", path.display()))?;
    let name = path
        .file_name()
        .ok_or_else(|| format!("{} has no file name", path.display()))?;
    Ok(fs::canonicalize(parent)?.join(name))
}

/// `sandbox-exec -f <profile> -D NAME=value ... <program> <program_args>...` as an argv array.
fn seatbelt_argv(
    profile: &Path,
    params: &BTreeMap<&'static str, PathBuf>,
    program: &Path,
    program_args: &[&OsStr],
) -> Vec<OsString> {
    let mut argv = vec![
        OsString::from(SANDBOX_EXEC),
        OsString::from("-f"),
        profile.as_os_str().to_owned(),
    ];
    for (name, value) in params {
        let mut define = OsString::from(*name);
        define.push("=");
        define.push(value.as_os_str());
        argv.push(OsString::from("-D"));
        argv.push(define);
    }
    argv.push(program.as_os_str().to_owned());
    argv.extend(program_args.iter().map(|arg| (*arg).to_owned()));
    argv
}

/// Prints an argv, one word per line, so the evidence shows exactly what ran.
fn print_argv(label: &str, argv: &[OsString]) {
    println!("== {label} argv:");
    for word in argv {
        println!("  {}", word.to_string_lossy());
    }
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

/// Writes `image` at its content-addressed store path under `dir`, `proj/.minos/blobs/sha256/<2 hex>/<62 hex>`,
/// with the store's 0700 directories, and returns the canonical path.
fn write_store_blob(dir: &Path, image: &[u8]) -> TestResult<PathBuf> {
    let hex = base16ct::lower::encode_string(&sha2_digest(image));
    let (shard, rest) = hex.split_at_checked(2).ok_or("digest hex is too short")?;
    let shard_dir = dir
        .join("proj")
        .join(".minos")
        .join("blobs")
        .join("sha256")
        .join(shard);
    fs::create_dir_all(&shard_dir)?;
    for ancestor in shard_dir.ancestors() {
        if ancestor == dir {
            break;
        }
        fs::set_permissions(ancestor, Permissions::from_mode(0o700))?;
    }
    let blob = shard_dir.join(rest);
    File::create_new(&blob)?.write_all(image)?;
    Ok(fs::canonicalize(&blob)?)
}

/// SHA-256 of `bytes`.
fn sha2_digest(bytes: &[u8]) -> [u8; 32] {
    use sha2::Digest as _;
    sha2::Sha256::digest(bytes).into()
}

/// The profile text with the `ROOT_IMG` ancestor rule and the comment lines just above it removed.
fn profile_without_ancestor_rule(text: &str) -> TestResult<String> {
    let mut lines: Vec<&str> = text.lines().collect();
    let at = lines
        .iter()
        .position(|line| line.trim() == ANCESTOR_RULE)
        .ok_or("the profile has no ROOT_IMG ancestor rule")?;
    let mut start = at;
    while start
        .checked_sub(1)
        .and_then(|before| lines.get(before))
        .is_some_and(|line| line.starts_with(';'))
    {
        start = start.saturating_sub(1);
    }
    lines.drain(start..=at);
    let mut stripped = lines.join("\n");
    stripped.push('\n');
    if stripped.contains(r#"path-ancestors (param "ROOT_IMG")"#) {
        return Err("the stripped profile still names ROOT_IMG ancestors".into());
    }
    Ok(stripped)
}

/// Boots `REPORT_BOOT_JOB` from a store-path root image under `profile` and returns what the run left behind.
fn store_root_run(label: &str, profile: &Path) -> TestResult<(VmRun, PathBuf)> {
    let vmm = signed_vmm()?;
    let image = minimal_root_image(&guest()?)?;
    let dir = supervise::short_job_dir()?;
    let files = JobFiles::in_dir(dir.path())?;
    let root = write_store_blob(files.dir(), image.bytes())?;
    println!("== {label}: root image at {}", root.display());
    File::create_new(files.input())?.write_all(&input_disk_image(REPORT_BOOT_JOB.as_bytes())?)?;
    File::create_new(files.scratch())?.set_len(SCRATCH_CAP_BYTES)?;
    let listener = supervise::bind_output(&files)?;
    let (krunfw, krunfw_sha256) = krunfw()?;
    let config = LaunchConfig::new(
        Kernel::new(krunfw.clone(), krunfw_sha256),
        Boot::RootDisk,
        Disks::new(&root, files.input(), files.scratch(), SCRATCH_CAP_BYTES),
        Vsock::new(VSOCK_PORT, files.socket()),
        Console::new(files.console(), CONSOLE_CAP_BYTES),
        MAX_DURATION_MS,
        MACHINE,
    );
    supervise::write_config(&files, &config)?;
    let params = BTreeMap::from([
        ("VMM", vmm.clone()),
        ("CONFIG", canonical_entry(files.config())?),
        ("KRUNFW", fs::canonicalize(&krunfw)?),
        ("KRUNFW_LINK", canonical_entry(Path::new(KRUNFW_LINK))?),
        ("ROOT_IMG", root.clone()),
        ("INPUT_IMG", canonical_entry(files.input())?),
        ("SCRATCH_IMG", canonical_entry(files.scratch())?),
        ("CONSOLE_LOG", canonical_entry(files.console())?),
        ("SOCK", canonical_entry(files.socket())?),
    ]);
    let argv = seatbelt_argv(profile, &params, &vmm, &[files.config().as_os_str()]);
    print_argv(label, &argv);
    let run = supervise::run(
        Launch::for_config(argv, &files, listener, &config).host_deadline(Duration::from_mins(2)),
    )?;
    print_run(label, &run)?;
    Ok((run, root))
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST, MINOS_KRUNFW, and /usr/bin/sandbox-exec; see 05-03-PLAN.md"]
fn store_root_boots_under_seatbelt() -> TestResult {
    let (run, _) = store_root_run("store_root_boots_under_seatbelt", &profile()?)?;

    assert_eq!(run.exit_code(), Some(0));
    assert!(!run.killed_at_deadline());
    assert!(
        run.console().contains(b"MINOS-GUEST-BOOT-OK"),
        "console lacks MINOS-GUEST-BOOT-OK"
    );
    let [connection] = run.connections() else {
        return Err(format!(
            "expected exactly one connection, got {}",
            run.connections().len()
        )
        .into());
    };
    let frame = connection.status_frame(MAX_FRAME_BYTES)?;
    assert_eq!(frame.get("v"), Some(&Value::from(1)));
    let booted = frame
        .get("steps")
        .and_then(Value::as_array)
        .is_some_and(|steps| steps.iter().any(|step| step.get("report_boot").is_some()));
    assert!(booted, "the frame has no report_boot result");
    Ok(())
}

#[test]
#[ignore = "needs a signed MINOS_VMM, MINOS_GUEST, MINOS_KRUNFW, and /usr/bin/sandbox-exec; see 05-03-PLAN.md"]
fn store_root_without_ancestor_rule_is_denied() -> TestResult {
    let stripped = profile_without_ancestor_rule(&fs::read_to_string(profile()?)?)?;
    let profile_dir = supervise::short_job_dir()?;
    let stripped_path = fs::canonicalize(profile_dir.path())?.join("no-root-ancestors.sb");
    File::create_new(&stripped_path)?.write_all(stripped.as_bytes())?;
    println!(
        "== store_root_without_ancestor_rule_is_denied: profile without the rule at {}",
        stripped_path.display()
    );

    let (run, _) = store_root_run("store_root_without_ancestor_rule_is_denied", &stripped_path)?;

    assert_eq!(run.exit_code(), Some(66), "preflight must refuse the root");
    assert!(
        !run.console().contains(b"MINOS-GUEST-BOOT-OK"),
        "the guest booted without the ancestor rule"
    );
    let stderr = run.vmm_stderr().excerpt(EXCERPT_CHARS);
    assert!(
        stderr
            .lines()
            .any(|line| line.contains("disks.root") && line.contains("Operation not permitted")),
        "no stderr line names disks.root and Operation not permitted"
    );
    Ok(())
}
