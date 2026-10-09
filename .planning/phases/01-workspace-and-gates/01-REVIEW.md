---
phase: 01-workspace-and-gates
reviewed: 2026-10-09T22:41:06Z
depth: standard
files_reviewed: 28
files_reviewed_list:
  - .cargo/config.toml
  - .gitignore
  - Cargo.toml
  - CLAUDE.md
  - deny.toml
  - rust-toolchain.toml
  - rustfmt.toml
  - crates/minos-core/Cargo.toml
  - crates/minos-core/clippy.toml
  - crates/minos-core/src/lib.rs
  - crates/minos-store/Cargo.toml
  - crates/minos-store/src/lib.rs
  - crates/minos-sandbox/Cargo.toml
  - crates/minos-sandbox/src/lib.rs
  - crates/minos-backend-krun/Cargo.toml
  - crates/minos-backend-krun/src/lib.rs
  - crates/minos-propose/Cargo.toml
  - crates/minos-propose/src/lib.rs
  - crates/minos-runner/Cargo.toml
  - crates/minos-runner/src/lib.rs
  - crates/minos-loop/Cargo.toml
  - crates/minos-loop/src/lib.rs
  - crates/minos-vmm/Cargo.toml
  - crates/minos-vmm/src/main.rs
  - crates/minos-guest/Cargo.toml
  - crates/minos-guest/src/main.rs
  - crates/minos-cli/Cargo.toml
  - crates/minos-cli/src/main.rs
findings:
  critical: 1
  warning: 6
  info: 7
  total: 14
status: issues_found
---

# Phase 1: Code Review Report

**Reviewed:** 2026-10-09T22:41:06Z
**Depth:** standard
**Files Reviewed:** 28
**Status:** issues_found

## Summary

This is a fresh review of the tree after gap-closure plan 01-04 (commits `4a0f49a`, `3751a25`, `53140e8`). It supersedes the review at this path from before 01-04. The ten crate source files are one-line stubs; the deliverable under review is the policy surface: `[workspace.lints]`, the new `.cargo/config.toml` rustflags backstop, `deny.toml`, `crates/minos-core/clippy.toml`, the toolchain pin, and `CLAUDE.md` as design of record.

**What 01-04 fixed, verified independently here.** The three clippy lints are now `forbid` in `Cargo.toml:26-28` and in the `CLAUDE.md` Code rules block, so the prior CR-02 (crate-wide inner `#![allow(..., reason)]` bypass) is closed. The `.cargo/config.toml` backstop is sound: I probed rustc 1.97.1 directly and confirmed (a) plain rustc accepts `-Fclippy::unwrap_used` on the command line without an `unknown_lints` warning, so `cargo build`/`cargo test` are not polluted; (b) a command-line `-F` wins over a command-line `-A` in either order, so a member that replaces `[lints] workspace = true` with its own `unwrap_used = "allow"` table is still refused; (c) a command-line `-F` makes an inner `#![allow(unsafe_code)]` a hard E0453. `cargo doc --workspace --no-deps` is the fifth gate command in `CLAUDE.md` Commands, closing the prior WR-02. `cargo deny check` passes on the committed tree (`advisories ok, bans ok, licenses ok, sources ok`); `Cargo.lock` has zero git sources; every `[workspace.dependencies]` entry is an exact pin; all ten manifests carry `[lints] workspace = true`; no C-compiling crate (`cc`, `zstd-sys`, `bzip2-sys`) reaches either aarch64 target (`cargo tree --target` for both triples; they exist in the lock only for x86_64 arms of `msb_krun_vmm`).

**What is still open.** The one blocking finding from the prior review was not touched by 01-04 and still reproduces today: `deny.toml:12` `unmaintained = "workspace"` hides RUSTSEC-2025-0141 (`bincode 2.0.1`, reachable only through `msb_krun`, inside the hypervisor-entitled `minos-vmm`). With cargo-deny's default scope the advisories check fails; no planning artifact names the advisory or justifies the narrowed scope. The remaining warnings are gate-coverage gaps and design-of-record drift: `CLAUDE.md`'s Workspace section still contradicts the DAG `deny.toml` enforces; the `Cargo.toml:29` comment still overstates GATE-05 for non-forbid lints (R-03 in the evidence shows the gap); `minos-core`'s purity bans stop at the clock; the C ban is keyed on `cc` only; `tokio` is in `minos-vmm`'s darwin graph with no fence; and the five gate commands never compile `minos-guest` for the target it will actually run on.

