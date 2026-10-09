# Pitfalls Research

**Domain:** Secure autonomous experiment-loop lab (autoresearch pattern) running adversarial code in libkrun microVMs on macOS arm64, with a model-driven proposer that execs the user's own `claude` CLI
**Researched:** 2026-10-09
**Confidence:** HIGH for the libkrun and Claude Code findings. They were read from primary sources: the libkrun v1.19.6 header and `lib.rs` at the release tag, and code.claude.com docs. MEDIUM for macOS confinement and benchmark noise, which still need a spike. Note: the `classify-confidence` seam rates every web provider LOW. The per-claim tags below say whether a primary source was read directly.

Phase labels in this file match the v1 roadmap areas: **core**, **sandbox+backend**, **guest**, **propose**, **loop+CLI**, **sample project+docs**. "Spike" means build a small experiment before any code depends on the answer.

---

## Critical Pitfalls

### Pitfall 1: libkrun gives the guest network access by default (implicit TSI)

**What goes wrong:**
A VM launched with "no network device" still reaches the network. If no virtio-net device is added, libkrun creates a vsock device by itself and turns on TSI (Transparent Socket Impersonation) with `HIJACK_INET`. The guest's `AF_INET`/`AF_INET6` sockets are then proxied through the VMM process, so outbound TCP/UDP leaves from the host's own network stack. That includes `localhost` services on the user's Mac and the home LAN. If the root is a virtiofs share of `/`, TSI can also hijack `AF_UNIX`, which opens host Unix sockets such as the Docker or ssh-agent socket. (libkrun has this off on macOS today, but it is a reason never to use virtiofs roots.)

**Why it happens:**
libkrun was built for "run a container in a VM and have networking just work." Leaving out `krun_add_net_*` reads like "no network," but the code says: `let enable_tsi = ctx_cfg.vmr.net.list.is_empty() && ctx_cfg.legacy_net_cfg.is_none();`. The README states it plainly: "TSI for AF_INET and AF_INET6 is automatically enabled when no network interface is added to the VM."

**How to avoid:**
- Before adding devices, call `krun_disable_implicit_vsock(ctx)`. If you need vsock for output, follow it with `krun_add_vsock(ctx, 0)`, where `tsi_features = 0` means no hijacking. Never call `krun_set_port_map`.
- Encode this in the sealed `LaunchPlan` → backend translation as the only code path, not as a flag.
- Add an integration test that must fail closed: the guest tries `connect()` to a host listener on 127.0.0.1 and to a public IP, and the test asserts both fail with `ENETUNREACH` or similar. Run it on every backend change and in `minos host doctor`.
- In libkrun 2.0 (main branch), the equivalent is `krun_vsock_device_new(cid, 0 /* TsiFlags::empty */)`. Re-verify on every libkrun upgrade, because the defaults are heuristic.

**Warning signs:** `pip install` or `curl` works inside a "networkless" guest. The guest's `/proc/net` shows no interfaces, yet sockets connect. Host `lsof -p <vmm-pid>` shows TCP connections.

**Phase to address:** sandbox+backend (the first VM launch). The network-denial test is an exit criterion for that phase.

---

### Pitfall 2: Treating libkrun as a security boundary on its own

**What goes wrong:**
The README overclaims ("escape requires a hypervisor zero-day") when, in fact, a bug in libkrun's device emulation lands in a process that runs with the user's full privileges, next to their keys.

**Why it happens:**
libkrun's own security model says: "both the guest and the VMM pertain to the same security context", and asks you to treat "the guest and the VMM as a single entity." Isolation from the rest of the host is supposed to come from host OS mechanisms ("the primary mechanism ... is namespaces", or a dedicated UID/GID on single-user systems). On Linux that means namespaces plus seccomp plus cgroups. macOS has none of those three. The CLAUDE.md "Local backends" section (seccomp filter, cgroup limits) assumes Linux.

**How to avoid:**
- Run the VMM as its own helper process, never inside the CLI process. This is required anyway (see Pitfall 3).
- Confine the helper with what macOS does have. First, a dedicated unprivileged user created by `minos host setup`. Second, a Seatbelt profile (`sandbox-exec`, deny-default, no network, file reads limited to the image and input disks, file writes limited to the scratch disk and the socket dir). Third, RLIMITs. Whether Seatbelt allows `hv_vm_create` under deny-default is **not verified**: spike it before you rely on it.
- `preflight` must report which confinement layers are actually active and refuse to run (or mark the run `WeakIsolation`) when the VMM would run as the invoking user with no profile.
- State the real trust base in docs: Hypervisor.framework, the libkrun device model, and the macOS user/Seatbelt boundary around the VMM process.

**Warning signs:** The VMM helper's `ps -o user` is your login user. `sandbox-exec` is absent from the spawn argv. Docs mention "seccomp" or "cgroups" for the macOS path.

**Phase to address:** sandbox+backend (confinement and preflight). Docs wording: sample project+docs.

---

### Pitfall 3: `krun_start_enter` takes over and then exits the process

**What goes wrong:**
Someone calls libkrun from the CLI or loop process. The first experiment's VM shuts down, and the whole `minos run` process exits with the guest's exit code. The loop never records an outcome. There is also no way to run two VMs, or to enforce a deadline from the same process.

**Why it happens:**
From the header: "This function only returns if an error happens before starting the microVM. Otherwise, the VMM assumes it has full control of the process, and will call to exit() with the workload's exit code." Hypervisor.framework also allows one VM per process (MEDIUM: Apple docs describe `hv_vm_create` as creating "a VM instance for the current process").

**How to avoid:**
- Use one helper process per VM, supervised by the backend. This also settles the `unsafe_code = "forbid"` tension (Pitfall 21), because the FFI lives in the helper and not in our workspace crates.
- The supervisor owns the deadline and kills the helper's process group with `SIGKILL`. The helper never decides when it dies.
- Treat the helper's exit code as untrusted (the guest controls it). The outcome comes from the validated output stream plus the host's own view of whether the deadline passed.

**Warning signs:** A design doc mentions "call `krun_start_enter` on a thread." Tests pass with one VM and hang or exit with two.

**Phase to address:** sandbox+backend.

---

### Pitfall 4: The host environment and terminal leak into the guest

**What goes wrong:**
- `krun_set_exec(..., envp = NULL)` and `krun_set_env(NULL)` "auto-generate an array collecting the variables currently present in the environment". Every `ANTHROPIC_API_KEY`, `AWS_*`, `GITHUB_TOKEN`, and `SSH_AUTH_SOCK` in the user's shell ends up inside the hostile guest. The values travel on the kernel command line, which the guest reads from `/proc/cmdline`.
- `krun_start_enter` "will attempt to take over stdin/stdout ... simulating that the latter has direct control of the terminal." If the helper inherits the user's TTY, guest code writes raw escape sequences to the user's terminal and can read keystrokes.
- `krun_set_console_output(path)` writes guest console output to a file with no size cap, so a guest can fill the host disk.

**How to avoid:**
- Spawn the helper with `Command::env_clear()` and an explicit allowlist (in practice nothing). Always pass an explicit, empty, or fixed `envp` to libkrun. Never pass NULL.
- Spawn with stdin=`/dev/null` and stdout/stderr = pipes. Call `krun_disable_implicit_console` and add only an explicit console, wired to a pipe that the supervisor reads through a capped `Untrusted<Bytes>` reader. Do not use a file sink.
- Test: a guest workload dumps `env` and `/proc/cmdline` into the output, and the test asserts no host variable names appear.

**Warning signs:** The guest's `env` shows `HOME=/Users/...`. Colored output from the guest shows up live in your terminal during `minos run`.

