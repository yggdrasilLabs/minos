# Phase 4 Evidence

Captured command output for every deliberate violation in Phase 4.

## Layering: minos-loop cannot link minos-source (04-01)

Deliberate violation, never committed: `minos-source.workspace = true` added to `crates/minos-loop/Cargo.toml`
`[dependencies]`. Command:

```
cargo deny check bans 2>&1 | tail -40
```

The refusal (exit 2, `bans FAILED`):

```
error[banned]: crate 'minos-source = 0.1.0' is explicitly banned
   ┌─ /Users/seanmcauliffe/Repos/minos/deny.toml:68:14
   │
68 │   { crate = "minos-source", wrappers = ["minos-cli"], reason = "layering: only the CLI packs a source tree (CLI-11); it execs git read-only for the clean check (04-CONTEXT D-02)" },
   │              ━━━━━━━━━━━━                                       ───────────────────────────────────────────────────────────────────────────────────────────────────────────────── reason
   │              │                                                   
   │              banned here
   │
   ├ minos-source v0.1.0 (*)

[unused-wrapper warnings elided]
bans FAILED
```

The text graph abbreviates the dependents of `minos-source` as `(*)`. The same violation's JSON diagnostic
names them: `minos-cli` (allowed) and `minos-loop` (refused).

```
$ cargo deny --format json check bans 2>&1 | grep '"banned"'
{"fields":{"code":"banned","graphs":[{"Krate":{"name":"minos-source","version":"0.1.0"},"parents":[{"Krate":{"name":"minos-cli","version":"0.1.0"}},{"Krate":{"name":"minos-loop","version":"0.1.0"},"parents":[{"Krate":{"name":"minos-cli","version":"0.1.0"},"repeat":true}]}]}],"labels":[{"column":14,"line":68,"message":"banned here","span":"minos-source"},{"column":65,"line":68,"message":"reason","span":"layering: only the CLI packs a source tree (CLI-11); it execs git read-only for the clean check (04-CONTEXT D-02)"}],"message":"crate 'minos-source = 0.1.0' is explicitly banned","severity":"error"},"type":"diagnostic"}
```

```
$ cargo tree -i minos-source -e normal --target all
minos-source v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-source)
├── minos-cli v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-cli)
└── minos-loop v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-loop)
    └── minos-cli v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-cli)
```

Restored with `git checkout -- crates/minos-loop/Cargo.toml` and `cargo build -p minos-source`;
`cargo deny check` then printed `advisories ok, bans ok, licenses ok, sources ok`.

## Clippy bans fire (04-01)

Each crate got a temporary `tests/zz_clippy_probe.rs` with one `#[test]` function per `clippy.toml` entry that
calls the banned method or names the banned type once (no `unwrap`, `expect`, or indexing; results bound with
`let _ =`). The public re-export paths (`tar::Archive`, `tar::Entry`, `tar::Builder`, `tar::Header`,
`ignore::WalkBuilder`) resolve and fire; no definition-path form was needed. Each run below reports exactly one
`disallowed_methods` or `disallowed_types` error per entry and no other diagnostic.

### minos-store (4 entries, 4 errors)

```
$ cargo clippy -p minos-store --all-targets -- -D warnings 2>&1 | grep -E 'disallowed|-->'
error: use of a disallowed method `std::fs::File::create`
 --> crates/minos-store/tests/zz_clippy_probe.rs:5:13
  = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_methods
  = note: `-D clippy::disallowed-methods` implied by `-D warnings`
  = help: to override `-D warnings` add `#[allow(clippy::disallowed_methods)]`
error: use of a disallowed method `std::fs::write`
  --> crates/minos-store/tests/zz_clippy_probe.rs:10:13
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_methods
error: use of a disallowed type `std::collections::HashMap`
  --> crates/minos-store/tests/zz_clippy_probe.rs:15:13
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_types
   = note: `-D clippy::disallowed-types` implied by `-D warnings`
   = help: to override `-D warnings` add `#[allow(clippy::disallowed_types)]`
error: use of a disallowed type `std::collections::HashSet`
  --> crates/minos-store/tests/zz_clippy_probe.rs:20:13
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_types
error: could not compile `minos-store` (test "zz_clippy_probe") due to 4 previous errors
```

### minos-sandbox (7 entries, 7 errors)

```
$ cargo clippy -p minos-sandbox --all-targets -- -D warnings 2>&1 | grep -E 'disallowed|-->'
error: use of a disallowed method `tar::Archive::unpack`
 --> crates/minos-sandbox/tests/zz_clippy_probe.rs:9:23
  = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_methods
  = note: `-D clippy::disallowed-methods` implied by `-D warnings`
  = help: to override `-D warnings` add `#[allow(clippy::disallowed_methods)]`