No cargo command run during this review created a `target/` directory (`cargo tree`, `cargo deny`, and a scratch-dir `rustc` probe only resolve or compile outside the repo). `target/` was absent at the end of the review.

## Narrative Findings (AI reviewer)

## Critical Issues

### CR-01: `unmaintained = "workspace"` silently hides RUSTSEC-2025-0141 (`bincode`) in the VMM dependency path; unchanged since the prior review and still unrecorded

**File:** `deny.toml:12`
**Issue:** cargo-deny 0.20.2 defaults `[advisories] unmaintained` to `"all"`. Line 12 narrows it to `"workspace"`, which reports unmaintained advisories only for direct workspace dependencies. Re-run today with a scratch copy of `deny.toml` with that one word changed (`cargo deny --config <scratch> check advisories`):

```text
error[unmaintained]: Bincode is unmaintained
   ├ ID: RUSTSEC-2025-0141
   ├ Advisory: https://rustsec.org/advisories/RUSTSEC-2025-0141
     ├── msb_krun_devices v0.1.41
     │   ├── msb_krun v0.1.41
     │   │   └── minos-vmm v0.1.0
     │   └── msb_krun_vmm v0.1.41
advisories FAILED
```

The file's own header (line 2: "Never add an exception, ignore, or allowlist entry just to get green") and line 11 ("An advisory hit is a stop-and-report, never an `ignore`") are contradicted by a config key that is a blanket, unnamed ignore of every transitive unmaintained advisory, present and future. Searching `.planning/` for `bincode` or `RUSTSEC-2025-0141` finds only incidental `cargo tree` dumps inside `01-GATE-EVIDENCE.md`; `01-02-PLAN.md` T-1-13 says "unmaintained (direct)" with no reason, `01-02-SUMMARY.md:82` says "unmaintained direct dependencies" with no reason, and `STACK.md:257` just repeats the key. The hit was never reported to the user. The crate it hides sits inside `minos-vmm`, the only process that carries the hypervisor entitlement. The supply-chain rule in `CLAUDE.md` ("`cargo deny check` gates every build") only means something if the gate reports what it finds; an explicit `ignore` entry names the advisory, carries a reason, and makes cargo-deny warn when it stops matching, none of which the scope narrowing does.

**Fix:**
```toml
[advisories]
unmaintained = "all"      # default scope; the transitive VMM path is exactly where this matters
yanked = "deny"
ignore = [
  # Reviewed 2026-10-09. bincode 2.0.1 is pulled only by msb_krun_devices and msb_krun_vmm (the pinned
  # VMM). No alternative without replacing msb_krun. Re-review on every msb_krun bump. cargo-deny warns
  # when this entry stops matching, so it cannot go stale silently.
  { id = "RUSTSEC-2025-0141", reason = "bincode 2 via msb_krun =0.1.41; VMM is replaced, not patched" },
]
```
Record the advisory and the decision in `01-GATE-EVIDENCE.md` and next to the `msb_krun` row in `.planning/research/STACK.md`, since it is a property of that pin. Re-run `cargo deny check` and record the exit code.

## Warnings

### WR-01: `CLAUDE.md` Workspace section contradicts the crate list and dependency direction that `deny.toml` enforces

