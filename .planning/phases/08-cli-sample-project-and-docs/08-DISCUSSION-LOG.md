# Phase 8: CLI, Sample Project, and Docs - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-10-28 (discussion began 2026-10-24)
**Phase:** 08-cli-sample-project-and-docs
**Areas discussed:** Sample project workload and objective, minos check diagnostics, Addressing and the report-to-adopt flow, Progress/status/log rendering

---

## Sample project workload and objective

### Q1. What should the objective measure, given the judge sees only judge data plus the candidate's out/ files?

| Option | Description | Selected |
|--------|-------------|----------|
| Quality from output files | Judge scores out/ against held-out expected output in its private data; nothing new in the guest or runner | ✓ |
| Speed, measured by the trusted guest | PID 1 times the run stage and the runner passes it into the judge's input; new guest field, section, and plumbing | |
| Both: quality objective, speed advisory | Quality judged; host-measured vCPU time shown as information only | |

**User's choice:** Quality from output files.
**Notes:** Confirmed before asking that `StageReport` carries only stage and end, and that the judge's input disk holds only `JUDGE_DATA` and `CANDIDATE_OUT`.

### Q2. Which toolchain should the sample's candidate use inside the VM?

| Option | Description | Selected |
|--------|-------------|----------|
| Rust, std only, no dependencies | Real build stage; measures the RLIMIT_AS default; large image, a compile per trial | ✓ |
| Python script, no build | Tiny image, fast trials; build stage and RLIMIT_AS unexercised | |
| Rust with a vendored dependency | Shows offline dependency baking; bigger first sample | |

**User's choice:** Rust, std only, no dependencies.

### Q3. What should the sample's workload be?

| Option | Description | Selected |
|--------|-------------|----------|
| Log field extraction against a private answer key | The CLAUDE.md logparse sketch; accuracy objective, held-out; row-count gate | ✓ |
| Bin-packing heuristic | Validity gate, bins-used objective; held-out instances in the snapshot; more judge logic | |
| You decide | Claude picks a small deterministic improvable workload | |

**User's choice:** Log field extraction against a private answer key.

### Q4. How should the judge program be implemented and reach the judge VM?

First form of the question offered "POSIX shell plus awk script" as the recommended option, with a Rust binary built into the image and a committed prebuilt binary as alternatives. The user rejected the question: "why would we even consider awk for this? your reasoning makes no sense." The question was reformulated with the judge fixed as a Rust crate.

| Option | Description | Selected |
|--------|-------------|----------|
| Compile in the judge VM from pushed source | judge push uploads the crate's source with the answer key; the judge command runs cargo in the judge VM with the target dir on /scratch, then runs the result | ✓ |
| Build on the host for aarch64 musl and push the binary | Documented cargo command; gitignored binary pushed with the key; manual rebuild step | |
| You decide | Pick by measured in-VM compile time | |

**User's choice:** Compile in the judge VM from pushed source.
**Notes:** Recorded as a standing rule: nothing in Minos, samples and judges included, is proposed in a scripting language.

**Continue or next:** Next area.

---

## minos check diagnostics

### Q1. How should check record its diagnostic run?

| Option | Description | Selected |
|--------|-------------|----------|
| Reuse a calibrated diagnostic run when the frame matches | Match manifest, base tree, root image, judge version with generation 0 complete; --fresh forces new trials | ✓ |
| A fresh diagnostic run on every check | Always pays [judge] trials launches | |
| You decide | | |

**User's choice:** Reuse a calibrated diagnostic run when the frame matches.

### Q2. What should check do when the lock, root image, or pushed judge data is missing or stale?

| Option | Description | Selected |
|--------|-------------|----------|
| Refuse with the exact command to run | Names minos env build or minos judge push; no implicit build | ✓ |
| Run env build and judge push itself when needed | check becomes the one command; hidden docker build; judge dir needs a manifest field | |
| Build only with an explicit --build flag | Two ways to build | |

**User's choice:** Refuse with the exact command to run.

### Q3. How should check handle the baseline trial count and the noise floor?

| Option | Description | Selected |
|--------|-------------|----------|
| Manifest count stays; check reports and advises | Runs [judge] trials; prints per-trial values, spread, min_improvement, noise verdict, advice; sample measured across boots to set its trials and confirm the default | ✓ |
| Adaptive trials until the spread stabilises | Reopens 07 D-05 | |
| You decide | | |

**User's choice:** Manifest count stays; check reports and advises.

### Q4. Should check cover the single_shot full-file batch output cap open question?

| Option | Description | Selected |
|--------|-------------|----------|
| Advise from the writable surface size | Bytes times batch through a conservative bytes-per-token constant against the cap; advisory beside the route table; constants in core limits | ✓ |
| Refuse to route single_shot when it would exceed the cap | Heuristic becomes a gate in ProposerSpec::eligible | |
| Leave it to the run's infra-failure breaker | Open question stays open | |

