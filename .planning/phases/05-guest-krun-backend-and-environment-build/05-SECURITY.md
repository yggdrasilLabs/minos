---
phase: 05
slug: guest-krun-backend-and-environment-build
status: verified
# threats_open = count of OPEN threats at or above workflow.security_block_on severity (the blocking gate)
threats_open: 0
asvs_level: 1
created: 2026-10-19
---

# Phase 05 — Security

> Per-phase security contract: threat register, accepted risks, and audit trail.

Audit scope: the 111 threats declared in the `<threat_model>` blocks of plans 05-01 to 05-22 (register authored at plan time), verified at ASVS level 1 against the code under `crates/` at HEAD `6730383`, after the review fixes CR-01, CR-02, and WR-01 to WR-04 (05-REVIEW-FIX.md). `block_on: high`. Evidence is a `file:line` in `crates/` (plus `Cargo.toml`, `Cargo.lock`, `deny.toml`). A test counts only where the mitigation plan names it. For the process threats (Repudiation of the evidence record itself) the anchor is the test or check in `crates/` plus the verbatim record in `05-EVIDENCE.md`. Plans and summaries are not cited as evidence. No implementation file was modified by this audit.

All paths below are relative to the repository root. Abbreviations: `core` = `crates/minos-core`, `store` = `crates/minos-store`, `sandbox` = `crates/minos-sandbox`, `krun` = `crates/minos-backend-krun`, `vmm` = `crates/minos-vmm`, `guest` = `crates/minos-guest`, `cli` = `crates/minos-cli`.

---

## Trust Boundaries

| Boundary | Description | Data Crossing |
|----------|-------------|---------------|
| guest processes -> host output socket | Any guest process can connect to the output vsock port; only PID 1 holds the per-launch token | Chunked output stream (hostile until reassembled and authenticated) |
| host -> guest input disk | The backend writes the `MINOSJB2` disk; PID 1 validates it as hostile | Guest job, base tarball, diff, judge data (secret), candidate output, the token |
| PID 1 (root) -> candidate (uid 64000) | The privilege boundary inside the VM | Stage argv, rlimits, a four-key environment |
| candidate -> /dev, /proc, vsock, /out | Candidate processes can reach kernel interfaces, the output port, and its output directory | Token (must stay unreadable), output files PID 1 packs as root |
| guest -> helper stderr / console pipe | Guest-influenced bytes reach what the supervisor classifies and shows | VMM log lines, console bytes (hostile) |
| guest -> host status | PID 1's status file returns inside the authenticated stream but is parsed as hostile | `GuestStatus` JSON |
| supervisor -> helper config / argv | The host writes the launch config and the Seatbelt argv; the helper validates the config as hostile | Paths, kernel digest, budgets |
| helper -> host | The entitled helper is the guest's neighbor on the host, confined by Seatbelt and rlimits | File reads, one AF_UNIX connect |
| host probes / host tools -> backend and terminal | codesign, sysctl, nc, cat, ps, docker output is parsed and printed | Tool output (passed through `SafeText`) |
| previous runs -> sweep | Leftover job directories and pid files may be stale or reused | pids, job directories with input disks |
| store path / blob bytes -> backend and launcher | The root image path and blob bytes enter a launch | Root image path, capped blob bytes |
| minos.toml / minos.lock -> parsers | User-editable files gate sizes and which image a run may use | Manifest and lock TOML |
| working tree -> build context -> builder -> packer | The user's files become the builder's context; the builder's tar is hostile input to the packer | Snapshot tarball, rootfs tar |
| verification -> record | The evidence is what the user and the next phase trust | Gate and VM test output |

---

## Threat Register