**File:** `CLAUDE.md:79-97` (Workspace), `deny.toml:46-52`
**Issue:** `CLAUDE.md` is "the design of record until real docs exist" and says "When a decision changes, edit the relevant section in the same change." 01-04 edited the Code rules and Commands sections but left Workspace as it was. It still lists `minos-backend-lambda`, `minos-backend-chv`, and `minos-orchestrator` (deferred by D-07) and omits `minos-loop` and `minos-vmm` (which exist under `crates/`). It still says "Dependency direction is strict: `core <- store, sandbox <- backends <- runner`", i.e. the runner depends on backend crates, while `deny.toml:49` allows only `minos-cli` to depend on `minos-backend-krun` (runner is generic over `Backend`, per ARCHITECTURE.md A4). It still says "With three backends, use an `AnyBackend` enum", which A4 dropped. A developer following `CLAUDE.md` would add `minos-backend-krun.workspace = true` to `crates/minos-runner/Cargo.toml` and be refused by `cargo deny check` with no explanation in the design doc.

**Fix:** Update `CLAUDE.md` Workspace in the phase-closing commit: crate list (add `minos-loop`, `minos-vmm`; mark `minos-orchestrator`, `minos-backend-lambda`, `minos-backend-chv` as later milestones), direction line (`core <- store, sandbox <- backend-krun; runner: core, store, sandbox, generic over Backend; loop: core, store, propose; vmm: no internal crate; cli composes everything`), and the `AnyBackend` sentence. If instead `CLAUDE.md` is authoritative, change `deny.toml:49` to `wrappers = ["minos-runner", "minos-cli"]`. Both files must agree.

### WR-02: The `Cargo.toml:29` comment still claims a guarantee the gate does not provide for non-forbid lints; GATE-05 wording has the same overclaim

**File:** `Cargo.toml:29-31`, `.planning/REQUIREMENTS.md:16`
**Issue:** Line 29 reads "only `#[expect(..., reason = "...")]` may suppress a lint". The phase's own evidence (01-GATE-EVIDENCE.md R-03, and D-02 as amended) shows that an inner `#![allow(<any non-forbid lint>, reason = "...")]` at the top of a crate passes `cargo clippy --workspace --all-targets -- -D warnings`, because `clippy::allow_attributes` lints outer attributes only. `forbid` closed this for the three locked lints only. So the comment is false for every pedantic, style, suspicious, and `missing_docs` lint, and GATE-05 ("no `allow(clippy::...)` appears anywhere") is enforced for those only by the one-time grep C-02, which is not one of the five gate commands. A reader of `Cargo.toml` would believe the gate catches something it does not.

**Fix:** Reword the comment to what is enforced, and make C-02 a recurring check rather than a one-time one:
```toml
# D-02 / GATE-05: outer #[allow] is denied and every allow/expect needs a reason. The three forbid
# lints above cannot be suppressed at all. For every other lint an inner #![allow(..., reason)]
# still passes clippy (01-GATE-EVIDENCE.md R-03); the per-phase C-02 grep catches it, not the gate.
```
and add the C-02 command to the phase verification protocol alongside D-15's C-01 (`grep -rnE '^\s*#!?\[allow\(' crates/ --include='*.rs'` must print nothing).

### WR-03: `minos-core`'s clippy bans cover only the clock and hash collections; the filesystem, environment, process, network, and stdio invariants in `CLAUDE.md` have no lint

**File:** `crates/minos-core/clippy.toml:3-10`
**Issue:** `CLAUDE.md` minos-core section: "Core has no async, no AWS, no filesystem, no clock, and no randomness beyond seeded generators." The gate enforces the clock (`SystemTime::now`, `Instant::now`) and stable ordering (`HashMap`, `HashSet`), both proven to fire (V-09, V-10). `std::fs`, `std::env`, `std::process`, `std::net`, `std::io::stdin`, `std::thread::sleep`, and `println!`/`eprintln!`/`dbg!` remain callable from core with nothing to fail. `rand_core 0.10.1` is pulled without `os_rng` (verified: no `getrandom` under `minos-core` in `cargo tree`), so unseeded randomness is currently excluded by dependency choice only, which a feature flag on any future dep could change. The mechanism costs one line per entry and, per Pitfall 3 in the research, every path must be a real `std` path and proven once with a probe like V-09 (a mistyped path only warns).