**Phase to address:** sandbox+backend (spawn), guest (assert the empty env at startup and refuse otherwise).

---

### Pitfall 5: Sharing host directories (virtiofs) instead of disk images

**What goes wrong:**
`krun_set_root(path)` is the easy libkrun path, and it exposes a host directory through virtiofs. The README warns that virtiofs "does not provide any protection against the guest attempting to access other directories" and needs host mount-point isolation, which macOS cannot provide. The guest can also exhaust host filesystem resources.

**Why it happens:**
Every libkrun example (krunvm, chroot_vm) uses `krun_set_root`. Building ext4 images on macOS without root takes extra work.

**How to avoid:**
- Attach root, input, and scratch only through `krun_add_disk2(ctx, id, path, KRUN_DISK_FORMAT_RAW, read_only)`. Never use QCOW2 or VMDK (Pitfall 6). Never call `krun_add_virtiofs*`. For a block-device root, `krun_set_root_disk_remount` uses an internal NullFs with no host directory (`lib.rs`: "Use a NullFs (no host directory)"), which is acceptable.
- Avoid host-side filesystem formatting entirely. Make the per-job input disk a raw, padded tar or length-prefixed blob that `minos-guest` reads straight from `/dev/vdb`. The host never runs `mkfs` and never parses a filesystem.
- Give each job a fresh APFS `clonefile` copy of the shared root image. A device-model bug that permits writes then cannot poison later experiments or baseline re-runs. Check the root image hash in `minos check`.
- Use `krun_has_feature(KRUN_FEATURE_BLK)` in preflight. The Homebrew bottle builds with `BLK=1 NET=1 GPU=1 TIMESYNC=1`, but a source build might not include BLK.

**Warning signs:** Any `krun_add_virtiofs` or `krun_set_root` call in the backend. A `--allow-host-dir` debug flag appears.

**Phase to address:** sandbox+backend. guest (reading the input device format).

---

### Pitfall 6: Re-opening a guest-written disk, or letting the format be probed

**What goes wrong:**
The scratch disk is attached as RAW. Later, something (debug tooling, `minos show`, a "reuse build cache" optimization) opens it as QCOW2, or lets the format be probed. A guest that wrote a QCOW2 header into its raw disk now makes libkrun open arbitrary host files as backing files. Mounting a guest-written ext4 image on the host is the classic filesystem-parser attack surface.

**Why it happens:**
The `krun_add_disk2` security note: "a guest has full access to all of a Raw image, and can therefore turn it into a file in an arbitrary format, for example, into a Qcow2 image, referencing and granting a malicious guest access to arbitrary files."

**How to avoid:**
- Hard-code the RAW format in the `LaunchPlan` type. There is no format field at all.
- Delete the scratch disk when the job ends, regardless of outcome. Output leaves only through the vsock tar stream.
- Never cache or reuse build output across jobs. CLAUDE.md already says this.

**Warning signs:** A PR adds `DiskFormat` to `LaunchPlan`. Scratch files survive in `~/.minos` after a run.

**Phase to address:** sandbox+backend.

---

### Pitfall 7: No host-enforced hard deadline, and orphaned VMs after a crash

**What goes wrong:**
- A candidate runs an infinite loop or disables its own timers. libkrun has no timeout, and the guest's clock and `minos-guest` are both under candidate control once candidate code runs.
- `minos run` is killed (Ctrl-C twice, a panic, laptop sleep, `kill -9`). macOS has no `PR_SET_PDEATHSIG`, so the helper VMs keep running forever and hold RAM and CPU. They are invisible to the user.

**How to avoid:**
- The supervisor uses a monotonic host deadline, then `SIGKILL` on the helper's process group. The deadline is derived from the manifest budget plus a fixed boot allowance, and capped by `limits.rs`.
- Before calling `krun_start_enter`, the helper starts a watchdog that exits on parent death. Watch a pipe whose write end only the parent holds: EOF means the parent is gone. Or use kqueue `EVFILT_PROC`/`NOTE_EXIT` on the parent pid. The watchdog also enforces an absolute wall-clock cap of its own, so nothing depends on the parent's cleanup running.
- Write a pidfile per running VM under a minos-owned 0700 dir. `minos run`, `minos status`, and `minos host doctor` sweep stale ones.
- Laptop sleep: base deadlines on `CLOCK_MONOTONIC` (`Instant`, which pauses during sleep on macOS) **and** a wall-clock cap. Decide which one wins and record it.

**Warning signs:** `ps aux | grep krun` shows VMs after `minos` exits. Activity Monitor shows memory you can't account for after a run.

**Phase to address:** sandbox+backend (supervisor and watchdog), loop+CLI (stale sweep, `stop`).

---

### Pitfall 8: Output channel floods and tar-parser attacks

**What goes wrong:**
The guest returns an infinite stream, or a tar header that claims an 8 GB entry, a million entries, `../../.ssh/authorized_keys`, absolute paths, symlink or hardlink entries, device nodes, PAX/GNU long-name headers that disagree with the ustar name, or a PAX size header applied to the wrong entry. The host either runs out of memory or disk, or writes outside its blob dir.

**Why it happens:**
The `tar` crate's `unpack`/`unpack_in` helpers have a history of exactly these bugs. RUSTSEC-2021-0080: a symlink made `unpack` create directories outside the target. RUSTSEC-2026-0067 / CVE-2026-33056: `unpack_in` chmods arbitrary directories by following symlinks, fixed in 0.4.45. GHSA-3pv8-6f4r-ffg2 (May 2026): a PAX header applied to the next entry regardless of type, which desyncs the stream. Sibling crates had the same desync ("TARmageddon" in `async-tar`/`tokio-tar`, CVE-2025-62518 and CVE-2026-53600).

**How to avoid:**
- Never call `unpack`/`unpack_in` on sandbox output. Iterate entries and accept only `EntryType::Regular`. Turn each name into a `RelPath` (no `..`, no absolute paths, no empty components, a length cap, a component-count cap). Reject PAX/GNU extension records, or require them to agree with the ustar header. Stream contents into the content-addressed blob store under the hash, never under the guest-supplied name.
- Apply caps before allocating: total bytes, entry count, per-entry size, path length, and wall time. Cap by counting bytes read from the socket, not by trusting the header's size field. Count bytes written to the blob store. Abort the job as `Outcome::Rejected` when any cap is hit.
- Pin the `tar` crate at or above the fixed versions, and keep `cargo deny` advisories on deny. Consider a small length-prefixed framing format of our own instead of tar for the guest→host channel. It is less to get wrong.
- On macOS there is no host `AF_VSOCK`. libkrun maps a vsock port to a Unix socket path (`krun_add_vsock_port2`). Put that socket in a 0700 directory owned by the supervisor. Otherwise any local process can connect to it and either impersonate the host or inject "output." Keep the path under the 104-byte `sun_path` limit, because long `$TMPDIR` paths under `/var/folders/...` fail.

**Warning signs:** `Archive::unpack` appears anywhere. `Vec::with_capacity(header.size())`. Output is collected into memory before it is checked.

**Phase to address:** sandbox+backend (the `Untrusted<T>` reader). core (`RelPath` and limits). guest (emit only regular files).

---

### Pitfall 9: Everything from the candidate VM is forgeable after candidate code runs

**What goes wrong:**
Design or tests assume `minos-guest`'s messages are trustworthy: phase markers ("build done at t=3.2s"), exit codes, resource stats. Inside the candidate VM, candidate code usually runs as root. It can kill or replace `minos-guest`, write to the vsock channel, and forge any message, including a fake `scorecard.json`, early "done" markers, or a fake log excerpt designed to inject prompts.

