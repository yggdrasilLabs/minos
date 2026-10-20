---
status: complete
phase: 06-proposers
source: [06-01-SUMMARY.md, 06-02-SUMMARY.md, 06-03-SUMMARY.md, 06-04-SUMMARY.md, 06-05-SUMMARY.md, 06-06-SUMMARY.md, 06-07-SUMMARY.md, 06-08-SUMMARY.md, 06-09-SUMMARY.md, 06-10-SUMMARY.md]
started: 2026-10-20T14:20:09Z
updated: 2026-10-20T14:42:11Z
---

## Current Test

[testing complete]

## Notes

- The recorded final gate (06-EVIDENCE.md "## Final gate", HEAD a114940) and 06-VERIFICATION.md predate the code review fixes (WR-01 to WR-05, IN-01 to IN-04). 06-REVIEW-FIX.md records a full workspace gate after the fixes (1378 passed, 0 failed, 23 ignored) but says the live suite and cargo clean were not rerun, and target/ exists. Tests 1, 9, and 11 cover that.
- Coverage blocks that the classifier could not validate, so their entries became human checkpoints (never dropped): 06-02 D1 (verification kind "build") and D5 (kind "property"); 06-03 D3 and D5 (kind "property"); 06-07 D6 (human_judgment true with no rationale); 06-08 D3 (kind "manual", status "pending"). Allowed kinds are unit, integration, e2e, automated_ui, manual_procedural, other; allowed statuses are pass, fail, unknown.

## Tests

### 1. Full gate on HEAD after the code review fixes
expected: HEAD is ee7a802, ten fix(06) commits past the recorded final gate (a114940), with 19 files in crates/ changed. With TRYBUILD and MINOS_CLAUDE_LIVE unset, run in order: cargo fmt --all --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace; cargo deny check; cargo doc --workspace --no-deps; cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings. All six exit 0. cargo test --workspace reports 0 failed (the fix report recorded 1378 passed, 23 ignored) and the live target's line reads "1 passed; 0 failed; 3 ignored", so the gate made no model call. cargo deny prints "advisories ok, bans ok, licenses ok, sources ok". The C-02 grep, grep -rnE '#!?\[allow\(' crates --include='*.rs', prints nothing.
result: pass

### 2. Propose-only caps live in limits.rs with reasons
expected: Open crates/minos-propose/src/limits.rs. Every cap only minos-propose enforces is a documented const giving its unit and reason (history generations and table bytes, excerpt bytes and count, steer-note bytes and count, prompt overhead, MAX_PROMPT_BYTES at 1 MiB, MAX_RATIONALE_CHARS at 2000 with a 4x byte ceiling, file content, deadlines, envelope cap). Caps shared with other crates are imported from minos_core::limits, never redefined. Const assertions prove the overhead budget plus the surface-proportional fences plus the four content caps fit MAX_PROMPT_BYTES, so cargo build -p minos-propose compiling is the proof.
result: pass
coverage_id: 06-02/D1

### 3. Strict parser drops every malformed shape
expected: cargo test -p minos-propose --test parse passes. The target has one test per DropReason except Sweep (unknown field, missing field, non-writable path, duplicate path, oversize rationale or content, empty or unchanged diff, and so on), a round-trip test, and a proptest that feeds arbitrary JSON and never panics: every malformed input is counted as a drop, never repaired.
result: pass
coverage_id: 06-02/D5

### 4. Prompt bytes identical under shuffled inputs, stable prefix
expected: cargo test -p minos-propose --test prompt passes, including the proptest that shuffles history rows and steer notes and asserts byte-identical prompt bytes and PromptHash, the golden snapshot with its SHA-256 pin, and the prefix-stability test showing the prefix (instructions, objectives, program.md, surface files) is unchanged across history, notes, and generation.
result: pass
coverage_id: 06-03/D3

### 5. Held-out values and names never reach a prompt
expected: cargo test -p minos-propose --test held_out passes: a proptest varies every held-out metric value and asserts the prompt bytes and hash do not change, and a direct test asserts neither the held-out metric's name nor its value appears anywhere in the prompt.
result: pass
coverage_id: 06-03/D5

### 6. Sweep tier proposes with no model
expected: cargo test -p minos-propose --test sweep passes (10 tests). sweep::propose(spec, input) in crates/minos-propose/src/sweep.rs takes no provider parameter, and grep -c '\.complete(' crates/minos-propose/src/sweep.rs prints 0, so the sweep cannot call a model. The same run and generation give byte-identical diffs; drops are SameAsParent, BackToBase, Sweep, Compose, DuplicateInBatch.
result: pass

### 7. Single-shot tier and Proposer dispatch over the fake provider
expected: cargo test -p minos-propose --test single_shot --test proposer passes. grep -c '\.complete(' crates/minos-propose/src/single_shot.rs prints 1 (exactly one model call per propose step), and grep -rn 'dyn ModelProvider' crates/minos-propose prints nothing (no trait objects). Proposer without a provider proposes sweeps only and returns NoProvider for single-shot; planned and agentic return TierNotInThisRelease.
result: pass

