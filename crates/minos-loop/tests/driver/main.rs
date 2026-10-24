//! The driver's tests, on `MemoryStore`, `FakeProvider`, and an in-test fake job runner, with no VM and no model
//! login (LOOP-09).
//!
//! Every driver scenario lives in this one test target, so the shared `support` module's items are all used
//! inside one crate and `-D warnings` sees no dead code. Later plans add their modules here beside `flow`.

mod flow;
mod support;