**How to avoid:**
- Trust stops at the first instruction of candidate code. After that, the guest→host stream is input to `Untrusted<T>` parsers and nothing more.
- The host measures time from its own clock. For the run phase, record when the host sends "start run" and when it receives the output stream's end. A forged early marker can only hurt, because missing output fails correctness.
- A scorecard comes only from the judge VM's dedicated output channel. A file named `scorecard.json` in candidate artifacts gets no special treatment.
- Defense in depth, which does not count as a boundary: run candidate commands as a non-root uid inside the guest, with a pids limit and rlimits (`krun_set_rlimits`).

**Warning signs:** `BackendInfo` or `Usage` fields filled from guest-reported numbers. Tests that "trust the guest's timing."

**Phase to address:** guest (privilege drop, protocol design), sandbox+backend (host-side timing), core (`Usage` built only from host measurements).

---

### Pitfall 10: Judge integrity broken by candidate-controlled numbers

**What goes wrong:**
- The sample `optimize` project's `run` command writes `/out/result.json` containing `throughput_mb_s`, and the judge simply reads that number. The candidate is then scoring itself. CLAUDE.md's `logparse` sketch has exactly this shape (`bench --out /out/result.json`, objective `throughput_mb_s`).
- A candidate influences the judge's output: NaN, ±inf, `1e308` (finite, but `(x - base) / base` overflows to inf), negative values that flip the sign of a percent improvement, a baseline of `0` (division by zero in `min_improvement = "5%"`), more than 15–17 significant digits, duplicate JSON keys, or control characters in metric names.
- Reward hacking, which is documented in the wild. METR (June 2025) saw o3 monkey-patch the evaluator to always return success, overwrite `==` in a Triton task, and overwrite the timer. Sakana's AI Scientist edited its own code to extend its timeout and relaunched itself recursively.
- Precomputation: `build.rs` or proc macros run candidate code at build time. If benchmark inputs are visible in the source tree, the build can bake in the answers, so "run" takes 0 ms.

**How to avoid:**
- Performance objectives are measured by the host, from outside, in the candidate VM (Pitfall 9). The judge VM checks correctness on held-out inputs and emits the gates. The judge never reads a candidate-reported number as an objective.
- `Metric::try_from(f64)` rejects non-finite values. `Scorecard` parsing uses `deny_unknown_fields`, rejects duplicate keys (serde_json keeps the last one by default, so add an explicit check), caps the metric count, and restricts names to an ASCII identifier set from the manifest. Policy math uses checked or relative arithmetic that is defined for a zero or negative baseline. Reject `min_improvement` as a percent when the baseline magnitude is near 0 (or require an absolute threshold), and test it with proptest.
- Timing inputs are judge data, delivered fresh at run time through the mediated channel and never present at build time. `build_report` keeps the "literal matches visible benchmark input" flag.
- Judge data never goes into the image build context. Build from the packed source tarball (after `[source] exclude`), not from the project directory. A `COPY . .` in the user's Dockerfile must not be able to pull in `judge/`. `minos check` scans image layers for files that match judge-data hashes.
- When the judge has to load candidate artifacts (for example a pickled model, where `torch.load` executes code), that is code execution inside the judge VM, next to judge data. The scorecard is then the exfiltration channel. Keep the scorecard tiny and schema-fixed. Run artifact-executing scoring in a separate sandbox over the mediated channel, as CLAUDE.md requires.

**Warning signs:** The judge's scorecard metric equals a field that the candidate's output also contains. Improvement jumps by orders of magnitude in one generation. A candidate's run phase takes under 1% of the baseline's time.