| Threat ID | Category | Component | Severity | Disposition | Mitigation (evidence) | Status |
|-----------|----------|-----------|----------|-------------|-----------------------|--------|
| T-05-01-01 | Denial of service | `[environment] memory_mib` / `scratch_mib` | medium | mitigate | Range checks and the D-05 budget rule in `Manifest::try_from`: core/src/manifest.rs:1645-1667 (`memory_from`, `ManifestError::MemoryBudget`); bounds core/src/limits.rs:207-226; tests core/tests/manifest.rs:1641-1687 | closed |
| T-05-01-02 | Tampering | `BlobHasher` naming | high | mitigate | Plain SHA-256, no domain: core/src/id.rs:212-231; split-invariance proptest and FIPS "abc" vector core/tests/blob_hasher.rs:10-11,30-52; used by store/src/file/blobs.rs:140 | closed |
| T-05-01-03 | Tampering | Duplicated bounds (core, sandbox, minos-vmm) | medium | mitigate | Sandbox re-exports core's constants sandbox/src/spec.rs:33; vmm restatement vmm/src/config.rs:38-46 pinned by its test vmm/src/config.rs:416-421 and the paired test sandbox/tests/spec.rs:289-313 | closed |
| T-05-01-04 | Information disclosure | Manifest error messages | low | accept | See Accepted Risks Log AR-05-01 | closed |
| T-05-02-01 | Spoofing | Candidate forging an output stream on the vsock port | high | mitigate | 32-byte token in every chunk, constant-time `ct_eq` core/src/chunk.rs:102-118, checked in `parse` core/src/chunk.rs:367-369 (`ChunkFault::Token`); redacted `Debug`, no `Display`/`PartialEq` core/src/chunk.rs:88-93,125-129 | closed |
| T-05-02-02 | Tampering | Gaps, duplicates, oversize, mis-sized chunks, second or forged END | high | mitigate | Distinct `ChunkFault` variants all mapped to `Reason::ChunkProtocol` core/src/chunk.rs:236-327; END digest and totals check the whole payload core/src/chunk.rs:523-555 | closed |
| T-05-02-03 | Denial of service | Connection or byte flooding of the receiver | medium | mitigate | `max_total` and `max_connections = ceil(max_total/4096)+1` checked before buffering; poisoned after first fault core/src/chunk.rs:405-480 | closed |
| T-05-02-04 | Repudiation | Guest that died mid-send blamed as protocol violation | medium | mitigate | `ChunkFault::is_incomplete` (NoEnd) core/src/chunk.rs:298-303; host maps it to `Incomplete` krun/src/chunks.rs:259, classified `Crashed(NoStatusFrame)` krun/src/supervise.rs:720-724 | closed |
| T-05-02-05 | Tampering | Hand-edited `minos.lock` | medium | mitigate | `deny_unknown_fields` core/src/lock.rs:198,315,457; duplicates and non-canonical `base_images` refused core/src/lock.rs:329-340,476; byte cap before parse core/src/lock.rs:36,391-396; golden snapshot plus SHA-256 pin core/tests/lock.rs:63-74, refusal tests :128-263 | closed |
| T-05-03-01 | Elevation of privilege | Widening the Seatbelt profile for the store root | high | mitigate | Only rule added is metadata-only `(allow file-read-metadata (path-ancestors (param "ROOT_IMG")))` krun/src/minos-vmm.sb:33-36; profile pinned by SHA-256 (see T-05-09-01) | closed |
| T-05-03-02 | Tampering | Weakening minos-vmm's console path checks | medium | mitigate | Superseded by 05-07 with a stronger control: the console is no config field at all, the constant `STDOUT_CONSOLE = "/dev/fd/1"` vmm/src/config.rs:49-55; a config with a console object is refused vmm/src/config.rs:551-560; preflight refuses any stdout that is not a FIFO vmm/src/preflight.rs:112,126-147, tested :317-346 | closed |
| T-05-03-03 | Denial of service | Console flood through the pipe | medium | mitigate | Reader takes `cap + 1` bytes, raises `over`, kills the group krun/src/supervise.rs:558-576; classified `Crashed(ConsoleCap)` :705; test `console_cap_kills_group` :1101-1124 | closed |
| T-05-03-04 | Spoofing | Spike token in the job JSON | low | accept | See Accepted Risks Log AR-05-02 | closed |
| T-05-03-05 | Repudiation | An assumed proof result steering later plans | medium | mitigate | Code built on the recorded selections cites them: console pipe vmm/src/config.rs:49-55, ROOT_IMG rule krun/src/minos-vmm.sb:33-36; selections written from recorded runs with verbatim output in 05-EVIDENCE.md "Mechanism selections" (W0-CONSOLE, W0-ROOT, W0-ROOT-DENY, W0-CHUNKS) | closed |
| T-05-04-01 | Tampering | Input disk sections | high | mitigate | Per-section SHA-256 checked after read core/src/guest.rs:1325-1330; BASE bound to `TreeId`, DIFF to `BlobId` by `check_role` core/src/guest.rs:1381-1428 | closed |
| T-05-04-02 | Denial of service | Oversized or malformed disk tables | medium | mitigate | Header and whole table validated (kind, uniqueness, order, cap, canonical offset) before any section byte core/src/guest.rs:1211-1255,1276-1299; zero padding checked :1304-1314 | closed |
| T-05-04-03 | Spoofing | Token delivery | high | mitigate | Token only in the disk header core/src/guest.rs:1149,1291; `RawGuestJob` has no token field and `deny_unknown_fields` core/src/guest.rs:240,409-424; PID 1 unlinks `/dev/vdb` (chmod 0 fallback) guest/src/linux.rs:189-199 | closed |
| T-05-04-04 | Information disclosure | Free text in the status | medium | mitigate | Status types are enums, numbers, `RelPath` only, `deny_unknown_fields` core/src/guest.rs:521-640,829-840; `MAX_STATUS_BYTES` cap before parse core/src/guest.rs:48,770-778 | closed |
| T-05-04-05 | Elevation of privilege | A job that runs stages the role should not | medium | mitigate | Fixed stage list per role core/src/guest.rs:78-83, enforced in `try_from` :436-443; judge job carries no base, diff, or writable set :456-466 | closed |
| T-05-04-06 | Denial of service | Large judge data exhausting PID 1's memory | medium | mitigate | `MAX_JUDGE_DATA_BYTES` 96 MiB core/src/guest.rs:900; const assertions hold each role's caps to `MAX_GUEST_INPUT_BYTES` (half the reserve) :915,935-950; cap refused from the table before any section byte :1229-1235; buffers dropped after unpack guest/src/run.rs:297,306,377,385 | closed |
| T-05-05-01 | Tampering | Symlink or swapped file redirecting the helper | high | mitigate | `verified_path`: `symlink_metadata` on every dir, device and inode matched across checked path, opened handle, and canonical path store/src/file/blobs.rs:85-112; store/src/file/mod.rs:157-215; tests store/tests/file_blobs.rs:297-377 | closed |
| T-05-05-02 | Tampering | Partial blob visible after a crash | medium | mitigate | Temp in `blobs/staging`, `sync_all`, rename, fsync shard and staging store/src/file/blobs.rs:55-66,129-167 | closed |
| T-05-05-03 | Denial of service | Unbounded stream filling the disk | medium | mitigate | `take(max + 1)` and `PutTooLarge` store/src/file/blobs.rs:138,152-156; `NamedTempFile` removed on drop; test store/tests/file_blobs.rs:244-255 | closed |
| T-05-05-04 | Tampering | Blob modified on disk after env build verified it | medium | accept | See Accepted Risks Log AR-05-03 | closed |
| T-05-06-01 | Elevation of privilege | A launch with no Seatbelt layer | high | mitigate | `preflight` refuses every krun plan without `ConfinementLayer::Seatbelt` sandbox/src/preflight.rs:314-317; `KrunBackend::launch` re-checks the host whatever the plan was cleared against krun/src/backend.rs:105-118,128-129 | closed |
| T-05-06-02 | Information disclosure | Judge data handed in as candidate input | high | mitigate | Role-shaped `InputDisk` parts with no judge-data part sandbox/src/spec.rs:128-150; `SandboxSpec::new` refuses a cross-role input :290-296; only `SandboxSpec<Judge>::secrets` attaches judge data :342-352; `JudgeDataIsInput` over every part sandbox/src/preflight.rs:319-326; compile-fail sandbox/tests/ui/fail/candidate_secrets.rs | closed |
| T-05-06-05 | Repudiation | Silent fallback on reduced hardening | medium | mitigate | Layers in `Capabilities` and `BackendInfo` sandbox/src/preflight.rs:221-244, core/src/ledger.rs:370-440; `DedicatedUser` never inserted krun/src/host.rs:386-405, test :1252-1290 | closed |
| T-05-06-06 | Tampering | Root image path with traversal or NUL | medium | mitigate | `VerifiedPath::new` refuses relative, `.`/`..`/empty components, over-long, NUL sandbox/src/blob_source.rs:112-136; minos-vmm re-checks canonical regular files vmm/src/preflight.rs:116-122,177-193 | closed |
| T-05-07-01 | Tampering | Guest-influenced log text forging a watchdog marker | high | mitigate | Every log line prefixed `minos-vmm: log `, non-printable replaced, total capped vmm/src/log.rs:7-32, tests :206-247; markers matched as exact whole lines with control chars replaced, together with the exit code krun/src/supervise.rs:609-621,707-735, test `markers_are_whole_lines` :1048-1066 | closed |
| T-05-07-02 | Elevation of privilege | A writable or shared device reaching the guest | high | mitigate | Two `read_only(true)` raw disks and one `unix_connect` route vmm/src/launch.rs:53-74; v2 config has no scratch or boot field and refuses them vmm/src/config.rs:154-172,528-547; clippy bans on virtio-fs, hijack, remap, format, layers, env, custom console and vsock vmm/clippy.toml; `msb_krun` without `net` Cargo.toml:109 | closed |
| T-05-07-03 | Denial of service | Helper writing large files or core dumps on the host | medium | mitigate | `RLIMIT_CORE` 0, `RLIMIT_FSIZE` 0, `RLIMIT_NOFILE` bounded, set first in `run` before config read vmm/src/main.rs:157-161,190-212; failure exits 66 :53,126. Amended by 05-17: `NOFILE_LIMIT` is 8192, not 1024 (vmm/src/main.rs:72), still a bound | closed |
| T-05-07-04 | Information disclosure | Console bytes on host disk | medium | mitigate | Console is the stdout pipe vmm/src/config.rs:55, preflight refuses a non-pipe stdout vmm/src/preflight.rs:126-147; Seatbelt allows writes only to `/dev/fd/1` krun/src/minos-vmm.sb:16 | closed |
| T-05-07-05 | Repudiation | Drift between helper parser and backend serializer | medium | mitigate | One golden file pinned by SHA-256 on both sides: vmm/src/config.rs:404-407,476; krun/src/launch_config.rs:155-185 (byte-for-byte compare); pin re-computed in this audit and matches `0b1a1208…4a7b` | closed |
| T-05-SC | Tampering | The `log` workspace dependency | high | mitigate | Exact pin `log = "=0.4.34"` Cargo.toml:101; registry source and checksum Cargo.lock:688-691; `cargo deny` gates advisories (yanked deny), licenses, sources (unknown registry and git denied) deny.toml:9-23,94-97 | closed |
| T-05-08-01 | Elevation of privilege | Candidate running as root | high | mitigate | Fixed uid/gid 64000 guest/src/stage.rs:39-42; stage spawned with `.uid/.gid` guest/src/linux.rs:466-469; trampoline refuses PID 1 and euid 0 :298-304; limits only lowered (cur = max) :313-330. Strengthened by WR-01: root remounted `nosuid,nodev` :87-90,203-213, test krun/tests/vm.rs:193 | closed |
| T-05-08-02 | Denial of service | Candidate exhausting memory, processes, files, CPU | high | mitigate | `StageLimits::for_stage` AS = allowance, FSIZE = scratch cap, NPROC 512, NOFILE 4096, CPU from budget guest/src/stage.rs:84-133; CORE 0 set by trampoline guest/src/linux.rs:313-319. Amended by CR-01: CPU = (budget s + 1) x vCPUs, clamped by `MAX_CPU_SECONDS` guest/src/stage.rs:102,121-133 | closed |
| T-05-08-03 | Spoofing | Unauthenticated output | high | mitigate | `ChunkSink` holds the disk-header token and encodes every frame with `ChunkStream::new(&token, …)` guest/src/sink.rs:110-121,165-176; token taken from `InputSections` guest/src/linux.rs:129-140 | closed |
| T-05-08-04 | Tampering | Corrupt or mismatched input sections | medium | mitigate | `DiskJobSource` uses core's `read_input_disk` and `check_role` guest/src/source.rs:242-311; failures are `SetupStep::Input` with the token and (CR-02) the real role guest/src/run.rs:224-243 | closed |
| T-05-08-05 | Information disclosure | Host values in the stage environment | medium | mitigate | `STAGE_ENV` fixed four keys guest/src/stage.rs:73-80; `.env_clear().envs(STAGE_ENV)` guest/src/linux.rs:470-471; `exit_env` asserts the exact map krun/tests/exit.rs:263-352 | closed |
| T-05-09-01 | Tampering | Modified profile file weakening confinement | high | mitigate | `include_str!` krun/src/seatbelt.rs:20; passed with `-p` :119-135; SHA-256 pinned by `profile_is_pinned` :149,207-210 (pin re-computed in this audit: matches `cda08b0f…8490`) | closed |
| T-05-09-02 | Elevation of privilege | Profile parameter unset or relative | high | mitigate | `ProfileParams` refuses relative and `..` paths krun/src/seatbelt.rs:100-115, test :279; profile params equal argv `-D` names :195-205; only network rule names SOCK :213-227, krun/src/minos-vmm.sb:19 | closed |
| T-05-09-03 | Repudiation | Wrong guest binary packed as `/init` | medium | mitigate | `check_guest_elf` (64-bit, LE, `EM_AARCH64`, no `PT_INTERP`) krun/src/rootfs.rs:358-420; called before packing cli/src/env_build.rs:270-276, by the locator cli/src/locate.rs:208, and the host probe krun/src/host.rs:875 | closed |
| T-05-10-01 | Information disclosure | Symlink or hard link in `/out` making root PID 1 pack a root-only file | high | mitigate | Kill-all after every stage guest/src/linux.rs:442,521-547; `symlink_metadata`, one-link regular files, `O_NOFOLLOW`, device and inode match guest/src/pack.rs:181-226, guest/src/nofollow.rs:19-56; any refusal is `Invalid` with no `out/` entries guest/src/pack.rs:120-141 | closed |
| T-05-10-02 | Spoofing | Candidate placing a scorecard in its output | high | mitigate | Only a Judge status adds `.minos/scorecard.json`; a candidate scorecard is `PackError::Role` guest/src/pack.rs:319-342; `into_scorecard` only on judge output sandbox/src/untrusted.rs:273-285; compile-fail sandbox/tests/ui/fail/candidate_scorecard.rs | closed |
| T-05-10-03 | Tampering | Diff applied with fuzz or offset search | high | mitigate | `Diff::parse` against the writable set, `ExperimentId::of(base, diff) == job.exp()`, core strict `apply` guest/src/workspace.rs:323-358; on `Failed` every stage is `Skipped` guest/src/run.rs:324-332,435-457 | closed |
| T-05-10-04 | Elevation of privilege | Archive entries escaping the destination | medium | mitigate | `RelPath` names, regular files only, mode 0644/0755 only, `create_new` guest/src/workspace.rs:183-240; tar unpack methods banned guest/clippy.toml | closed |
| T-05-10-05 | Denial of service | Oversized output or logs | medium | mitigate | Core output caps core/src/limits.rs:152,277-288; `LOG_CAP_BYTES` per log guest/src/pack.rs:62,88-110; scorecard cap guest/src/pack.rs:324-327 | closed |
| T-05-11-01 | Elevation of privilege | Launch on a host whose Seatbelt stopped enforcing | high | mitigate | Offline file and loopback denial probes with controls; inconclusive fails closed as `SeatbeltNotEnforcing` krun/src/host.rs:670-870; layer only when both pass :390-405; preflight refusal sandbox/src/preflight.rs:314-317 | closed |
| T-05-11-02 | Tampering | Tampered or swapped libkrunfw | high | mitigate | Fixed Cellar path and pinned SHA-256 krun/src/host.rs:41-45,337; minos-vmm re-hashes in preflight before `launch::build` (the `dlopen`) at every launch vmm/src/preflight.rs:116-122,219-250, vmm/src/main.rs:162-163 | closed |
| T-05-11-03 | Elevation of privilege | Unsigned or under-entitled helper | medium | mitigate | `codesign --display --entitlements - --xml` checks both keys krun/src/host.rs:50-55,443-497; `EntitlementMissing` text says "run minos host setup" sandbox/src/preflight.rs:74-78 | closed |
| T-05-11-04 | Denial of service / disclosure | Helper and input disk surviving kill -9 | high | mitigate | Owner lock per job dir; sweep kills only a ppid-1 helper with the exact argv, then removes the dir krun/src/jobdir.rs:380-430; watchdog exits 126 on parent loss vmm/src/watchdog.rs:26-38,67-88; `exit_kill9` krun/tests/exit.rs:433-541 | closed |
| T-05-11-05 | Tampering | PID reuse making the sweep kill an unrelated process | medium | mitigate | `ps` ppid and exact command compared before any kill; EPERM and unexplained states are `Unverified` krun/src/jobdir.rs:408-428 | closed |
| T-05-11-06 | Tampering | Symlinked or shared job root | medium | mitigate | `JobRoot::open_at` refuses a symlink, another owner, a mode other than 0700 krun/src/jobdir.rs:157-190 | closed |
| T-05-11-07 | Information disclosure | Host tool output reaching the terminal | low | mitigate | Probe details through `SafeText` krun/src/host.rs:13,201-239; tools run with `env_clear` and a fixed `PATH` krun/src/host.rs:951-960 | closed |
| T-05-12-01 | Spoofing | Candidate reading the token from `/dev/vdb` or PID 1's memory | high | mitigate | Disk read before any stage, then unlink (chmod 0 fallback) guest/src/linux.rs:126-128,189-199; `/proc` `hidepid=invisible` :80,115; stage uid 64000 :466-469; `exit_forge` and `writes_outside_scratch_fail` assert `/dev/vdb` and `/proc/1/environ` unreadable krun/tests/exit.rs:551-581, krun/tests/vm.rs:244-279 | closed |
| T-05-12-02 | Tampering | Candidate processes racing the output packer | high | mitigate | `kill(-1, SIGKILL)` (`kill_process_group(Pid::INIT)`) and reap until ECHILD after every stage guest/src/linux.rs:442,521-547; packer never follows links (T-05-10-01) | closed |
| T-05-12-03 | Elevation of privilege | Candidate as root or with supplementary groups | high | mitigate | `Command::uid/gid` from root PID 1 (std drops groups) guest/src/linux.rs:466-469; trampoline refuses PID 1 and euid 0 :298-304; `exit_env` sees `groups: []` (05-EVIDENCE.md Final gate) | closed |
| T-05-12-04 | Denial of service | Fork bombs, memory, CPU, file-size abuse | medium | mitigate | Trampoline sets NPROC, AS, CPU, FSIZE, NOFILE, CORE and exits 126 on any failure guest/src/linux.rs:313-330; PID 1 stage deadline `wait_stage` :485-509; kill-all after every stage :442 | closed |
| T-05-12-05 | Information disclosure | Host values in the candidate environment | medium | mitigate | `env_clear` then `STAGE_ENV` guest/src/linux.rs:470-471; `exit_env` asserts env map and exact `/proc/cmdline` tokens krun/tests/exit.rs:263-352 | closed |
| T-05-12-06 | Tampering | Writes outside scratch | medium | mitigate | Read-only SquashFS root vmm/src/launch.rs:69, remounted ro guest/src/linux.rs:87-90; tmpfs `/scratch` nosuid,nodev :83,217-228 with `/tmp` and `/out` bound from it :378-385; `writes_outside_scratch_fail` krun/tests/vm.rs:244-279 | closed |
| T-05-12-07 | Repudiation | Probe binary shipped in user images | low | mitigate | `env build` packs only the checked guest bytes as `/init` cli/src/env_build.rs:270-283; no `minos-probe` reference in cli or backend sources; probe documented as a fixture guest/src/bin/minos-probe.rs:1-7; placed only by the harness krun/tests/harness/mod.rs:66 | closed |
| T-05-13-01 | Spoofing | Candidate forging the output stream | high | mitigate | Token from `/dev/urandom` (character device checked) krun/src/input_disk.rs:37-54; written only into the disk header and the receiver krun/src/backend.rs:137-157; wrong token is a fault that raises the flag krun/src/chunks.rs:224,315-320, kills the VM and classifies `ChunkProtocol` krun/src/supervise.rs:536-537,706 | closed |
| T-05-13-02 | Information disclosure | Token leaking through logs, config, argv, blob | high | mitigate | Token flows only to `write_input_disk` and `ChunkReceiver::spawn` krun/src/backend.rs:137-157; `LaunchConfig` has no token field krun/src/launch_config.rs:20-31; `ChunkToken` redacted `Debug`, no `Display` core/src/chunk.rs:88-93,125-129 | closed |
| T-05-13-03 | Tampering | Runner bug pairing the wrong job with a plan | high | mitigate | `job_matches` recomputes `LaunchKey` and checks role, base, diff, every stage budget, memory before any write krun/src/input_disk.rs:163-182,196-230 | closed |
| T-05-13-04 | Denial of service | Connection floods, empty or trickling connections | medium | mitigate | Per-connection `MAX_CHUNK_BYTES + 1` read cap and 5 s total deadline krun/src/chunks.rs:45-54,350-396; reassembler caps (T-05-02-03); first fault kills the VM krun/src/supervise.rs:536-537. WR-04 moved a VMM-side read error to the host side; caps unchanged | closed |
| T-05-13-05 | Tampering | Overwriting or reusing an input disk | medium | mitigate | 0600 temp file in the 0700 job dir, synced, `persist_noclobber` krun/src/input_disk.rs:264-285; job dir is a `TempDir` deleted on drop krun/src/jobdir.rs:201-227 | closed |
| T-05-14-01 | Tampering | Packing the macOS guest build as `/init` | medium | mitigate | Locator accepts a guest only after `check_guest_elf`, rejected candidates keep their reason cli/src/locate.rs:161-173,208-214, test :321-330 | closed |
| T-05-14-02 | Elevation of privilege | Signing with extra entitlements | medium | mitigate | Committed plist embedded with `include_bytes!` cli/src/host.rs:44; byte equality test :971-977; both keys verified after signing :99-139 | closed |
| T-05-14-03 | Spoofing | Environment variables redirecting binary or kernel lookup | medium | mitigate | `locate` reads no environment variable (doc cli/src/locate.rs:105; no `env::var` in the file); `MINOS_*` read only inside an `#[ignore]` test krun/src/host.rs:1359-1364 and the ignored VM tests | closed |
| T-05-14-04 | Information disclosure | Terminal escapes in tool output | low | mitigate | `SafeText` for every probe and tool string cli/src/host.rs:15,302-350,597-598,649; errors cli/src/main.rs:49 | closed |
| T-05-14-05 | Tampering | A blob path that is a symlink | medium | mitigate | `FileBlobSource::root_image` uses `FileStore::verified_path` then `VerifiedPath::new` cli/src/blob_source.rs:45-55 | closed |
| T-05-15-01 | Tampering | Guest text forging a watchdog marker | high | mitigate | Same controls as T-05-07-01: krun/src/supervise.rs:609-621,707-735; vmm/src/log.rs:7-32; forged-prefix and embedded-marker tests krun/src/supervise.rs:1048-1066 | closed |
| T-05-15-02 | Denial of service | A VM outliving its plan | high | mitigate | Host deadline = max duration + 30 s krun/src/supervise.rs:48, krun/src/backend.rs:176-182; group kill in `watch` krun/src/supervise.rs:519-552; helper watchdog vmm/src/watchdog.rs:67-75; bounded drop kill (WR-03) krun/src/supervise.rs:246-284,450-457; sweep (T-05-11-04) | closed |
| T-05-15-03 | Information disclosure | Host environment reaching helper or guest | high | mitigate | `env_clear`, stdin null on spawn krun/src/supervise.rs:330-350; test with `/usr/bin/env` :1212-1217; `exit_env` krun/tests/exit.rs:263-352 | closed |
| T-05-15-04 | Elevation of privilege | A launch without Seatbelt | high | mitigate | Every spawn is `seatbelt::argv` krun/src/backend.rs:175; host check krun/src/backend.rs:105-118; preflight refusal sandbox/src/preflight.rs:314-317 | closed |
| T-05-15-05 | Denial of service | Console flooding the host | medium | mitigate | Pipe branch only (the file branch was removed): `cap + 1` read and group kill krun/src/supervise.rs:558-576 | closed |
| T-05-15-06 | Repudiation | Loosening the EPERM check without proof | medium | mitigate | EPERM accepted only when `waitid(WNOWAIT)` proves the unreaped helper exited krun/src/supervise.rs:197-212,287-300; sweep still treats EPERM as `Unverified` krun/src/jobdir.rs:423-426; 20-run and 5-round proof verbatim in 05-EVIDENCE.md "05-15 EPERM on a zombie-only process group" | closed |
| T-05-15-07 | Information disclosure | Input disk with judge data left on disk | medium | mitigate | `KrunRunning` drops the `JobDir` (a `TempDir`) on wait and on drop krun/src/backend.rs:196-206,247-251, krun/src/jobdir.rs:201-227; sweep removes orphans krun/src/jobdir.rs:380-445 | closed |
| T-05-16-01 | Information disclosure | Judge data or secrets baked into the root image | high | mitigate | Context is exactly the snapshot tarball (exclusions, denylist, secret scan, clean check) cli/src/env_build.rs:251-258,279-281; fake builder records contexts cli/tests/env_build.rs:85-150,302-328,402-419,422 | closed |
| T-05-16-02 | Tampering | A Dockerfile replacing `/init` | high | mitigate | Packer removes any image `/init` and injects the trusted guest after the parse krun/src/rootfs.rs:302-303,829-844; `check_guest_elf` before packing cli/src/env_build.rs:270-276 | closed |
| T-05-16-03 | Tampering | Hostile rootfs tar from the builder | high | mitigate | `PackLimits` (1 GiB tar, entries, entry bytes, path bytes) and path rules krun/src/rootfs.rs:35-69,287-305; tar unpack banned krun/clippy.toml; no unpack call in krun or cli sources | closed |
| T-05-16-04 | Repudiation | Changed image silently replacing the locked one | medium | mitigate | `LockMismatch` naming both digests without `--update` cli/src/env_build.rs:215-221,316-320; atomic synced lock rewrite :419-430; lineage half recorded as deferred to Phase 7 :37; `BackendInfo.root_image` core/src/ledger.rs:388-440; test cli/tests/env_build.rs:375 | closed |
| T-05-16-05 | Spoofing | Builder inheriting the user's environment | medium | mitigate | `env_clear` then `HOME` and a fixed `PATH` only, argv array cli/src/builder.rs:186-210,411-421; test :559-567 | closed |
| T-05-16-06 | Tampering | Base image by tag instead of digest | medium | mitigate | `base_images` refuses tags and missing digests, unknown instructions, variables core/src/image.rs:60-64,143-181,319-349; called by env build cli/src/env_build.rs:267; digests recorded in the lock; test cli/tests/env_build.rs:487 | closed |
| T-05-16-07 | Denial of service | Unbounded build time or output | low | mitigate | `BUILD_DEADLINE` 30 min cli/src/builder.rs:42,463; 1 GiB tar cap krun/src/rootfs.rs:62-68; `put_reader` cap cli/src/env_build.rs:296-299 | closed |
| T-05-17-01 | Tampering | Regression letting the candidate write outside scratch or read the input disk | high | mitigate | `writes_outside_scratch_fail` asserts six outside targets fail with an OS error and `/proc/1/environ`, `/dev/vdb` unreadable krun/tests/vm.rs:244-279 | closed |
| T-05-17-02 | Information disclosure | Build output leaking between experiments | high | mitigate | `two_sequential_experiments_share_no_build_output` krun/tests/vm.rs:349-378 | closed |
| T-05-17-03 | Spoofing | Candidate delivering a scorecard | high | mitigate | `candidate_output_never_carries_a_scorecard` krun/tests/vm.rs:449-486; `into_scorecard` only on judge output sandbox/src/untrusted.rs:273-285 | closed |
| T-05-17-04 | Denial of service | Budgets not enforced in the VM | medium | mitigate | `over_budget_candidates_are_stopped_in_the_vm` (AS, NPROC, scratch) krun/tests/vm.rs:281-346; `stage_timeout_is_reported_by_the_guest` :489-515 | closed |
| T-05-17-05 | Repudiation | Tests weakened to pass | medium | mitigate | The named assertions are strict in code (exact env map and cmdline tokens krun/tests/exit.rs:304-329; every target must fail with an OS error krun/tests/vm.rs:255-277); review item IN-06's suggested loosening was not applied (05-REVIEW-FIX.md "Out of scope") | closed |
| T-05-17-06 | Denial of service | Default `RLIMIT_AS` too tight for real toolchains | low | accept | See Accepted Risks Log AR-05-04 | closed |
| T-05-18-01 | Information disclosure | Judge data or secrets in the real build context | high | mitigate | Docker reproducibility test asserts every recorded context is clean cli/tests/env_build.rs:548-590 (with `assert_clean_context` :302-328); context is the snapshot only cli/src/env_build.rs:279-281 | closed |
| T-05-18-02 | Tampering | Switching to a host-directory context | high | mitigate | Build argv ends in `-` (tar context on stdin) cli/src/builder.rs:386-403, pinned in test `builder_docker_argv` :521-540; A2 fallback not needed (05-EVIDENCE.md "05-18 env build") | closed |
| T-05-18-03 | Repudiation | Claiming tar reproducibility that does not hold | medium | mitigate | The test asserts only the image digest cli/tests/env_build.rs:541-545,570; recorded runs in 05-EVIDENCE.md "05-18 env build", "What the equal digests do and do not prove" | closed |
| T-05-18-04 | Tampering | Fixture edits inside the repository | low | mitigate | Test fixture project built in a `tempfile::tempdir()` cli/tests/env_build.rs:232-233; CLI runs on a scratchpad copy (05-EVIDENCE.md:1193-1194); `git status` shows no change under `crates/` | closed |
| T-05-19-01 | Information disclosure / Elevation | Guest egress through TSI or a network device | high | mitigate | `exit_egress`: every connect fails with an OS error, host canaries untouched, interfaces empty or `lo`, no forbidden virtio device krun/tests/exit.rs:179-247 | closed |
| T-05-19-02 | Information disclosure | Host environment or identity in the guest | high | mitigate | `exit_env` with a random canary, host `HOME` and `USER`, exact env keys, exact cmdline tokens krun/tests/exit.rs:263-352 | closed |
| T-05-19-03 | Denial of service | VMs surviving kill -9 of the launcher | high | mitigate | `exit_kill9`: helper gone within the bound, no process names the job, sweep removes the dir krun/tests/exit.rs:433-541 | closed |
| T-05-19-04 | Spoofing | Candidate forging the output stream or reading the token | high | mitigate | `exit_forge`: `/dev/vdb` and `/proc/1/environ` unopenable, guessed-token stream is `Ended::ChunkProtocol`, genuine judge stream accepted as control krun/tests/exit.rs:551-604 | closed |
| T-05-19-05 | Repudiation | An accepted host-derived boot token | low | accept | See Accepted Risks Log AR-05-05; the allowance in code is exactly one all-digit `KRUN_BOOT_START_NS=` token krun/tests/exit.rs:249-254,306-329 | closed |
| T-05-20-01 | Repudiation | Marking the phase complete without the exit tests | high | mitigate | The three tests exist as real ignored integration tests krun/tests/exit.rs:181,265,435; 05-EVIDENCE.md "## Final gate" records `exit_egress ... ok`, `exit_env ... ok`, `exit_kill9 ... ok` (4 passed) on the final build before REQUIREMENTS.md KRUN-03, KRUN-05, GUEST-05 were checked; re-run after the review fixes (05-REVIEW-FIX.md "Verification record" step 9: 4 passed) | closed |
| T-05-20-02 | Tampering | Stale signed helper or guest skewing VM results | medium | mitigate | The harness refuses a guest or probe that is not a static aarch64 ELF krun/tests/harness/mod.rs:248-256 and launches only through preflight against probed capabilities (unsigned helper refused, T-05-11-03); the build, test `--no-run`, sign, entitlement check, then VM run order is recorded verbatim in 05-EVIDENCE.md "Build, sign, guest" | closed |
| T-05-20-03 | Repudiation | Weakened assertions to reach green | medium | mitigate | Same anchor as T-05-17-05; 05-20 recorded evidence-only deviations with no code change | closed |
| T-05-20-04 | Tampering | A lint allow slipping in | low | mitigate | C-02 `grep -rnE '#!?\[allow\(' crates --include='*.rs'` re-run by this audit: no output (exit 1) | closed |
| T-05-21-01 | Tampering | Drift between helper parser and backend serializer | medium | mitigate | `launch_config_matches_golden` compares byte for byte and pins the SHA-256 krun/src/launch_config.rs:155-185 | closed |
| T-05-21-02 | Elevation of privilege | Writable scratch disk or initramfs path surviving in the backend | medium | mitigate | `Disks` holds only `root` and `input` krun/src/launch_config.rs:92-95; serializer test asserts no `console`, `boot`, `scratch`, `initramfs` key :205-220; no scratch or initramfs reference in supervise, jobdir, or backend; minos-vmm v2 refuses both vmm/src/config.rs:528-547 | closed |
| T-05-21-03 | Repudiation | Losing the spike evidence with the harness | low | accept | See Accepted Risks Log AR-05-06 | closed |
| T-05-22-01 | Tampering | A forged or wrong-role status file | high | mitigate | `take_status`: `MAX_STATUS_BYTES` cap, `deny_unknown_fields` parse, `WrongRole` for a role other than R sandbox/src/untrusted.rs:205-228, core/src/guest.rs:829-840. CR-02 narrowed the role check: a `setup: failed {input}` status is accepted with either role (it is an infrastructure failure by its setup field; still token-authenticated, capped, strictly parsed); `WrongRole` still raised for every other step, tests sandbox/tests/untrusted.rs:478+ | closed |
| T-05-22-02 | Information disclosure | Console escape sequences reaching a terminal | medium | mitigate | Console leaves `Finished` only through `console_excerpt` (a `LogExcerpt`) sandbox/src/finished.rs:129-137; no other console accessor | closed |
| T-05-22-03 | Repudiation | Chunk-protocol break blamed on infrastructure and retried forever | medium | mitigate | Phase 5 half: `Ended::ChunkProtocol` is a value, never an `InfraError` sandbox/src/finished.rs:58-61; chunk faults classify to it krun/src/supervise.rs:706,717-719; stream discarded sandbox/src/finished.rs:95-99; `Reason::ChunkProtocol` core/src/outcome.rs:108. The runner half is Phase 7 (crates/minos-runner is a stub); see Carried-forward obligations | closed |
| T-05-22-04 | Elevation of privilege | One clearance producing two results | medium | mitigate | `Finished::new` consumes `Cleared<R>` sandbox/src/finished.rs:88-108; compile-fail sandbox/tests/ui/fail/finished_twice.rs with one E0382 in finished_twice.stderr | closed |

