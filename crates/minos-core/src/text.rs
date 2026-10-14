//! Safe rendering of hostile text: the `SafeText` allowlist renderer and `LogExcerpt`.
//!
//! Every string that originated in a sandbox or a model is printed only through this module, which keeps printable
//! characters plus newline and tab and drops every control character and bidi override. Filled by plan 03-04.
