# Phase 6 Evidence

This file collects Phase 6's recorded runs: commands, exit codes, and verbatim output that the design and the plans cite.

## Clippy bans fire (06-02)

`crates/minos-propose/clippy.toml` bans `std::collections::HashMap` and `std::collections::HashSet` (the prompt is
hashed into `PromptHash`, so iteration order must be stable). To prove both paths resolve and fire, `src/lib.rs`
temporarily gained two `#[must_use]` public functions, one constructing a `HashMap<u8, u8>` and one a
`HashSet<u8>` (no `unwrap`, `expect`, or indexing). Each probe line names the type twice (the annotation and the
constructor), so the run reports two errors per entry, all `disallowed_types`, and no other diagnostic. All four
errors carry the configured note `order must be stable: the prompt is hashed into PromptHash; use BTreeMap or
BTreeSet`.

```
$ cargo clippy -p minos-propose --all-targets -- -D warnings 2>&1 | grep -E 'disallowed|-->|could not'
error: use of a disallowed type `std::collections::HashMap`
  --> crates/minos-propose/src/lib.rs:22:50
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_types
   = note: `-D clippy::disallowed-types` implied by `-D warnings`
   = help: to override `-D warnings` add `#[allow(clippy::disallowed_types)]`
error: use of a disallowed type `std::collections::HashMap`
  --> crates/minos-propose/src/lib.rs:22:14
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_types
error: use of a disallowed type `std::collections::HashSet`
  --> crates/minos-propose/src/lib.rs:29:46
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_types
error: use of a disallowed type `std::collections::HashSet`
  --> crates/minos-propose/src/lib.rs:29:14
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_types
error: could not compile `minos-propose` (lib test) due to 4 previous errors
error: could not compile `minos-propose` (lib) due to 4 previous errors
```

Exit code: 101.

The probe functions were then removed and the same command rerun:

```
$ cargo clippy -p minos-propose --all-targets -- -D warnings
    Checking minos-propose v0.1.0 (/Users/seanmcauliffe/Repos/minos/crates/minos-propose)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.07s
```

Exit code: 0.

## Live provider tests (06-09)

Date: 2026-10-20

Requirements: PROP-02 (case a), PROP-03 (cases b and c). Three real `haiku` calls on the user's own subscription (06-CONTEXT D-17 as extended).

- Who ran it: the user, by hand, in their own logged-in terminal at the repository root (branch `main`), and pasted the output back at the 06-09 Task 1 checkpoint. No agent ran a live test, set `MINOS_CLAUDE_LIVE`, or ran the user's `claude`.
- Variables unset with `env -u`: none. The command line carries no `env -u` prefix, and the preflight refused nothing (D-03 extended).
- `claude --version`: `2.1.304 (Claude Code)`
- Link target of `~/.local/bin/claude`: `~/.local/share/claude/versions/2.1.304`
- Exit code: 0

Commands, in order:

```
~/.local/bin/claude --version
readlink ~/.local/bin/claude
MINOS_CLAUDE_LIVE=1 cargo test -p minos-propose --test live -- --ignored --nocapture --test-threads=1; echo "exit: $?"
```

Redactions: the home path is shown as `~`, and any email, organization, or account value would be `<redacted>`. The output held no email, organization, or account value; only the three home paths changed. The suite prints no `auth status` field and discards the CLI's stderr (06-08), and no cost figure is printed. Nothing else is altered; the shell prompt lines are the user's paste as given.

Output:

```text
(base) ➜  minos git:(main) ✗ ~/.local/bin/claude --version
2.1.304 (Claude Code)
(base) ➜  minos git:(main) ✗ readlink ~/.local/bin/claude
~/.local/share/claude/versions/2.1.304
(base) ➜  minos git:(main) ✗ MINOS_CLAUDE_LIVE=1 cargo test -p minos-propose --test live -- --ignored --nocapture --test-threads=1; echo "exit: $?"
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.04s
     Running tests/live.rs (target/debug/deps/live-93d7155bccd12a21)

running 3 tests
test live_golden_argv_in_hostile_cwd ... == claude: 2.1.304 (~/.local/share/claude/versions/2.1.304)
== (b) exit: exit status: 0, stdout bytes: 3017
== (b) marker in the envelope: false
== (b): canary-session-start: absent
== (b): canary-user-prompt: absent
== (b): canary-mcp: absent
== (b) valid proposals: 2, dropped: 0
ok
test live_positive_control_fixture_is_live ... == claude: 2.1.304 (~/.local/share/claude/versions/2.1.304)
== (c) exit: exit status: 0, stdout bytes: 3057
== (c) marker in the envelope: true
== (c): canary-session-start: FIRED
== (c): canary-user-prompt: FIRED
== (c): canary-mcp: FIRED
ok
test live_production_provider_ignores_hostile_project ... == (a) claude version: 2.1.304
== (a) proposals: 2, dropped: 0
== (a) usage: input 2, output 378, cache creation 1119, cache read 2976
== (a) origin model: claude-haiku-5-5, prompt hash: 291e3bd22b6eb0b8e4f84064aac1af71feba5f8107bbab8a0870d58f40bab55c
== (a): canary-session-start: absent
== (a): canary-user-prompt: absent
== (a): canary-mcp: absent
ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 21.41s

