# Phase 7: Runner and Loop - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-10-22
**Phase:** 7-runner-and-loop
**Areas discussed:** Judge data push and versioning, Baseline calibration and resume, Ledger provenance additions, Caps, stops, and prices

---

## Judge data push and versioning

### Who picks the `[judge] version` label?

| Option | Description | Selected |
|--------|-------------|----------|
| The manifest (Recommended) | `push` reads `[judge] version` from `minos.toml`; a different tree under a registered label is refused unless `--update`, printing both digests (the `env build` D-10 gate) | ✓ |
| An explicit `--version N` flag | The user names the label on every push; the manifest must match at run time | |
| Auto-increment | `push` picks the next free label and prints it; the manifest can silently point at an old version | |

**User's choice:** The manifest.

### Where does the label-to-data mapping live?

| Option | Description | Selected |
|--------|-------------|----------|
| In `minos.lock` (Recommended) | A `[[judge]]` table per label beside `[[image]]`; `Lock::check` refuses a run whose label has no entry; digests only, so committing reveals nothing | ✓ |
| A separate committed judge lock file | A second lock with its own `v` tag and golden file | |
| Only inside `.minos/` (not committed) | A store-local index; a clone has no record and the lock cannot gate it | |

**User's choice:** In `minos.lock`.

### How is the judge directory packed, and which hygiene applies?

| Option | Description | Selected |
|--------|-------------|----------|
| Same packer, denylist and scan on (Recommended) | `minos-source`'s deterministic ustar packer; tarball SHA-256 is the `TreeId`; denylist and secret scan with no override; 96 MiB cap refused | ✓ |
| Same packer, denylist on, no secret scan | The data never reaches a prompt or a candidate | |
| Opaque tarball, no hygiene | Pack whatever is in the directory | |

**User's choice:** Same packer, denylist and scan on.

### Should `push` record per-file digests so the source pack refuses overlap?

| Option | Description | Selected |
|--------|-------------|----------|
| Record digests, refuse overlap (Recommended) | The entry lists each file's path and SHA-256; the source pack refuses a packed file equal to any judge file, naming the paths | ✓ |
| Record digests, warn only | Report in the pack report and continue | |
| Tree digest only, no overlap check | Smaller entry; the check stays deferred | |

**User's choice:** Record digests, refuse overlap.

---

## Baseline calibration and resume

### How many baseline trials does generation 0 run, and who sets it?

| Option | Description | Selected |
|--------|-------------|----------|
| Manifest knob with a default (Recommended) | `[judge] trials`, default 3, range-checked against `MAX_TRIALS`; a project fact like the memory budget | ✓ |
| A fixed constant | One number in `limits.rs` | |
| A CLI flag | `minos run --trials N` per invocation | |

**User's choice:** Manifest knob with a default.

### Do `check` and `run` share one calibration?

| Option | Description | Selected |
|--------|-------------|----------|
| Every run calibrates itself (Recommended) | `run` always performs its own generation 0; `check` drives the same code on a diagnostic run of its own | ✓ |
| `check` starts the run, `run` resumes it | One run spans both commands | |

**User's choice:** Every run calibrates itself.

### On resume, what does the loop re-do inside an unconcluded generation?

| Option | Description | Selected |
|--------|-------------|----------|
| Stage from the ledger, never re-propose (Recommended) | No `Proposed` → propose once (`Spent` first); `Proposed` present → run missing trials by `LaunchKey`; all judged → conclude | ✓ |
| Re-propose when fewer than `batch` proposals are recorded | Call again until `batch` proposals exist | |
| You decide | Claude picks the rule | |

**User's choice:** Stage from the ledger, never re-propose.

### How is `Appended::Conflict` handled?

| Option | Description | Selected |
|--------|-------------|----------|
| Make it a store error (Recommended) | `Err(StoreError::Conflict { seq })`, non-retryable like `Corrupt`; the driver exits and records nothing | ✓ |
| Keep `Ok(Conflict)`; the loop stops on it | Discipline at each call site | |
| Treat Conflict as Duplicate | The first record silently wins | |

**User's choice:** Make it a store error.

---

## Ledger provenance additions

### Should failure logs survive a restart?

| Option | Description | Selected |
|--------|-------------|----------|
| Optional `logs` field on `Judged` (Recommended) | `logs: Option<BTreeMap<Stage, BlobId>>` naming the capped raw stage logs; every reader goes through `LogExcerpt` | ✓ |
| Store only the sanitized excerpt | Same field, but the blob is the 4 KiB stripped tail | |
| Live session only | No ledger change | |

