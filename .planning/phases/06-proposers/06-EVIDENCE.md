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
