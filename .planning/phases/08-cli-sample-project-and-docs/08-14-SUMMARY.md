---
phase: 08-cli-sample-project-and-docs
plan: 14
subsystem: public-docs
status: complete
tags: [docs, security, readme, overclaim-test, claude-md, provider-terms]
requires:
  - the Phase 8 command tree and exit codes (08-12)
  - the sample on real VMs, the RLIMIT_AS result, and the live provider rerun on claude 2.1.312 (08-13, 08-EVIDENCE.md)
  - adopt's hash gate and printed patch commands (08-10)
  - the sample's /scratch/candidate/out judge path and digest-only FROM rule (08-05)
provides:
  - SECURITY.md (threat model, what Minos isolates, trust base, preflight, recorded exceptions, private reporting)
  - README.md (what Minos is, status, install, the logparse quick start, commands, cost, provider terms, security, license)
  - crates/minos-cli/tests/docs_words.rs (the overclaim scan and required content, 4 tests)
  - the /scratch/candidate/out correction in CLAUDE.md and the store_judge_input rustdoc
  - CLAUDE.md CLI, Report and adopt, Workspace, Testing, and Open questions brought to what Phase 8 built
affects: [08-15 marks DOCS-03, DOCS-04, COST-02 and runs the phase gate]
tech-stack:
  added: []
  patterns:
    - "Public-doc wording is a gate: a Rust test reads the documents from the repository root and names the file and item it fails on"
    - "Whole-word bans split on every character that is not ASCII alphanumeric or _, so exactly stays allowed"
key-files:
  created:
    - SECURITY.md
    - README.md
    - crates/minos-cli/tests/docs_words.rs
  modified:
    - CLAUDE.md
    - crates/minos-sandbox/src/untrusted.rs
key-decisions:
  - "docs_words bans ironclad as a substring, exact as a whole word, and cannot be escaped as a phrase after collapsing whitespace, so a line break cannot hide it"
  - "SECURITY.md states the escape target as a goal tested against, not a guarantee, and lists side channels and concurrent VMs as unaddressed"
  - "README's run excerpts come from 08-EVIDENCE.md and say the model was a scripted stand-in, so their token counts are not real"
  - "The decided minos stop item left CLAUDE.md's Open questions; its facts now live in the CLI section's stop paragraph and the loop and ledger sections"
requirements-completed: []
metrics:
  started: 2026-10-28T20:26:19Z
  completed: 2026-10-28T20:31:52Z
  duration: "about 6 minutes of edit-and-commit time after context reading"
  tasks: "2 of 2"
---

# Phase 8 Plan 14: SECURITY.md, README.md, and the Overclaim Test Summary

Minos now has its two public documents, and a gate test keeps them from overclaiming. SECURITY.md states the threat model, what is isolated and how, the trust base, what `minos host doctor` and the launch preflight check (and what they do not), and the recorded exceptions. README.md covers install, the `logparse` quick start, commands, cost, and the `claude` provider's terms, with only facts that the code or 08-EVIDENCE.md records. The design of record and one rustdoc line now say that the judge reads `/scratch/candidate/out/<file>`.

## What was built