error: use of a disallowed method `tar::Entry::unpack`
  --> crates/minos-sandbox/tests/zz_clippy_probe.rs:17:27
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_methods
error: use of a disallowed method `tar::Entry::unpack_in`
  --> crates/minos-sandbox/tests/zz_clippy_probe.rs:27:27
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_methods
error: use of a disallowed type `std::collections::HashMap`
  --> crates/minos-sandbox/tests/zz_clippy_probe.rs:34:13
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_types
   = note: `-D clippy::disallowed-types` implied by `-D warnings`
   = help: to override `-D warnings` add `#[allow(clippy::disallowed_types)]`
error: use of a disallowed type `std::collections::HashSet`
  --> crates/minos-sandbox/tests/zz_clippy_probe.rs:39:13
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_types
error: use of a disallowed type `std::fs::File`
  --> crates/minos-sandbox/tests/zz_clippy_probe.rs:44:13
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_types
error: use of a disallowed type `std::fs::OpenOptions`
  --> crates/minos-sandbox/tests/zz_clippy_probe.rs:49:13
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_types
error: could not compile `minos-sandbox` (test "zz_clippy_probe") due to 7 previous errors
```

### minos-source (13 entries, 13 errors)

```
$ cargo clippy -p minos-source --all-targets -- -D warnings 2>&1 | grep -E 'disallowed|-->'
error: use of a disallowed method `tar::Builder::append_data`
  --> crates/minos-source/tests/zz_clippy_probe.rs:10:23
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_methods
   = note: `-D clippy::disallowed-methods` implied by `-D warnings`
   = help: to override `-D warnings` add `#[allow(clippy::disallowed_methods)]`
error: use of a disallowed method `tar::Builder::append_path`
  --> crates/minos-source/tests/zz_clippy_probe.rs:15:23
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_methods
error: use of a disallowed method `tar::Builder::append_path_with_name`
  --> crates/minos-source/tests/zz_clippy_probe.rs:20:23
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_methods
error: use of a disallowed method `tar::Builder::append_file`
  --> crates/minos-source/tests/zz_clippy_probe.rs:26:27
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_methods
error: use of a disallowed method `tar::Builder::append_dir`
  --> crates/minos-source/tests/zz_clippy_probe.rs:32:23
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_methods
error: use of a disallowed method `tar::Builder::append_dir_all`
  --> crates/minos-source/tests/zz_clippy_probe.rs:37:23
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_methods
error: use of a disallowed method `tar::Header::set_metadata`
  --> crates/minos-source/tests/zz_clippy_probe.rs:43:34
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_methods
error: use of a disallowed method `tar::Header::set_metadata_in_mode`
  --> crates/minos-source/tests/zz_clippy_probe.rs:50:34
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_methods
error: use of a disallowed method `ignore::WalkBuilder::max_filesize`
  --> crates/minos-source/tests/zz_clippy_probe.rs:56:43
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_methods
error: use of a disallowed method `std::env::var`
  --> crates/minos-source/tests/zz_clippy_probe.rs:61:13
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_methods
error: use of a disallowed method `std::env::var_os`
  --> crates/minos-source/tests/zz_clippy_probe.rs:66:13
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_methods
error: use of a disallowed type `std::collections::HashMap`
  --> crates/minos-source/tests/zz_clippy_probe.rs:71:13
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_types
   = note: `-D clippy::disallowed-types` implied by `-D warnings`
   = help: to override `-D warnings` add `#[allow(clippy::disallowed_types)]`