**Fix:**
```toml
disallowed-methods = [
  { path = "std::time::SystemTime::now", reason = "minos-core has no clock: time arrives as an argument" },
  { path = "std::time::Instant::now",    reason = "minos-core has no clock: time arrives as an argument" },
  { path = "std::thread::sleep",         reason = "minos-core has no clock" },
  { path = "std::env::var",              reason = "minos-core reads no environment" },
  { path = "std::env::var_os",           reason = "minos-core reads no environment" },
  { path = "std::env::args",             reason = "minos-core reads no environment" },
  { path = "std::fs::read",              reason = "minos-core has no filesystem" },
  { path = "std::fs::read_to_string",    reason = "minos-core has no filesystem" },
  { path = "std::fs::write",             reason = "minos-core has no filesystem" },
  { path = "std::io::stdin",             reason = "minos-core has no I/O" },
]
disallowed-types = [
  { path = "std::collections::HashMap",  reason = "order must be stable: use BTreeMap" },
  { path = "std::collections::HashSet",  reason = "order must be stable: use BTreeSet" },
  { path = "std::fs::File",              reason = "minos-core has no filesystem" },
  { path = "std::process::Command",      reason = "minos-core spawns nothing" },
  { path = "std::net::TcpStream",        reason = "minos-core has no network" },
  { path = "std::net::TcpListener",      reason = "minos-core has no network" },
  { path = "std::net::UdpSocket",        reason = "minos-core has no network" },
]
disallowed-macros = [
  { path = "std::println",  reason = "minos-core has no I/O; return values, do not print" },
  { path = "std::eprintln", reason = "minos-core has no I/O; return values, do not print" },
  { path = "std::dbg",      reason = "minos-core has no I/O" },
]
```
Prove each new path fires once (one probe per table is enough) and record it as V-09b/V-10b.

### WR-04: The "no C" ban keys on compilation (`cc`) only; crates that link system C libraries through `pkg-config`, `vcpkg`, `cmake`, or `bindgen` are not banned, and `pkg-config` is already in the lock

**File:** `deny.toml:27-37`
**Issue:** The list stops `openssl*`, `git2`/`libgit2-sys`, `gix`, `zstd-sys`, `liblzma-sys`, and `cc`. `cc` catches crates that compile C; it does not catch a `-sys` crate that links an already-installed C library via `pkg-config` or `vcpkg`, nor a `cmake`- or `bindgen`-driven build. `pkg-config` is in `Cargo.lock` today (via `bzip2-sys` and `zstd-sys`, x86_64-only arms of `msb_krun_vmm`), so the pattern is one feature flag or one `cfg` change away from the aarch64 graph. The `cc` comment on line 28 already anticipates giving `cc` a wrapper for the rustls provider; when that happens the compilation backstop is gone and the linking helpers are the only line left, and the two rustls providers (`ring`, `aws-lc-sys`) are C and assembly.

**Fix:**
```toml
  { crate = "pkg-config", reason = "links system C libraries without compiling; no C in our tree" },
  { crate = "vcpkg",      reason = "links system C libraries without compiling; no C in our tree" },
  { crate = "cmake",      reason = "builds C/C++ in our tree" },
  { crate = "bindgen",    reason = "generates FFI to C in our tree" },
  { crate = "bzip2-sys",  reason = "C; only reachable on x86_64 via msb_krun_vmm" },
  { crate = "libz-sys",   reason = "C; flate2 must stay on miniz_oxide or zlib-rs" },
  { crate = "aws-lc-sys", reason = "C; the rustls provider is a reviewed exception, not a default" },
  { crate = "ring",       reason = "C and asm; the rustls provider is a reviewed exception, not a default" },
```
Re-run `cargo deny check` (none of these is in either aarch64 graph today, so it stays green) and record it.

### WR-05: `tokio` is in `minos-vmm`'s `aarch64-apple-darwin` graph and nothing fences it

