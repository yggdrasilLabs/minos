# Phase 7 Evidence

This file collects Phase 7's recorded runs: each section names the plan that made it, the command, and the verbatim output.

## Clippy bans fire (07-03)

`crates/minos-runner/clippy.toml` bans eight paths and `crates/minos-loop/clippy.toml` bans nine:

| Entry | Kind | minos-runner | minos-loop |
|---|---|---|---|
| `std::time::SystemTime::now` | `disallowed-methods` | yes | yes |
| `std::time::Instant::now` | `disallowed-methods` | yes | yes |
| `std::env::var` | `disallowed-methods` | yes | yes |
| `std::env::var_os` | `disallowed-methods` | yes | yes |
| `minos_core::id::ExperimentId::of` | `disallowed-methods` | no | yes (D-12) |
| `std::collections::HashMap` | `disallowed-types` | yes | yes |
| `std::collections::HashSet` | `disallowed-types` | yes | yes |
| `std::process::Command` | `disallowed-types` | yes | yes |
| `std::fs::File` | `disallowed-types` | yes | yes |

`std::thread::sleep` is deliberately not banned in `minos-loop`: D-16's infrastructure retry pauses with it
(07-RESEARCH Pattern 9). The loop's clippy.toml says so in a header comment.

**Probes.** Each crate's `src/lib.rs` (a one-line doc comment stub) temporarily gained one documented
`#[must_use]` public function per banned item, with no `unwrap`, `expect`, or indexing:

- a call for each method: `std::time::SystemTime::now() > std::time::UNIX_EPOCH`,
  `std::time::Instant::now().elapsed().as_nanos()`, `std::env::var("MINOS_PROBE").is_ok()`,
  `std::env::var_os("MINOS_PROBE").is_some()`;
- a construction for each collection type: `std::collections::HashMap::<u8, u8>::new().len()` and
  `std::collections::HashSet::<u8>::new().len()` (a first attempt that took `&HashMap<u8, u8>` as a parameter also
  tripped the pedantic `implicit_hasher` lint, so the probes were changed to constructions to leave only the ban
  diagnostics);
- a parameter annotation for the other two types: `command: &std::process::Command` (returning
  `command.get_program().is_empty()`) and `file: &std::fs::File` (returning `file.metadata().is_ok()`);
- in `minos-loop` only, two functions taking `&TreeId` and `&Diff` and returning the call's result: one through the
  module path `minos_core::id::ExperimentId::of(base, diff)` (line 57) and one through the crate-root re-export
  `minos_core::ExperimentId::of(base, diff)` (line 66), proving the ban follows the definition and not the spelling.

Each configured entry produced exactly one error (two for `ExperimentId::of`, one per spelling), every error is
`disallowed_methods` or `disallowed_types`, every error carries its configured reason as its note, and no other
diagnostic appeared.

```
$ cargo clippy -p minos-runner --all-targets -- -D warnings 2>&1 | grep -E 'disallowed|-->|could not|= note: '
error: use of a disallowed method `std::time::SystemTime::now`
 --> crates/minos-runner/src/lib.rs:6:5
  = note: LOOP-03: the runner reads no clock; a VM's time is Finished::elapsed, measured by the backend
  = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_methods
  = note: `-D clippy::disallowed-methods` implied by `-D warnings`
  = help: to override `-D warnings` add `#[allow(clippy::disallowed_methods)]`
error: use of a disallowed method `std::time::Instant::now`
  --> crates/minos-runner/src/lib.rs:12:5
   = note: LOOP-03: the runner reads no clock; a VM's time is Finished::elapsed, measured by the backend
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_methods
error: use of a disallowed method `std::env::var`
  --> crates/minos-runner/src/lib.rs:18:5
   = note: the runner reads no environment; its inputs arrive validated
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_methods
error: use of a disallowed method `std::env::var_os`
  --> crates/minos-runner/src/lib.rs:24:5
   = note: the runner reads no environment; its inputs arrive validated
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_methods
error: use of a disallowed type `std::collections::HashMap`
  --> crates/minos-runner/src/lib.rs:30:5
   = note: order must be stable: use BTreeMap or BTreeSet
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_types
   = note: `-D clippy::disallowed-types` implied by `-D warnings`
   = help: to override `-D warnings` add `#[allow(clippy::disallowed_types)]`