### 8. CLAUDE.md describes the provider as built
expected: In CLAUDE.md, the paragraph starting "The provider as built is ClaudeCli" names ClaudeCli::preflight as the only constructor, the $HOME/.local/bin/claude link read and never followed with the directory ownership checks (WR-05), re-resolution before every call, the 13 refused credential and redirect variables plus CLAUDE_CODE_USE_*, the auth status fields with no apiKeySource key, fresh 0700 minos-claude-* directories (WR-01) and the leaked_dirs retry (IN-04), the 15-minute deadline with the 5 s drain grace (WR-04), the api_error_status classifier with ProviderLimit and ProviderAuth stops, an over-cap reply as an empty call (WR-03), and the {rationale, files} output. The Open questions say "Moot for v1" twice and hold the --bare entry. .claude/CLAUDE.md has the "Phase 6 corrections to the stack block" section before Conventions.
result: pass
coverage_id: 06-07/D6

### 9. Live suite passes on your own install at HEAD
expected: You run it, on your logged-in machine, under your own subscription; no agent runs it. The hand run recorded in 06-09 was on claude 2.1.304 before the review fixes changed locate.rs, process.rs, and mod.rs, and 06-REVIEW-FIX.md says the live tests were not rerun. Run ~/.local/bin/claude --version, then MINOS_CLAUDE_LIVE=1 cargo test -p minos-propose --test live -- --ignored --nocapture --test-threads=1. Result: 3 passed, 0 failed, exit 0, and the preflight refuses nothing. Case (a) parses a {rationale, files} structured output and prints token usage. Case (b) leaves canary-session-start, canary-user-prompt, and canary-mcp absent with the marker false. Case (c), the positive control, fires all three canaries with the marker true. If the CLI version is newer than 2.1.304, say so.
result: pass
coverage_id: 06-08/D3

### 10. Live run recorded and redacted
expected: 06-EVIDENCE.md has "## Live provider tests (06-09)" (line 47) recording who ran it, the CLI version 2.1.304 and its link target, the exact commands, that nothing was unset, exit 0, the verbatim output with only home paths changed to ~, a canary and marker table, the 4475 tokens for case (a), and what each case proves. The section holds no home path, email, organization, or account value. CLAUDE.md "Verified facts, local (as of 2026-10-20)" has the 2.1.304 bullet citing 06-EVIDENCE.md Live provider tests, after the kept 2.1.296 line.
result: pass

### 11. cargo clean closes the phase (GATE-06)
expected: After every test above, run cargo clean as the last cargo command of the phase; no cargo command runs after it. test ! -d target exits 0 and target/ no longer exists. (06-REVIEW-FIX.md records that no clean ran after the review fixes, which is why target/ exists now.)
result: pass

### 12. StopReason::ProviderAuth with tag 7, fixed Display text, serde name provider_auth, documented tag
expected: StopReason::ProviderAuth with tag 7, fixed Display text, serde name provider_auth, documented tag
result: pass
source: automated
coverage_id: 06-01/D1

### 13. Golden wire line for the new stop reason; all existing snapshots byte-unchanged; pin over 25 lines
expected: Golden wire line for the new stop reason; all existing snapshots byte-unchanged; pin over 25 lines
result: pass
source: automated
coverage_id: 06-01/D2

### 14. HashMap/HashSet banned by clippy.toml, proven to fire
expected: HashMap/HashSet banned by clippy.toml, proven to fire
result: pass
source: automated
coverage_id: 06-02/D2

### 15. OutputSchema::for_batch golden snapshot, SHA-256 pin, structure, determinism, refusals
expected: OutputSchema::for_batch golden snapshot, SHA-256 pin, structure, determinism, refusals
result: pass
source: automated
coverage_id: 06-02/D3

### 16. ModelProvider abstraction; ModelOutput has no Debug, Display, Clone, Deref (compile-time check)
expected: ModelProvider abstraction; ModelOutput has no Debug, Display, Clone, Deref (compile-time check)
result: pass
source: automated
coverage_id: 06-02/D4

### 17. History rows: gates as pass/fail, signed percent deltas on visible objectives, fixed phrases, sanitized rationales, window, order, caps
expected: History rows: gates as pass/fail, signed percent deltas on visible objectives, fixed phrases, sanitized rationales, window, order, caps
result: pass
source: automated
coverage_id: 06-03/D1

### 18. Golden prompt snapshot and SHA-256 pin
expected: Golden prompt snapshot and SHA-256 pin
result: pass
source: automated
coverage_id: 06-03/D2

### 19. Hostile excerpt with a guessed END marker and a forged notes heading stays inside its fence
expected: Hostile excerpt with a guessed END marker and a forged notes heading stays inside its fence
result: pass
source: automated
coverage_id: 06-03/D4

### 20. Golden 18-element argv, six-variable child environment, refused-name scan, version parsing
expected: Golden 18-element argv, six-variable child environment, refused-name scan, version parsing
result: pass
source: automated
coverage_id: 06-04/D1