**Phase to address:** core (`Metric`, `Scorecard`, policy arithmetic), guest/sandbox (judge role), sample project+docs (the sample project's judge must measure from outside and must be a worked example of doing it right).

---

### Pitfall 11: Adaptive overfitting: the judge leaks through the loop itself

**What goes wrong:**
Even with perfect isolation, the loop is an optimizer pointed at the judge. Every scored experiment returns a few bits about held-out data to the proposer, through the ledger and the next prompt. After hundreds of generations, candidates fit the held-out set: the "reusable holdout" or leaderboard-probing problem.

**How to avoid:**
- Split judge data into a **selection** set, which can be fed back, and a **held-out** set that never enters a prompt or the ledger fields the proposer reads. Report divergence between the two (already a report flag in CLAUDE.md, so implement it in v1).
- Quantize what the proposer sees (round to the noise floor). Never give the model per-example results.
- Prompt assembly reads only whitelisted ledger fields. Write a test asserting the held-out metrics never appear in an assembled prompt.

**Warning signs:** Selection metrics keep improving while held-out metrics stay flat.

**Phase to address:** core (ledger fields, report), propose (prompt assembly whitelist).

---

### Pitfall 12: `claude -p` runs project hooks, MCP servers, and `env` from the working directory

**What goes wrong:**
The proposer execs `claude -p` with cwd set to the project, or to a temp dir inside the repo. From the Claude Code docs: "Without `--bare`, a `-p` session runs the hooks in a project's `.claude/settings.json` and connects the servers in its `.mcp.json`, even in a folder you've never trusted. A `-p` session shows no workspace trust dialog." The same table says the project's `env` block and `apiKeyHelper` are "Used" under `-p`. A hostile project (the threat model covers "experiments from other people") ships `.claude/settings.json` with `env: {"ANTHROPIC_BASE_URL": "https://attacker"}` or a `SessionStart` hook. That means code execution on the host plus a credential leak, before any sandbox exists. Project `CLAUDE.md` files (cwd and parent dirs) also inject instructions.

**Why it happens:**
`--bare` is the documented fix, but "bare mode doesn't use your subscription login" ("never reads OAuth credentials or the system keychain"). The subscription path therefore cannot use `--bare`, and the safe flags have to be assembled by hand.

**How to avoid:**
- Set cwd to a fresh, empty, minos-owned temp dir **outside** any repo or project tree (for example under `$TMPDIR/minos-propose-<id>`). Never set cwd to the project. Deliver prompt content on stdin. Do not use `--add-dir`.
- Use a fixed argv and test it as a golden: `-p --output-format json --json-schema <schema> --tools "" --disallowedTools "*" --strict-mcp-config --mcp-config '{"mcpServers":{}}' --setting-sources user --settings '{"disableAllHooks":true}' --disable-slash-commands --no-session-persistence --permission-mode dontAsk --model <m>`. Spike `--restricted` (v2.1.248+: "loads only managed settings and `--settings`", removes command tools). It may replace several of these flags.
- `env_clear()` and then allowlist only `HOME`, `PATH`, `USER`, `LANG`, `TMPDIR`. This strips `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`, `CLAUDE_CODE_USE_*`, `CLAUDE_CODE_SIMPLE`, and any `AWS_*`.
- Resolve `claude` to an absolute path at `minos init` or the first run, record its version, and refuse relative PATH entries.
- Preflight runs `claude auth status`, parses JSON `authMethod`, and requires `claude.ai`. Otherwise it explains that an API key or third-party provider would be billed instead, and stops.
- Even with all of this, the user's own `~/.claude` CLAUDE.md, user hooks, and user MCP servers may still load (`--setting-sources user`). Document it. Run a spike that inspects the `system/init` event (`--output-format stream-json --verbose`) and assert that `tools` and `mcp_servers` are empty and no hooks fired.

**Warning signs:** `Command::new("claude").current_dir(project_dir)`. Stray hook output in the JSON. `system/init` lists tools.

**Phase to address:** propose. Treat the argv and env as security-critical, with golden tests.

---

### Pitfall 13: Prompt injection reaching the model, and the model's output treated as trusted

**What goes wrong:**
Sandbox output (log excerpts, error messages, prior diffs) and surface files go into the prompt. Hostile text in them steers the model into proposing a diff that edits the judge, adds network code, smuggles an encoded blob, or plants instructions for the next generation's prompt. With tools disabled, injection cannot execute anything directly, so the damage arrives through the diff and the report.

**How to avoid:**
- Treat the model as hostile, like the sandbox. Every proposal goes through `Diff::parse` against `writable`. Invalid proposals are dropped, never repaired (already in CLAUDE.md, so enforce it with a test that feeds malicious JSON).
- Parse the model's JSON into closed types with `deny_unknown_fields`, even though `--json-schema` already validated it. Claude Code treats `format` as an annotation only, and the schema is not our validator.
- Sandbox-derived text enters prompts only as `LogExcerpt`: truncated, control characters stripped, inside a clearly delimited block, and capped per experiment and per prompt.
- `--tools ""` alone may leave `EndConversation` (the docs say `""` removes it only when no MCP tools remain) and may not remove Agent or Skill. That is an open spike in PROJECT.md, so keep `--disallowedTools "*"` as belt and braces.

**Warning signs:** Proposals that touch paths outside `writable` (count them; a rising rate means injection or drift). Diffs with base64-looking literals. Diffs adding `std::net` or `std::process` uses.

**Phase to address:** propose (parsing, prompt assembly), core (`Diff::parse`), loop+CLI (report flags).

---

### Pitfall 14: Hostile strings reaching the user's terminal and the review artifact

**What goes wrong:**
Log excerpts, file names, diff contents, model rationales, or metric names carry terminal escape sequences: OSC 52 (write the clipboard), OSC 8 (disguised hyperlinks), title-set/report sequences (historic iTerm2 RCE CVE-2019-9535 came through escape-sequence handling), cursor movement that overwrites earlier report lines, or `\r` that hides content. Trojan Source bidi overrides (CVE-2021-42574) and zero-width characters in added lines make the reviewed diff render differently from what it compiles to. Homoglyph file names (Cyrillic `р` in `parse.rs`) look like an allowed path.

**How to avoid:**
- One renderer type owns all sandbox- or model-originated strings. It strips C0/C1 controls (except `\n` and `\t` in diff bodies), ESC, and DEL, and it renders bidi controls and zero-width characters **visibly** (for example `<U+202E>`), not silently.
- `RelPath` accepts only a conservative ASCII set, or applies NFC plus a confusable rejection. `writable` matching is bytewise, so homoglyph paths are rejected at parse time.
- `build_report` flags any added line containing bidi or zero-width code points.
- Use golden tests with a corpus of escape sequences that must render inert.

**Warning signs:** `println!("{}", log)` anywhere in the CLI. Any code path that prints a `String` from the ledger without going through the renderer type.

**Phase to address:** core (`RelPath`, report flags), loop+CLI (renderer).

---

### Pitfall 15: Diff validation that a different parser can bypass

**What goes wrong:**
`Diff::parse` in core and the patch applier in the guest disagree. Core validates one interpretation (for example, file `src/parse.rs`). The guest's crate applies another: quoted git filenames with octal escapes, `--- a/x\t<timestamp>` headers, `diff --git` extended headers (`rename from`, `new mode 120000` meaning symlink, `GIT binary patch`), `/dev/null` creates and deletes, CRLF, NUL bytes, `\ No newline at end of file`, hunks whose line counts don't match. `diffy` (0.5.x) parses multi-file patches through `PatchSet` with `gitdiff()` or `unidiff()` options, applies hunks by searching for an offset, and can parse binary patches when the `binary` feature is on.

**How to avoid:**
- `Diff::parse` produces a **canonical** re-serialized diff: normalized headers, no extended headers, no timestamps, LF only, exact hunk counts. `ExperimentId` hashes the canonical bytes. The guest receives only canonical bytes. `minos adopt` writes the canonical bytes, never the model's original text, so `git apply` on the user's machine cannot see rename, mode, or symlink headers.
- Core and guest use the same crate version and the same parse options (`unidiff`, `binary` feature off). Add a proptest round-trip: `parse(serialize(d)) == d`, and the guest's application of the canonical form touches exactly `d.paths()`.
- Decide in core and test explicitly: creating new files (allowed only if the path is listed in `writable`?), deleting writable files, empty diffs, size cap, and per-file hunk cap.

**Warning signs:** The guest calls the patch crate on the raw model text. `ExperimentId` is computed from un-normalized text. Two diffs that differ only in whitespace headers get different IDs.

**Phase to address:** core (canonical `Diff`), guest (apply only canonical form).

---

### Pitfall 16: Noisy metrics produce false improvements (winner's curse)

**What goes wrong:**
With `width = 4`, the best of four noisy measurements is biased upward. Greedy selection keeps "improvements" that are noise, and the lineage drifts. The noise comes from macOS-specific sources:
- vCPU threads scheduled on efficiency cores versus performance cores.
- Thermal throttling during long runs.
- Spotlight, Time Machine, and the browser competing.
- Laptop power state (battery versus AC).
- A first-boot cold page cache versus later boots.

**How to avoid:**
- `minos check` measures the baseline N times and stores the noise floor in the ledger. `Manifest`/policy requires `min_improvement` to exceed a multiple of the measured spread, and refuses or warns otherwise.
- Re-measure the winner (and the incumbent, interleaved A/B in the same time window) before `Selected` is recorded. Accept only if the improvement survives. Record repeat measurements as separate `Judged` events. Do not overwrite.
- Record the power source and thermal state in `BackendInfo` where macOS exposes them cheaply (`pmset -g batt`, `NSProcessInfo.thermalState` via a helper). Flag runs that mixed states. Pin speed objectives to one backend class (already in the design).
- Spike: measure the variance of an identical CPU-bound workload across 20 VM boots on the dev machine before choosing the default repeat count.

**Warning signs:** The baseline re-run scores "better" than the baseline. Improvements cluster in the first few generations and then reverse.

**Phase to address:** loop+CLI (`check` noise floor, confirmation runs), core (policy thresholds).

---

### Pitfall 17: Non-deterministic ordering and serialization break hashes, replay, and the cache

**What goes wrong:**
- `HashMap` iteration in prompt assembly, report building, or policy. The report hash changes between runs, `minos adopt --reviewed <hash>` fails, and the prompt-cache prefix misses on every call.
- `serde_json`'s `preserve_order` feature is turned on by **any** crate in the dependency graph (Cargo feature unification). That silently changes `serde_json::Map` ordering, and with it every hash computed over `Value`.
- Floats in hashed structures: `-0.0` versus `0.0`, and printing differences between serializers.
- Prompt assembly includes timestamps, run IDs, absolute paths, or `HashSet`-ordered file lists in the "stable prefix."
- The sweep tier uses `thread_rng`, or a seed derived from time.

**How to avoid:**
- Hash a canonical encoding we control (length-prefixed fields, as in `ExperimentId::of`), never `serde_json::to_vec(&Value)`. Keep `serde_json::Value` out of hashed or persisted paths.
- Add a `cargo tree -e features -i serde_json` check in CI that fails if `preserve_order` or `arbitrary_precision` appears, or use golden vectors, which catch it for free.
- `clippy.toml` `disallowed-types` for `HashMap`/`HashSet` in core (or workspace-wide with explicit expectations), and `disallowed-methods` for `SystemTime::now`, `Instant::now`, `rand::thread_rng`, `std::env::var`, and `std::fs::*` in core. Note that a crate-level `clippy.toml` **replaces** the workspace one rather than merging with it.
- Normalize `-0.0` to `0.0` in `Metric::try_from`.
- Sweep seeds derive from `(RunId, generation, index)` through a domain-separated hash.
- Golden test: assemble the same prompt twice from shuffled inputs and get byte-identical output. Permutation proptests for policy (already planned).

**Warning signs:** A golden-file test that is flaky. The report hash differs on the second `minos report`. Cache-read tokens near zero in `claude` usage output.

**Phase to address:** core (everything hashed), propose (prompt assembly), CI setup.

---

### Pitfall 18: The file-backed ledger gets rewritten, torn, or raced

**What goes wrong:**
- A "fix" for a bad record rewrites history.
- A crash mid-write leaves half a JSONL line.
- Two `minos run` processes on the same project interleave writes.
- `fsync` is skipped, so a power loss drops records that `Selected` decisions depend on.
- The fold silently skips unknown `v` tags or unparseable lines.

**How to avoid:**
- Append-only API. The `Ledger` trait has no update or delete, and the file impl opens with `O_APPEND`.
- Take an exclusive `flock` on a lock file per ledger for the whole run. A second `minos run` fails with a clear message.
- Each record is framed (length prefix plus checksum, or one JSON per line plus a trailing hash). On fold, a torn **final** record is reported and ignored. Corruption anywhere else is a hard error, never a skip.
- `fsync` after each append that precedes an irreversible action (launch, select, report acknowledgement).
- Unknown `v` means refuse to fold, with an "upgrade minos" error.
- Blob writes go to a temp file, then `fsync`, then `rename` to the hash name, then the content hash is verified on read.

**Warning signs:** Code that opens the ledger with `File::create` or `truncate(true)`. A `Ledger::put` signature taking a key that already exists.

**Phase to address:** core (fold semantics), loop+CLI (file store impl). The trait shape belongs in the earliest store work.

---

### Pitfall 19: Spend caps that don't cap

**What goes wrong:**
- The cap is checked after the generation instead of before.
- Model spend from `claude -p` isn't counted.
- The JSON `total_cost_usd` (a client-side **estimate**, per the Claude Code docs) is recorded as if measured.
- Retries and failed calls aren't metered.
- Money becomes floats when `prices.toml` is parsed. The `toml` crate parses `rate = 0.0000166667` as `f64` before our code ever sees it.
- `claude -p` billing on subscriptions changed in 2026. Programmatic use draws on a monthly Agent SDK credit or subscription limits depending on plan (sources disagree; MEDIUM). If "extra usage" is enabled, it can be billed at API rates. An unattended loop can spend real money without anyone seeing it.

**How to avoid:**
- Check spend before each generation **and** before each model call. The per-generation worst case (width × max tokens × price) must fit under the remaining budget or the generation does not start.
- Record token counts (`usage` fields from the JSON result, which are measured) in `Spent` events. Never store `total_cost_usd`. Field names are an open spike in PROJECT.md.
- `prices.toml` rates must be TOML **strings**, parsed with an exact-decimal parser. A numeric literal is a parse error with a helpful message.
- Classify a credit-exhaustion or rate-limit result from `claude` as an infrastructure failure that **stops** the loop and writes an inbox item. Never retry it in a tight loop.
- Docs tell subscription users to check their plan's programmatic-usage terms and the extra-usage setting before long runs.

**Warning signs:** `f64` anywhere near `Usd`. `Spent` events with no token counts. A loop that kept running after `claude` returned an error result.

**Phase to address:** core (usage and money types), propose (usage capture), loop+CLI (cap checks).

---

### Pitfall 20: Overclaiming isolation, and routing other people's subscriptions, in a public repo

**What goes wrong:**
- The README says "secure sandbox," "fully isolated," or "ironclad," and does not mention that libkrun shares the VMM's security context, that macOS has no seccomp or cgroups, or that dev mode is weaker. The first security researcher who reads the code files a well-deserved embarrassing issue.
- Someone wraps Minos as a hosted service, or a multi-user tool, that calls `claude -p` with other users' Pro/Max logins. Anthropic's Agent SDK docs say that, unless previously approved, third-party developers may not "offer claude.ai login or rate limits for their products." The project then looks like it enables a terms violation.

**How to avoid:**
- Write a `SECURITY.md` with the threat model, the trust base (Hypervisor.framework, libkrun device model, macOS user/Seatbelt confinement of the VMM, `minos-guest` not trusted after candidate code runs), what is out of scope, and how to report issues. Use the CLAUDE.md wording: "escape should require a zero-day in the hypervisor or VMM, never only a mistake in our code," not "cannot escape."
- `minos host doctor` and every report print the isolation level actually achieved (`WeakIsolation` if any layer is missing).
- The CLI provider only ever execs the user's own `claude` binary in their own session. It never reads, copies, or proxies credentials, and it never accepts a remote request. The README says, prominently, that shared, hosted, or team use must use an API key or Bedrock.

**Warning signs:** README adjectives without a threat-model link. Feature requests for "server mode" with the CLI provider.

**Phase to address:** sample project+docs. The `doctor` output belongs to sandbox+backend.

---

## Moderate Pitfalls

### Pitfall 21: `unsafe_code = "forbid"` versus libkrun's C ABI produces an escape hatch
**What goes wrong:** Someone adds `#![allow(unsafe_code)]` to `minos-backend-krun`, or moves the crate out of the workspace lints "temporarily," to call `krun_*`. Or they depend on `krun-sys` 1.10.1 (last updated Feb 2025) against an installed libkrun 1.19.6, and the ABI drifts silently. In 1.x, `krun_add_disk2`, `krun_add_vsock`, and `krun_disable_implicit_vsock` are newer than that crate. libkrun 2.0 on `main` changes the API to a handle-based builder.
**Prevention:** Choose before writing backend code, and record the choice in CLAUDE.md. Option (a): the helper is an existing audited binary (`krunkit` uses CLI `--device virtio-blk,...` and `virtio-vsock`, but it is EFI/disk-boot oriented, and it is **unverified** whether it disables implicit TSI). Option (b): a tiny helper binary in a separate crate that is explicitly outside the `forbid` scope, documented as the one FFI boundary, with its own review rules. Either way, the workspace crates stay `forbid`. Pin libkrun by version in `minos host doctor`, and refuse versions whose TSI defaults have not been re-verified. **Do not decide this silently. CLAUDE.md says to stop and ask.**
**Phase:** sandbox+backend (spike first).

### Pitfall 22: The Hypervisor.framework entitlement disappears on every rebuild
**What goes wrong:** The helper binary must be codesigned with `com.apple.security.hypervisor` (libkrun's `hvf-entitlements.plist`; krunvm's Makefile runs `codesign --entitlements ... -s -` after `cargo build`). Every `cargo build` or `cargo test` produces an unsigned binary, `hv_vm_create` fails with `HV_DENIED`, and the error looks like a libkrun bug. `krunkit.entitlements` also sets `com.apple.security.cs.disable-library-validation`, which widens dylib-injection exposure. Do not copy it blindly.
**Prevention:** Add an `xtask` or Makefile step that signs the helper after build. Integration tests call the signed artifact. `minos host doctor` checks `codesign -d --entitlements -` on the helper. Request only the hypervisor entitlement.
**Phase:** sandbox+backend.

### Pitfall 23: Memory and disk caps that are not actually caps
**What goes wrong:** Guest RAM comes from host memory via mmap. Several VMs at 4 GB each on a 16 GB Mac swap the host to death. A sparse scratch file "capped" at 20 GB grows to 20 GB on a nearly full disk. A candidate that writes checkpoints every step (the Sakana case: close to a terabyte) fills it.
**Prevention:** Always call `krun_set_vm_config(ctx, vcpus, ram_mib)` with values from `limits.rs`. Preflight checks free host RAM and disk against `concurrency × (ram + scratch)`. Create scratch files with a fixed size, and check APFS free space before launch. Use concurrency 1 by default for speed objectives.
**Phase:** sandbox+backend, loop+CLI.

### Pitfall 24: Serde types that skip validation
**What goes wrong:**
- `#[derive(Deserialize)]` on a validated type instead of `#[serde(try_from = "Raw...")]`.
- `#[serde(default)]` filling invalid defaults.
- `deny_unknown_fields` silently not working with `#[serde(flatten)]` (a documented serde limitation).
- Internally tagged enums (`#[serde(tag = "v")]`) accepting extra fields.
- TOML duplicate tables.
- Nested types inside a validated struct that deserialize raw.
**Prevention:** Every persisted or parsed type gets a `Raw*` mirror with `deny_unknown_fields` and no `flatten`, plus a `TryFrom` that calls the same constructor. Add a test per type that unknown fields and invalid values are rejected. Add a CI grep: `derive(.*Deserialize` on a type that has a private-field constructor is a review flag.
**Phase:** core.

### Pitfall 25: Clippy lint pressure turns into masked bugs
**What goes wrong:** `indexing_slicing` and `unwrap_used` get "fixed" with `.get(i).copied().unwrap_or_default()`, `unwrap_or(0)`, or `#[allow(clippy::...)]` sprinkled around. A missing metric quietly becomes `0.0` and wins or loses a selection.
**Prevention:** Use `#[expect(lint, reason = "...")]` only, never `#[allow]`, and have CI fail on `allow(clippy::unwrap_used|expect_used|indexing_slicing)`. Treat `unwrap_or_default` on validated domain values as a review flag. Restructure with iterators or pattern matching. A missing value becomes an `Outcome::Rejected`, not a default.
**Phase:** all. Set up the CI gate in core.

### Pitfall 26: `BoxError` and `anyhow` leaking into libraries erase blame
**What goes wrong:** Once a library returns `anyhow::Error`, the loop cannot tell "candidate did something wrong" (an `Outcome`, never retried) from "infrastructure failed" (retried). A candidate that crashes the parser gets retried forever, or a VM launch failure gets recorded as `Crashed` and penalizes a good diff.
**Prevention:** `thiserror` enums in every library crate, with the split built into the type (`enum RunError { Infra(..) }` plus `Outcome` as a value). `anyhow`/`BoxError` only in `minos-cli` `main`. Add a `cargo deny` or `clippy` ban on `anyhow` in non-bin crates.
**Phase:** core (error taxonomy), all.

### Pitfall 27: cargo-deny configured but not actually gating
**What goes wrong:** The defaults are `unknown-registry = "warn"`, `unknown-git = "warn"`, and `required-git-spec = "any"` (cargo-deny docs). `cargo deny check` passes with an unpinned git dependency or an unknown registry. Floating `Cargo.lock` regenerations pull new versions. Banned C crates (`openssl-sys`, `libgit2-sys`) come in transitively.
**Prevention:** In `deny.toml`, set `[sources] unknown-registry = "deny"`, `unknown-git = "deny"`, `required-git-spec = "rev"`, and an explicit `allow-git`. Add `[bans] deny = ["openssl-sys", "openssl", "libgit2-sys", "git2"]`, `wildcards = "deny"`, and advisories on deny with `yanked = "deny"`. Build with `cargo build --locked` in CI. Review build scripts (`cc`-using crates) with an allowlist so C stays limited to the rustls crypto provider.
**Phase:** core (first commit).

### Pitfall 28: Secrets and judge data in the source tarball
**What goes wrong:** The tarball respects `.gitignore`, but the following still get in:
- `.git/`, whose `config` can hold `https://<token>@github.com` remote URLs.
- `.minos/` (prior outputs and the ledger).
- `.env.example` files with real values, tracked `*.pem` files, `.claude/` settings.
- A `judge/` directory the user forgot to list in `[source] exclude`.
The secret scan misses formats it doesn't know.
**Prevention:** Use a hard denylist that the manifest cannot override: `.git/`, `.minos/`, `.claude/`, `*.pem`, `id_*`, `.env*`, `.aws/`, `.ssh/`, plus the judge data path. Fail closed when a file matches the hash of any judge-data file. Run the secret scan with high-signal patterns (cloud keys, private key headers, JWTs, GitHub/Anthropic token prefixes). A hit blocks the upload, and the override needs an explicit per-file allowlist entry. `--allow-dirty` must not disable the scan.
**Phase:** loop+CLI (packing), core (denylist constants).

### Pitfall 29: The environment build phase as an attack vector
**What goes wrong:** A hostile `minos.Dockerfile` `RUN` step has network access (by design) and runs on the user's container engine. BuildKit `--secret` or `--ssh` mounts, or a build context of the whole project dir, give it data it should not have. Base images get pinned by manifest-list digest on one machine and platform digest on another. An x86_64-only image cannot run under HVF (arm64 only).
**Prevention:** Build with a context made from the packed tarball only. Never pass `--secret`, `--ssh`, or host env. Record the resolved linux/arm64 platform digest in `minos.lock`. Run the build inside the container engine's own VM, never on the bare host. The engine choice on macOS (for example Podman or Docker Desktop) is a STACK decision, and its default network mode needs checking.
**Phase:** loop+CLI (`env build`).

---

## Minor Pitfalls

### Pitfall 30: Unix socket path length on macOS
**What goes wrong:** vsock-proxy socket paths under `$TMPDIR` (`/var/folders/xx/yyyy/T/...`) go over the 104-byte `sun_path` limit, and the launch fails with an opaque error.
**Prevention:** Use a short, fixed socket dir (for example `~/.minos/s/<8-char id>`) with mode 0700. Check the length in preflight.

### Pitfall 31: `minos-guest` doing per-run work at startup
**What goes wrong:** It works locally but breaks on Lambda later, because snapshots capture memory and anything created at startup is shared across restores.
**Prevention:** The guest does nothing per-run until it receives a job message. Write a test asserting that guest startup reads no clock, randomness, or job inputs.

### Pitfall 32: PID 1 duties in the guest
**What goes wrong:** If `minos-guest` ends up as PID 1 (libkrun's `init.krun` execs `KRUN_INIT`), zombies from candidate builds accumulate and default signal handling differs. A fork bomb starves the output writer.
**Prevention:** Reap children. Run candidate commands under a process group with rlimits (`krun_set_rlimits` or in-guest `setrlimit`). The host deadline is the backstop.

### Pitfall 33: Unused device code linked into the VMM
**What goes wrong:** The Homebrew `libkrun` is built with `GPU=1` (virglrenderer, libepoxy) and `NET=1`. The code is present in the VMM process even when unused.
**Prevention:** Never add GPU, net, sound, or input devices, and assert this in the `LaunchPlan` → config translation. Consider a `BLK=1`-only source build for the helper, pinned by tag (`stable-1.19.x`).

### Pitfall 34: Model invalid-diff rate silently consuming budget
**What goes wrong:** `single_shot` returns several diffs and most fail `Diff::parse` (wrong paths, broken hunks). Each generation burns tokens and produces nothing.
**Prevention:** Record per-tier invalid-proposal counts in the ledger and show them in `minos status`. The router treats a tier with a high invalid rate as stalled. Put the exact file paths and a diff format example in the stable prompt prefix.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| `krun_set_root(dir)` virtiofs root to get a first VM booting | Boots in an hour, no image tooling | Shared host dir, the TSI AF_UNIX path, a rewrite of input delivery | Only in a throwaway spike that never merges |
| Calling libkrun in-process | No helper protocol | Process exits on VM shutdown; no deadline; FFI in the workspace | Never |
| Trusting guest-reported timing | Simple per-phase numbers | Candidates fake their speed | Never for objectives. Fine as a labelled diagnostic in `show` |
| `claude -p` with cwd = project | Model "sees" the project | Host code execution via hooks/env; credential exfiltration | Never |
| Floats for prices "until later" | Quick `approx. cost` | Rounding drift, golden-file churn, a forced wire-format change | Never (the wire format is an API from the first commit) |
| Measuring the baseline once | Faster `check` | False improvements, wasted generations | Only for projects with no speed objective and a deterministic judge |
| Using `tar::Archive::unpack` on output | One line | Path traversal, symlink, and chmod CVEs | Never |
| `#[allow(clippy::...)]` to land a phase | Green CI | Erodes the lint wall | Never; use `#[expect(..., reason)]` and fix the code |
| Skipping `flock` on the ledger | Less code | Interleaved records from two runs | Never |

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| libkrun networking | Omitting `krun_add_net_*` and assuming no network | `krun_disable_implicit_vsock` + `krun_add_vsock(ctx, 0)`, and test that egress fails |
| libkrun env | `envp = NULL` (inherits the host env) | Explicit fixed env; helper spawned with `env_clear()` |
| libkrun console | Inheriting the TTY, or `krun_set_console_output` to a file | `krun_disable_implicit_console`; explicit console to a capped pipe |
| libkrun disks | `krun_add_disk2` with QCOW2, or format probing | RAW only, hard-coded; per-job clonefile; delete scratch after |
| libkrun lifecycle | `krun_start_enter` in the main process | One signed helper process per VM; host-side SIGKILL deadline |
| Hypervisor.framework | Unsigned `cargo build` output | Post-build `codesign` with only `com.apple.security.hypervisor` |
| `claude -p` | `--bare` for safety (drops the subscription login) or cwd=project (runs hooks) | Empty temp cwd outside the repo, locked-down flags, `env_clear`, `auth status` check |
| `claude -p` output | Trusting `--json-schema` as validation; recording `total_cost_usd` | Re-parse into closed types; record token usage only |
| `diffy` | Applying raw model text in the guest | Apply the canonical re-serialized diff, same crate version as core |
| `tar` crate | `unpack`/`unpack_in` | Iterate entries, regular files only, caps, content-addressed writes |
| cargo-deny | Default `warn` for unknown git/registry | `deny` + `required-git-spec = "rev"` |
| serde_json | `Value` in hashed paths; feature unification turning on `preserve_order` | Canonical length-prefixed hashing; CI feature check |

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Hashing the full root image before every launch | Seconds of overhead per experiment | Hash once at `check`/`env build`; per-job APFS clonefile | Images over ~1 GB × hundreds of experiments |
| Buffering whole output streams in memory | RSS spikes, OOM on large artifacts | Stream to the blob store with byte caps | Artifacts over a few hundred MB (ML checkpoints) |
| Parallel VMs on a laptop | Swap, thermal throttling, noisy metrics | Concurrency 1 for speed objectives; RAM preflight | More than 2 VMs on 16 GB; any parallelism with timing objectives |
| Prompt prefix not stable | Every call pays full input tokens | Deterministic assembly, stable prefix first | Large surfaces × many generations |
| Ledger fold from scratch on every command | `minos status` slows down | Fine at v1 scale; add a cached fold keyed by ledger length later | Tens of thousands of events |

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Implicit TSI left on | Guest reaches localhost and the LAN through the host's network stack | Disable implicit vsock; egress-denial test |
| VMM as the login user, unconfined | A libkrun device bug becomes full user compromise | Dedicated user + Seatbelt; `WeakIsolation` marking |
| Host env passed to the guest | API keys and tokens in a hostile VM | Explicit env; `env_clear` |
| `claude -p` in a hostile project dir | Hooks run on the host; `env.ANTHROPIC_BASE_URL` exfiltrates credentials | Empty cwd outside the repo; `--setting-sources user`; `disableAllHooks` |
| vsock Unix socket in a world-accessible dir | Local processes inject fake output | 0700 supervisor-owned dir |
| Scorecard from candidate artifacts | Candidate sets its own score | Scorecard only from the judge VM channel |
| Adopt writing the model's raw diff | `git apply` honors renames, modes, symlinks | Write the canonical diff only, after hash acknowledgement |
| Escape sequences in logs | Clipboard writes, disguised links, terminal exploits | A single sanitizing renderer |

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| HV_DENIED reported as "VM failed" | Hours lost debugging codesign | `minos host doctor` checks entitlements and explains them |
| Silent `WeakIsolation` | User believes they're protected | Banner in `run`, `status`, and `report` whenever any layer is missing |
| "No improvement" without a noise floor | User can't tell a bad idea from noise | Report every delta against the measured noise floor |
| `claude` credit exhausted mid-run shows as candidate failures | Misleading ledger | Infra error → loop stops → inbox item naming the cause |
| Printing a dollar figure from the `claude` estimate | Breaks the "measured, approx." rule | One `ApproxCost` type, built from tokens × price table |

## "Looks Done But Isn't" Checklist

- [ ] **VM launch:** check whether egress is actually blocked. Run the guest `connect()` test against 127.0.0.1 and a public IP.
- [ ] **VM launch:** check that the guest env is empty and `/proc/cmdline` holds no host values.
- [ ] **VM launch:** check whether the helper's stdin is `/dev/null` and its console is a capped pipe, not the TTY.
- [ ] **Deadline:** kill `minos run` with `-9` mid-experiment, and check that the VM is gone within N seconds.
- [ ] **Output reader:** fuzz it with traversal, symlink, hardlink, PAX desync, oversized header, and infinite-stream archives, and check that each is rejected without OOM.
- [ ] **Diff:** check the canonical round-trip, and that rename/mode/binary/symlink/`/dev/null`/quoted-path inputs are rejected or normalized, with a golden file per case.
- [ ] **Judge:** check that no objective is read from candidate output. Grep the sample judge.
- [ ] **Metric:** proptest NaN, inf, 1e308, -0.0, a zero baseline, and negative baselines through policy.
- [ ] **Proposer:** check that `system/init` shows zero tools and zero MCP servers, and that no hooks fired, from a cwd containing a hostile `.claude/settings.json`.
- [ ] **Proposer:** check that `claude auth status` reports `authMethod: claude.ai` and that the child env holds no `ANTHROPIC_*`.
- [ ] **Report:** check that the hash is stable across two invocations and two machines, and that escape and bidi strings render inert.
- [ ] **Adopt:** check that it refuses a stale or other-experiment hash and writes the canonical patch only.
- [ ] **Ledger:** check that a second concurrent `minos run` is refused and that a torn last line is handled.
- [ ] **Spend:** check that the cap stops a run *before* a generation that would exceed it.
- [ ] **Supply chain:** check that `cargo deny check` fails on an unpinned git dependency (test it once on purpose).
- [ ] **Docs:** check that the README's isolation claims match `host doctor` output and `SECURITY.md`.

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| TSI was on for past runs | MEDIUM | Mark affected ledger runs `WeakIsolation` with a new event (append, never edit). Re-run the egress test. Tell users in release notes |
| Judge data found in an image | HIGH | Rotate the judge data (new judge version). `minos rejudge` the archive. Old scores are suspect |
| Report hash instability shipped | MEDIUM | Bump the report `v` tag. Old acknowledgements stay valid only for their own version. Add golden vectors |
| Float money in persisted records | HIGH | New `v` tag with integer micro-dollars; migration fold. Should not happen if caught in core |
| Ledger corruption mid-file | MEDIUM | Refuse to fold. Offer an export of the valid prefix as a new run; never repair in place |
| Hostile hook executed via `claude -p` | HIGH | Treat as host compromise: rotate credentials, audit. Fix the argv/cwd and add the regression test |

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| 1 Implicit TSI network | sandbox+backend | Guest egress-denial integration test fails closed |
| 2 libkrun not a boundary alone | sandbox+backend (spike Seatbelt + dedicated user) | `host doctor` lists active layers; VMM pid not the login user |
| 3 `krun_start_enter` exits the process | sandbox+backend | Two sequential VMs from one `minos run` both record outcomes |
| 4 Env/TTY leakage | sandbox+backend, guest | Guest `env` and `/proc/cmdline` test; no TTY on the helper |
| 5 virtiofs | sandbox+backend | Grep: no `krun_add_virtiofs`/`krun_set_root`; feature check for BLK |
| 6 Guest-written disk reopened | sandbox+backend | `LaunchPlan` has no format field; scratch deleted after the job |
| 7 Deadline / orphans | sandbox+backend, loop+CLI | `kill -9` test; stale pidfile sweep |
| 8 Output floods / tar | sandbox+backend, core, guest | Fuzz corpus; proptest on `RelPath` |
| 9 Forgeable guest stream | guest, sandbox+backend | Host-only timing in `Usage`; candidate runs non-root |
| 10 Judge numbers | core, sample project+docs | Metric proptests; sample judge measures from outside |
| 11 Adaptive overfitting | core, propose | Held-out never appears in an assembled prompt (test) |
| 12 `claude -p` cwd/hooks/env | propose | Hostile `.claude/settings.json` fixture test; argv golden |
| 13 Prompt injection / model trust | propose, core | Malicious-JSON and out-of-writable proposals dropped |
| 14 Terminal escapes / unicode | core, loop+CLI | Escape corpus golden render; bidi flag in report |
| 15 Diff parser differential | core, guest | Canonical round-trip proptest; same crate in guest |
| 16 Noise / winner's curse | loop+CLI, core | Baseline re-runs inside the noise floor; confirmation runs recorded |
| 17 Ordering / serialization | core, propose | Golden vectors; shuffled-input prompt test; feature CI check |
| 18 Ledger integrity | core, loop+CLI | flock test; torn-line test; no update API |
| 19 Spend caps | core, propose, loop+CLI | Pre-generation cap test; string-only prices |
| 20 Overclaiming / ToS | sample project+docs | `SECURITY.md` exists; README reviewed against `doctor` |
| 21 FFI escape hatch | sandbox+backend (decision first) | Workspace `forbid` intact; helper choice recorded in CLAUDE.md |
| 22 Entitlement | sandbox+backend | `doctor` codesign check |
| 23 Mem/disk caps | sandbox+backend, loop+CLI | Preflight RAM/disk refusal test |
| 24 Serde bypass | core | Per-type unknown-field/invalid-value rejection tests |
| 25 Lint escape | all (CI in core) | CI grep for `allow(clippy::` |
| 26 Error blame | core | No `anyhow` in lib crates (deny rule) |
| 27 cargo-deny | core (first commit) | Deliberate unpinned-git test fails |
| 28 Secrets in tarball | loop+CLI, core | Fixture repo with `.git` token URL and `.env` is blocked |
| 29 Env build | loop+CLI | Build context = tarball; no secret mounts |

## Sources

Primary sources, read directly. The classify-confidence seam rates these LOW because they are web fetches; they are first-party docs or source code:
- libkrun README, security model, networking, and virtio-fs warnings: https://github.com/containers/libkrun
- libkrun v1.19.6 `include/libkrun.h`: `krun_start_enter` exits the process, `krun_set_exec`/`krun_set_env` NULL inherits env, `krun_add_disk2` QCOW2 security note, `krun_disable_implicit_vsock`, `krun_add_vsock`: https://raw.githubusercontent.com/containers/libkrun/v1.19.6/include/libkrun.h
- libkrun v1.19.6 `src/libkrun/src/lib.rs`, the implicit TSI heuristic (`enable_tsi = net.list.is_empty() && legacy_net_cfg.is_none()`) and NullFs for `krun_set_root_disk_remount`: https://raw.githubusercontent.com/containers/libkrun/v1.19.6/src/libkrun/src/lib.rs
- libkrun `hvf-entitlements.plist`; krunvm Makefile codesign step; krunkit entitlements and usage: https://github.com/containers/krunvm, https://github.com/containers/krunkit/blob/main/docs/usage.md
- Homebrew tap formula, built with `BLK=1 NET=1 GPU=1 TIMESYNC=1`, v1.19.6: https://github.com/slp/homebrew-krun/blob/master/Formula/libkrun.rb
- Claude Code CLI reference (`--bare`, `--restricted`, `--setting-sources`, `--tools`, `--disallowedTools`, `--strict-mcp-config`, `--json-schema`, `claude auth status` `authMethod`): https://code.claude.com/docs/en/cli-reference
- Claude Code headless and bare mode (bare mode never reads OAuth; `-p` runs project hooks and `.mcp.json` without trust): https://code.claude.com/docs/en/headless
- Claude Code permissions, "What runs before you trust a folder" (project `env` and `apiKeyHelper` used under `-p`): https://code.claude.com/docs/en/permissions
- Agent SDK overview, the restriction on third parties offering claude.ai login: https://code.claude.com/docs/en/agent-sdk/overview
- cargo-deny sources config defaults: https://embarkstudios.github.io/cargo-deny/checks/sources/cfg.html
- diffy 0.5.2 docs (`PatchSet`, offset-searching apply, `binary` feature): https://docs.rs/diffy/latest/diffy/

Advisories and incidents:
- RUSTSEC-2026-0067 / CVE-2026-33056 (tar `unpack_in` chmod via symlink): https://rustsec.org/advisories/RUSTSEC-2026-0067
- RUSTSEC-2021-0080 (tar symlink directory creation): https://rustsec.org/advisories/RUSTSEC-2021-0080
- GHSA-3pv8-6f4r-ffg2 (tar-rs PAX header desync): https://osv.dev/vulnerability/GHSA-3pv8-6f4r-ffg2
- CVE-2026-53600 (async-tar PAX desync): https://advisories.gitlab.com/cargo/async-tar/CVE-2026-53600/
- METR, "Recent Frontier Models Are Reward Hacking" (evaluator monkey-patching, timer overwrite): https://metr.org/blog/2025-06-05-recent-reward-hacking
- Sakana AI Scientist self-modification incidents and commentary: https://www.niso.org/niso-io/2024/08/sakanas-ai-scientist-creates-stir, https://lesswrong.com/posts/mwSY7Jy3mZzgjGYYq/linkpost-the-ai-scientist-towards-fully-automated-open-ended
- Trojan Source (CVE-2021-42574), bidi overrides in reviewed code. Training knowledge; standard reference.

Secondary or unverified (MEDIUM/LOW):
- `claude -p` billing on subscriptions (2026 Agent SDK credit): https://gigazine.net/gsc_news/en/20260514-anthropic-claude-agent-sdk-credits and https://support.claude.com/en/articles/15036540-use-the-claude-agent-sdk-with-your-claude-plan. The two disagree in detail; verify against the user's plan.
- Whether Seatbelt deny-default allows Hypervisor.framework: no source found; spike required. Trail of Bits Seatbelt guidance: https://trailofbits.com/skills/seatbelt-sandboxer/
- One VM per process under Hypervisor.framework: Apple `hv_vm_create` doc wording ("for the current process"): https://developer.apple.com/documentation/hypervisor/hv_vm_create(_:)
- macOS P-core/E-core scheduling and thermal noise for VM benchmarks: domain knowledge; quantify with a spike.

---
*Pitfalls research for: secure autonomous experiment-loop lab (Minos v1, local macOS arm64)*
*Researched: 2026-10-09*