error: use of a disallowed type `std::collections::HashSet`
  --> crates/minos-source/tests/zz_clippy_probe.rs:76:13
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_types
error: could not compile `minos-source` (test "zz_clippy_probe") due to 13 previous errors
```

The three probe files were deleted after capture; `cargo clippy -p minos-store -p minos-sandbox -p minos-source
--all-targets -- -D warnings` then exits 0.

## Method for the 04-09 sections

Each section first runs the test unmodified with `--exact` and shows its `test result: ok` line. Then a probe
(a temporary `println!` of the counts or of the error's `Display`, added to the test) is run with `--nocapture`, and
its output is pasted. Each probed file was restored with `git checkout -- <file>` straight after the capture, and
`git status --porcelain crates` printed nothing afterwards. No probe edit is committed.

## Torn final line is tolerated (STORE-02, D-13)

```
$ cargo test -p minos-store --test file_ledger -- --exact torn_tail_without_newline_is_tolerated
test torn_tail_without_newline_is_tolerated ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 23 filtered out; finished in 0.03s
```

The test appends three lines, then writes the first half of a fourth line with no newline, as a crash mid-append
would. Probe: print the read's line count, torn-tail count, and the file length before and after the next append.

```
$ cargo test -p minos-store --test file_ledger -- --exact torn_tail_without_newline_is_tolerated --nocapture
PROBE before append: lines=3 torn_tail=1 file_bytes=752
PROBE after append: lines=4 torn_tail=0 file_bytes=860
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 23 filtered out; finished in 0.04s
```

The read returns the three complete lines and reports one torn tail without truncating (the test asserts the bytes
are unchanged). The next append cut the 108 torn bytes (752 - 108 = 644, the three complete lines) and wrote the
216-byte fourth line (644 + 216 = 860); the test asserts the file equals the three lines plus the new line.
Restored with `git checkout -- crates/minos-store/tests/file_ledger.rs`.

## Unknown v tag refuses the ledger (STORE-02)

```
$ cargo test -p minos-store --test file_ledger -- --exact an_unknown_version_mid_file_is_refused
test an_unknown_version_mid_file_is_refused ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 23 filtered out; finished in 0.00s
```

The ledger holds three lines; line 2 has its `record.v` edited from `"1"` to `"2"`. Probe: write the same bytes
to a fresh store and print the `StoreError` Display of a read and of an append.

```
$ cargo test -p minos-store --test file_ledger -- --exact an_unknown_version_mid_file_is_refused --nocapture
PROBE read: the ledger of run 6a65b71b41e24c3b7d21e317e6a8fc29a41fcd29085ec93bbe13aa9b464c56cb is corrupt at line 2: a record has a version this build of minos does not read; upgrade minos
PROBE append: the ledger of run 6a65b71b41e24c3b7d21e317e6a8fc29a41fcd29085ec93bbe13aa9b464c56cb is corrupt at line 2: a record has a version this build of minos does not read; upgrade minos
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 23 filtered out; finished in 0.00s
```

The error names the run, the 1-based line, and the kind (`CorruptKind::UnknownVersion`), and no file bytes. The
test also asserts that the refused append left the file byte-identical. Restored with
`git checkout -- crates/minos-store/tests/file_ledger.rs`.

## Tampered blob fails its check (STORE-04)

```
$ cargo test -p minos-store --test file_blobs -- --exact tampered_blob_is_refused
test tampered_blob_is_refused ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 7 filtered out; finished in 0.03s
```

The test stores `abc`, flips the first byte of the blob file on disk, and calls `get`. Probe: print the error's
Display.

```
$ cargo test -p minos-store --test file_blobs -- --exact tampered_blob_is_refused --nocapture
PROBE get: blob ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad does not hash to its id
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 7 filtered out; finished in 0.03s
```

`ba7816bf...15ad` is SHA-256(`abc`). The error is `StoreError::BlobTampered`, and the flipped bytes never reach the
caller. Restored with `git checkout -- crates/minos-store/tests/file_blobs.rs`.

## Planted secret blocks the pack (CLI-11, D-06)

```
$ cargo test -p minos-source --test refusals -- --exact planted_secret_blocks
test planted_secret_blocks ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 11 filtered out; finished in 0.01s
```

The test plants an AWS-access-key-shaped value (assembled at run time, so the repository holds no such literal) on
line 4 of `src/lib.rs` and packs with `CleanPolicy::AllowDirty`, so the scan cannot be skipped by the dirty-tree
path. Probe: print the `SnapshotError::Secrets` Display, and print whether any 4-byte window of the planted value
appears in that text.

```
$ cargo test -p minos-source --test refusals -- --exact planted_secret_blocks --nocapture
PROBE secrets: the secret scan found 1 match(es); remove them or exclude the files:
  src/lib.rs:4: matches aws-access-key-id
