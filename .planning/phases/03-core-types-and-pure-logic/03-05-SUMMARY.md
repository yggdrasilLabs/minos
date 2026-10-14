---
phase: 03-core-types-and-pure-logic
plan: 05
subsystem: minos-core
status: complete
tags: [core, manifest, toml, image, dockerfile, judge-integrity, judge-version, sha256]
requires:
  - "03-01: RelPath (is_within, ascii_case_key), digest_id!, IdHasher, ImageDigest, TreeId, MetricName, ModelId, limits.rs"
provides:
  - "image.rs: ImageRef (name@sha256:<64 hex> only), ImageError, base_images (Dockerfile FROM-line pins)"
  - "manifest.rs: RawManifest and its 13 raw tables, Manifest (parse_toml, serde try_from), ManifestError, InvalidField, ValueError"
  - "manifest.rs leaf types: ProjectName, ProjectKind, ParamName, Arch, CommandArgv, DurationMs, Goal, MinImprovement, Tier, Eligibility, IneligibleReason, ProposerSpec, IntRange, ChoiceList, ParamDomain, SweepParam, SourceExclude"
  - "manifest.rs validated parts: Budget, PolicySpec, Objective, JudgeSpec (paths), Environment, Commands"
  - "JudgeVersion::of(&TreeId, &JudgeSpec) under minos/judge/v1 with a golden vector"
affects: [03-07, 03-08, 03-10, 03-11, 03-12, 03-13]
tech-stack:
  added: []
  patterns:
    - "Raw mirror with deny_unknown_fields on every table, Option for absent fields, defaults filled in try_from"
    - "ValueError (limit only) from leaf parsers, attributed to a dotted field path as InvalidField by the manifest"
    - "Judge paths derived from every judge argv entry, failing closed on path-like entries that are not RelPaths"
    - "Dockerfile read with Docker's own continuation rules, failing closed where a directive could change the reading"
key-files:
  created: []
  modified:
    - crates/minos-core/src/image.rs
    - crates/minos-core/src/manifest.rs
    - crates/minos-core/tests/manifest.rs
decisions:
  - "Leaf parsers (DurationMs, MinImprovement, CommandArgv, SourceExclude, ProjectName, ParamName) return ValueError, which carries only the allowed range; the manifest attributes it to its dotted field path as InvalidField. Multi-field leaves (IntRange, ChoiceList, SweepParam, ProposerSpec) return InvalidField with fixed paths such as param.step"
  - "base_images checks every comment line for the escape and syntax directives, not only the leading ones: an escape character other than \\ is EscapeDirective, an unpinned syntax frontend is Unpinned. Every ImageRef failure on a FROM line (tag, no digest, bad digest, bad name) is Unpinned { line }"
  - "ImageError gains MalformedFrom { line } for a FROM flag without =value or trailing words other than AS <name>, and EscapeDirective { line }"
  - "Judge-path derivation also refuses . and .. and an option without = that contains / or \\ (JudgeArgPath), so a judge argument that names the project root or a short option with an attached path is never silently ignored"
  - "MinImprovement stores the written text for Display and parses a percentage by shifting the decimal text (5% is parsed as 5e-2), so the ratio is the nearest double to the written value; text over 32 bytes is refused"
  - "spend_usd may be 0 (a sweep-only run spends nothing); only the upper bound MAX_SPEND_USD is enforced. The judge label must be 1 to MAX_JUDGE_LABEL"
  - "Proposers are stored sorted by tier, and a second entry of the same tier is Field(proposer.kind)"
  - "[[param]] needs all of min, max, and step, or choices alone; any other mix is Field(param)"
  - "Excludes are stored sorted and deduplicated; covers() matches equality or a component-boundary prefix on ascii_case_key, for directory and file entries alike"
metrics:
  duration: "15 min"
  completed: 2026-10-14
  tasks: 2
  files: 3
---

# Phase 3 Plan 05: Manifest, image pins, and JudgeVersion Summary

`Manifest::try_from(RawManifest)` enforces every CORE-03 rule. The surface is exact files (D-01), disjoint from readable, from every path the judge command names, from `program.md`, and from the Dockerfile. No surface path may sit under an exclude, and no two project paths may collide under case folding. Budgets and breaker limits are bounded with defaults (D-13, D-14). The policy is greedy with exactly one objective, and at least one proposer must be eligible (CORE-11). `ImageRef` and `base_images` enforce digest-only base images on the Dockerfile. `JudgeVersion` hashes the judge data tree plus the canonical judge spec; its golden vector is `3a2f24af...bc12`.

## Performance

- **Duration:** 15 min
- **Started:** 2026-10-14T13:53:39Z
- **Completed:** 2026-10-14T14:08:14Z
- **Tasks:** 2 (four TDD commits)
- **Files modified:** 3

## Accomplishments

