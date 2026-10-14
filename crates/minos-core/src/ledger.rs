//! The frozen `Event` set, the versioned `Record` wrapper, and `EventKey`.
//!
//! Run state is never stored; it is rebuilt by folding these events. The `v` tag is how the wire format changes.
//! Filled by plan 03-07.
