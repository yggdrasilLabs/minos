# Phase 4 Deferred Items

Out-of-scope discoveries logged during execution. Not fixed by the plan that found them.

## Flaky `supervise_run_kills_the_whole_process_group_at_the_host_deadline` (found during 04-01)

- Test: `crates/minos-backend-krun/src/supervise.rs`, `supervise::tests::supervise_run_kills_the_whole_process_group_at_the_host_deadline` (Phase 2 code; last touched in 4b659e2).
- Symptom: `cargo test --workspace` failed once with `Error: "probing the process group: Operation not permitted (os error 1)"`. The next full run passed (729 passed, 0 failed), and 1 of 8 repeated runs of the single test failed the same way. The test also passes outside the command sandbox.
- Likely mechanism (unproven): the probe loop `test_kill_process_group(group)` accepts only `ESRCH` as "group gone" and treats every other errno as fatal. On macOS, signalling a process group whose remaining member is a zombie awaiting reaping by launchd can return `EPERM` rather than `ESRCH`.
- Not caused by 04-01 (no change to `minos-backend-krun`). Prove the mechanism before changing the test; do not loosen it to accept `EPERM` without that proof.
- Resolved in 05-15: the mechanism is proven (05-EVIDENCE.md "05-15 EPERM on a zombie-only process group"): a group whose only members are unreaped zombies answers `EPERM` 5 of 5 times. The test was replaced by `deadline_kills_every_member_of_the_group`, whose probe accepts `EPERM` only while `ps` shows no live member and still ends only on `ESRCH`.
