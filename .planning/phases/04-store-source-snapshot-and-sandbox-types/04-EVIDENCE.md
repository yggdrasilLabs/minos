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