**File:** `crates/minos-vmm/Cargo.toml:14`, `deny.toml:27-40`
**Issue:** `.claude/CLAUDE.md` decision 10 says "v1 is synchronous ... Nothing in v1 needs an async runtime" and lists tokio under "What NOT to Use". `cargo tree -i tokio --target aarch64-apple-darwin` today: `tokio 1.53.2 <- msb-imago 0.1.7 <- msb_krun_devices 0.1.41 <- msb_krun <- minos-vmm`, an unconditional dependency of the pinned VMM. So the stack document's claim is already false for that binary, and because `tokio` is in `Cargo.lock`, a future `tokio.workspace = true` in any `minos-*` crate adds nothing to the lockfile and trips neither `unused`, `multiple-versions`, nor any ban. The decision has no gate.

**Fix:**
```toml
  { crate = "tokio", wrappers = ["msb-imago"], reason = "v1 is synchronous (STACK decision 10); tokio enters only through msb_krun's imago, never a minos-* crate" },
```
`wrappers` is direct-dependent based (verified in research), so this permits the one existing edge and rejects any `minos-*` crate that adds tokio. Correct decision 10 in `.claude/CLAUDE.md` to "no Minos crate uses tokio; the pinned VMM links it internally."

### WR-06: The five gate commands never compile, lint, or test `minos-guest` for the target it runs on; the forbid backstop and all clippy lints are blind to its real code

**File:** `crates/minos-guest/Cargo.toml:18-20`, `rust-toolchain.toml:5`, `CLAUDE.md:407-416` (Commands)
**Issue:** `minos-guest` declares `rustix` and `vsock` under `[target.'cfg(target_os = "linux")'.dependencies]`, and `rust-toolchain.toml` installs no Linux target (D-09 defers `aarch64-unknown-linux-musl` to Phase 5). On the macOS host, every gate command builds the guest for `aarch64-apple-darwin`, where those dependencies do not exist. The moment the guest has real code it must be behind `#[cfg(target_os = "linux")]`, at which point `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and the `.cargo/config.toml` forbid backstop all see an empty crate. The guest is PID 1 inside every sandbox and parses hostile input (the diff, the job spec); it is the crate where `indexing_slicing` and `unwrap_used` matter most, and GATE-01 ("every crate is held to the workspace lints") will be silently untrue for it. `deny.toml` already checks the musl graph (D-09), so the supply-chain half is covered; the lint half is not. Separately, note that when `--target` is passed, `[build] rustflags` still applies to target artifacts but no longer to host build scripts and proc-macros, which is fine here because no workspace crate has either.

**Fix:** Record the gap now so Phase 5 cannot close without it: add to `CLAUDE.md` Commands a sixth line, marked as Phase 5 onward, `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings` (and, once a Linux runner exists, the matching `cargo test`), and add `targets = ["aarch64-unknown-linux-musl"]` to `rust-toolchain.toml` in the same Phase 5 change. Until then, keep the guest free of `cfg(target_os = "linux")` gates on anything the host can compile (the tar and `minos-core` paths), so the host gate covers as much of the guest as possible.

## Info

### IN-01: `.gitignore` does not exclude insta's pending-snapshot files

**File:** `.gitignore:1`
**Issue:** `insta` is a dev-dependency of `minos-core` and `minos-propose`. Outside the gate (which runs with `INSTA_UPDATE=no` per `.claude/CLAUDE.md`), a local snapshot mismatch writes `*.snap.new` or `*.pending-snap` next to the test, and nothing stops them being committed as if they were accepted goldens.
**Fix:** Append `*.snap.new` and `*.pending-snap`. Keep `proptest-regressions/` committed.

### IN-02: No `[profile.release]` hardening for binaries that parse hostile input

**File:** `Cargo.toml` (no `[profile.release]` table)
**Issue:** Release builds default to `overflow-checks = false`, so integer overflow in size caps, length-prefixed hashing, and pico-dollar accumulation wraps silently in exactly the binaries that handle sandbox output (`minos-guest`, the tar parser in `minos-cli`/`minos-sandbox`). The core rules push toward checked arithmetic, but a profile setting makes every wrap a panic without relying on each call site.
**Fix:**
```toml
[profile.release]
overflow-checks = true
```
`panic = "abort"` for `minos-guest` (PID 1 has nothing to unwind into) belongs to the phase that gives it a body.

### IN-03: `members = ["crates/*"]` plus opt-in `[lints] workspace = true` still lets a new directory escape the non-backstop lints

**File:** `Cargo.toml:3`
**Issue:** 01-04 mitigated this twice (the four-lint rustflags backstop, and D-15's per-plan C-01 check), which is the right shape. What remains uncovered for a crate that omits the table is `missing_docs`, `allow_attributes`, the clippy groups, and the rustdoc table, and C-01 is a plan acceptance criterion rather than a gate command. Listing members explicitly would turn "add a crate" into an edit of the same file that holds the lints.
**Fix:** Replace the glob with an explicit list and a one-line comment stating that every new member needs `[lints] workspace = true`, a `deny.toml` layering entry, and (if core-like) its own `clippy.toml`. If the glob is kept, run C-01 as part of every phase's verification, not only in plans that add crates.

### IN-04: `clap`'s `derive` feature is declared and linked, but D-12/F-01 establish that `#[derive(clap::Parser)]` cannot pass the gate

