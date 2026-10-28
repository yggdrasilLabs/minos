---
created: 2026-10-28T22:39:14.607Z
title: Explain why the judge is a separate program
area: docs
files:
  - examples/logparse/README.md:9-30
  - README.md:86-103
  - examples/logparse/judge/src/main.rs
---

## Problem

During the Phase 8 UAT on 2026-10-28 the user's first reaction to the sample was that the judge being a
different binary "feels weird". The sample README says the judge is a second Rust program in `judge/` and that
Minos compiles and runs it in its own VM, but it does not say why a project cannot score itself with a `cargo
bench` or a test target in the candidate crate. A new user meets the shape before the reason.

## Solution

Add a short "Why the judge is its own program" passage to the sample README (and a sentence in the root README's
quick start) that gives the reason from the invariants rather than the mechanism:

- Judge integrity: a judge linked into the candidate crate would run candidate code in the judge VM with the
  private data in reach, and the candidate's build (Cargo.toml, build.rs, dependencies) could shape the judge
  binary. A separate crate with no dependency on the candidate is what keeps the scorer out of the candidate's
  reach.
- The judge is a grader of output files, the same shape as a competition scorer: it reads `/scratch/candidate/out/`
  and its own answer key, never the candidate's source, and never a number the candidate reports about itself.
- Keeping `judge/` out of the snapshot and the prompt is why the proposer cannot see the scoring logic or key.

Consider whether an `optimize` project with a pure timing objective could get a built-in harness-measured judge
later (no user judge program at all), which would remove the ceremony for the simplest case. TBD; a design
question for a later milestone, not a Phase 8 change.
