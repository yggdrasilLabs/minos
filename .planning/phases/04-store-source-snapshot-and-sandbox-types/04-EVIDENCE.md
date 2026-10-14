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