PROBE contains any 4-char window of the key: false
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 11 filtered out; finished in 0.02s
```

Checked before pasting: the text names the file, the line, and the pattern only; no 4-byte window of the planted
value appears in it, and the captured output holds no access-key prefix at all. The vector itself is not pasted. Restored
with `git checkout -- crates/minos-source/tests/refusals.rs`.

## Path outside the grammar fails the pack (CLI-11, D-01)

```
$ cargo test -p minos-source --test refusals -- --exact offenders_are_listed_together
test offenders_are_listed_together ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 11 filtered out; finished in 0.00s
```

The project holds a name with a space, a symlink, a FIFO, two hard-linked files, and a 101-byte final component.
Probe: print the `SnapshotError::Offenders` Display.

```
$ cargo test -p minos-source --test refusals -- --exact offenders_are_listed_together --nocapture
PROBE offenders: the project holds entries that cannot be packed:
  a.txt: a file with more than one hard link
  b.txt: a file with more than one hard link
  bad name.txt: the name is outside the path grammar ([A-Za-z0-9._+-], no leading -)
  link: a symbolic link; links are never followed or packed
  pipe: not a regular file (a FIFO, socket, or device)
  xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx.txt: the path does not fit a ustar header (255 bytes, final component 100)
test offenders_are_listed_together ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 11 filtered out; finished in 0.00s
```

Every offender is listed in one error, sorted by path, and the pack produces no tarball. Restored with
`git checkout -- crates/minos-source/tests/refusals.rs`.

## Sandbox invariants are compile errors (SBX-01 to SBX-04, D-14)

```
$ env -u TRYBUILD cargo test -p minos-sandbox --test compile_fail
test tests/ui/fail/candidate_network.rs [should fail to compile] ... ok
test tests/ui/fail/candidate_scorecard.rs [should fail to compile] ... ok
test tests/ui/fail/candidate_secrets.rs [should fail to compile] ... ok
test tests/ui/fail/cleared_clone.rs [should fail to compile] ... ok
test tests/ui/fail/cleared_literal.rs [should fail to compile] ... ok
test tests/ui/fail/judge_network.rs [should fail to compile] ... ok
test tests/ui/fail/launch_bare_plan.rs [should fail to compile] ... ok
test tests/ui/fail/launch_plan_literal.rs [should fail to compile] ... ok
test tests/ui/fail/untrusted_debug.rs [should fail to compile] ... ok
test tests/ui/fail/untrusted_deref.rs [should fail to compile] ... ok
test tests/ui/fail/untrusted_display.rs [should fail to compile] ... ok
test tests/ui/fail/untrusted_field.rs [should fail to compile] ... ok
test tests/ui/pass/judge_flow.rs [should pass] ... ok
test sandbox_invariants_are_compile_time_facts ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.21s
```

(trybuild's ANSI colour codes stripped.) The first `error` line of each committed snapshot,
`grep -m1 '^error' crates/minos-sandbox/tests/ui/fail/<file>`:

```
candidate_network.stderr: error[E0599]: no method named `network` found for struct `SandboxSpec<R>` in the current scope
candidate_scorecard.stderr: error[E0599]: no method named `into_scorecard` found for struct `Untrusted<GuestOutput<minos_sandbox::Candidate>>` in the current scope
candidate_secrets.stderr: error[E0599]: no method named `secrets` found for struct `SandboxSpec<minos_sandbox::Candidate>` in the current scope
cleared_clone.stderr: error[E0599]: no method named `clone` found for struct `Cleared<R>` in the current scope
cleared_literal.stderr: error[E0451]: field `plan` of struct `Cleared` is private
judge_network.stderr: error[E0599]: no method named `network` found for struct `SandboxSpec<R>` in the current scope
launch_bare_plan.stderr: error[E0308]: mismatched types
launch_plan_literal.stderr: error[E0451]: fields `key`, `root`, `input`, `budget`, `machine`, `arch`, `max_duration_ms`, `judge_data` and `role` of struct `LaunchPlan` are private
untrusted_debug.stderr: error[E0277]: `Untrusted<GuestOutput<minos_sandbox::Judge>>` doesn't implement `Debug`
untrusted_deref.stderr: error[E0614]: type `Untrusted<GuestOutput<minos_sandbox::Judge>>` cannot be dereferenced
untrusted_display.stderr: error[E0277]: `Untrusted<GuestOutput<minos_sandbox::Judge>>` doesn't implement `std::fmt::Display`
untrusted_field.stderr: error[E0616]: field `0` of struct `Untrusted` is private
```

Twelve cases, each a compile error for the intended reason: no role can attach a network, only judge output has
`into_scorecard`, only a Judge spec takes secrets, `Cleared` cannot be cloned or built by literal, `launch` refuses
a bare `LaunchPlan`, `LaunchPlan` cannot be built by literal, and `Untrusted` has no Debug, Display, Deref, or
public field. `git status --porcelain crates` printed nothing after this task.

## Final gate

Run on 2026-10-15 from the repository root, in this order, after the Task 1 and Task 2 commits (HEAD `11f7ed3`),
with `TRYBUILD` unset in the environment. Every step exited 0.

1. `cargo fmt --all --check`: exit 0, no output.
2. `cargo clippy --workspace --all-targets -- -D warnings`: exit 0.
   ```
       Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.20s
   ```
3. `cargo test --workspace` (`TRYBUILD` unset): exit 0. Over every test binary and doc-test run: 911 passed,
   0 failed, 26 ignored. The known flaky `minos-backend-krun` supervise test did not fail. The Phase 4 binaries:
   ```
   minos-store   unittests src/lib.rs      test result: ok. 5 passed; 0 failed; 0 ignored
   minos-store   tests/contract.rs         test result: ok. 24 passed; 0 failed; 0 ignored
   minos-store   tests/file_blobs.rs       test result: ok. 8 passed; 0 failed; 0 ignored
   minos-store   tests/file_ledger.rs      test result: ok. 24 passed; 0 failed; 0 ignored
   minos-sandbox unittests src/lib.rs      test result: ok. 1 passed; 0 failed; 0 ignored
   minos-sandbox tests/backend.rs          test result: ok. 6 passed; 0 failed; 0 ignored
   minos-sandbox tests/compile_fail.rs     test result: ok. 1 passed; 0 failed; 0 ignored
   minos-sandbox tests/output_tar.rs       test result: ok. 35 passed; 0 failed; 1 ignored
   minos-sandbox tests/preflight.rs        test result: ok. 6 passed; 0 failed; 0 ignored
   minos-sandbox tests/spec.rs             test result: ok. 10 passed; 0 failed; 0 ignored
   minos-sandbox tests/untrusted.rs        test result: ok. 11 passed; 0 failed; 0 ignored
   minos-source  unittests src/lib.rs      test result: ok. 7 passed; 0 failed; 0 ignored
   minos-source  tests/clean.rs            test result: ok. 10 passed; 0 failed; 0 ignored
   minos-source  tests/denylist.rs         test result: ok. 5 passed; 0 failed; 0 ignored
   minos-source  tests/pack.rs             test result: ok. 8 passed; 0 failed; 0 ignored
   minos-source  tests/refusals.rs         test result: ok. 12 passed; 0 failed; 0 ignored
   minos-source  tests/scan.rs             test result: ok. 9 passed; 0 failed; 0 ignored
   ```
   The one ignored `output_tar` test is the corpus writer, run by hand only.
4. `cargo deny check`: exit 0.
   ```
   advisories ok, bans ok, licenses ok, sources ok
   ```
   Warnings only: seven `duplicate` entries (bitflags, getrandom, miniz_oxide, nix, rand_chacha, rand_core, syn) and
   three `unused-wrapper` entries for layering edges that do not exist yet; both predate this plan.
5. `cargo doc --workspace --no-deps`: exit 0, no warning or error line.
6. `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`: exit 0.
7. C-01 printed 11 `ok` lines and no `MISSING` line (backend-krun, cli, core, guest, loop, propose, runner,
   sandbox, source, store, vmm). C-02 `grep -rnE '#!?\[allow\(' crates --include='*.rs'` printed nothing (grep
   exit 1).
8. Unchanged bans: `git log --diff-filter=A --format=%h -- crates/minos-source/clippy.toml` is `56467aa`, the 04-01
   commit that created all three crate files, and
   `git log --oneline 56467aa..HEAD -- crates/minos-store/clippy.toml crates/minos-sandbox/clippy.toml crates/minos-source/clippy.toml`
   printed nothing. `git diff --quiet -- crates/minos-core/clippy.toml` exited 0, and no commit since `56467aa`
   touched it.

Every step exited 0, so `cargo clean` ran next, as the last cargo command of this plan:

```
$ cargo clean
     Removed 36320 files, 5.0GiB total
```
