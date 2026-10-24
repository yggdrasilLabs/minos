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

## Final gate

Run on 2026-10-24 from the repository root against one final build at HEAD `22471c1` (docs(07-17): name the VM tests' path variables without spelling them in locate.rs), in this order, with `TRYBUILD`, `MINOS_CLAUDE_LIVE`, and every `MINOS_*` variable unset. `rustc --version`: rustc 1.97.1 (8bab26f4f 2026-07-14); `sw_vers -productVersion`: 27.0.1. Every step exited 0. Output is verbatim; the only edit is the home directory written as `~`. No VM was launched and no model call was made: `runner_vm` reports `1 passed; 3 ignored` (its one non-ignored test parses the fixture manifests, 07-14), the krun `exit` and `vm` targets report `4 ignored` and `11 ignored`, and the live provider target reports `3 ignored`. The real-VM run of the production runner is the record in "## Runner on real VMs (07-14)" above (`3 passed`), which this plan did not rerun.

### A first pass, and the one fix

A first pass of the whole sequence at HEAD `69562a5` passed every step except the test-variable check, which printed one line:

```text
crates/minos-cli/src/locate.rs:4://! environment can redirect a launch to another binary or kernel. `MINOS_VMM`, `MINOS_GUEST`, and `MINOS_KRUNFW`
```

That line is the module documentation of `locate.rs` (written in 05-14), which states that the three variables are read only by the ignored VM tests, "never here". No code in `crates/minos-cli/src` reads them (`grep -rnE 'env::var|var_os' crates/minos-cli/src` finds only `HOME` and `PATH` reads in `host.rs` and `builder.rs`). The fix is in the owning file, not in the check: commit `22471c1` rewords the comment to name "the three path variables of the ignored VM tests (CLAUDE.md Commands)" with the same meaning. The whole sequence was then rerun from the first step; the rerun is recorded below.

### The six gate commands

1. `cargo fmt --all --check`: exit 0, no output.

2. `cargo clippy --workspace --all-targets -- -D warnings`: exit 0.

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.99s
```

3. `cargo test --workspace`: exit 0. 88 result lines; summed over them, 1673 passed, 0 failed, 26 ignored. The ignored tests are the ignored VM tests (`minos-backend-krun` `--test exit` 4 and `--test vm` 11, its unit tests `probe_host_on_this_mac` and `seatbelt_denial_probes_hold`, and `minos-cli` `--test runner_vm` 3), the live provider tests (`minos-propose` `--test live` 3), `docker_env_build_reproduces` (`--test env_build`), `doctor_runs_on_this_mac` (`--test host_cmd`), and the one ignored case in `minos-sandbox` `--test output_tar`. The two timing-sensitive `minos-propose` process tests logged in deferred-items.md (07-08) passed in this run. The driver target's modules ran 14 `flow::`, 5 `caps::`, 6 `breaker::`, 4 `stops::`, 3 `pins::`, 2 `resume::`, and 4 `lock::` tests.

```text
Running unittests src/lib.rs (target/debug/deps/minos_backend_krun-de667b6d468a05d5)
  test result: ok. 105 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 1.01s
Running tests/exit.rs (target/debug/deps/exit-89797bbe80950b05)
  test result: ok. 0 passed; 0 failed; 4 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/vm.rs (target/debug/deps/vm-e5c499f08fd31caf)
  test result: ok. 0 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running unittests src/lib.rs (target/debug/deps/minos_cli-c57c0ceb80c02b1e)
  test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
Running unittests src/main.rs (target/debug/deps/minos-8be5801841aa92f3)
  test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/env_build.rs (target/debug/deps/env_build-b6919ecdd6fff5d3)
  test result: ok. 11 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.32s
Running tests/host_cmd.rs (target/debug/deps/host_cmd-59ed9ec6906cd89a)
  test result: ok. 3 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.64s
Running tests/judge_push.rs (target/debug/deps/judge_push-6a38f43bdf819406)
  test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.28s
Running tests/runner_vm.rs (target/debug/deps/runner_vm-52eaec11667c4c8f)
  test result: ok. 1 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running unittests src/lib.rs (target/debug/deps/minos_core-49fc12ea1cbf8830)
  test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
Running tests/blob_hasher.rs (target/debug/deps/blob_hasher-707f9e80b322bd98)
  test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.42s
Running tests/breaker.rs (target/debug/deps/breaker-2ff3318f14691bb8)
  test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
Running tests/chunk.rs (target/debug/deps/chunk-a347b9643cd47640)
  test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.36s
Running tests/compile_fail.rs (target/debug/deps/compile_fail-a3a00ca939061789)
  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s
Running tests/cumulative.rs (target/debug/deps/cumulative-9d1e976812ba5839)
  test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/golden_ids.rs (target/debug/deps/golden_ids-497e7c7cba4ea636)
  test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
Running tests/golden_wire.rs (target/debug/deps/golden_wire-262db30471de5b5d)
  test result: ok. 40 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
Running tests/guest_wire.rs (target/debug/deps/guest_wire-4dcba75952710cce)
  test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.33s
Running tests/lock.rs (target/debug/deps/lock-0b9c75eeaf2fb245)
  test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.34s
Running tests/manifest.rs (target/debug/deps/manifest-a0fa7e3a5345ed90)
  test result: ok. 108 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/money.rs (target/debug/deps/money-7d18a657477363e4)
  test result: ok. 48 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
Running tests/prop_apply.rs (target/debug/deps/prop_apply-30468656d0d75d83)
  test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
Running tests/prop_diff.rs (target/debug/deps/prop_diff-197ae67a0139e54c)
  test result: ok. 67 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
Running tests/prop_fold.rs (target/debug/deps/prop_fold-1f0fe15139f12fe7)
  test result: ok. 53 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.65s
Running tests/prop_keys.rs (target/debug/deps/prop_keys-17df50f8439e2e3f)
  test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.55s
Running tests/prop_metric.rs (target/debug/deps/prop_metric-4a01fc67e5382ed1)
  test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
Running tests/prop_path.rs (target/debug/deps/prop_path-6e2e2ce60e1618ac)
  test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
Running tests/prop_policy.rs (target/debug/deps/prop_policy-035f70840cfa23d8)
  test result: ok. 48 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
Running tests/render.rs (target/debug/deps/render-8edc7b6a74cf13c1)
  test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.71s
Running tests/report.rs (target/debug/deps/report-681406ca40a8ab98)
  test result: ok. 47 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.17s
Running tests/route.rs (target/debug/deps/route-77df815e137baac1)
  test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/sweep.rs (target/debug/deps/sweep-150d5ae104985aaf)
  test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
Running unittests src/lib.rs (target/debug/deps/minos_guest-ff9cca34304ea5ea)
  test result: ok. 63 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
Running unittests src/main.rs (target/debug/deps/minos_guest-2d97dc2b4d1632e5)
  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running unittests src/bin/minos-probe.rs (target/debug/deps/minos_probe-5627d13d51fedf66)
  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running unittests src/lib.rs (target/debug/deps/minos_loop-a00d7bc7c115d5ac)
  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/blocks.rs (target/debug/deps/blocks-11c7a318f37d4cc5)
  test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
Running tests/driver/main.rs (target/debug/deps/driver-d1655d2a7ed01692)
  test result: ok. 38 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.47s
Running tests/stage.rs (target/debug/deps/stage-369e7f0e0053c16a)
  test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.18s
Running unittests src/lib.rs (target/debug/deps/minos_propose-a32aa428f41afb82)
  test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.45s
Running tests/argv_golden.rs (target/debug/deps/argv_golden-09453002981372c8)
  test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
Running tests/classify.rs (target/debug/deps/classify-236650040b6f26f1)
  test result: ok. 40 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/claude_cli.rs (target/debug/deps/claude_cli-13682ca879a63f71)
  test result: ok. 33 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.41s
Running tests/held_out.rs (target/debug/deps/held_out-a8bd5b9799a069d6)
  test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
Running tests/history.rs (target/debug/deps/history-f95f4dab2411ab06)
  test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
Running tests/live.rs (target/debug/deps/live-bf4d1a1b44e80a99)
  test result: ok. 1 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/locate.rs (target/debug/deps/locate-c7dbb9b19ee5a88f)
  test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
Running tests/parse.rs (target/debug/deps/parse-1de677cb9e8c5df8)
  test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.24s
Running tests/prompt.rs (target/debug/deps/prompt-f123387b0b58cc6b)
  test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
Running tests/proposer.rs (target/debug/deps/proposer-b60b5d3333082710)
  test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/provider.rs (target/debug/deps/provider-0ae5ad271e5aa784)
  test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
Running tests/schema.rs (target/debug/deps/schema-65de61a220300659)
  test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
Running tests/single_shot.rs (target/debug/deps/single_shot-ff20b4601b1a68a5)
  test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/sweep.rs (target/debug/deps/sweep-05223db0e614f4e0)
  test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running unittests src/lib.rs (target/debug/deps/minos_runner-a714a6fc4b190877)
  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/map.rs (target/debug/deps/map-c8a9de130b694980)
  test result: ok. 53 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.30s
Running tests/pipeline.rs (target/debug/deps/pipeline-b6d2b33949970350)
  test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
Running tests/usage.rs (target/debug/deps/usage-780fccf98eeecc26)
  test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running unittests src/lib.rs (target/debug/deps/minos_sandbox-4b7e5db11ee5745b)
  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/backend.rs (target/debug/deps/backend-abb00b226a85bc2a)
  test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/blob_source.rs (target/debug/deps/blob_source-d424f0dbb56fc55a)
  test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/compile_fail.rs (target/debug/deps/compile_fail-0c7906e0638ae0d5)
  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.88s
Running tests/output_tar.rs (target/debug/deps/output_tar-adbd864b297ee011)
  test result: ok. 35 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.62s
Running tests/preflight.rs (target/debug/deps/preflight-8a0a614f85e9d937)
  test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/spec.rs (target/debug/deps/spec-939ba99885943f40)
  test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
Running tests/untrusted.rs (target/debug/deps/untrusted-5b1c00c016a9cea1)
  test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
Running unittests src/lib.rs (target/debug/deps/minos_source-5429adc68d00303c)
  test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
Running tests/clean.rs (target/debug/deps/clean-4d8227224d71bbcc)
  test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
Running tests/denylist.rs (target/debug/deps/denylist-9d6bd90c60079a07)
  test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/judge.rs (target/debug/deps/judge-1848dd9d099c7c3a)
  test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.21s
Running tests/pack.rs (target/debug/deps/pack-c81bffdb36d1cb5a)
  test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
Running tests/refusals.rs (target/debug/deps/refusals-ecc47c6e3342518b)
  test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.19s
Running tests/scan.rs (target/debug/deps/scan-72ef2660ffb8f8c1)
  test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
Running unittests src/lib.rs (target/debug/deps/minos_store-f91087a99a88b128)
  test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/contract.rs (target/debug/deps/contract-0ccd6f1ff8a87df3)
  test result: ok. 43 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.43s
Running tests/file_blobs.rs (target/debug/deps/file_blobs-7940e65c52fed831)
  test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
Running tests/file_ledger.rs (target/debug/deps/file_ledger-498662032decf0ee)
  test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.24s
Running unittests src/main.rs (target/debug/deps/minos_vmm-04be6e1974430353)
  test result: ok. 38 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
Doc-tests minos_backend_krun
  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Doc-tests minos_cli
  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Doc-tests minos_core
  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Doc-tests minos_guest
  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Doc-tests minos_loop
  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Doc-tests minos_propose
  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Doc-tests minos_runner
  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Doc-tests minos_sandbox
  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Doc-tests minos_source
  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Doc-tests minos_store
  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

4. `cargo deny check`: exit 0. Every warning header and the last line (the same seven duplicate-crate warnings as the Phase 5 and Phase 6 final gates):

```text
warning[duplicate]: found 2 duplicate entries for crate 'bitflags'
warning[duplicate]: found 2 duplicate entries for crate 'getrandom'
warning[duplicate]: found 2 duplicate entries for crate 'miniz_oxide'
warning[duplicate]: found 2 duplicate entries for crate 'nix'
warning[duplicate]: found 2 duplicate entries for crate 'rand_chacha'
warning[duplicate]: found 2 duplicate entries for crate 'rand_core'
warning[duplicate]: found 2 duplicate entries for crate 'syn'

advisories ok, bans ok, licenses ok, sources ok
```

5. `cargo doc --workspace --no-deps`: exit 0, no warning or error line.

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.64s
   Generated ~/Repos/minos/target/doc/minos_backend_krun/index.html and 12 other files
```

6. `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`: exit 0.

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.09s
```

### C-01, C-02, and the Phase 7 checks

7. C-01, `for f in crates/*/Cargo.toml; do grep -A1 -xF '[lints]' "$f" | grep -qxF 'workspace = true' && echo "ok $f" || echo "MISSING $f"; done`: eleven `ok` lines, no `MISSING`.

```text
ok crates/minos-backend-krun/Cargo.toml
ok crates/minos-cli/Cargo.toml
ok crates/minos-core/Cargo.toml
ok crates/minos-guest/Cargo.toml
ok crates/minos-loop/Cargo.toml
ok crates/minos-propose/Cargo.toml
ok crates/minos-runner/Cargo.toml
ok crates/minos-sandbox/Cargo.toml
ok crates/minos-source/Cargo.toml
ok crates/minos-store/Cargo.toml
ok crates/minos-vmm/Cargo.toml
```

8. C-02, `grep -rnE '#!?\[allow\(' crates --include='*.rs'`: no output (grep exit 1, no match).

9. LOOP-01, `grep -rnE '\.(take_status|into_scorecard|into_output|into_log_excerpt|into_diff_texts|store_judge_input|store_tail)\(' crates/*/src --include='*.rs' | grep -vE '^crates/minos-(runner|sandbox)/'`: no output (pipeline exit 1). No production crate other than `minos-runner` and `minos-sandbox` calls an `Untrusted` exit or a sink exit.

10. Lane check, `grep -n 'minos-' crates/minos-loop/Cargo.toml crates/minos-runner/Cargo.toml`: the runner names only `minos-core`, `minos-store`, and `minos-sandbox`; the loop names only `minos-core`, `minos-store`, and `minos-propose`. Neither names a backend crate, and the loop does not name `minos-sandbox` or `minos-runner`.

```text
crates/minos-runner/Cargo.toml:2:name = "minos-runner"
crates/minos-runner/Cargo.toml:14:minos-core.workspace = true
crates/minos-runner/Cargo.toml:15:minos-store.workspace = true
crates/minos-runner/Cargo.toml:16:minos-sandbox.workspace = true
crates/minos-loop/Cargo.toml:2:name = "minos-loop"
crates/minos-loop/Cargo.toml:14:minos-core.workspace = true
crates/minos-loop/Cargo.toml:15:minos-store.workspace = true
crates/minos-loop/Cargo.toml:16:minos-propose.workspace = true
```

11. Test-variable check, `grep -rn 'MINOS_VMM\|MINOS_GUEST\|MINOS_KRUNFW' crates/minos-cli/src`: no output (grep exit 1), after the fix above.

12. Validation targets: every `--test <name>` that 07-VALIDATION.md names (22 names: `blocks`, `compile_fail`, `contract`, `cumulative`, `driver`, `env_build`, `file_ledger`, `golden_ids`, `golden_wire`, `judge`, `judge_push`, `lock`, `manifest`, `map`, `money`, `pipeline`, `prop_fold`, `prop_policy`, `runner_vm`, `stage`, `untrusted`, `usage`) exists as `crates/*/tests/<name>.rs` or `crates/*/tests/<name>/main.rs`, and each ran in step 3. The plan's own loop, `ls crates/*/tests/$t.rs crates/*/tests/$t/main.rs`, printed `MISSING` for all 22 under bash, because `ls` exits non-zero when any one of its two arguments is absent and no target has both forms. The check was run instead with `compgen -G` on each form, an either-or test, and printed 22 `ok` lines and no `MISSING`.

Result: every gate command, C-01, C-02, the LOOP-01 check, the lane check, and the test-variable check pass on the final code.
