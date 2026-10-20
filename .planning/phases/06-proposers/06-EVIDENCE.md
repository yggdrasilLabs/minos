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