- **image.rs.**
  - `ImageRef` grammar: `<name>@sha256:<64 lowercase hex>`. A `:` is allowed only in the first segment of a multi-segment name (a registry port). A `:` in the last segment is `Tag`, even beside a digest.
  - `base_images` joins continued lines and drops comment and blank lines inside a continuation, as Docker does. It skips `--name=value` flags and accepts `scratch` and earlier stage aliases (case-insensitive). It refuses `$` images (`Variable`), unpinned or malformed refs (`Unpinned`), a FROM with no image (`EmptyFrom`), and a malformed FROM (`MalformedFrom`). It returns refs sorted and deduplicated through a `BTreeSet`.
- **Leaf types.**
  - `CommandArgv`: 1 to 64 entries, at most 4096 bytes each, no NUL, a non-empty first entry, and `program()` computed at construction.
  - `DurationMs`: `<1-6 digits><s|m|h>`, from 1 s to 1 h.
  - `MinImprovement`: the `threshold_at` multiply form, never a division.
  - `Tier` (declaration order is the rank) and `Eligibility`. `IneligibleReason` has fixed text: `NoParams`, `NoModel`, `NotInThisRelease`, `SurfaceTooLarge`.
  - Sweep params: integer ranges (`IntRange`, point count computed in `i128`, at most `MAX_PARAM_POINTS`) and choice lists only.
  - `SourceExclude::covers`: case-insensitive, at component boundaries.
- **Manifest.**
  - There are 14 raw structs with `deny_unknown_fields`, no serde defaults, and no flattening.
  - The 9 rule stages are listed in order in the module docs.
  - `parse_toml` keeps typed errors. `toml::from_str::<Manifest>` runs the same rules through serde `try_from`.
- **Judge paths.**
  - Every judge argv entry is read, and so is the text after `=` in an option. Absolute paths are skipped and one leading `./` is stripped. A valid `RelPath` becomes a judge path. A path-like entry that is not a valid `RelPath` is `JudgeArgPath { index }`.
  - So `["python3", "judge/score.py"]` makes `judge/score.py` a judge path, and that path cannot be writable.
- **JudgeVersion.** The macro `crate::id::digest_id!` defines it under `minos/judge/v1`. The hash covers the data digest, argc, and each argv entry; the gates in sorted order; and the objectives in metric order, each with a goal code (1 or 2) and a held-out flag plus metric. The label and `min_improvement` are excluded.

## Task Commits

1. **Task 1: image.rs and the manifest leaf types**
   - RED `483a24a` (test)
   - GREEN `3f0ca31` (feat)
2. **Task 2: RawManifest to Manifest, JudgeSpec, JudgeVersion, rule tests**
   - RED `0e5a8e7` (test)
   - GREEN `fc630f0` (feat)

## Files Created/Modified

- `crates/minos-core/src/image.rs`: ImageRef, ImageError, base_images
- `crates/minos-core/src/manifest.rs`: the raw mirror, validated manifest, leaf types, ManifestError, judge-path derivation, JudgeVersion
- `crates/minos-core/tests/manifest.rs`: 89 tests. They cover:
  - one test per leaf behavior and one per manifest rule;
  - the five judge-argument tests the plan names, plus dot, slashed-flag, and case-fold variants;
  - the unknown-key check in every table, including the inline `requires` table, the top level, and `[[param]]`;
  - the serde path, the golden vector, and the JudgeVersion change and invariance tests.

## Verification

| Command | Result |
|---|---|
| `cargo fmt --all --check` | ok |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0 |
| `cargo test --workspace` | exit 0; manifest 89 passed |
| `cargo deny check` | advisories ok, bans ok, licenses ok, sources ok (the existing minos-sandbox `unmatched wrapper` warning only) |
| `cargo doc --workspace --no-deps` | ok, 0 warnings |
| C-01 per-crate `[lints] workspace = true` | no MISSING |
| C-02 `grep -rnE '#!?\[allow\(' crates --include='*.rs'` | prints nothing |
| `test ! -e crates/minos-core/tests/snapshots` | true |
| `grep -n 'serde(default' / 'flatten' crates/minos-core/src/manifest.rs` | print nothing |
| `grep -c deny_unknown_fields crates/minos-core/src/manifest.rs` | 14 |
| `cargo test -p minos-core --test manifest judge_` | 11 passed, including the four named judge-argument tests |

The JudgeVersion golden vector is `3a2f24af3506ea83d0d0450c710b7813c5db892a487399d34a6d8cf894b0bc12`. It was computed with `printf ... | shasum -a 256` over 149 input bytes, and the recipe is beside the literal. The writable-overlaps-readable test asserts this exact message: `src/parse.rs is writable, so it cannot also be in surface.readable`.

## Decisions Made