**File:** `Cargo.toml:60`, `crates/minos-cli/Cargo.toml:25`
**Issue:** 01-GATE-EVIDENCE.md F-01 shows `clap_derive 4.6.7` emits `#[allow(clippy::restriction)]` on generated impls (confirmed in the pinned source: `src/derives/into_app.rs:43,91`, `value_enum.rs:65`), which is E0453 under the three `forbid` lints. The stack document still says "clap 4.6.7 with derive", the workspace dependency still enables `derive`, and `minos-cli` links the proc-macro crate for a feature it cannot use. The decision is correctly flagged for the user in the 01-04 summary; the manifest and the stack doc just do not reflect it yet.
**Fix:** Either drop `features = ["derive"]` now (builder API per F-02) and update `.claude/CLAUDE.md` decision 5, or keep it and record the pending user decision next to the dependency so the next phase that touches `minos-cli` does not rediscover it.

### IN-05: `deny.toml:51` reason text misattributes the `JobRunner` trait

**File:** `deny.toml:51`
**Issue:** "loop reaches the runner only through its JobRunner trait" reads as if `JobRunner` belongs to `minos-runner`. The layering forbids both `loop -> runner` and `runner -> loop`, and ARCHITECTURE.md:121 says `minos-loop` defines `JobRunner` and the CLI holds the adapter that implements it over `minos-runner`. A future reader may try to make `minos-runner` implement a loop trait and be refused by this very entry.
**Fix:** `reason = "loop defines JobRunner; the CLI adapts minos-runner to it, so neither depends on the other"`.

### IN-06: `CLAUDE.md:370` still says nano-dollar precision; the stack research corrected it to pico-dollars

**File:** `CLAUDE.md:370`
**Issue:** `.claude/CLAUDE.md` decision 8 states that published AWS rates have 10 fractional digits, that nano-dollars (9) cannot represent them, and says "Update CLAUDE.md to pico-dollars." The Cost section still says nano. The money code is a later phase, but `CLAUDE.md` is the design of record and the file's own rule is to keep it current when a decision changes.
**Fix:** Change "nano-dollar precision" to "pico-dollar precision (10^-12 USD; published AWS rates have 10 fractional digits)".

### IN-07: `.cargo/config.toml` rustflags also apply to any `cargo install` run with the repo as the current directory

**File:** `.cargo/config.toml:7-8`
**Issue:** Cargo's config discovery is based on the current directory, including for `cargo install`. Running `cargo install cargo-insta` (or any tool whose root crate uses `unsafe`) from inside this repo compiles that root crate with `-Funsafe_code` and fails with a confusing error, because cap-lints applies to its dependencies but not to the installed package itself. This is a documented Cargo gotcha, not a defect in the backstop, but the comment block lists every other caveat and should list this one.
**Fix:** Add one line to the comment: "`cargo install` from inside this directory also gets these flags; run tool installs from elsewhere."

---

_Reviewed: 2026-10-09T22:41:06Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
