# Task: improve the log parser

`logparse` reads `data/input.log` and writes one tab-separated row per input line. Each row is the 1-based line
number followed by four fields that `parse_line` in `src/parse.rs` extracts from that line. Improve `src/parse.rs`
so every input line yields the right fields. It is the only file you may change; `src/main.rs` and
`src/record.rs` are shown for context.

## Output spec

Each input line yields four fields, rendered in this order and joined by tabs, with `-` for a field the line does
not carry:

1. `timestamp`, normalized to `YYYY-MM-DDTHH:MM:SSZ`.
2. `level`, one of `DEBUG`, `INFO`, `WARN`, `ERROR`. Input spellings are case-insensitive; `warning` means `WARN`
   and `err` means `ERROR`.
3. `component`.
4. `latency_ms`, a non-negative integer.

Lines come in three formats. Any format may have runs of spaces between tokens and leading or trailing spaces.

- **A:** `<ISO timestamp ending in Z> <level> <component>: <message>`. The latency comes from a
  `latency_ms=<n>` token in the message.
  Example: `2026-10-28T14:03:11Z INFO auth: login ok user=alice latency_ms=12`
  yields `2026-10-28T14:03:11Z`, `INFO`, `auth`, `12`.
- **B:** `<Mon> <D> <HH:MM:SS> <host> <component>[<pid>]: <message>`. The year is 2026 and the time is UTC.
  There is no level, so the level is `-`. The latency comes from a final `<n>ms` token.
  Example: `Oct 28 14:03:12 web01 nginx[2211]: GET /api/orders 200 15ms`
  yields `2026-10-28T14:03:12Z`, `-`, `nginx`, `15`.
- **C:** `[YYYY/MM/DD HH:MM:SS] <level> <component>: <message>`. The latency comes from `took <n> ms` in the
  message.
  Example: `[2026/10/28 14:03:13] INFO db: query took 42 ms`
  yields `2026-10-28T14:03:13Z`, `INFO`, `db`, `42`.

A line with no latency token has latency `-`.

## Examples and scoring

`data/examples.tsv` shows the expected rows for some of the input lines, in exactly the form the binary writes.
Scoring also covers input lines whose answers are not shown, so hard-coding the visible answers does not help,
and a change that does so is flagged in review. Write a parser for the formats above.

## Rules

- Keep `pub fn parse_line(line: &str) -> Record` and its behaviour of returning a `Record`.
- Std only: add no dependencies.
- The crate forbids `unwrap`, `expect`, and slice indexing (`x[i]`, `&s[a..b]`); use `get`, `strip_prefix`,
  `split_once`, slice patterns, and iterators instead. Do not add `allow` or `expect` attributes.