See `decisions` in the frontmatter. The main ones are `ValueError` for leaf parsers, directive checks on every comment line, and the two extra `ImageError` variants.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing critical functionality] Dockerfile directives that change how the file is read**
- **Found during:** Task 1
- **Issue:** A `# escape=`` directive makes Docker stop treating `\` as a continuation. A plain line joiner would then fold a real `FROM alpine:3.20` into the instruction before it and never see it, which is a bypass of the digest rule. A `# syntax=` directive pulls a frontend image by tag.
- **Fix:** Every comment line is checked. An escape directive other than `\` is `EscapeDirective { line }`, and an unpinned syntax directive is `Unpinned { line }`. Comment and blank lines inside a continuation are dropped, as Docker drops them.
- **Files modified:** crates/minos-core/src/image.rs
- **Verification:** `base_images_rejects_a_changed_escape_character`, `base_images_rejects_an_unpinned_syntax_frontend`, `base_images_skips_comments_inside_a_continuation`
- **Committed in:** 3f0ca31

**2. [Rule 2 - Missing critical functionality] Malformed FROM lines fail closed**
- **Found during:** Task 1
- **Issue:** `FROM --platform linux/arm64 img@sha256:...` (a flag without `=`) or trailing words after the image would otherwise be read loosely.
- **Fix:** Added `ImageError::MalformedFrom { line }`. The enum is `#[non_exhaustive]`.
- **Files modified:** crates/minos-core/src/image.rs
- **Verification:** `base_images_rejects_a_flag_without_a_value`
- **Committed in:** 3f0ca31

**3. [Rule 2 - Missing critical functionality] More judge arguments refused**
- **Found during:** Task 2
- **Issue:** Under the plan's derivation, `.` and `..` (which name the project root or its parent), `./`, and an option without `=` that contains a path (`-dsrc/parse.rs`) would contribute nothing, so a judge argument naming project files could go unchecked.
- **Fix:** Each of these is `JudgeArgPath { index }`. Like the plan's own bare-word rule, this can only reject more manifests.
- **Files modified:** crates/minos-core/src/manifest.rs
- **Verification:** `rejects_dot_and_slashed_flag_judge_arguments`
- **Committed in:** fc630f0

**4. [Rule 2 - Missing critical functionality] One proposer per tier**
- **Found during:** Task 2
- **Issue:** `MAX_PROPOSERS` is 4, "one per tier", but nothing stopped two entries of the same tier. That would make the routing ladder ambiguous.
- **Fix:** A repeated tier is `Field(proposer.kind)`, and proposers are stored sorted by tier.
- **Files modified:** crates/minos-core/src/manifest.rs
- **Verification:** `rejects_a_second_proposer_of_the_same_tier`, `proposers_are_kept_in_tier_order`
- **Committed in:** fc630f0

**5. [Rule 3 - Blocking] `declared_eligibility` acceptance grep**
- **Found during:** Task 2 acceptance check
- **Issue:** The function was a `pub const fn`, so `grep -n 'pub fn declared_eligibility'` printed nothing.
- **Fix:** Made it a plain `pub fn`. Behavior is unchanged.
- **Committed in:** fc630f0

**6. [Design note] `ValueError` added**
- The plan's leaf constructors needed an error type before the field path is known. `ValueError` carries the allowed range only, and `InvalidField::at(field, error)` attributes it.

**Total deviations:** 4 Rule 2 (fail-closed additions), 1 Rule 3, 1 design note. **Impact:** each one only rejects more inputs or adds a type. No rule from the plan was weakened.

## Issues Encountered

None.

## Known Stubs

None. `IneligibleReason::SurfaceTooLarge` exists and is produced by routing in 03-11, as the plan states.

## Threat Model Coverage

| Threat | Mitigation | Where |
|---|---|---|
| T-03-05-01 | Exact-set disjointness; program and Dockerfile excluded from writable; every judge argv path excluded, with fail-closed rejection; exclude coverage; case-fold check | manifest.rs; the rejects_writable_* and rejects_*_judge_* tests |
| T-03-05-02 | `ascii_case_key` collision check over writable, readable, program, Dockerfile, and judge paths; case-insensitive `covers` | rejects_a_case_fold_collision, rejects_judge_argument_case_fold_collision |
| T-03-05-03 | `deny_unknown_fields` on all 14 raw structs; no serde defaults; no flatten | rejects_an_unknown_key_in_every_table |
| T-03-05-04 | Range checks with D-14 minimums of 1 | rejects_stall_step_out_of_range, rejects_breaker_limits_out_of_range, rejects_budgets_out_of_range |
| T-03-05-05 | ImageRef refuses tags; base_images refuses unpinned, variable, malformed, escape-changed, and unpinned-syntax Dockerfiles | image tests |
| T-03-05-06 | JudgeVersion golden vector; change and invariance tests | judge_version_* |
| T-03-05-07 | CommandArgv is an argv array with NUL and size checks; no shell string is built | command_argv_* |

No new threat surface beyond the register: the module does no I/O and opens no network or file path.

## Next Phase Readiness

Ready for 03-06 onward:
- 03-07 freezes `Tier` and `JudgeVersion` into the events and writes the CLAUDE.md Manifest paragraph.
- 03-08 builds the sweep on `SweepParam`.
- 03-11 builds routing on `ProposerSpec::declared_eligibility` and `IneligibleReason::SurfaceTooLarge`.

## Self-Check: PASSED

- crates/minos-core/src/image.rs, crates/minos-core/src/manifest.rs, and crates/minos-core/tests/manifest.rs exist.
- Commits 483a24a, 3f0ca31, 0e5a8e7, and fc630f0 are in `git log`.
