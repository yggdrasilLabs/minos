---
phase: 08-cli-sample-project-and-docs
plan: 03
subsystem: minos-source
tags: [source, tarball, read-back, surface, cli-seam]
status: complete
requires:
  - minos-source snapshot and pack::tarball (Phase 4)
  - minos-core Tree, RelPath, MAX_BASE_TAR_BYTES
provides:
  - minos_source::read_files
  - minos_source::ReadError (Missing, Malformed)
  - minos_source::Snapshot::surface
affects:
  - check and run (BaseTree and RunInputs.program from the packed bytes)
  - report, show, and adopt (surface from the stored base blob alone)
tech-stack:
  added: []
  patterns:
    - strict in-memory ustar reader (raw entries, typeflag 0 only, end marker and trailing-zero check), mirroring minos-sandbox's output reader
key-files:
  created:
    - crates/minos-source/src/read.rs
    - crates/minos-source/tests/read.rs
  modified:
    - crates/minos-source/src/lib.rs
    - CLAUDE.md
key-decisions:
  - "read_files checks every entry, requested or not, and keeps only the requested entries' bytes; a malformed archive is refused whatever is asked"
  - "The stream is capped at MAX_BASE_TAR_BYTES (80 MiB) before parsing, the same cap the CLI reads the stored base blob under"
  - "Snapshot::surface partitions one read into the surface tree and the program bytes instead of removing the program from a Tree (Tree has no remove, and core is outside this plan)"
requirements-completed: [CLI-05, CLI-08]
coverage:
  - deliverable: "read_files strict read-back with caps and fixed-text errors"
    human_judgment: false
    verification:
      - kind: test
        ref: "crates/minos-source/tests/read.rs"
        status: pass
      - kind: command
        ref: "cargo clippy -p minos-source --all-targets -- -D warnings"
        status: pass
  - deliverable: "Snapshot::surface returns the packed surface tree and program bytes"
    human_judgment: false
    verification:
      - kind: test
        ref: "crates/minos-source/tests/read.rs#surface_returns_the_packed_surface_and_program"
        status: pass
duration: 5 min
completed: 2026-10-28
---

# Phase 8 Plan 03: Snapshot Read-back Summary

`minos_source::read_files` reads Minos's own canonical ustar back in memory under the snapshot caps (regular files only, `RelPath` names, no repeats, two zero end blocks and nothing after), and `Snapshot::surface` returns the run's base surface tree and program bytes from the packed tarball, so the CLI never builds them from the working tree.

## Performance

- Duration: about 5 min
- Tasks: 1 (TDD: RED and GREEN commits)
- Files: 2 created, 2 modified

## Accomplishments

- `read_files(tarball, paths) -> Result<Tree, ReadError>`: refuses a stream over `MAX_BASE_TAR_BYTES` or not block-aligned, a non-ustar header, any typeflag but `0`, a name outside the `RelPath` grammar, a repeated path, more than 10,000 entries, an entry over 16 MiB (from the header, before any data is read; then read through `take(size + 1)` and compared), content over 64 MiB, a truncated entry, a missing end marker, and data after it. Missing requested paths are all listed, sorted.
- `ReadError`: `Missing { paths: Vec<RelPath> }` and `Malformed { what: &'static str }`; no variant carries file bytes (a test checks a refusal's text does not echo content).
- `Snapshot::surface(&Manifest) -> Result<(Tree, Vec<u8>), ReadError>`: one read of writable, readable, and program paths, partitioned into the surface tree and the program bytes.
- 14 tests in `crates/minos-source/tests/read.rs`.

## Task Commits

1. Task 1 RED: `36d18e6` test(08-03): add failing tests for the snapshot read-back
2. Task 1 GREEN: `004c9ca` feat(08-03): read a snapshot tarball back into a surface tree

No REFACTOR commit was needed.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing critical] Malformed texts beyond the plan's list**
- Found during: Task 1
- Issue: the plan's fixed texts had no wording for a truncated entry, a missing end marker, or data after the end marker, which the strict reader must distinguish.
- Fix: added "an entry's data is truncated", "the archive has no end marker", and "data follows the end marker"; every text in the plan's list is used as written.
- Files: crates/minos-source/src/read.rs
- Commit: 004c9ca

**2. [Rule 2 - Docs current] CLAUDE.md workspace layout names the read-back**
- Found during: Task 1
- Issue: CLAUDE.md's `minos-source` line lists the crate's contents; the project rule is to keep it current in the same change.
- Fix: one line added naming `read_files` and `Snapshot::surface`.
- Files: CLAUDE.md
- Commit: 004c9ca

Total deviations: 2 auto-fixed (both Rule 2). Impact: none on scope; the reader is stricter in naming, not in behaviour beyond the plan.

## Verification

- `cargo fmt --all --check`: pass
- `cargo clippy -p minos-source --all-targets -- -D warnings`: pass
- `cargo test -p minos-source`: pass (read: 14 tests)
- `cargo doc -p minos-source --no-deps`: pass, no warnings (a private intra-doc link to the module was replaced by inline rule text)
- C-02 grep: prints nothing
- Acceptance greps: all four print one line

## Threat Flags

None. T-08-03-01 to 03 are mitigated as planned: nothing is extracted, names go through `RelPath`, sizes are checked from the header before reading, and errors carry paths and fixed text only.

## Next Phase Readiness

Ready for the CLI plans that build `BaseTree` and `RunInputs.program` (`check`, `run`) and rebuild the surface from the stored base blob (`report`, `show`, `adopt`).

## Self-Check: PASSED

- FOUND: crates/minos-source/src/read.rs
- FOUND: crates/minos-source/tests/read.rs
- FOUND: 36d18e6
- FOUND: 004c9ca