*Status: open · closed · open — below high threshold (non-blocking)*
*Severity: critical > high > medium > low — only open threats at or above workflow.security_block_on (high) count toward threats_open*
*Disposition: mitigate (implementation required) · accept (documented risk) · transfer (third-party)*

---

## Accepted Risks Log

| Risk ID | Threat Ref | Rationale | Accepted By | Date |
|---------|------------|-----------|-------------|------|
| AR-05-01 | T-05-01-04 | Manifest errors carry field paths and numbers only, never manifest text (e.g. `ManifestError::MemoryBudget { memory_mib, scratch_mib }` core/src/manifest.rs:1583-1586), so the residual disclosure is of the user's own values to the user. | Plan 05-01 threat model (user-approved plan) | 2026-10-19 |
| AR-05-02 | T-05-03-04 | The spike-era token in the job JSON was a test-only step removed with the spike protocol in 05-12; production tokens live only in the root-only input disk header. Verified: `RawGuestJob` has no token field and refuses unknown keys core/src/guest.rs:240,409-424. | Plan 05-03 threat model | 2026-10-19 |
| AR-05-03 | T-05-05-04 | D-14: no per-launch rehash of a multi-hundred-MB root image. The store root is 0700 and user-owned (store/src/file/mod.rs:71-80,174), `verified_path` still enforces the symlink and inode checks, and `Blobs::get` rehashes for every other reader. The attacker in scope is the guest, which never writes the store. | User decision 05-CONTEXT D-14 | 2026-10-19 |
| AR-05-04 | T-05-17-06 | `RLIMIT_AS` at the default 768 MiB allowance may break a real toolchain build (Go, JVM, V8, sanitizers); not measurable with the probe fixtures. Defaults are manifest-tunable (D-06), and the question is recorded in CLAUDE.md Open questions with Phase 8's sample project as owner. A too-tight limit fails honest candidates; it does not weaken isolation. | Plan 05-17 threat model; CLAUDE.md Open questions | 2026-10-19 |
| AR-05-05 | T-05-19-05 | `KRUN_BOOT_START_NS=<digits>` in the guest `/proc/cmdline` is written unconditionally by msb_krun 0.1.41; it reveals the host clock at boot, which the guest clock also reveals. Allowed as exactly one all-digit token, every other token matched exactly (krun/tests/exit.rs:249-254,306-329); PID 1 clears its environment before any stage. Removal path (vendor the one `msb_krun` crate) recorded in CLAUDE.md Open questions. | User, at the 05-19 checkpoint (recorded in CLAUDE.md, REQUIREMENTS.md GUEST-05) | 2026-10-19 |
| AR-05-06 | T-05-21-03 | Retiring the Phase 2 spike harness and the Wave 0 proofs loses no evidence: their output stays verbatim in 02-SPIKE-EVIDENCE.md and 05-EVIDENCE.md, and the permanent successors are `krun/tests/exit.rs` and `krun/tests/vm.rs`. | Plan 05-21 threat model | 2026-10-19 |

