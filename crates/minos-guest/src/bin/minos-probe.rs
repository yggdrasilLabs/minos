//! A test fixture that runs as the candidate (or the judge) inside the VM tests and measures what a stage can
//! and cannot do: egress, its environment and identity, its limits, writes outside scratch, the input disk node,
//! a forged output chunk, markers, output volume, and a judge scorecard.
//!
//! It is built with the guest but never packed into a user image: `minos env build` writes only the
//! `minos-guest` binary, as `/init`, and only the VM test harness puts this binary at
//! `/usr/local/bin/minos-probe` in its own test image.
//!
//! `minos-probe <command> [args]` runs one command. Most commands write one JSON object with fixed keys to the
//! out file they are given, and exit 0 when the probe itself ran, whatever it measured; a usage error exits 2 and
//! a probe that could not run or write its result exits 1.

use std::process::ExitCode;

fn main() -> ExitCode {
    entry()
}

#[cfg(target_os = "linux")]
fn entry() -> ExitCode {
    probe::run()
}

#[cfg(not(target_os = "linux"))]
fn entry() -> ExitCode {
    use std::io::Write as _;
    drop(writeln!(
        std::io::stderr(),
        "minos-probe: runs only inside a Linux VM"
    ));
    ExitCode::FAILURE
}

#[cfg(target_os = "linux")]
mod probe {
    //! The Linux commands.

