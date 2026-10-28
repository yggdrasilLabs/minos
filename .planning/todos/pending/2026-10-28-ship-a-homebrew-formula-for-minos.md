---
created: 2026-10-28T22:39:14.607Z
title: Ship a Homebrew formula for minos
area: tooling
files:
  - README.md:27-70
  - crates/minos-vmm/minos-vmm.entitlements
  - crates/minos-backend-krun/src/host.rs
---

## Problem

The install flow in README.md is the developer-checkout flow: two `cargo build` invocations (the release
binaries and the musl guest), putting `target/release` on `PATH`, then `minos host setup` to sign `minos-vmm`,
and `host setup` again after every rebuild that relinks the helper. The user raised this during the Phase 8 UAT
on 2026-10-28 as a DX gap, not a Phase 8 defect: it is packaging work for a later milestone.

## Solution

A `brew install minos` formula that collapses the flow to `brew install minos` then `minos host doctor`:

- Build `minos`, `minos-vmm`, and the musl guest, and install all three into one bin directory. The locator
  already accepts "next to `minos`" as a valid layout (05-CONTEXT D-04), so no code changes.
- Run the ad-hoc `codesign` with `crates/minos-vmm/minos-vmm.entitlements` in the formula's install step, as the
  krunkit formula in the libkrun tap does, so `host setup` becomes a repair command rather than a setup step.
- Depend on `libkrunfw` from the `libkrun/krun` tap.

Open design points to settle first:

- `host::KRUNFW_SHA256` pins libkrunfw 5.6.2 at its Cellar path. A `brew upgrade libkrunfw` would break every
  run until Minos re-pins, and a formula makes that collision more likely. Decide how the pin tracks the tap.
- Homebrew's `rust` formula lacks the `aarch64-unknown-linux-musl` target, so the guest needs a prebuilt release
  asset or a CI-built bottle.

Docker, `~/.config/minos/prices.toml`, and the logged-in `claude` CLI remain the user's steps; no installer can do
those.
