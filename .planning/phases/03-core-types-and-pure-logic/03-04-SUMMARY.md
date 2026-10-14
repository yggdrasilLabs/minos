---
phase: 03-core-types-and-pure-logic
plan: 04
subsystem: minos-core
status: complete
tags: [core, money, usage, prices, renderer, safetext, proptest]
requires:
  - "03-01: ModelId (BTreeMap key, serde as string), MAX_LOG_EXCERPT_BYTES"
provides:
  - "usage.rs: Usage (tokens, compute, checked_add, total_tokens, six u64 accessors)"
  - "money.rs: PICO_PER_DOLLAR, RATE_FRACTION_DIGITS, RATE_MAX_INTEGER_DIGITS, PRICES_VERSION, Rate, RateError, ComputeRates, TokenRates, RawPrices, Prices, PricesError, Cost, CostError, ApproxCost"
  - "text.rs: is_invisible_format, SafeText (new, from_utf8_lossy, inline, as_str, len, is_empty), LogExcerpt (from_bytes, text, is_truncated)"
affects: [03-07, 03-11, 03-12, 03-13]
tech-stack:
  added: []
  patterns:
    - "Exact decimal parse by byte walk into u128 pico-dollars; reject, never round"
    - "One scaled u128 accumulator (1e-12 USD / 1_024_000) with a single deferred division, round half up in remainder form so it cannot overflow"
    - "Allowlist renderer: keep printable plus newline and tab, drop is_control, show an explicit invisible-format table as <U+XXXX>"
key-files:
  created:
    - crates/minos-core/tests/money.rs
    - crates/minos-core/tests/render.rs
  modified:
    - crates/minos-core/src/usage.rs
    - crates/minos-core/src/money.rs
    - crates/minos-core/src/text.rs
decisions:
  - "Rate rejection order: Empty, Whitespace, Exponent, Sign, Underscore, BadByte, MissingInteger, second-dot BadByte, IntegerTooLong, EmptyFraction, FractionTooLong. The FractionTooLong Display is exactly 'a rate has at most 12 fractional digits and is never rounded'"
  - "PricesError::Version carries the version found ({ found: u32 }), matching ScorecardError::Version"
  - "TokenRates private fields drop the _token suffix (pedantic struct_field_names) and serde renames them back to the TOML keys input_token, output_token, cache_creation_input_token, cache_read_input_token; accessors keep the *_token names"
  - "Cost::of with Some(model) always looks the model up, so an unpriced model fails closed even with zero tokens; with None, any non-zero token count is TokensWithoutModel"
  - "Cost and ApproxCost have no serde: the ledger stores Usage only, so no dollar type can be persisted"
  - "LogExcerpt drops up to three leading UTF-8 continuation bytes after a cut, so a tail that starts inside a character does not begin with U+FFFD"
metrics:
  duration: "6 min"
  completed: 2026-10-14
  tasks: 3
  files: 5
---

# Phase 3 Plan 04: Usage, exact money, and the SafeText renderer Summary

This plan adds three things:

- **`Usage`:** six `u64` counters with the `claude -p` token names, and no dollar field.
- **Money:** pico-dollar `Rate`s parsed from strings, which fail closed on a 13th fractional digit. A `prices.toml` whose numeric rates are located toml type errors. An exact `Cost` held as one scaled `u128` and divided once, rounding half up. `ApproxCost` as the only dollar formatter.
- **`SafeText`:** an allowlist renderer that drops every C0 and C1 control and shows bidi, zero-width, separator, tag, and variation-selector characters as `<U+XXXX>`. A capped `LogExcerpt` is built on it.

## What was built

- **Usage (COST-01).**
  - Fields: `input_tokens`, `output_tokens`, `cache_creation_input_tokens`, `cache_read_input_tokens` (the SPIKE-07 names), `vcpu_ms`, and `mem_mib_ms`.
  - It derives serde with `deny_unknown_fields`, and every field is required.
  - Methods: `const` constructors `tokens` and `compute`, `checked_add` and `total_tokens` (both return `None` on overflow), and one accessor per field.
  - It has no `u128`, because it travels inside the internally tagged `Event` enum.