    use std::collections::BTreeMap;
    use std::error::Error;
    use std::ffi::OsString;
    use std::fs::{self, File, OpenOptions};
    use std::io::{self, Read, Write};
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, Shutdown, SocketAddr, TcpStream};
    use std::os::unix::fs::MetadataExt;
    use std::path::{Path, PathBuf};
    use std::process::{Child, Command, ExitCode, Stdio};
    use std::time::Duration;

    use minos_core::chunk::{ChunkStream, ChunkToken};
    use minos_core::guest::OUTPUT_VSOCK_PORT;
    use minos_guest::pattern::fill_pattern;
    use serde::Serialize;
    use vsock::{VMADDR_CID_HOST, VsockStream};

    type ProbeResult = Result<(), Box<dyn Error>>;

    /// Connect timeout of every egress attempt.
    const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
    /// Most bytes read from `/proc/cmdline` and the like.
    const SMALL_FILE_CAP: u64 = 64 * 1024;
    /// Bytes in a MiB.
    const MIB: usize = 1_048_576;
    /// Largest file `output-fill` writes: core's per-entry output cap.
    const FILL_FILE_BYTES: u64 = 8 * 1024 * 1024;
    /// Where `fill-scratch` writes, under the scratch mount.
    const SCRATCH_FILL: &str = "/scratch/tmp/minos-probe-fill";
    /// Where `scorecard` writes.
    const SCORECARD: &str = "/out/scorecard.json";
    /// The scorecard `scorecard` writes: one passing metric.
    const SCORECARD_JSON: &str =
        r#"{"v":1,"metrics":[{"name":"probe_ok","value":1,"held_out":false}]}"#;
    /// The `/proc/self/limits` rows `env` reports.
    const LIMIT_ROWS: [&str; 6] = [
        "Max address space",
        "Max file size",
        "Max processes",
        "Max cpu time",
        "Max open files",
        "Max core file size",
    ];
    /// The usage line.
    const USAGE: &str = "usage: minos-probe egress|env|write-outside|forge|hang|marker-write|marker-check|output-fill|alloc|spawn|fill-scratch|scorecard|exit ...";

    /// Exit code of a usage error.
    const EXIT_USAGE: u8 = 2;

    pub(super) fn run() -> ExitCode {
        let args: Option<Vec<String>> = std::env::args_os()
            .skip(1)
            .map(|arg: OsString| arg.into_string().ok())
            .collect();
        let Some(args) = args else {
            return usage();
        };
        let words: Vec<&str> = args.iter().map(String::as_str).collect();
        let ran = match words.as_slice() {
            ["egress", out, ipv4, ipv6] => egress(out, ipv4, ipv6, None),
            ["egress", out, ipv4, ipv6, host] => egress(out, ipv4, ipv6, Some(host)),
            ["env", out] => env(out),
            ["write-outside", out] => write_outside(out),
            ["forge", out] => forge(out),
            ["hang"] => hang(),
            ["marker-write", path] => marker_write(path),
            ["marker-check", path, out] => marker_check(path, out),
            ["output-fill", dir, bytes] => output_fill(dir, bytes),
            ["alloc", out, mib] => alloc(out, mib),
            ["spawn", out, count] => spawn(out, count),
            ["fill-scratch", out] => fill_scratch(out),
            ["scorecard"] => scorecard(),
            ["exit", code] => return exit(code),
            _ => return usage(),
        };
        match ran {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                drop(writeln!(io::stderr(), "minos-probe: {error}"));
                ExitCode::FAILURE
            }
        }
    }

    fn usage() -> ExitCode {
        drop(writeln!(io::stderr(), "{USAGE}"));
        ExitCode::from(EXIT_USAGE)
    }

    /// Writes `value` as JSON to `out`.
    fn write_json(out: &str, value: &impl Serialize) -> ProbeResult {
        let mut bytes = serde_json::to_vec_pretty(value)?;
        bytes.push(b'\n');
        fs::write(out, bytes)?;
        Ok(())
    }

    /// The outcome of one attempt: whether it worked and, if not, the OS error number.
    #[derive(Serialize)]
    struct Attempt {
        target: String,
        ok: bool,
        os_error: Option<i32>,
    }

    impl Attempt {
        fn of<T>(target: impl Into<String>, result: &io::Result<T>) -> Self {
            Self {
                target: target.into(),
                ok: result.is_ok(),
                os_error: result.as_ref().err().and_then(io::Error::raw_os_error),
            }
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // egress
    // -----------------------------------------------------------------------------------------------------------

    #[derive(Serialize)]
    struct Egress {
        attempts: Vec<Attempt>,
        net_ifaces: Vec<String>,
        virtio_devices: Vec<VirtioDevice>,
    }

    #[derive(Serialize)]
    struct VirtioDevice {
        name: String,
        device: String,
    }

    fn egress(out: &str, ipv4: &str, ipv6: &str, host: Option<&str>) -> ProbeResult {
        let ipv4_port: u16 = ipv4.parse()?;
        let ipv6_port: u16 = ipv6.parse()?;
        let mut targets = vec![
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), ipv4_port),
            SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), ipv6_port),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)), 443),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)), 53),
            SocketAddr::new(
                IpAddr::V6(Ipv6Addr::new(0x2606, 0x4700, 0x4700, 0, 0, 0, 0, 0x1111)),
                443,
            ),
        ];
        if let Some(host) = host {
            targets.push(SocketAddr::new(host.parse()?, ipv4_port));
        }
        let attempts = targets
            .iter()
            .map(|target| {
                Attempt::of(
                    target.to_string(),
                    &TcpStream::connect_timeout(target, CONNECT_TIMEOUT),
                )
            })
            .collect();
        let virtio_devices = dir_names(Path::new("/sys/bus/virtio/devices"))
            .into_iter()
            .map(|name| {
                let device = read_small(
                    &Path::new("/sys/bus/virtio/devices")
                        .join(&name)
                        .join("device"),
                )
                .trim()
                .to_owned();
                VirtioDevice { name, device }
            })
            .collect();
        write_json(
            out,
            &Egress {
                attempts,
                net_ifaces: dir_names(Path::new("/sys/class/net")),
                virtio_devices,
            },
        )
    }

    /// Sorted entry names of a directory; an unreadable directory or entry gives a marked name.
    fn dir_names(path: &Path) -> Vec<String> {
        let mut names: Vec<String> = match fs::read_dir(path) {
            Ok(entries) => entries
                .map(|entry| match entry {
                    Ok(entry) => entry.file_name().to_string_lossy().into_owned(),
                    Err(error) => format!("<unreadable entry: {}>", error.kind()),
                })
                .collect(),
            Err(error) => vec![format!("<unreadable: {}>", error.kind())],
        };
        names.sort();
        names
    }

    /// A small file's text, at most [`SMALL_FILE_CAP`] bytes, or a marked string when it cannot be read.
    fn read_small(path: &Path) -> String {
        let mut bytes = Vec::new();
        match File::open(path).and_then(|file| file.take(SMALL_FILE_CAP).read_to_end(&mut bytes)) {
            Ok(_) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(error) => format!("<unreadable: {}>", error.kind()),
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // env
    // -----------------------------------------------------------------------------------------------------------

    #[derive(Serialize)]
    struct Env {
        vars: BTreeMap<String, String>,
        cmdline: String,
        uid: u32,
        euid: u32,
        gid: u32,
        egid: u32,
        groups: Vec<u32>,
        cwd: String,
        limits: Vec<String>,
        /// The permission bits of the probe's own binary, setid bits included, so a test can prove a setuid
        /// copy kept its bit in the image and still did not raise `euid` (the root is mounted `nosuid`).
        exe_mode: u32,
    }

    fn env(out: &str) -> ProbeResult {
        let vars = std::env::vars_os()
            .map(|(key, value)| {
                (
                    key.to_string_lossy().into_owned(),
                    value.to_string_lossy().into_owned(),
                )
            })
            .collect();
        let groups = rustix::process::getgroups()?
            .into_iter()
            .map(rustix::process::Gid::as_raw)
            .collect();
        let cwd = std::env::current_dir()?.to_string_lossy().into_owned();
        let limits = read_small(Path::new("/proc/self/limits"))
            .lines()
            .filter(|line| LIMIT_ROWS.iter().any(|row| line.starts_with(row)))
            .map(str::to_owned)
            .collect();
        let exe_mode = fs::metadata("/proc/self/exe")?.mode() & 0o7777;
        write_json(
            out,
            &Env {
                vars,
                cmdline: read_small(Path::new("/proc/cmdline")).trim().to_owned(),
                uid: rustix::process::getuid().as_raw(),
                euid: rustix::process::geteuid().as_raw(),
                gid: rustix::process::getgid().as_raw(),
                egid: rustix::process::getegid().as_raw(),
                groups,
                cwd,
                limits,
                exe_mode,
            },
        )
    }

    // -----------------------------------------------------------------------------------------------------------
    // write-outside
    // -----------------------------------------------------------------------------------------------------------

    #[derive(Serialize)]
    struct WriteOutside {
        outside: Vec<Attempt>,
        reads: Vec<Attempt>,
        inside: Vec<Attempt>,
    }

    /// The file name every write attempt uses.
    const WRITE_NAME: &str = "minos-probe-write";

    fn create_in(dir: &str) -> Attempt {
        let path = Path::new(dir).join(WRITE_NAME);
        let created = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .and_then(|mut file| file.write_all(b"minos-probe\n"));
        Attempt::of(path.to_string_lossy(), &created)
    }

    fn write_outside(out: &str) -> ProbeResult {
        let outside = [
            "/",
            "/etc",
            "/root",
            "/usr",
            "/scratch/judge",
            "/scratch/candidate",
        ]
        .into_iter()
        .map(create_in)
        .collect();
        let reads = ["/proc/1/environ", "/dev/vdb"]
            .into_iter()
            .map(|path| Attempt::of(path, &File::open(path)))
            .collect();
        let inside = ["/tmp", "/out"].into_iter().map(create_in).collect();
        write_json(
            out,
            &WriteOutside {
                outside,
                reads,
                inside,
            },
        )
    }

    // -----------------------------------------------------------------------------------------------------------
    // forge
    // -----------------------------------------------------------------------------------------------------------

    #[derive(Serialize)]
    struct Forge {
        chunks: Vec<Attempt>,
    }

    fn forge(out: &str) -> ProbeResult {
        let token = ChunkToken::from_bytes([0; 32]);
        let payload = b"forged by minos-probe";
        let chunks = ChunkStream::new(&token, payload)
            .enumerate()
            .map(|(index, chunk)| {
                let kind = if index == 0 { "data" } else { "end" };
                let sent = VsockStream::connect_with_cid_port(VMADDR_CID_HOST, OUTPUT_VSOCK_PORT)
                    .and_then(|mut stream| {
                        stream.write_all(&chunk)?;
                        stream.shutdown(Shutdown::Write)
                    });
                Attempt::of(kind, &sent)
            })
            .collect();
        write_json(out, &Forge { chunks })
    }

    // -----------------------------------------------------------------------------------------------------------
    // hang, markers, exit
    // -----------------------------------------------------------------------------------------------------------

    fn hang() -> ! {
        loop {
            std::thread::sleep(Duration::from_hours(1));
        }
    }

    fn marker_write(path: &str) -> ProbeResult {
        fs::write(path, b"minos-probe marker\n")?;
        Ok(())
    }

    #[derive(Serialize)]
    struct MarkerCheck {
        path: String,
        exists: bool,
    }

    fn marker_check(path: &str, out: &str) -> ProbeResult {
        write_json(
            out,
            &MarkerCheck {
                path: path.to_owned(),
                exists: fs::symlink_metadata(path).is_ok(),
            },
        )
    }

    fn exit(code: &str) -> ExitCode {
        code.parse::<u8>().map_or_else(|_| usage(), ExitCode::from)
    }

    fn scorecard() -> ProbeResult {
        fs::write(SCORECARD, SCORECARD_JSON)?;
        Ok(())
    }

    // -----------------------------------------------------------------------------------------------------------
    // output-fill, fill-scratch
    // -----------------------------------------------------------------------------------------------------------

    /// Writes `bytes` bytes of the pattern under `dir`, in files of at most [`FILL_FILE_BYTES`] each.
    fn output_fill(dir: &str, bytes: &str) -> ProbeResult {
        let total: u64 = bytes.parse()?;
        let dir = PathBuf::from(dir);
        fs::create_dir_all(&dir)?;
        let mut buf = vec![0_u8; MIB];
        let mut written = 0_u64;
        let mut index = 0_u32;
        while written < total {
            let mut file = File::create_new(dir.join(format!("fill-{index:04}")))?;
            let mut in_file = 0_u64;
            while in_file < FILL_FILE_BYTES && written < total {
                let want = FILL_FILE_BYTES
                    .saturating_sub(in_file)
                    .min(total.saturating_sub(written));
                let len = usize::try_from(want).map_or(MIB, |want| want.min(MIB));
                let chunk = buf.get_mut(..len).ok_or("chunk out of range")?;
                fill_pattern(written, chunk);
                file.write_all(chunk)?;
                let len = u64::try_from(len)?;
                in_file = in_file.saturating_add(len);
                written = written.saturating_add(len);
            }
            index = index.checked_add(1).ok_or("too many files")?;
        }
        Ok(())
    }

    #[derive(Serialize)]
    struct FillScratch {
        path: String,
        bytes_written: u64,
        os_error: Option<i32>,
    }

    /// Writes one file under the scratch mount until a write fails, records how far it got, and removes it.
    fn fill_scratch(out: &str) -> ProbeResult {
        let mut file = File::create_new(SCRATCH_FILL)?;
        let mut buf = vec![0_u8; MIB];
        let mut written = 0_u64;
        let failure = loop {
            fill_pattern(written, &mut buf);
            match write_counted(&mut file, &buf) {
                (count, None) => written = written.saturating_add(count),
                (count, Some(error)) => {
                    written = written.saturating_add(count);
                    break error;
                }
            }
        };
        drop(file);
        fs::remove_file(SCRATCH_FILL)?;
        write_json(
            out,
            &FillScratch {
                path: SCRATCH_FILL.to_owned(),
                bytes_written: written,
                os_error: failure.raw_os_error(),
            },
        )
    }

    /// Writes `data`, counting every byte accepted, and stops at the first error, so the count is exact even when
    /// the error arrives partway through.
    fn write_counted(out: &mut impl Write, data: &[u8]) -> (u64, Option<io::Error>) {
        let mut rest = data;
        let mut written = 0_u64;
        while !rest.is_empty() {
            match out.write(rest) {
                Ok(0) => return (written, Some(io::Error::from(io::ErrorKind::WriteZero))),
                Ok(count) => {
                    written = written.saturating_add(u64::try_from(count).unwrap_or(u64::MAX));
                    rest = rest.get(count..).unwrap_or_default();
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return (written, Some(error)),
            }
        }
        (written, None)
    }

    // -----------------------------------------------------------------------------------------------------------
    // alloc, spawn
    // -----------------------------------------------------------------------------------------------------------

    #[derive(Serialize)]
    struct Alloc {
        requested_mib: u64,
        allocated_mib: u64,
        error: Option<String>,
    }

    /// Allocates and touches `mib` MiB in 1 MiB steps, stopping at the first allocation that fails.
    fn alloc(out: &str, mib: &str) -> ProbeResult {
        let requested: u64 = mib.parse()?;
        let mut blocks: Vec<Vec<u8>> = Vec::new();
        let mut error = None;
        while u64::try_from(blocks.len())? < requested {
            let mut block = Vec::new();
            if block.try_reserve_exact(MIB).is_err() {
                error = Some("allocation refused".to_owned());
                break;
            }
            // Touch every page, so the memory is committed and not only reserved.
            block.resize(MIB, 0xA5);
            if blocks.try_reserve(1).is_err() {
                error = Some("allocation refused".to_owned());
                break;
            }
            blocks.push(block);
        }
        let allocated = u64::try_from(blocks.len())?;
        drop(blocks);
        write_json(
            out,
            &Alloc {
                requested_mib: requested,
                allocated_mib: allocated,
                error,
            },
        )
    }

    #[derive(Serialize)]
    struct Spawn {
        requested: u64,
        started: u64,
        os_error: Option<i32>,
    }

    /// Starts up to `count` children that sleep (this binary's `hang`), records how many started before one
    /// failed, then kills and reaps them.
    fn spawn(out: &str, count: &str) -> ProbeResult {
        let requested: u64 = count.parse()?;
        let me = std::env::current_exe()?;
        let mut children: Vec<Child> = Vec::new();
        let mut os_error = None;
        while u64::try_from(children.len())? < requested {
            match Command::new(&me)
                .arg("hang")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                Ok(child) => children.push(child),
                Err(error) => {
                    os_error = error.raw_os_error();
                    break;
                }
            }
        }
        let started = u64::try_from(children.len())?;
        for child in &mut children {
            drop(child.kill());
            drop(child.wait());
        }
        write_json(
            out,
            &Spawn {
                requested,
                started,
                os_error,
            },
        )
    }
}