### 21. Credential and redirect variable names refused; values never kept or printed
expected: Credential and redirect variable names refused; values never kept or printed
result: pass
source: automated
coverage_id: 06-04/D2

### 22. Pattern 7 result classification with usage, no dollar field, stop-reason mapping
expected: Pattern 7 result classification with usage, no dollar field, stop-reason mapping
result: pass
source: automated
coverage_id: 06-04/D3

### 23. Token usage returned with every parsed classification; string-typed dollar fields classify like numeric twins
expected: Token usage returned with every parsed classification; string-typed dollar fields classify like numeric twins
result: pass
source: automated
coverage_id: 06-04/D4

### 24. Native-install locator: only the versions-directory symlink layout, owned, not writable, executable, version-named
expected: Native-install locator: only the versions-directory symlink layout, owned, not writable, executable, version-named
result: pass
source: automated
coverage_id: 06-04/D5

### 25. Capped runner: stdin, exact env, cwd, exit code, signal, deadline group kill, straggler, stdout cap, stderr tail, spawn failure
expected: Capped runner: stdin, exact env, cwd, exit code, signal, deadline group kill, straggler, stdout cap, stderr tail, spawn failure
result: pass
source: automated
coverage_id: 06-06/D1

### 26. Preflight refusals: credential names, environment, locate, work root, version, auth status; Preflighted on a clean subscription login with no PII bound
expected: Preflight refusals: credential names, environment, locate, work root, version, auth status; Preflighted on a clean subscription login with no PII bound
result: pass
source: automated
coverage_id: 06-06/D2

### 27. Each query runs once with exactly the six variables in a fresh empty minos-claude-* directory under the canonical TMPDIR, removed afterwards
expected: Each query runs once with exactly the six variables in a fresh empty minos-claude-* directory under the canonical TMPDIR, removed afterwards
result: pass
source: automated
coverage_id: 06-06/D3

### 28. ClaudeCli::complete returns Completion::Structured, the envelope's token usage, and resolved_model claude-haiku-5-5; Debug shows the version and never account data
expected: ClaudeCli::complete returns Completion::Structured, the envelope's token usage, and resolved_model claude-haiku-5-5; Debug shows the version and never account data
result: pass
source: automated
coverage_id: 06-07/D1

### 29. The call runs argv(model, schema) exactly, with exactly six variables, the prompt bytes on stdin, in a fresh empty minos-claude-* directory directly under the canonical TMPDIR that is gone afterwards
expected: The call runs argv(model, schema) exactly, with exactly six variables, the prompt bytes on stdin, in a fresh empty minos-claude-* directory directly under the canonical TMPDIR that is gone afterwards
result: pass
source: automated
coverage_id: 06-07/D2

### 30. A 429 envelope (success, is_error true, exit 1) is ProviderError::Stop { Limit, usage: Some(..) }
expected: A 429 envelope (success, is_error true, exit 1) is ProviderError::Stop { Limit, usage: Some(..) }
result: pass
source: automated
coverage_id: 06-07/D3

### 31. Re-resolution: a moved link re-runs --version and auth status once and succeeds; an unmoved link runs no preflight step; an auth refusal is a ProviderAuth stop with no -p run; an old version is Infra Preflight; a removed link is Infra BinaryMissing
expected: Re-resolution: a moved link re-runs --version and auth status once and succeeds; an unmoved link runs no preflight step; an auth refusal is a ProviderAuth stop with no -p run; an old version is Infra Preflight; a removed link is Infra BinaryMissing
result: pass
source: automated
coverage_id: 06-07/D4

### 32. The same Proposer/single-shot code over ClaudeCli and FakeProvider gives equal Proposals; through ClaudeCli the origin is Origin::Model { claude-haiku-5-5, PromptHash::of(stdin bytes) }
expected: The same Proposer/single-shot code over ClaudeCli and FakeProvider gives equal Proposals; through ClaudeCli the origin is Origin::Model { claude-haiku-5-5, PromptHash::of(stdin bytes) }
result: pass
source: automated
coverage_id: 06-07/D5

### 33. Three ignored live tests exist, each failing closed unless MINOS_CLAUDE_LIVE is 1; production code never reads the variable
expected: Three ignored live tests exist, each failing closed unless MINOS_CLAUDE_LIVE is 1; production code never reads the variable
result: pass
source: automated
coverage_id: 06-08/D1

### 34. The fixture's hooks, MCP server, and CLAUDE.md are well formed and name the canary paths exactly; the control argv drops exactly the three isolation flags and the --mcp-config value
expected: The fixture's hooks, MCP server, and CLAUDE.md are well formed and name the canary paths exactly; the control argv drops exactly the three isolation flags and the --mcp-config value
result: pass
source: automated
coverage_id: 06-08/D2

### 35. CLAUDE.md Testing and Commands describe the live suite, its gate variable, and its run command
expected: CLAUDE.md Testing and Commands describe the live suite, its gate variable, and its run command
result: pass
source: automated
coverage_id: 06-08/D4

## Summary

total: 35
passed: 35
issues: 0
pending: 0
skipped: 0
blocked: 0

## Gaps

[none yet]