- **Rate (CORE-14).**
  - Grammar: `[0-9]{1,6}(\.[0-9]{1,12})?`, read by walking bytes. Accumulation uses checked arithmetic, and the fraction is padded to 12 digits by multiplication.
  - It implements `FromStr`. Serde goes through `try_from`/`into` `String`, so a JSON or TOML number is refused.
  - Its canonical `Display` trims trailing fractional zeros and leading integer zeros, and round-trips through `parse`.
  - `RateError` has 11 variants, none of which carries input text.
- **Prices (COST-01).**
  - Wire types: `RawPrices { v, compute, models: BTreeMap<ModelId, TokenRates> }`, plus `ComputeRates` and `TokenRates`, all with `deny_unknown_fields`.
  - Methods: `Prices::parse_toml`, `compute()`, `model(&ModelId)`, and `models()`.
  - A numeric rate gives toml's located message. Observed text: `line 4`, `invalid type: floating point`, and `expected a string`.
- **Cost.**
  - Formula: `scaled = vcpu_pico * vcpu_ms * 1024 + gib_pico * mem_mib_ms + sum(token_pico * tokens * 1_024_000)`. Every multiplication and addition is checked, and overflow is `CostError::Overflow`.
  - `micro_dollars` uses `scaled / 1_024_000_000_000`, plus one when the remainder is at least `512_000_000_000`. This is round half up, and it cannot overflow.
  - `checked_add` sums in the scaled unit, so a total is rounded once.
- **ApproxCost.** `from_micro` and `micro`. `Display` rounds to cents half up and writes `approx. cost: $D.CC`.
- **SafeText (CORE-15).**
  - Handling: newline and tab are kept. Every other `char::is_control` is dropped (C0, DEL, and C1, including ESC, CR, and U+009B). The `is_invisible_format` table is shown as `<U+XXXX>` (uppercase hex, at least four digits). Everything else is kept.
  - `inline` also shows newline and tab as `<U+000A>` and `<U+0009>`.
  - It has no `Deref`, no `From<String>`, and no constructor that skips sanitizing.
- **LogExcerpt.**
  - The cap is the smaller of `max_bytes` and `MAX_LOG_EXCERPT_BYTES`. It takes the raw tail with `.get(start..)` and skips leftover continuation bytes after a cut.
  - It then decodes lossily and sanitizes. If expansion pushed the text past the cap, it drops leading characters at char boundaries.

Exact products (all asserted in `aws_rate_products_are_exact`):

| Rate | Usage | Micro-dollars | Display |
|---|---|---|---|
| `0.0000276944` per vCPU-s | 3_600_000 vCPU-ms | 99_700 | `approx. cost: $0.10` |
| `0.0000036667` per GiB-s | 1_024_000 MiB-ms | 4 | |
| `0.0000166667` per GiB-s | 1024 MiB x 3_600_000 ms | 60_000 | |
| `0.000003` per input token | 1_000_000 tokens | 3_000_000 | `approx. cost: $3.00` |

## Verification

| Command | Result |
|---|---|
| `cargo fmt --all --check` | ok |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0 |
| `cargo test --workspace` | exit 0. money: 44 passed (40 tests and 4 proptests). render: 29 passed (26 tests and 3 proptests). Every other suite passes as before |
| `cargo deny check` | advisories, bans, licenses, and sources all ok. The only warning is the existing `unmatched wrapper` for minos-sandbox |
| `cargo doc --workspace --no-deps` | ok |
| C-01 `[lints] workspace = true` | all crates ok, no MISSING |
| C-02 `grep -rnE '#!?\[allow\(' crates --include='*.rs'` | prints nothing |
| `grep -nE '\b(f32\|f64)\b' money.rs usage.rs` | prints nothing |
| `grep -n u128 usage.rs` | prints nothing |
| `grep -nE 'strip-ansi\|vte' crates/minos-core/Cargo.toml` | prints nothing |
| `grep -n 'impl.*Deref' text.rs` | prints nothing |
| `test ! -e crates/minos-core/tests/snapshots` | true |

