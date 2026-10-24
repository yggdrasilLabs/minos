//! The driver's tests, on `MemoryStore`, `FakeProvider`, and an in-test fake job runner, with no VM and no model
//! login (LOOP-09).
//!
//! Every driver scenario lives in this one test target, so the shared `support` module's items are all used
//! inside one crate and `-D warnings` sees no dead code. Each scenario family is one module: `flow` (07-13),
//! `caps`, `breaker`, `stops`, and `pins` (07-15), and `resume` (07-16).

mod breaker;
mod caps;
mod flow;
mod pins;
mod resume;
mod stops;
mod support;