**User's choice:** Optional `logs` field on `Judged`.

### How does the root image enter the run's identity?

| Option | Description | Selected |
|--------|-------------|----------|
| Optional `root_image` on `RunStarted` (Recommended) | From `Lock::check` at start; every start and resume requires equality with the lock, else refuses with both digests | ✓ |
| Pin from recorded `BackendInfo` only | Compare the lock to existing `Judged` events; nothing to pin against before the first trial | |
| Fold the lock into `ManifestId` | Changes a golden vector; ties the manifest's identity to a file it does not contain | |

**User's choice:** Optional `root_image` on `RunStarted`.

### What does the JUDGE-04 backend pin look like?

| Option | Description | Selected |
|--------|-------------|----------|
| Pin every run to its first `Judged` (Recommended) | Refuse a `Judged` whose `(backend, arch)` differs from the run's first, whatever the objective | ✓ |
| An objective flag | `kind = "speed"` marks hardware-dependent objectives | |
| Pin through `[environment] requires` | Add `backend` beside `arch` | |

**User's choice:** Pin every run to its first `Judged`.

### Should core add the `CumulativeDiff` newtype?

| Option | Description | Selected |
|--------|-------------|----------|
| Add the newtype (Recommended) | Produced only by `compose` and `diff_trees` against the base; `Experiment` and `JobRequest::candidate` take it | ✓ |
| No newtype | The loop's single admission function is the only place | |
| You decide | Claude picks | |

**User's choice:** Add the newtype.

---

## Caps, stops, and prices

### What happens when `prices.toml` is missing or lacks a rate?

| Option | Description | Selected |
|--------|-------------|----------|
| Refuse to start or resume (Recommended) | The CLI parses the table first; a missing file, parse error, or missing compute or model rate refuses the run naming the path and keys | ✓ |
| Built-in default rates | Embedded rates, labelled in output | |
| Sweep-only runs skip the table | Only the missing-model check relaxed; compute rates still required | |

**User's choice:** Refuse to start or resume.

### Where are the safe points for stops and cap checks?

| Option | Description | Selected |
|--------|-------------|----------|
| Every generation, model call, and launch (Recommended) | Re-fold before each; a running VM is awaited and recorded; the driver never kills a VM on a stop | ✓ |
| Between generations only | One check per generation | |

**User's choice:** Every generation, model call, and launch.

### How does the spend cap treat compute?

| Option | Description | Selected |
|--------|-------------|----------|
| Metered only, overshoot by at most one step (Recommended) | `spend_ok` prices recorded `Spent` only; the docs state the overshoot | ✓ |
| Reserve the worst case before launch | Price the full stage budgets before launch and refuse | |

**User's choice:** Metered only.

### How is an infrastructure failure retried?

| Option | Description | Selected |
|--------|-------------|----------|
| Retry the same step after a short fixed pause (Recommended) | Streak in memory, success resets, `StopRequested { InfraFailures }` at the limit, nothing recorded for a failed attempt | ✓ |
| Retry immediately, no pause | Same counting, no wait | |
| No retry: the first failure stops | `max_infra_failures` becomes a count across the run | |

**User's choice:** Retry the same step after a short fixed pause.

---

## Claude's Discretion

- The `JobRunner` trait's shape, the fake job runner, and the CLI adapter.
- The runner's internals: candidate output blob hand-off, `JudgeData` attachment, host-measured usage, the per-stage log blob cap, which guest files are the stage logs, and the `(Ended, GuestStatus, output)` to `Outcome` table.
- The exact `CumulativeDiff` API.
- The `[[judge]]` lock entry's fields, the per-file digest encoding, what `push` prints, and the `--update` wording.
- The retry pause, the `trials` upper bound, and the log blob cap, each a constant with a reason.
- How an unscored baseline after generation 0 is reported.
- The observer seam for Phase 8's progress line; the `driver.lock` wording.
- Plan count and ordering.

## Deferred Ideas

- The CLI commands and progress line (Phase 8).
- Concurrent candidates and K8 measurements (NEXT-04).
- Worst-case compute reservation before launch (rejected for v1).
- An objective `kind = "speed"` flag or `requires.backend` (rejected for v1).
- A separate judge lock file (rejected).
- Judge data over 96 MiB (open question, unchanged).
- `minos rejudge` and `minos steer` (NEXT-01, NEXT-02); champion re-measurement (NEXT-05).
- A `StopReason` for an unscored baseline (not added).
