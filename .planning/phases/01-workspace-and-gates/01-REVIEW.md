---
phase: 01-workspace-and-gates
reviewed: 2026-10-09T21:48:26Z
depth: standard
files_reviewed: 26
files_reviewed_list:
  - .gitignore
  - Cargo.toml
  - rust-toolchain.toml
  - rustfmt.toml
  - deny.toml
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
  critical: 2
  warning: 6
  info: 3
  total: 11
status: issues_found
---

# Phase 1: Code Review Report

**Reviewed:** 2026-10-09T21:48:26Z
**Depth:** standard
**Files Reviewed:** 26
**Status:** issues_found

## Summary

Phase 1 delivers a workspace skeleton and its gate configuration. The ten crate source files are one-line stubs with nothing to review. The substantive targets were the policy files, and the review treated them as security-relevant code: every lint, ban, and allowlist entry was checked against `CLAUDE.md` (Code rules, Workspace dependency direction, Supply chain), `.claude/CLAUDE.md` (pinned versions, cargo-deny shape), and the pinned crate sources in `~/.cargo/registry`.

What holds up: every `[workspace.dependencies]` entry is an exact `=x.y.z` pin and matches the stack table; every member opts into `[lints] workspace = true`; no C-compiling crate (`cc`, `zstd-sys`, `bzip2-sys`) reaches either aarch64 target (verified with `cargo tree --target` for both triples; the x86_64-only `cfg` gates in `msb_krun_vmm` keep them out); no git or non-crates.io source is in `Cargo.lock`; `cargo deny check` passes as committed; the internal DAG in `deny.toml` is consistent with `.planning/research/ARCHITECTURE.md` and is stricter than `CLAUDE.md`; the build-script allowlist entries I spot-checked are benign cfg probes.

Two findings block the phase. Both are places where the gate is weaker than the policy file itself claims, which is exactly the class of defect `CLAUDE.md` tells us to protect against ("Never weaken a security invariant to make something compile or pass"):

1. `deny.toml` sets `unmaintained = "workspace"`, which silently hides a real RustSec advisory (RUSTSEC-2025-0141, `bincode 2.0.1`, reachable only through `msb_krun`, the VMM). With cargo-deny's default scope the gate fails. No planning document records that this hit exists or that the scope was narrowed to avoid it.
2. The three `CLAUDE.md`-locked lints are at `deny`, so an inner `#![allow(clippy::unwrap_used, reason = "...")]` disables them for a whole crate and passes the gate. The comment in `Cargo.toml` says only `#[expect]` may suppress a lint; that is not true. The research already proved the gap and the `forbid` fix; it is parked as an open decision, but the phase cannot be called gated while it is open.

The warnings cover gate coverage gaps (rustdoc lint not exercised, core purity bans limited to the clock, C-linking ban keyed on `cc` only, `tokio` present with no fence) and design-of-record drift (`CLAUDE.md` Workspace section no longer matches the DAG that `deny.toml` enforces).

No cargo command run during this review created a `target/` directory; `cargo tree`, `cargo deny`, and `cargo metadata` only resolve.

## Narrative Findings (AI reviewer)

## Critical Issues

### CR-01: `unmaintained = "workspace"` silently hides a RustSec advisory in the VMM dependency path

**File:** `deny.toml:12`
**Issue:** cargo-deny 0.20's default for `[advisories] unmaintained` is `"all"`. Line 12 narrows it to `"workspace"`, which reports unmaintained advisories only for direct workspace dependencies. I re-ran the advisories check with the default scope (a scratch copy of `deny.toml` with the one word changed) and it fails:

```text
error[unmaintained]: Bincode is unmaintained
  ID: RUSTSEC-2025-0141
  bincode v2.0.1
  └── msb_krun_devices v0.1.41 / msb_krun_vmm v0.1.41
      └── msb_krun v0.1.41
          └── minos-vmm v0.1.0
advisories FAILED
```