error: use of a disallowed type `std::collections::HashSet`
  --> crates/minos-runner/src/lib.rs:36:5
   = note: order must be stable: use BTreeMap or BTreeSet
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_types
error: use of a disallowed type `std::process::Command`
  --> crates/minos-runner/src/lib.rs:41:32
   = note: the runner starts VMs only through Backend::launch and spawns no process
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_types
error: use of a disallowed type `std::fs::File`
  --> crates/minos-runner/src/lib.rs:47:26
   = note: the runner stores through Blobs and launches through Backend; it opens no file
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_types
error: could not compile `minos-runner` (lib test) due to 8 previous errors
error: could not compile `minos-runner` (lib) due to 8 previous errors
```

Exit code: 101.

```
$ cargo clippy -p minos-loop --all-targets -- -D warnings 2>&1 | grep -E 'disallowed|-->|could not|= note: '
error: use of a disallowed method `std::time::SystemTime::now`
 --> crates/minos-loop/src/lib.rs:6:5
  = note: the loop reads no clock: started_at and elapsed time come from the CLI
  = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_methods
  = note: `-D clippy::disallowed-methods` implied by `-D warnings`
  = help: to override `-D warnings` add `#[allow(clippy::disallowed_methods)]`
error: use of a disallowed method `std::time::Instant::now`
  --> crates/minos-loop/src/lib.rs:12:5
   = note: the loop reads no clock: started_at and elapsed time come from the CLI
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_methods
error: use of a disallowed method `std::env::var`
  --> crates/minos-loop/src/lib.rs:18:5
   = note: the loop reads no environment; its inputs arrive validated
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_methods
error: use of a disallowed method `std::env::var_os`
  --> crates/minos-loop/src/lib.rs:24:5
   = note: the loop reads no environment; its inputs arrive validated
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_methods
error: use of a disallowed type `std::collections::HashMap`
  --> crates/minos-loop/src/lib.rs:30:5
   = note: order must be stable: use BTreeMap or BTreeSet
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_types
   = note: `-D clippy::disallowed-types` implied by `-D warnings`
   = help: to override `-D warnings` add `#[allow(clippy::disallowed_types)]`
error: use of a disallowed type `std::collections::HashSet`
  --> crates/minos-loop/src/lib.rs:36:5
   = note: order must be stable: use BTreeMap or BTreeSet
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_types
error: use of a disallowed type `std::process::Command`
  --> crates/minos-loop/src/lib.rs:41:32
   = note: the loop proposes through ModelProvider and runs jobs through JobRunner; it spawns no process
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_types
error: use of a disallowed type `std::fs::File`
  --> crates/minos-loop/src/lib.rs:47:26
   = note: the loop touches storage only through the Ledger, Blobs, and RunLock traits
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_types
error: use of a disallowed method `minos_core::id::ExperimentId::of`
  --> crates/minos-loop/src/lib.rs:57:5
   = note: D-12: name an experiment through Experiment::candidate(CumulativeDiff) or a recorded ID, never by hashing a bare Diff
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_methods
error: use of a disallowed method `minos_core::id::ExperimentId::of`
  --> crates/minos-loop/src/lib.rs:66:5
   = note: D-12: name an experiment through Experiment::candidate(CumulativeDiff) or a recorded ID, never by hashing a bare Diff
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_methods
error: could not compile `minos-loop` (lib test) due to 10 previous errors
error: could not compile `minos-loop` (lib) due to 10 previous errors
```

Exit code: 101.

Per-entry checklist (each configured entry fired at least once):

- [x] `std::time::SystemTime::now`: runner 6:5, loop 6:5
- [x] `std::time::Instant::now`: runner 12:5, loop 12:5
- [x] `std::env::var`: runner 18:5, loop 18:5
- [x] `std::env::var_os`: runner 24:5, loop 24:5
- [x] `std::collections::HashMap`: runner 30:5, loop 30:5
- [x] `std::collections::HashSet`: runner 36:5, loop 36:5
- [x] `std::process::Command`: runner 41:32, loop 41:32
- [x] `std::fs::File`: runner 47:26, loop 47:26
- [x] `minos_core::id::ExperimentId::of`: loop 57:5 (module path) and 66:5 (crate-root re-export)

The probe functions were then removed. Both `src/lib.rs` files are byte-identical to before (SHA-256
`9ff5e3b35d73ef8a34f088af1566609e00a683f4526366d34245ac254c6867c9` for minos-runner and
`313db20a4e964f58b3b67db708c09aeff5868cf812ad5ca6e5588bc64af4f7ba` for minos-loop, before and after;
`git diff --exit-code` on both exits 0), and the same commands were rerun:

```
$ cargo clippy -p minos-runner --all-targets -- -D warnings
    Checking minos-runner v0.1.0 (~/Repos/minos/crates/minos-runner)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.10s