exit: 0
```

Under `--nocapture` each test's printed lines sit between its name and its `ok`, so the verbatim output splits each result across lines. Read together, the three results are:

- `test live_production_provider_ignores_hostile_project ... ok`
- `test live_golden_argv_in_hostile_cwd ... ok`
- `test live_positive_control_fixture_is_live ... ok`

and the summary line is `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 21.41s`, then `exit: 0`.

The `1 filtered out` is `the_fixture_and_the_control_argv_are_well_formed`, the suite's one test that is not ignored (06-08), which the `--ignored` run skips.

Canaries in the fixture root after each case (checked 2 s after the CLI exited, `CANARY_SETTLE`), and whether the fixture `CLAUDE.md`'s marker `MINOSCANARYWORD` appeared:

| Case | `canary-session-start` | `canary-user-prompt` | `canary-mcp` | `MINOSCANARYWORD` |
|---|---|---|---|---|
| (a) production provider, process cwd = fixture | absent | absent | absent | in no proposal rationale (asserted; the envelope is not printed for this case) |
| (b) golden argv, CLI cwd = fixture | absent | absent | absent | not in the envelope (`false`) |
| (c) positive control, isolation flags removed, CLI cwd = fixture | FIRED | FIRED | FIRED | in the envelope (`true`) |

Tokens: case (a) reported usage of input 2, output 378, cache creation 1119, and cache read 2976, 4475 tokens in all. Cases (b) and (c) run the CLI directly and print no usage line, so the output does not give the three calls' total; the recorded figure is 4475 tokens for (a) alone. No dollar figure was recorded.

Case (a) recorded `Origin::Model` with the model `claude-haiku-5-5` (the CLI's resolution of the alias `haiku`, as the provider reports it) and prompt hash `291e3bd22b6eb0b8e4f84064aac1af71feba5f8107bbab8a0870d58f40bab55c`. Cases (a) and (b) each parsed 2 valid proposals with 0 dropped, the batch the fixture asks for.

What each case proves, and no more:

- (a) The production provider works end to end on the installed CLI (PROP-02): `ClaudeCli::preflight` re-resolved the native install and passed its `auth status` check, and `Proposer<ClaudeCli>`'s single-shot path returned a `{rationale, files}` structured output that parsed into 2 proposals touching only `src/k.txt`, with token usage recorded. It is not cwd-isolation evidence: `ClaudeCli` always runs the CLI in its own fresh `minos-claude-*` directory, so the fixture was the test process's working directory, never the CLI's. It shows only that the provider does not inherit the caller's directory.
- (b) With the fixture project as the CLI's own working directory, the golden argv's `--safe-mode`, `--strict-mcp-config`, and empty `--mcp-config` kept the fixture's `SessionStart` and `UserPromptSubmit` hooks, its `.mcp.json` server, and its `CLAUDE.md` from taking effect on 2.1.304, and a valid structured output still arrived.
- (c) Without those three flags the same call fired all three canaries and the marker appeared, so the fixture is live on 2.1.304 and (b)'s absent canaries are not vacuous.
- (b) and (c) together carry the PROP-03 proof, on `claude` 2.1.304 under subscription login, for this fixture. They say nothing about a later CLI version, which needs a rerun of this suite.

## Final gate

Run on 2026-10-20 from the repository root against one final build at HEAD `a114940` (docs(06-09): complete live provider run record plan; no code changed in 06-09 or this plan), in this order, with `TRYBUILD` and `MINOS_CLAUDE_LIVE` unset. `rustc --version`: rustc 1.97.1 (8bab26f4f 2026-07-14); `sw_vers -productVersion`: 27.0.1. Every step exited 0. Output is verbatim; the only edit is the home directory written as `~`. No agent ran a live test or the user's `claude`, and the gate made no model call: the live target reports `3 ignored`.

### The six gate commands

1. `cargo fmt --all --check`: exit 0, no output.

2. `cargo clippy --workspace --all-targets -- -D warnings`: exit 0.

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.10s
```

