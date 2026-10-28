---
status: partial
phase: 08-cli-sample-project-and-docs
source: [08-VERIFICATION.md]
started: 2026-10-28T20:55:23Z
updated: 2026-10-28T22:38:02Z
---

## Current Test

[testing paused — 3 items outstanding; deferred by the user on 2026-10-28, resume with /gsd-verify-work 8]

## Tests

### 1. README quick start with the real binary and a real model call
expected: From examples/logparse, `minos env build`, `judge push judge`, `check`, `run --generations 3`, `status`, `report <exp>`, and `adopt <exp> --reviewed <hash>` each work as the README says, with the real single_shot haiku provider (not the scripted stand-in), `approx. cost` shows real token counts, and the patch applies with `patch -p1` and with the printed `git apply --directory` form. Needs a rebuild after `cargo clean`, the signed `minos-vmm`, the musl guest, Docker, a logged-in `claude`, and a priced `claude-haiku-5-5` in `~/.config/minos/prices.toml`. Spends real model calls on your subscription.
result: [pending]

### 2. Read README.md and SECURITY.md as a new user
expected: Neither document claims more than the code does; the trust base, preflight list, recorded exceptions (`KRUN_BOOT_START_NS`, missing `_minos` user), and the provider-terms paragraph read clearly. The `docs_words` test only scans for banned words and required names; whether prose overclaims is a judgment call.
result: [pending]

### 3. Re-run the VM suite and live provider suite if the code changes before milestone close
expected: `cargo test -p minos-cli --test sample_vm -- --ignored --nocapture --test-threads=1` (with `MINOS_VMM`, `MINOS_GUEST`, `MINOS_KRUNFW` set, after the build, signing, and musl guest build) passes, and `MINOS_CLAUDE_LIVE=1 cargo test -p minos-propose --test live -- --ignored --nocapture --test-threads=1` passes on the installed `claude`. Verified on 2026-10-28 (claude 2.1.312) from the recorded evidence in 08-EVIDENCE.md; only needed again if code changes.
result: [pending]

## Summary

total: 3
passed: 0
issues: 0
pending: 3
skipped: 0
blocked: 0

## Gaps