- **SECURITY.md** has six sections:
  - **Threat model.** Both actors are in scope. The stated target is that escape requires a zero-day in Hypervisor.framework or the macOS kernel, the libkrun VMM, or the guest kernel together with one of those. This is framed as a goal, not a guarantee, and the document notes there has been no outside audit.
  - **What Minos isolates:**
    - No network device in any VM, including the TSI point: the kernel has it compiled in, and egress stays off only because the VMM never enables it, which `exit_egress` guards.
    - Read-only disk images in, and a chunked, token-authenticated tar stream over vsock out (`exit_forge`).
    - The candidate and the judge run in separate VMs, and the judge reads `/scratch/candidate/out/`.
    - Judge data is kept out of the snapshot and out of the build context.
    - Scores come only from host measurement.
    - Output is rendered through the allowlist.
    - The review gate.
  - **Trust base.** Hypervisor.framework and the macOS kernel, `msb_krun` 0.1.41 in `minos-vmm`, libkrunfw 5.6.2 (Linux 6.12.109), the Seatbelt profile and its allowances, both halves of the rlimits, `minos-guest` as PID 1, the Docker builder at build time, the `claude` CLI outside every sandbox, and the cargo-deny-checked dependency tree.
  - **What the preflight checks.** Each `host doctor` probe in plain words, then the per-launch `Refusal` list. It states that no run is recorded without Seatbelt, and it lists what is not checked.
  - **Known limits and recorded exceptions:**
    - `KRUN_BOOT_START_NS`.
    - The missing `_minos` user, so every run records reduced hardening.
    - The window between hashing libkrunfw and loading it.
    - The vsock defects and the chunked workaround.
    - `sandbox-exec` is deprecated.
    - The Seatbelt profile allowances have not been minimized.
    - What a committed `minos.lock` reveals.
    - The `claude` CLI result is limited to 2.1.312 and the fixture it was tested with.
    - Side channels and concurrent VMs are unaddressed.
  - **Reporting a vulnerability.** Through GitHub private security advisories only. No address is published.
- **README.md:**
  - The three properties, each pointing to SECURITY.md.
  - Status: local macOS arm64 only.
  - Install: the `libkrun/krun` tap and libkrunfw 5.6.2; Docker with buildx (Docker Desktop is the tested setup); the pinned toolchain and the two `cargo build --release` lines; `minos host setup` and `host doctor`, including the expected "dedicated user: missing" line; `prices.toml` through `minos init`; and the `claude` install and environment that the preflight requires.
  - The `logparse` quick start, with excerpts from 08-EVIDENCE.md that are labelled as coming from a scripted provider, plus `patch -p1 -i` and the `git apply --directory` form.
  - The commands as 08-12 built them. Later-milestone commands are marked as not available. `status` is not called read-only, because it creates `driver.lock`.
  - Cost: `approx. cost`, metered, never a forecast, and caps can be passed by at most one job or call.
  - The provider paragraph: your own install, no credential handling, and the terms sentence pointing to API key or Bedrock. It cites the 2026-10-28 live check on `claude` 2.1.312.
  - Security and License (Apache-2.0 OR MIT).
- **`crates/minos-cli/tests/docs_words.rs`** has 4 tests:
  - A self-test of the checker: `exactly` and `inexact` pass, while `exact.`, `Exact-match`, `EXACT`, `IronClad`, and a `cannot be` / `escaped` phrase split across a line break are each caught once.
  - The three-document overclaim scan.
  - SECURITY.md's eight required names.
  - README.md's five required items.

  Each failure names the file and the item. The file has no unwrap, expect, indexing, or allow.
- **untrusted.rs:** the `store_judge_input` rustdoc now says PID 1 unpacks under `/scratch/candidate`, so the judge reads `/scratch/candidate/out/<file>`. This is a documentation change only; the behavior is unchanged.
- **CLAUDE.md:**
  - minos-sandbox has the same path correction, citing `CANDIDATE_DIR` and Pitfall 11.
  - CLI now has shipped and later-milestone command blocks with flags, plus addressing, `check`, `run`, and `stop` paragraphs (D-05 to D-10, D-13, D-14).
  - Report and adopt describes the build as shipped: the hash gate, `ReportAcknowledged` before `create_new`, `minos-<12 hex>.patch` or `--out`, removing the file on a failed write, and a separate paragraph on applying the patch with the `--directory` caveat.
  - The Workspace line for `minos-cli` names the command modules.
  - Testing gets one bullet for the 13 non-ignored Phase 8 targets, including `docs_words`, and which of them run `assert_no_banned_words`.
  - Open questions: the stop item is removed now that it is decided and implemented, and the `batch` item records that `check` prints the D-08 advisory, leaving only the validation of the constant open.