3. `cargo test --workspace`: exit 0. 77 result lines; summed over them, 1357 passed, 0 failed, 23 ignored. The ignored tests are the ignored VM tests (`minos-backend-krun` `--test exit` 4 and `--test vm` 11, its unit tests 2), the ignored host tests in `minos-cli` (`env_build` 1, `host_cmd` 1), one in `minos-sandbox` `output_tar`, and the three live cases in `minos-propose` `--test live`. The live target's line is `test result: ok. 1 passed; 0 failed; 3 ignored`: the one passing test is the model-free fixture check (06-08), so no live case ran. Each binary or doc-test run and its result line, verbatim:

```text
Running unittests src/lib.rs (target/debug/deps/minos_backend_krun-de667b6d468a05d5)
  test result: ok. 105 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 1.01s
Running tests/exit.rs (target/debug/deps/exit-89797bbe80950b05)
  test result: ok. 0 passed; 0 failed; 4 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/vm.rs (target/debug/deps/vm-e5c499f08fd31caf)
  test result: ok. 0 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running unittests src/lib.rs (target/debug/deps/minos_cli-2528dd3032eb0290)
  test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
Running unittests src/main.rs (target/debug/deps/minos-b658dda34b167de6)
  test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/env_build.rs (target/debug/deps/env_build-e2e188592163062a)
  test result: ok. 10 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.29s
Running tests/host_cmd.rs (target/debug/deps/host_cmd-e96a894c3f53f8bd)
  test result: ok. 3 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.60s
Running unittests src/lib.rs (target/debug/deps/minos_core-15efc0247bb89ee9)
  test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
Running tests/blob_hasher.rs (target/debug/deps/blob_hasher-404aa5f8c53c3c0e)
  test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.08s
Running tests/breaker.rs (target/debug/deps/breaker-a65177c074fe7c4f)
  test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
Running tests/chunk.rs (target/debug/deps/chunk-171fba7cfe1d6412)
  test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.19s
Running tests/golden_ids.rs (target/debug/deps/golden_ids-f39770f0b885d95d)
  test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
Running tests/golden_wire.rs (target/debug/deps/golden_wire-8653368d6a99caa8)
  test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
Running tests/guest_wire.rs (target/debug/deps/guest_wire-51c78d87bb923b94)
  test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.30s
Running tests/lock.rs (target/debug/deps/lock-19d347079632309b)
  test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
Running tests/manifest.rs (target/debug/deps/manifest-a5565b731af1e4aa)
  test result: ok. 104 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/money.rs (target/debug/deps/money-f530f79b61383759)
  test result: ok. 44 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
Running tests/prop_apply.rs (target/debug/deps/prop_apply-5b70179c09232b5e)
  test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
Running tests/prop_diff.rs (target/debug/deps/prop_diff-bdf434a540fc4a97)
  test result: ok. 67 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
Running tests/prop_fold.rs (target/debug/deps/prop_fold-ff32a195a2b6776d)
  test result: ok. 40 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.52s
Running tests/prop_keys.rs (target/debug/deps/prop_keys-dbee27f417c8b69b)
  test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.53s
Running tests/prop_metric.rs (target/debug/deps/prop_metric-7848825970330f01)
  test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
Running tests/prop_path.rs (target/debug/deps/prop_path-2b3958736776e24d)
  test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
Running tests/prop_policy.rs (target/debug/deps/prop_policy-87ff2a4a298fa784)
  test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.19s
Running tests/render.rs (target/debug/deps/render-c8dc92c5bb3c0c21)
  test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.66s
Running tests/report.rs (target/debug/deps/report-da118aa3a96756e6)
  test result: ok. 47 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
Running tests/route.rs (target/debug/deps/route-e11fa91934e0004f)
  test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/sweep.rs (target/debug/deps/sweep-1509ad9dfccf5702)
  test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
Running unittests src/lib.rs (target/debug/deps/minos_guest-ff9cca34304ea5ea)
  test result: ok. 63 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
Running unittests src/main.rs (target/debug/deps/minos_guest-2d97dc2b4d1632e5)
  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running unittests src/bin/minos-probe.rs (target/debug/deps/minos_probe-5627d13d51fedf66)
  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running unittests src/lib.rs (target/debug/deps/minos_loop-c6c9bea1a882e2b9)
  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running unittests src/lib.rs (target/debug/deps/minos_propose-a32aa428f41afb82)
  test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.32s
Running tests/argv_golden.rs (target/debug/deps/argv_golden-09453002981372c8)
  test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
Running tests/classify.rs (target/debug/deps/classify-236650040b6f26f1)
  test result: ok. 39 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/claude_cli.rs (target/debug/deps/claude_cli-13682ca879a63f71)
  test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.33s
Running tests/held_out.rs (target/debug/deps/held_out-a8bd5b9799a069d6)
  test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
Running tests/history.rs (target/debug/deps/history-f95f4dab2411ab06)
  test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
Running tests/live.rs (target/debug/deps/live-bf4d1a1b44e80a99)
  test result: ok. 1 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/locate.rs (target/debug/deps/locate-c7dbb9b19ee5a88f)
  test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
Running tests/parse.rs (target/debug/deps/parse-1de677cb9e8c5df8)
  test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.20s
Running tests/prompt.rs (target/debug/deps/prompt-f123387b0b58cc6b)
  test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
Running tests/proposer.rs (target/debug/deps/proposer-b60b5d3333082710)
  test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/provider.rs (target/debug/deps/provider-0ae5ad271e5aa784)
  test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
Running tests/schema.rs (target/debug/deps/schema-65de61a220300659)
  test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
Running tests/single_shot.rs (target/debug/deps/single_shot-ff20b4601b1a68a5)
  test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/sweep.rs (target/debug/deps/sweep-05223db0e614f4e0)
  test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running unittests src/lib.rs (target/debug/deps/minos_runner-dfbc0be15af3b718)
  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running unittests src/lib.rs (target/debug/deps/minos_sandbox-4b7e5db11ee5745b)
  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/backend.rs (target/debug/deps/backend-abb00b226a85bc2a)
  test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/blob_source.rs (target/debug/deps/blob_source-d424f0dbb56fc55a)
  test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/compile_fail.rs (target/debug/deps/compile_fail-0c7906e0638ae0d5)
  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.31s
Running tests/output_tar.rs (target/debug/deps/output_tar-adbd864b297ee011)
  test result: ok. 35 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.52s
Running tests/preflight.rs (target/debug/deps/preflight-8a0a614f85e9d937)
  test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/spec.rs (target/debug/deps/spec-939ba99885943f40)
  test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
Running tests/untrusted.rs (target/debug/deps/untrusted-5b1c00c016a9cea1)
  test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
Running unittests src/lib.rs (target/debug/deps/minos_source-5429adc68d00303c)
  test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
Running tests/clean.rs (target/debug/deps/clean-4d8227224d71bbcc)
  test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
Running tests/denylist.rs (target/debug/deps/denylist-9d6bd90c60079a07)
  test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/pack.rs (target/debug/deps/pack-c81bffdb36d1cb5a)
  test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
Running tests/refusals.rs (target/debug/deps/refusals-ecc47c6e3342518b)
  test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.20s
Running tests/scan.rs (target/debug/deps/scan-72ef2660ffb8f8c1)
  test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
Running unittests src/lib.rs (target/debug/deps/minos_store-f91087a99a88b128)
  test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/contract.rs (target/debug/deps/contract-0ccd6f1ff8a87df3)
  test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.62s
Running tests/file_blobs.rs (target/debug/deps/file_blobs-7940e65c52fed831)
  test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s
Running tests/file_ledger.rs (target/debug/deps/file_ledger-498662032decf0ee)
  test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.57s
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

4. `cargo deny check`: exit 0. Last line and every warning header (the same seven duplicate-crate warnings as the Phase 5 final gate):

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
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
   Generated ~/Repos/minos/target/doc/minos_backend_krun/index.html and 12 other files
```