This contradicts the file's own policy on lines 2 and 11 ("Never add an exception, ignore, or allowlist entry just to get green"; "An advisory hit is a stop-and-report, never an `ignore`"). Narrowing the scope is a blanket, unrecorded ignore of every transitive unmaintained advisory, present and future, and the crate it currently hides sits inside `minos-vmm`, the process that holds the hypervisor entitlement. No planning artifact (`01-RESEARCH.md`, `01-02-PLAN.md`, `01-02-SUMMARY.md`, `STACK.md`) names `bincode` or RUSTSEC-2025-0141; `01-02-PLAN.md` T-1-13 says "unmaintained (direct)" without saying why direct-only was chosen, so the hit was never reported to the user. An explicit `ignore` entry is the mechanism cargo-deny provides for a reviewed hit: it names the advisory, carries a reason, and cargo-deny warns when it stops matching anything. The scope narrowing does none of that.

The supply-chain rule in `CLAUDE.md` ("`cargo deny check` gates every build") is only meaningful if the gate reports what it finds.

**Fix:**
```toml
[advisories]
unmaintained = "all"      # default scope: transitive crates in the VMM path matter most
yanked = "deny"
ignore = [
  # Reviewed 2026-10-09. bincode 2.0.1 is pulled only by msb_krun_devices and msb_krun_vmm
  # (the pinned VMM); no alternative exists without replacing msb_krun. Re-review on every
  # msb_krun bump. cargo-deny warns when this entry stops matching, so it cannot go stale silently.
  { id = "RUSTSEC-2025-0141", reason = "bincode 2 via msb_krun =0.1.41; no safe upgrade; VMM is replaced, not patched" },
]
```
Record the advisory and the decision in `01-GATE-EVIDENCE.md` (or the SUMMARY) and in `.planning/research/STACK.md` next to the `msb_krun` entry, since it is a property of that pin.

### CR-02: The three locked lints can be disabled crate-wide by an inner `#![allow(..., reason = "...")]`; GATE-05 is not met and the config comment overstates the guarantee