**User's choice:** Advise from the writable surface size.

**Continue or next:** Next area.

---

## Addressing and the report-to-adopt flow

### Q1. How should users name a run and an experiment?

| Option | Description | Selected |
|--------|-------------|----------|
| Unique prefix, run defaults to the latest | Hex prefix matching exactly one ID; ambiguous or absent lists candidates; --run defaults to the latest run | ✓ |
| Full IDs only | 64-character tokens everywhere | |
| Prefix for experiments, full ID or 'latest' for runs | Two rules | |

**User's choice:** Unique prefix, run defaults to the latest.

### Q2. Where is ReportAcknowledged recorded?

| Option | Description | Selected |
|--------|-------------|----------|
| adopt --reviewed <hash> is the acknowledgment | report prints and appends nothing; adopt rebuilds the report, requires an equal hash, appends ReportAcknowledged, then writes the patch | ✓ |
| report --ack records it; adopt checks is_acknowledged | Two commands; acknowledged-but-not-adopted state visible | |
| You decide | | |

**User's choice:** adopt --reviewed <hash> is the acknowledgment.

### Q3. Where should adopt write the patch file, and what if one exists?

| Option | Description | Selected |
|--------|-------------|----------|
| Project root, minos-<exp12>.patch, never overwrite, --out to redirect | create_new; prints the path and the git apply and patch -p1 commands | ✓ |
| Inside .minos/patches/<exp>.patch | Under the 0700 store dir; hidden location | |
| Stdout only | Nothing on disk; CLI-09 says a file | |

**User's choice:** Project root, minos-<exp12>.patch, never overwrite, --out to redirect.

### Q4. What should minos stop do when no driver holds the run?

| Option | Description | Selected |
|--------|-------------|----------|
| Refuse, naming the run, and record nothing | try_lock on driver.lock detects no driver; when held, append with the next ordinal and say the driver stops at its next safe point | ✓ |
| Append anyway and warn | Meaningless ledger lines | |
| You decide | | |

**User's choice:** Refuse, naming the run, and record nothing.

**Continue or next:** Next area.

---

## Progress, status, and log rendering

### Q1. What should the one-line-per-experiment progress line look like?

| Option | Description | Selected |
|--------|-------------|----------|
| Fixed-width columns, signed percent delta, VM elapsed | `g03 3f9a2c1d7e4b single_shot scored +6.2% 1m42s`; one Selected line; one approx. cost line per generation; plain text | ✓ |
| Key=value pairs per line | Greppable; longer | |
| You decide | | |

**User's choice:** Fixed-width columns, signed percent delta, VM elapsed.

### Q2. What should status show beyond the fold?

| Option | Description | Selected |
|--------|-------------|----------|
| Fold plus liveness and health | Run facts, incumbent versus baseline and noise floor, cost, stops, driver-held probe, Corrupt or Conflict with seq | ✓ |
| Fold only | No liveness or store health | |
| You decide | | |

**User's choice:** Fold plus liveness and health.

### Q3. How should log present the ledger?

| Option | Description | Selected |
|--------|-------------|----------|
| Rendered line per event, with --json for the stored lines | seq kind generation key fields; --generation and --exp filters; --json exact lines | ✓ |
| Rendered only | Users open the JSONL themselves | |
| You decide | | |

**User's choice:** Rendered line per event, with --json for the stored lines.

### Q4. What should show <exp> include?

| Option | Description | Selected |
|--------|-------------|----------|
| Full diff and rationale, lineage, outcomes; logs behind --logs | Header, per-trial outcomes, lineage, rationale via SafeText, full canonical diff; --logs appends LogExcerpt tails | ✓ |
| Summary only, diff behind --diff | Diff needs a flag | |
| You decide | | |

**User's choice:** Full diff and rationale, lineage, outcomes; logs behind --logs.

**Final check:** "I'm ready for context" (declined to explore init scaffolding and README/SECURITY wording further; left to Claude's discretion).

---

## Claude's Discretion

- `minos init` scaffold contents per kind and whether it writes a starter `prices.toml`.
- `README.md` and `SECURITY.md` structure and wording within the stated constraints.
- The sample's `program.md`, visible versus held-out split, `min_improvement` and `trials` values from measurement, advisory wording.
- `report`'s terminal layout and how `run` prints a resume refusal.
- Remaining flag names.

## Deferred Ideas

- A speed objective measured by the trusted guest (PID 1 timing passed into the judge VM).
- `check` building or pushing on its own, or a `--build` flag.
- An adaptive baseline trial count.
- Making an over-cap `single_shot` batch ineligible.
- `report --ack` as a separate step.
- Patches under `.minos/patches/`.
- `rejudge`, `steer`, `inbox`, `answer`, `params add`, `env shell` (later milestones).
- Moving CLAUDE.md design sections into `docs/`.