## Commits

| Task | Commit | Message |
|------|--------|---------|
| 1 | 995cf2f | docs(08-14): add SECURITY.md and bring CLAUDE.md's Phase 8 sections current |
| 2 (RED) | 87ceba0 | test(08-14): add the failing overclaim and required-content test for the public docs |
| 2 (GREEN) | 0426beb | feat(08-14): add README.md with install, the sample walkthrough, commands, cost, and provider terms |

## Verification

- Task 1's verify passed: SECURITY.md exists, `KRUN_BOOT_START_NS` and `/scratch/candidate/out` are found in both files, and `cargo doc -p minos-sandbox --no-deps` is clean.
- Task 1's acceptance checks passed:
  - The six `##` sections are present.
  - 23 lines match the trust-base and target names.
  - `/scratch/candidate/out` appears at least once in CLAUDE.md and in untrusted.rs.
  - 2 lines match `minos-<12 hex>.patch`/`--directory` in CLAUDE.md, and `docs_words` is present in CLAUDE.md.
- RED: `cargo test -p minos-cli --test docs_words` failed 2 of 4 tests, the two that read README.md, with `README.md: cannot be read`. GREEN: 4 passed.
- Task 2's acceptance greps each matched: `own install` 1, `Bedrock` 2, `SECURITY.md` 3, `brew tap libkrun/krun` 1, and `minos host doctor` 3.
- Plan verification:
  - `cargo fmt --all --check`: clean.
  - `cargo clippy --workspace --all-targets -- -D warnings`: clean.
  - `cargo doc --workspace --no-deps`: no warnings.
  - `grep -rnE '#!?\[allow\(' crates examples --include='*.rs'`: no output.
- No `cargo clean` was run, because 08-15 owns the phase-end gate. No requirement IDs were ticked.
- Human check for end-of-phase UAT: follow README's quick start with the real `minos` binary in a logged-in terminal. This spends one real model call and was not run by the agent.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Acceptance grep] Report and adopt split into two paragraphs**
- **Found during:** Task 1 acceptance.
- **Issue:** `minos-<12 hex>.patch` and `--directory` were in one paragraph, so `grep -n` printed one line where the check requires at least two.
- **Fix:** Applying the patch now has its own paragraph ("Applying the patch is the user's step"), which reads naturally on its own.
- **Commit:** 995cf2f

**2. [Accuracy] README's Docker wording**
- **Issue:** A first draft said that another Docker CLI "also works". That has not been tested (CLAUDE.md lists the alternatives as untested).
- **Fix:** It now says Docker Desktop is the tested setup and the others should work but are untested. The provider paragraph also gained the preflight's `$TMPDIR` rule, so a reader is not refused without warning.
- **Commit:** 0426beb

**3. Extra checker self-test**
- **Change:** `docs_words` also tests the checker itself, so a mistake in the word split cannot silently pass the documents.
- **Commit:** 87ceba0

None of these changes the scope. No invariant was touched, and the `untrusted.rs` change is to rustdoc text only.

## TDD Gate Compliance

Task 2 has a `test(08-14)` RED commit (87ceba0), which failed on the missing README.md, before its `feat(08-14)` GREEN commit (0426beb). No refactor commit was needed. Task 1 is documentation and is not TDD.

## Known Stubs

None.

## Threat Flags

None. The plan's threat register is mitigated:
- T-08-14-01: `docs_words` checks the banned words, and SECURITY.md lists the trust base, the missing `_minos` user, and the exceptions.
- T-08-14-02: reports go through private advisories, and no address is written.
- T-08-14-03: the README terms sentence is present, and the test requires `own install`, `API key`, and `Bedrock`.
- T-08-14-04: the `/scratch/candidate/out` correction is made in both places.

## Self-Check: PASSED

- FOUND: SECURITY.md, README.md, crates/minos-cli/tests/docs_words.rs
- FOUND: 995cf2f, 87ceba0, 0426beb