*Accepted risks do not resurface in future audit runs.*

---

## Carried-forward obligations (not open threats)

These are halves of Phase 5 mitigations that the plans scoped to a later phase. Each Phase 5 half is closed above; the later phase must implement the rest.

| Ref | Obligation | Owner |
|-----|------------|-------|
| T-05-22-03 | The runner records `Ended::ChunkProtocol` as `Rejected(Reason::ChunkProtocol)` and never retries it. | Phase 7 (`minos-runner`) |
| T-05-22-01 (CR-02) | The runner maps every `SetupStatus::Failed` to an infrastructure failure before it reads any other status field, since an input-step failure is accepted with either role. | Phase 7 |
| T-05-16-04 | D-10 lineage half: the lock's digest feeds the manifest image reference and hence `ManifestId`; `Lock::check` before a run. | Phase 7 (`minos run`) |
| T-05-04-06 | `minos judge push` refuses judge data over `MAX_JUDGE_DATA_BYTES` rather than truncating it. | Phase 7 |

---

## Threat Flags from SUMMARY.md

Every plan SUMMARY's `## Threat Flags` section reports "None" and maps its new surface to existing threat IDs (05-05, 05-06, 05-07, 05-09 to 05-17, 05-19 to 05-21). All mappings were checked against the register above. One informational note: 05-14 says `builder_status` reads the user's `PATH` and `HOME` to find and run docker; this maps to T-05-16-05 (the builder runs with a cleared environment, cli/src/builder.rs:196-210) and never selects a launch binary (T-05-14-03). No unregistered flags.