```

Exit code: 0.

```
$ cargo clippy -p minos-loop --all-targets -- -D warnings
    Checking minos-loop v0.1.0 (~/Repos/minos/crates/minos-loop)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.10s
```

Exit code: 0.


## Runner on real VMs (07-14)

Run on 2026-10-24 by the executor on this Mac (the 05-17 precedent), at commit `c215775`, after Tasks 1 and 2.
macOS 27.0.1 (build 26A434), arm64; libkrunfw 5.6.2 from `libkrun/krun`; rustc and cargo 1.97.1. Every command
ran at the repository root (`~/Repos/minos`).

Sequence and exit codes:

| # | Command | Exit |
|---|---------|------|
| 1 | `cargo build -p minos-vmm` | 0 |
| 2 | `cargo build -p minos-guest --release --target aarch64-unknown-linux-musl` | 0 |
| 3 | `codesign --force --sign - --options runtime --entitlements crates/minos-vmm/minos-vmm.entitlements target/debug/minos-vmm` | 0 |
| 4 | `codesign --display --entitlements - target/debug/minos-vmm` | 0 |
| 5 | `cargo run -q -p minos-cli -- host doctor` | 0 |
| 6 | `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest MINOS_KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib cargo test -p minos-cli --test runner_vm -- --ignored --nocapture --test-threads=1` | 0 |

No cargo command between the signing (3) and the test (6) relinked `minos-vmm`: `minos-cli` does not build it, and
the test target was already built (`Finished` in 0.05 s). The entitlement grep immediately before step 6 still found
`com.apple.security.hypervisor`.

`codesign --display --entitlements - target/debug/minos-vmm`:

```
Executable=~/Repos/minos/target/debug/minos-vmm
[Dict]
	[Key] com.apple.security.cs.disable-library-validation
	[Value]
		[Bool] true
	[Key] com.apple.security.hypervisor
	[Value]
		[Bool] true
```

`minos host doctor` (exit 0, no refusal). Shown is the rerun taken right after the test for this record; the run
before the test printed the same checks and the same result, and differed only in the name of the probe's
temporary directory:

```
binaries (fixed locations; no configuration file or environment variable):
  found     ~/Repos/minos/target/debug/minos-vmm  a regular file
  rejected  ~/Repos/minos/target/debug/minos-guest  not an ELF file (the macOS build of minos-guest is Mach-O)
  found     ~/Repos/minos/target/aarch64-unknown-linux-musl/release/minos-guest  a static AArch64 ELF
  kernel    /opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib  checked below

checks:
ok       helper present  a regular file of 17955744 bytes
  checked: ~/Repos/minos/target/debug/minos-vmm
ok       helper entitlements  both entitlements are set to true
  checked: ~/Repos/minos/target/debug/minos-vmm
  checked: /usr/bin/codesign
ok       guest kernel  sha256 59764391005357bbdd5218f3fd522dd8ab63722cbc96144752c7bddc2c8e5217
  checked: /opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib
