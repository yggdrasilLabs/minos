# Phase 4: Store, Source Snapshot, and Sandbox Types - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-10-14
**Phase:** 4-store-source-snapshot-and-sandbox-types
**Areas discussed:** Snapshot paths and clean-tree check, Secret scan and hard denylist, Store layout and durability, Sandbox types and the untrusted reader

---

## Snapshot paths and clean-tree check

### RelPath grammar for project files

| Option | Description | Selected |
|--------|-------------|----------|
| Refuse the pack, name the file (Recommended) | Fail closed with an error listing every offending path; the user excludes or renames it. The output tar reader inherits the grammar. No change to core. | ✓ |
| Widen ASCII to common punctuation | Add `@ ~ # % = , space` with a property test keeping every CORE-02 rejection. Touches core, diffs, and the manifest grammar. | |
| Widen to UTF-8 normal components | Accept any non-control UTF-8 component; reopens homoglyph and case-fold questions settled in 03-CONTEXT D-01. | |

**User's choice:** Refuse the pack, name the file.

### Clean-tree check with no git in the trusted path

| Option | Description | Selected |
|--------|-------------|----------|
| Exec `git status --porcelain` read-only (Recommended) | Spawn the user's git with a cleared env and hooks disabled, on the user's own tree. Not a repo or no git counts as dirty, so `--allow-dirty` is required. Git never touches the tarball. | ✓ |
| No git at all: TreeId continuity | Record the TreeId; a resumed run refuses a changed tree unless `--allow-dirty`. Nothing checks for uncommitted changes. | |
| Pure-Rust read of .git/index | Parse the index and HEAD ourselves. Our own parser for a hostile-ish binary format. | |

**User's choice:** Exec `git status --porcelain` read-only.
**Notes:** The no-git rule's reason is candidate-controlled trees; the user's working tree is never written by a candidate.

### Non-regular entries in the project

| Option | Description | Selected |
|--------|-------------|----------|
| Refuse the pack, name the entry (Recommended) | Any non-regular entry not ignored or excluded fails the pack with a listing. | ✓ |
| Skip with a printed warning | Leave it out with one warning per entry; the pack succeeds. | |
| Follow symlinks within the project | Pack a link to a regular in-tree file as that file's contents, with cycle and escape checks. | |

**User's choice:** Refuse the pack, name the entry.

### Pack-time exclusions

| Option | Description | Selected |
|--------|-------------|----------|
| Exclude, ignore files, and hard denylist all apply (Recommended) | Union of `.gitignore`, `.minosignore`, `[source] exclude`, and the hard denylist. `program.md`, `minos.toml`, and the Dockerfile packed unless excluded. `minos.lock` never packed. | ✓ |
| Only ignore files and the denylist | `[source] exclude` affects only prompt visibility; judge paths would enter the candidate's tree, contradicting WR-04's fix. | |

**User's choice:** Exclude, ignore files, and hard denylist all apply.

### Snapshot caps

| Option | Description | Selected |
|--------|-------------|----------|
| Fixed caps in limits.rs, refuse above (Recommended) | Total-bytes, file-count, and per-file caps as constants with reasons; Claude picks the numbers. | ✓ |
| Manifest-tunable caps | `[source] max_bytes` etc. with bounds; a new RawManifest field. | |
| You decide | Claude picks the mechanism and the numbers. | |

**User's choice:** Fixed caps, refuse above.
**Notes:** `limits.rs` says later caps live at the top of the enforcing module; CONTEXT.md records the intent as "fixed in code" and leaves placement to the planner.

---

## Secret scan and hard denylist

### Override after a hit

| Option | Description | Selected |
|--------|-------------|----------|
| No override: remove the secret (Recommended) | A hit blocks the pack and names the file and pattern; `--allow-dirty` never affects the scan. | ✓ |
| Per-file allowlist in minos.toml | `[source] allow_secret_in = [...]`; new optional RawManifest field. | |
| A CLI flag per run | `--allow-secret <path>` repeated per file. | |

**User's choice:** No override.

### Hard denylist contents

| Option | Description | Selected |
|--------|-------------|----------|
| Pitfall 28 list, fixed in code (Recommended) | `.git/`, `.minos/`, `.claude/`, `.aws/`, `.ssh/`, `.env*`, `*.pem`, `id_*`, `minos.lock`, plus judge data. Never user-editable. | ✓ |
| Smaller list: VCS and Minos state only | `.git/`, `.minos/`, `minos.lock`, judge data. | |
| You decide | Claude fixes the list from Pitfall 28. | |

**User's choice:** Pitfall 28 list, fixed in code.

### Secret patterns

| Option | Description | Selected |
|--------|-------------|----------|
| High-signal fixed set, anchored (Recommended) | PEM private-key headers, AWS access key IDs, GitHub tokens, Anthropic `sk-ant-`, OpenAI `sk-`, Slack `xox`, JWTs, Google `AIza`. Bounded regexes with test vectors. No entropy heuristic. | ✓ |
| Add a high-entropy string heuristic | Also flag long base64/hex runs; the repo's golden SHA-256 vectors would trip it. | |
| You decide | Claude picks and documents the set. | |

**User's choice:** High-signal fixed set, anchored.

### Scan scope