6. `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`: exit 0.

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.88s
```

### C-01 and C-02

C-01, `for f in crates/*/Cargo.toml; do grep -A1 -xF '[lints]' "$f" | grep -qxF 'workspace = true' && echo "ok $f" || echo "MISSING $f"; done`: eleven `ok` lines, no `MISSING` line.

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

C-02, `grep -rnE '#!?\[allow\(' crates --include='*.rs'`: printed nothing, grep exit 1 (0 bytes of output).

### Validation contract targets

`cargo test -p minos-propose --lib claude_cli::process` (the one non-`--test` command 06-VALIDATION.md names): exit 0, `test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`, all ten in `claude_cli::process::tests`. Every `--test <name>` in 06-VALIDATION.md has its file under `crates/minos-propose/tests/` or `crates/minos-core/tests/` (06-10 Task 1 verify loop, no `MISSING` line).

### Not run by this plan

- The live suite (`MINOS_CLAUDE_LIVE=1 cargo test -p minos-propose --test live -- --ignored ...`): run by the user at 06-09's checkpoint and recorded above in "Live provider tests (06-09)"; not rerun.
- The ignored VM tests (`cargo test -p minos-backend-krun --test vm` and `--test exit` with `--ignored`): Phase 6 changed no code in `minos-backend-krun`, `minos-vmm`, or `minos-guest`; last run in the Phase 5 final gate (05-EVIDENCE.md).

### cargo clean

Every step above exited 0, and SUMMARY, STATE, and ROADMAP were committed (`9a957a9`), so `cargo clean` ran next as the last cargo command of the phase; no cargo command ran after it. Afterwards `test ! -d target/debug` exited 0 and `target/` no longer exists.

```
$ cargo clean
     Removed 61913 files, 7.2GiB total
```

cargo clean: exit 0, run after the final gate (GATE-06)