ok       hypervisor  kern.hv_support 1, hw.ncpu 16, hw.memsize 51539607552; largest VM 8 vCPUs and 16384 MiB, leaving half the host's memory to the host
  checked: /usr/sbin/sysctl
ok       job socket path  41 of at most 103 bytes
  checked: /private/tmp/minos-501/mnsXXXXXXXX/o.sock
ok       sandbox-exec present  a regular file
  checked: /usr/bin/sandbox-exec
ok       Seatbelt denies network  the control connected; under the profile nc exited 1 and the listener accepted nothing: no stderr
  checked: /usr/bin/sandbox-exec
  checked: /usr/bin/nc
  checked: /bin/cat
ok       Seatbelt denies unlisted files  the listed file read back; the unlisted read exited 1: cat: /private/tmp/mnpHYnEsK2P/unlisted: Operation not permitted
  checked: /usr/bin/sandbox-exec
  checked: /usr/bin/nc
  checked: /bin/cat
ok       guest binary  a static aarch64 ELF of 1515688 bytes
  checked: ~/Repos/minos/target/aarch64-unknown-linux-musl/release/minos-guest

confinement: hardware, seatbelt, rlimits
dedicated user: missing (deferred, reduced hardening)

job sweep: /private/tmp/minos-501
  no job directories left over
builder: docker found at /Applications/Docker.app/Contents/Resources/bin/docker, daemon not running: start Docker Desktop before minos env build

result: every check passed
```

The VM test, verbatim (it prints only `JobResult` values: validated outcomes, host-measured usage, the backend, and
the stages that have log tails, never a guest byte):

```
$ MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest MINOS_KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib cargo test -p minos-cli --test runner_vm -- --ignored --nocapture --test-threads=1
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running tests/runner_vm.rs (target/debug/deps/runner_vm-21f1cfb19a5f04b6)

running 3 tests
test baseline_scores_on_real_vms ... == baseline: outcome Scored(Scorecard { metrics: {MetricName("probe_ok"): ScoredMetric { value: 1.0, held_out: false }, MetricName("probe_score"): ScoredMetric { value: 7.0, held_out: false }} }); candidate usage 1494 vcpu_ms, 1529856 mem_mib_ms; judge usage Some((644, 659456)); backend Krun on Arm64; logs [Build, Run, Judge]
ok
test candidate_artifacts_reach_the_judge_on_real_vms ... == candidate: outcome Scored(Scorecard { metrics: {MetricName("probe_ok"): ScoredMetric { value: 1.0, held_out: false }, MetricName("probe_score"): ScoredMetric { value: 7.0, held_out: false }} }); candidate usage 646 vcpu_ms, 661504 mem_mib_ms; judge usage Some((626, 641024)); backend Krun on Arm64; logs [Build, Run, Judge]
ok
test failed_gate_is_rejected_on_real_vms ... == failed gate: outcome Rejected(GateFailed { gate: MetricName("probe_ok") }); candidate usage 718 vcpu_ms, 735232 mem_mib_ms; judge usage Some((664, 679936)); backend Krun on Arm64; logs [Build, Run, Judge]
ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 5.48s
```

What it shows:

- **LOOP-01, LOOP-02:** each trial booted a Candidate VM and then a Judge VM through the production
  `Runner<KrunBackend, FileStore>` behind `RunnerJobs`, after `JobRunner::check` passed. The candidate test's run
  stage wrote 4096 bytes under `/out`; the runner re-packed `out/` as the judge's input and the judge scored.
- **LOOP-03:** usage is host-measured for both VMs: `mem_mib_ms` equals `vcpu_ms / 2 * 2048` in every row (for
  example 1494 vcpu_ms and 1529856 mem_mib_ms for the baseline's candidate), with no tokens.
- **JUDGE-03:** the judge whose `probe_ok` was 0 gave `Rejected(GateFailed { gate: probe_ok })` with no scorecard.
- **JUDGE-04:** every result records `Krun` on `Arm64`, and its build, run, and judge log tails are in the store under
  `MAX_STAGE_LOG_BYTES`.

test result: ok. 3 passed