| Option | Description | Selected |
|--------|-------------|----------|
| Every file that will be packed, bytes as-is (Recommended) | Only files surviving ignore, exclude, and the denylist; binaries included. | ✓ |
| Whole working tree, including ignored files | Scan everything under the project root. | |

**User's choice:** Every file that will be packed, bytes as-is.

---

## Store layout and durability

### Data root

| Option | Description | Selected |
|--------|-------------|----------|
| Project-local `.minos/` (Recommended) | `runs/<run-id>/ledger.jsonl` and `blobs/sha256/ab/cdef…`; matches Pitfall 28's denylist entry. | ✓ |
| User data dir `~/.local/share/minos/<project>/` | History outside the tree; needs a stable project key. | |
| Both: ledger local, blobs shared per user | Blobs dedupe across projects; two roots to lock. | |

**User's choice:** Project-local `.minos/`.

### Lock scope

| Option | Description | Selected |
|--------|-------------|----------|
| Per-run lock file, held for each append (Recommended) | `File::lock` on `<run>/ledger.lock` around check, write, fsync. Phase 7's `driver.lock` is separate. | ✓ |
| One lock for the whole store | A single `.minos/store.lock`. | |
| Lock held for the process lifetime | `steer` and `stop` from another terminal would fail. | |

**User's choice:** Per-run lock file, held for each append.

### fsync policy

| Option | Description | Selected |
|--------|-------------|----------|
| Every append: write, then fsync (Recommended) | One line, `sync_data`, then unlock. | ✓ |
| Only before irreversible actions | The caller asks for `sync()`; easy to forget. | |

**User's choice:** Every append.

### Torn final line and key index

| Option | Description | Selected |
|--------|-------------|----------|
| Ignore on read, truncate under lock on next append (Recommended) | `read` returns complete verified lines and reports the torn tail as a count; the next locked `append` truncates; key set rebuilt from the file under the lock. | ✓ |
| Ignore on read, never truncate | The torn bytes stay; the file is no longer clean JSONL. | |
| Refuse to open until repaired | A `minos repair` command that does not exist yet. | |

**User's choice:** Ignore on read, truncate under lock on next append.

---

## Sandbox types and the untrusted reader

### Judge-only scorecard

| Option | Description | Selected |
|--------|-------------|----------|
| Untrusted output carries the role (Recommended) | `Untrusted<GuestOutput<R>>`; `into_scorecard` only for `Judge`; a trybuild case proves it. | ✓ |
| Runtime check in the runner | `into_scorecard` for any role; convention in one crate. | |
| You decide | Claude picks, keeping SBX-04's three exits. | |

**User's choice:** Untrusted output carries the role.

### Launch key and the judge version

| Option | Description | Selected |
|--------|-------------|----------|
| Yes, JudgeVersion for every role (Recommended) | `H("minos/launch/v1", exp, role tag, judge, trial)`, matching `Judged`'s key. | ✓ |
| Only for the Judge role | Two key shapes; cached candidate output reused across judge versions. | |
| No: exactly exp, role, trial | A rejudge collides with the first judge launch's key. | |

**User's choice:** Yes, JudgeVersion for every role.

### Fuzz corpus without libFuzzer

| Option | Description | Selected |
|--------|-------------|----------|
| Committed corpus replayed by tests, plus proptest (Recommended) | Hostile tar fixtures each asserting the exact rejection, plus arbitrary-tar proptest; no new tooling. | ✓ |
| cargo-fuzz in an out-of-workspace crate | libFuzzer stays out of the gated graph but the repo carries C++ build tooling. | |
| You decide | Claude picks within the no-C rule. | |

**User's choice:** Committed corpus replayed by tests, plus proptest.

### Spec shape and launch return

| Option | Description | Selected |
|--------|-------------|----------|
| Blob IDs and digests, not bytes (Recommended) | `ImageDigest`, input-disk `BlobId`, `JobBudget`, machine size, launch key; Judge adds judge-data `BlobId`; `launch(Cleared) -> Running<R>`; `wait -> Untrusted<GuestOutput<R>>`. | ✓ |
| Byte buffers inline | A sealed plan carries megabytes. | |
| You decide | Claude picks within the role rules. | |

**User's choice:** Blob IDs and digests, not bytes.

---

## Claude's Discretion

- Which crate hosts the source packer and the git exec (offered as a further gray area; the user chose to proceed to context).
- Numeric caps for the snapshot, the output reader, blob reads, and machine size.
- The `Blobs` API beyond `put`, `get(id, max)`, `contains`; in-memory store concurrency.
- Exact regexes, hit reporting, and the exact `git` argv and allowlist.
- Whether the pack refuses a file whose hash equals a judge-data blob.
- `GuestOutput<R>` structure; `Capabilities`, `Refusal`, and `Host` shapes; role tag encoding; plan count and ordering.

## Deferred Ideas

- `CumulativeDiff` newtype (03-REVIEW WR-03) for Phase 7.
- A flag to relocate the `.minos/` data root.
- Per-file secret allowlist or an entropy heuristic.
- `minos repair` for mid-file ledger corruption.
- The `Agent` role (TIER-01).
- Concurrent candidate VMs (NEXT-04).
- Folding `GuestBytes` into `Untrusted<T>` in Phase 5.