## TDD Gate Compliance

All three tasks were committed RED, then GREEN. Each RED commit failed to compile because the API it tests did not exist yet. No refactor commits were needed.

- Task 1: RED `7ab54f1`, then GREEN `98aa737`
- Task 2: RED `e301b16`, then GREEN `9624e17`
- Task 3: RED `54bd9f6`, then GREEN `3bb0ce4`

## Commits

| Task | Commit | Message |
|---|---|---|
| 1 RED | 7ab54f1 | test(03-04): add failing Rate grammar and Usage tests |
| 1 GREEN | 98aa737 | feat(03-04): implement integer Usage and exact pico-dollar Rate |
| 2 RED | e301b16 | test(03-04): add failing Prices, Cost, and ApproxCost tests |
| 2 GREEN | 9624e17 | feat(03-04): implement Prices, exact Cost, and ApproxCost |
| 3 RED | 54bd9f6 | test(03-04): add failing SafeText escape corpus and LogExcerpt tests |
| 3 GREEN | 3bb0ce4 | feat(03-04): implement the SafeText allowlist renderer and LogExcerpt |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] TokenRates field names**
- **Found during:** Task 2
- **Issue:** Pedantic `struct_field_names` fails under `-D warnings` when all four private fields end in `_token`, and no `allow` is permitted (C-02).
- **Fix:** The private fields are now `input`, `output`, `cache_creation_input`, and `cache_read_input`, each with `#[serde(rename = "..._token")]`. The TOML keys and the public accessors are unchanged.
- **Commit:** 9624e17

**2. [Rule 3 - Blocking] Hex rendering without `format!` into a String**
- **Found during:** Task 3
- **Issue:** `out.push_str(&format!(...))` trips `format_push_string`. Using `write!` would mean discarding an infallible `fmt::Result`.
- **Fix:** `push_visible` writes the hex digits itself with `char::from_digit`, at least four digits, uppercase.
- **Commit:** 3bb0ce4

**3. [Rule 1 - Acceptance criterion] Usage doc wording**
- **Found during:** Task 1
- **Issue:** The module doc said "A `u128` would not survive...", which failed the `grep -n u128 usage.rs` = empty criterion. It also said `MicroVM` without backticks, which tripped `doc_markdown`.
- **Fix:** The doc now says "a 128-bit field" and "microVM".
- **Commit:** 98aa737

No other deviations. No dependency, lockfile, or clippy.toml change.

## Known Stubs

None. All three modules are fully implemented.

## Threat Model Coverage

| Threat | Mitigation | Where |
|---|---|---|
| T-03-04-01 | Allowlist drops every C0/C1 control including ESC, CR, and DEL. The corpus covers CSI, OSC 52, OSC 8 (BEL and ST forms), cursor moves, and CR overwrite | text.rs, render.rs |
| T-03-04-02 | Explicit invisible-format table shown as `<U+XXXX>`. A Trojan Source sample, plus proptests for no controls or invisibles and for idempotence | text.rs, render.rs |
| T-03-04-03 | `u128` pico-dollars from strings, one deferred division, round half up. No float in money.rs or usage.rs | money.rs |
| T-03-04-04 | A 13th fractional digit is `FractionTooLong`, never rounded | money.rs, money.rs tests |
| T-03-04-05 | Checked mul and add everywhere; `CostError::Overflow`. Extreme inputs are tested | money.rs |
| T-03-04-06 | `Usage` has no dollar field and rejects unknown fields. Cost types have no serde | usage.rs, money.rs |
| T-03-04-07 | `LogExcerpt` clamps to `MAX_LOG_EXCERPT_BYTES` after expansion. A proptest checks the cap | text.rs, render.rs |

No new threat surface beyond the register.

## Self-Check: PASSED

- All five created or modified files exist.
- Commits 7ab54f1, 98aa737, e301b16, 9624e17, 54bd9f6, and 3bb0ce4 are in `git log`.
