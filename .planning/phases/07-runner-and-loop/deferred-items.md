# Phase 07 deferred items

Out-of-scope discoveries logged by executors; not fixed in the plan that found them.

## From 07-08

- `minos-propose` unit tests `claude_cli::process::tests::stdout_over_its_cap_kills_the_run` and
  `claude_cli::process::tests::the_deadline_kills_the_whole_process_group` failed once in one of three
  `cargo test --workspace` runs on 2026-10-22 and passed in the other two and in three isolated runs of
  `cargo test -p minos-propose --lib claude_cli::process`. They are timing-sensitive process tests; 07-08 touches
  neither `minos-propose` nor its dependencies, so the flake predates this plan. Worth a look under load.