**File:** `Cargo.toml:26-31`
**Issue:** `unwrap_used`, `expect_used`, and `indexing_slicing` are at `deny`. `clippy::allow_attributes` only lints outer `#[allow]` attributes (clippy's documented behavior), and `allow_attributes_without_reason` is satisfied by any `reason`. So this line at the top of any `lib.rs` or `main.rs`

```rust
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, reason = "convenience")]
```

turns off all three locked lints for the whole crate and `cargo clippy --workspace --all-targets -- -D warnings` passes. The research verified this ("`#![allow(clippy::unwrap_used, reason = "...")]` plus an `unwrap()` passed clippy", `01-RESEARCH.md:573`) and verified the fix (`forbid` gives `error[E0453]`, `01-RESEARCH.md:575`). The requirement text for GATE-05 is "`allow(clippy::...)` never appears, only `#[expect(..., reason = ...)]`". The comment on line 29, "only `#[expect(..., reason = "...")]` may suppress a lint", is therefore false for the lints that matter most, and a reader of `Cargo.toml` would believe a guarantee the gate does not provide.

This is parked as open decision 1 in `01-01-PLAN.md` and `01-03-SUMMARY.md`. A gate phase cannot close with its headline lint gate bypassable by one line; either resolution below is acceptable, but one of them has to land.

`unsafe_code` is not affected: it is already `forbid`, so an inner allow is a hard error (proof V-01 and E0453).

**Fix (preferred, strictly stronger than CLAUDE.md's `deny`):**
```toml
[workspace.lints.clippy]
# Locked by CLAUDE.md. `forbid`, not `deny`: an inner `#![allow(..., reason = ...)]` is not
# caught by allow_attributes (it lints outer attributes only), and `forbid` cannot be allowed
# or expected anywhere (E0453). Tests use `?` in `fn test() -> Result<(), E>` instead of unwrap.
unwrap_used = "forbid"
expect_used = "forbid"
indexing_slicing = "forbid"
```
and edit `CLAUDE.md` Code rules in the same change (the file's own rule: "Keep this file current").

**Fix (alternative, if the user accepts the gap):** keep `deny`, rewrite the comment on lines 29-31 to say what is actually enforced ("outer `#[allow]` is denied; an inner `#![allow]` must carry a reason and is caught in review, not by the gate"), and amend the GATE-05 wording in `REQUIREMENTS.md` so the requirement and the gate agree.

## Warnings

### WR-01: `deny.toml` enforces a crate DAG that contradicts `CLAUDE.md`'s documented dependency direction, and `CLAUDE.md` was not updated in the same change

**File:** `deny.toml:46-52` (and `CLAUDE.md` Workspace section)
**Issue:** `CLAUDE.md` states "Dependency direction is strict: `core <- store, sandbox <- backends <- runner`", i.e. the runner depends on the backend crates, and separately "With three backends, use an `AnyBackend` enum, not trait objects". `deny.toml:49` allows only `minos-cli` to depend on `minos-backend-krun`, and `deny.toml:51` says the loop reaches the runner through a `JobRunner` trait. This follows `.planning/research/ARCHITECTURE.md` (runner is "generic over `B: Backend`", the CLI instantiates `Runner<KrunBackend, FileStore>`, Amendment A4 drops `AnyBackend`). The enforced policy is stricter than the design of record and is internally consistent, so the DAG itself is not the defect. The defect is that `CLAUDE.md` is still "the design of record until real docs exist", still lists `minos-orchestrator`, `minos-backend-lambda`, and `minos-backend-chv` but not `minos-loop` or `minos-vmm`, and still says `backends <- runner`. A reader following `CLAUDE.md` would add `minos-backend-krun.workspace = true` to `crates/minos-runner/Cargo.toml` and be refused by the gate with no explanation in the design doc. `CLAUDE.md` is explicit: "When a decision changes, edit the relevant section in the same change."

**Fix:** In the same commit that closes this phase, update the `CLAUDE.md` Workspace section: the crate list (add `minos-loop`, `minos-vmm`; mark the AWS crates as later milestones), the direction line to `core <- store, sandbox <- backend-krun; runner (core, store, sandbox; generic over Backend); loop (core, store, propose); cli composes everything`, and the `AnyBackend` sentence to match Amendment A4. If instead `CLAUDE.md` is authoritative, change `deny.toml:49` to `wrappers = ["minos-runner", "minos-cli"]`. Either way, both files must say the same thing.

### WR-02: `broken_intra_doc_links = "deny"` is declared but no gate command exercises it

**File:** `Cargo.toml:44-45`
**Issue:** `[workspace.lints.rustdoc]` is only read by rustdoc. The four gate commands in `CLAUDE.md` (`fmt`, `clippy`, `test`, `deny`) never invoke it; the comment on line 45 says so, and `01-03-SUMMARY.md` G-03 recorded `cargo doc --workspace --no-deps` exiting 101 while clippy and test exited 0 on the same broken link. D-12 adopted this lint as part of the gate; as configured it is documentation of intent, not a gate. This is open decision 3.

**Fix:** Add `cargo doc --workspace --no-deps` as a fifth plain cargo command in `CLAUDE.md` Commands (no script, consistent with D-01), and update ROADMAP Phase 1 criterion 1. If the user prefers not to, delete lines 44-45 rather than keep a `deny` that nothing runs, and note in D-12 that rustdoc lints are out of the gate.

### WR-03: `minos-core`'s clippy bans cover only the clock; the filesystem, environment, process, network, and stdio invariants in `CLAUDE.md` have no lint

**File:** `crates/minos-core/clippy.toml:3-10`
**Issue:** `CLAUDE.md` says "Core has no async, no AWS, no filesystem, no clock, and no randomness beyond seeded generators" and relies on that purity for durable replay and property tests. The gate enforces exactly one of those: `SystemTime::now` / `Instant::now`. `std::fs`, `std::env`, `std::process`, `std::net`, `std::io::stdin`, `std::thread::sleep`, and `println!`/`eprintln!`/`dbg!` are all available in core with no dependency to remove and no lint to fail. The mechanism is already in place and costs one line per entry; `01-RESEARCH.md` Pitfall 17 even lists some of them as "optional extra core bans". Note from Pitfall 3: a mistyped path only warns and exits 0, so each path below must be a real `std` path and should be proven once with a probe like V-09/V-10.

**Fix:**
```toml
disallowed-methods = [
  { path = "std::time::SystemTime::now", reason = "minos-core has no clock: time arrives as an argument" },
  { path = "std::time::Instant::now",    reason = "minos-core has no clock: time arrives as an argument" },
  { path = "std::env::var",      reason = "minos-core reads no environment" },
  { path = "std::env::var_os",   reason = "minos-core reads no environment" },
  { path = "std::env::args",     reason = "minos-core reads no environment" },
  { path = "std::thread::sleep", reason = "minos-core has no clock" },
  { path = "std::fs::read", reason = "minos-core has no filesystem" },
  { path = "std::fs::read_to_string", reason = "minos-core has no filesystem" },
  { path = "std::fs::write", reason = "minos-core has no filesystem" },
  { path = "std::io::stdin", reason = "minos-core has no I/O" },
]
disallowed-types = [
  { path = "std::collections::HashMap", reason = "order must be stable: use BTreeMap" },
  { path = "std::collections::HashSet", reason = "order must be stable: use BTreeSet" },
  { path = "std::fs::File",          reason = "minos-core has no filesystem" },
  { path = "std::process::Command",  reason = "minos-core spawns nothing" },
  { path = "std::net::TcpStream",    reason = "minos-core has no network" },
  { path = "std::net::TcpListener",  reason = "minos-core has no network" },
  { path = "std::net::UdpSocket",    reason = "minos-core has no network" },
]
disallowed-macros = [
  { path = "std::println",  reason = "minos-core has no I/O; return values, do not print" },
  { path = "std::eprintln", reason = "minos-core has no I/O; return values, do not print" },
  { path = "std::dbg",      reason = "minos-core has no I/O" },
]
```

### WR-04: The "no C" ban keys on compilation (`cc`) only; crates that link system C libraries through `pkg-config`, `vcpkg`, `cmake`, or `bindgen` are not banned

**File:** `deny.toml:27-37`
**Issue:** The ban list stops `openssl*`, `git2`/`libgit2-sys`, `gix`, `zstd-sys`, `liblzma-sys`, and `cc`. `cc` is the backstop for "no C compiled in our tree" and it does catch `bzip2-sys` transitively today (`bzip2-sys -> cc`). But a `-sys` crate can link an already-installed C library without compiling anything, via `pkg-config` or `vcpkg` (and `cmake` and `bindgen` are the other two common C-adjacent build helpers). `pkg-config` is in `Cargo.lock` right now (via `bzip2-sys` and `zstd-sys`, x86_64-only), so the pattern is already one feature flag away from the aarch64 graph. The `cc` comment also notes that `cc` "must get a wrapper when the rustls provider arrives"; when that happens the compilation backstop is gone and the linking helpers are the only line left. `ring` and `aws-lc-sys` (C and assembly) are the two rustls providers that would need an explicit decision then; naming them now means the decision is forced rather than accidental.

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

### WR-05: `tokio` is in the `aarch64-apple-darwin` graph of `minos-vmm` and nothing fences it

**File:** `crates/minos-vmm/Cargo.toml:14`, `deny.toml:27-40`
**Issue:** `.claude/CLAUDE.md` decision 10 says "v1 is synchronous ... Nothing in v1 needs an async runtime" and lists tokio under "What NOT to Use". `cargo tree --target aarch64-apple-darwin -p minos-vmm -i tokio` shows `tokio 1.53.2 <- msb-imago 0.1.7 <- msb_krun_devices 0.1.41 <- msb_krun`, an unconditional dependency of the pinned VMM (it is not behind the `net` or any other feature; only `msb_krun_devices`'s own tokio dep is windows-only). So tokio is linked into `minos-vmm` whether we like it or not, and the stack document's claim is wrong for that binary. More importantly, because tokio is already in `Cargo.lock`, a future `tokio.workspace = true` in any Minos crate would add nothing new to the lockfile and would not trip `unused`, `multiple-versions`, or any ban. The decision has no gate.

**Fix:** Fence it the same way `anyhow` and `msb_krun` are fenced, and correct the stack document:
```toml
  { crate = "tokio", wrappers = ["msb-imago"], reason = "v1 is synchronous (STACK decision 10); tokio enters only through msb_krun's imago, never a minos-* crate" },
```
`wrappers` is direct-dependent based (verified in research), so this allows the one existing edge and rejects any `minos-*` crate that adds tokio. Update `.claude/CLAUDE.md` decision 10 to say "no Minos crate uses tokio; the pinned VMM links it internally".

### WR-06: The build-script allowlist carries no per-entry review record, and the stack document's "exactly these" list no longer matches it

**File:** `deny.toml:60-71`
**Issue:** Lines 62-63 set the policy: "read the crate's build.rs before adding it here. Never add an entry automatically." The list was then produced by the opposite method (`01-RESEARCH.md` GATE-03: "derived from `cargo metadata` for both targets"), and only two of 25 entries say what the script does. `.claude/CLAUDE.md` Question 7 still says "the probe found exactly these build scripts" and lists 14; the file has 25. Several entries are there only because cargo-deny evaluates `minos-vmm` for `aarch64-unknown-linux-musl`, a configuration that is never built (`kvm-ioctls`, and the linux arms of `msb_krun`); that is the right direction to err, but it should be written down so nobody "cleans up" the list.

I spot-checked the less obvious scripts in the pinned registry sources. All are cfg probes: `target-tuple`, `radium`, `crossbeam-deque`, `serde_json`, `num-traits`, `msb-imago` (9 lines), `kvm-ioctls` (12 lines) read `env::var` and emit `cargo:rustc-cfg`; `msb_krun_hvf` emits one `rustc-link-lib=framework=Hypervisor`; `anyhow` and `zmij` spawn `rustc --version` to detect features; `rustversion`'s script lives at `build/build.rs`, which is why it looks like it has none. None compiles C or writes outside `OUT_DIR`. That review took minutes and is exactly what the file says must happen; it just was not recorded.

**Fix:** Annotate each entry with what its build script does and why it is accepted, group them (cfg probes / rustc version probes / framework link), and note which entries exist only for the musl evaluation of `minos-vmm`. Replace the "exactly these" sentence in `.claude/CLAUDE.md` Question 7 with a pointer to `deny.toml` as the source of truth. Also add `cmake`, `cc`, and `pkg-config` to the ban list (WR-04) so that a future allowlist entry for a C-compiling script cannot be the only thing standing between the graph and C.

## Info

### IN-01: `.gitignore` does not exclude insta's pending-snapshot files

**File:** `.gitignore:1`
**Issue:** `insta` is a workspace dev-dependency. A snapshot mismatch writes `*.snap.new` (or `*.pending-snap`) next to the test unless `INSTA_UPDATE=no` is set, and `.claude/CLAUDE.md` says to run the gate with `INSTA_UPDATE=no` precisely so missing snapshots fail. Outside the gate, a developer's local run will write these files, and the current `.gitignore` lets them be committed as if they were accepted goldens.
**Fix:** Append `*.snap.new` and `*.pending-snap` to `.gitignore`. Keep `proptest-regressions/` committed (it should not be ignored).

### IN-02: No `[profile.release]` hardening for a security tool

**File:** `Cargo.toml` (no `[profile.release]` table)
**Issue:** Release builds default to `overflow-checks = false`, so integer overflow in size caps, length-prefixed hashing, and micro-dollar accumulation wraps silently in exactly the binaries that handle hostile input (`minos-guest`, `minos-cli`'s tar parser). The core rules already push toward checked arithmetic, but a profile setting makes the wrap a panic in every crate rather than relying on each call site.
**Fix:**
```toml
[profile.release]
overflow-checks = true
debug-assertions = false
```
Consider `panic = "abort"` for `minos-guest` specifically once it exists (PID 1 has nothing to unwind into); that belongs in a later phase.

### IN-03: `members = ["crates/*"]` plus opt-in `[lints] workspace = true` means a new crate directory silently escapes every lint

**File:** `Cargo.toml:3`, every `crates/*/Cargo.toml` `[lints]` table
**Issue:** Workspace lints only apply to a member that opts in. With a glob `members`, dropping a new directory under `crates/` makes it a member with no lints, no `clippy.toml` bans, and (if it has no internal deps) no `deny.toml` layering entry. The research notes this as Pitfall 4 and it is open decision 2. There is no cargo-native enforcement; the mitigation is to make adding a crate a reviewed edit in the same file as the lints.
**Fix:** List members explicitly instead of globbing, and keep a comment that every new member needs `[lints] workspace = true`, a `deny.toml` layering entry, and (if it is `minos-core`-like) its own `clippy.toml`. Alternatively, keep the glob and add a one-line check to the phase verification protocol: `grep -L 'workspace = true' crates/*/Cargo.toml` must print nothing.

---

_Reviewed: 2026-10-09T21:48:26Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