---

## Review fixes that touched mitigations

| Fix | Threats affected | Effect |
|-----|------------------|--------|
| CR-01 | T-05-08-02, T-05-12-04 | `RLIMIT_CPU` scales by vCPU count, still capped by `MAX_CPU_SECONDS` |
| CR-02 | T-05-08-04, T-05-22-01 | Input failure carries the real role; host accepts an input-step failure with either role |
| WR-01 | T-05-08-01, T-05-12-03, T-05-12-06 | Root remounted `nosuid,nodev` before any per-run work; failure prevents any stage |
| WR-02 | T-05-04-06 (adjacent) | Full scratch reported as `SetupStep::ScratchFull`, still role-checked |
| WR-03 | T-05-15-02 | Drop kill and reap bounded by `KILL_WAIT` |
| WR-04 | T-05-13-04 | VMM-side socket error is a host error, not a candidate chunk fault; caps unchanged |

---

## Security Audit Trail

| Audit Date | Threats Total | Closed | Open | Run By |
|------------|---------------|--------|------|--------|
| 2026-10-19 | 111 | 111 (105 mitigate verified in code, 6 accepted and logged) | 0 | gsd-security-auditor |

---

## Sign-Off

- [x] All threats have a disposition (mitigate / accept / transfer)
- [x] Accepted risks documented in Accepted Risks Log
- [x] `threats_open: 0` confirmed
- [x] `status: verified` set in frontmatter

**Approval:** verified 2026-10-19
