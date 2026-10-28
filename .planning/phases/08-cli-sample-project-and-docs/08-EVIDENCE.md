# Phase 8 evidence

VM runs and measurements for plan 08-13 (DOCS-02, JUDGE-02; 08-CONTEXT D-02, D-04, D-07), run by the executor on
this Mac (the 05-17 and 07-14 precedent). Outputs are verbatim test and command output; the only edit is the home
directory written as `~`. The tests print only what the commands already rendered through `SafeText`, stage log tails
through `LogExcerpt`, and numbers.

- Date: 2026-10-28
- `rustc --version`: rustc 1.97.1 (8bab26f4f 2026-07-14)
- `cargo --version`: cargo 1.97.1 (c980f4866 2026-06-30)
- `sw_vers -productVersion`: 27.0.1 (build 26A434), arm64
- HEAD when the runs were made: ccec267 (test(08-13): add the ignored sample VM tests); the final-budgets run below
  ran on the same HEAD with the step 5 edits to `examples/logparse/minos.toml`, `crates/minos-core/src/limits.rs`,
  and `CLAUDE.md` in the working tree, committed with Task 2
- libkrunfw: /opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib (5.6.2, `libkrun/krun`)
- Docker: `docker --version`: Docker version 29.6.1, build 8900f1d; `docker buildx version`:
  github.com/docker/buildx v0.35.0-desktop.2 b554ce1decd8b509893b1e7c6227eabfb923d094
- `claude`: `~/.local/bin/claude` -> `~/.local/share/claude/versions/2.1.312` (not called by these tests; the provider
  is scripted)
- Environment for every VM run: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest MINOS_KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib`

## Build and sign

Every command ran at the repository root (`~/Repos/minos`), in this order. The test binary was built before the
signing, and no cargo command between the signing and the VM run relinked `minos-vmm` (`minos-cli` does not build it).

Command: `cargo build -p minos-vmm`

Exit code: 0

```text
   Compiling minos-vmm v0.1.0 (~/Repos/minos/crates/minos-vmm)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.91s
```

Command: `cargo build -p minos-guest --release --target aarch64-unknown-linux-musl`

Exit code: 0

```text
   Compiling minos-guest v0.1.0 (~/Repos/minos/crates/minos-guest)
    Finished `release` profile [optimized] target(s) in 6.31s
```

Command: `cargo test -p minos-cli --test sample_vm --no-run`

Exit code: 0

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.04s
  Executable tests/sample_vm.rs (target/debug/deps/sample_vm-df5368a586a7268c)
```

Command: `cargo build -p minos-cli`

Exit code: 0

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.04s
```

Command: `codesign --force --sign - --options runtime --entitlements crates/minos-vmm/minos-vmm.entitlements target/debug/minos-vmm`

Exit code: 0

```text
target/debug/minos-vmm: replacing existing signature
```

Command: `codesign --display --entitlements - target/debug/minos-vmm`

Exit code: 0

```text
Executable=~/Repos/minos/target/debug/minos-vmm
[Dict]
	[Key] com.apple.security.cs.disable-library-validation
	[Value]
		[Bool] true
	[Key] com.apple.security.hypervisor
	[Value]
		[Bool] true
```

## Host checks

Command: `cargo run -q -p minos-cli -- host doctor`

Exit code: 0

```text
binaries (fixed locations; no configuration file or environment variable):
  found     ~/Repos/minos/target/debug/minos-vmm  a regular file
  rejected  ~/Repos/minos/target/debug/minos-guest  not an ELF file (the macOS build of minos-guest is Mach-O)
  found     ~/Repos/minos/target/aarch64-unknown-linux-musl/release/minos-guest  a static AArch64 ELF
  kernel    /opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib  checked below

checks:
ok       helper present  a regular file of 17955744 bytes
  checked: ~/Repos/minos/target/debug/minos-vmm
ok       helper entitlements  both entitlements are set to true
  checked: ~/Repos/minos/target/debug/minos-vmm
  checked: /usr/bin/codesign
ok       guest kernel  sha256 59764391005357bbdd5218f3fd522dd8ab63722cbc96144752c7bddc2c8e5217
  checked: /opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib
ok       hypervisor  kern.hv_support 1, hw.ncpu 16, hw.memsize 51539607552; largest VM 8 vCPUs and 16384 MiB, leaving half the host's memory to the host
  checked: /usr/sbin/sysctl
ok       job socket path  41 of at most 103 bytes
  checked: /private/tmp/minos-501/mnsXXXXXXXX/o.sock
ok       sandbox-exec present  a regular file
  checked: /usr/bin/sandbox-exec
ok       Seatbelt denies network  the control connected; under the profile nc exited 1 and the listener accepted nothing: no stderr
  checked: /usr/bin/sandbox-exec
  checked: /usr/bin/nc
  checked: /bin/cat
ok       Seatbelt denies unlisted files  the listed file read back; the unlisted read exited 1: cat: /private/tmp/mnpFqKCoeAQ/unlisted: Operation not permitted
  checked: /usr/bin/sandbox-exec
  checked: /usr/bin/nc
  checked: /bin/cat
ok       guest binary  a static aarch64 ELF of 1516040 bytes
  checked: ~/Repos/minos/target/aarch64-unknown-linux-musl/release/minos-guest

confinement: hardware, seatbelt, rlimits
dedicated user: missing (deferred, reduced hardening)

job sweep: /private/tmp/minos-501
  no job directories left over
builder: docker buildx v0.35.0-desktop.2, daemon 29.6.1 (/Applications/Docker.app/Contents/Resources/bin/docker)

result: every check passed
```

Command: `docker buildx version`

Exit code: 0

```text
github.com/docker/buildx v0.35.0-desktop.2 b554ce1decd8b509893b1e7c6227eabfb923d094
```

Both exited 0, so step 3's host failure path did not apply.

## Sample end to end on real VMs (08-13)

Command: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest MINOS_KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib cargo test -p minos-cli --test sample_vm -- --ignored --nocapture --test-threads=1`

Exit code: 0

The command ran both tests. Its output is split at the second test's first line: the end-to-end test is below, the
measurement test's output is in the next three sections, and the pass count, which covers both, ends this section.
By the shell's clock the command ran from 19:16:20 to 19:42:19 UTC; libtest reported 408.44 s. The gap between the
two was not investigated. The candidate VM ran `cargo build --release --frozen` and the sample's run command; the
judge VM ran the manifest's `cargo run --release --frozen --quiet --target-dir /tmp/judge-target -- answers.tsv
/scratch/candidate/out/fields.tsv /out/scorecard.json`. The scripted provider returned the improved parser fixture
in generation 1 and the fixture with one added comment line in generation 2. The test asserted that both `check`
runs and the run recorded only krun on arm64 (`backend_classes` is exactly `{(Krun, Arm64)}`), that the baseline's
`Judged` trial indexes are 0, 1, and 2, that the second `check` reused the first run, that generation 1 is
`Improved`, and that `patch -p1` on a fresh copy of the sample's `src/parse.rs` yields the fixture byte for byte with
no `.orig` or `.rej` file. The `literal match` flags in the report are the fixture's own unit tests, which quote
visible input lines; the report flags them as designed.

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running tests/sample_vm.rs (target/debug/deps/sample_vm-df5368a586a7268c)

running 2 tests
test sample_end_to_end_on_real_vms ... == env build: root image 47340764c306ad0b1d20513e46df60fb5373dcc8ed64560dd2f0937b1dd0ba18, lock written true, 5727 files, 649 directories, 1657 symlinks in the image, 1 base images
== judge push judge: label 1, tree 8e157032719c3f33eefc3525b5ecf381d31f2ce401005fa37d649fafe203c3ab, 5 files, 20329 bytes, judge version a920dc2de1a972479355fa013f4a085e0f64bed89ebc953d18cf6528a054643e
== minos check
project: logparse (manifest 8f580744fb35)
base: tree fc4a25d807d1, 12 files, 23680 bytes packed
root image: 47340764c306
judge: version a920dc2de1a9, label 1, 5 files, 20329 bytes
memory: 2048 MiB = guest reserve 256 MiB + scratch 1024 MiB + candidate allowance 768 MiB
judge scratch: needs 37 MiB of 1024 MiB
routes:
  0 single_shot haiku: eligible
provider: scripted test provider
baseline: run 988820162f00, 3 trials (each boots a candidate VM and then a judge VM)
trial 0: scored accuracy 0.3020833333333333
trial 1: scored accuracy 0.3020833333333333
trial 2: scored accuracy 0.3020833333333333
noise floor: median 0.3020833333333333, spread 0 over 3 trials (0 skipped)
min_improvement: 5% (threshold 0.015104166666666667 at the median)
noise verdict: ok
approx. cost: $0.00 (in 0 tokens, out 0 tokens)
== minos check (again)
project: logparse (manifest 8f580744fb35)
base: tree fc4a25d807d1, 12 files, 23680 bytes packed
root image: 47340764c306
judge: version a920dc2de1a9, label 1, 5 files, 20329 bytes
memory: 2048 MiB = guest reserve 256 MiB + scratch 1024 MiB + candidate allowance 768 MiB
judge scratch: needs 37 MiB of 1024 MiB
routes:
  0 single_shot haiku: eligible
provider: scripted test provider
baseline: reused run 988820162f00 (no VM launched)
trial 0: scored accuracy 0.3020833333333333
trial 1: scored accuracy 0.3020833333333333
trial 2: scored accuracy 0.3020833333333333
noise floor: median 0.3020833333333333, spread 0 over 3 trials (0 skipped)
min_improvement: 5% (threshold 0.015104166666666667 at the median)
noise verdict: ok
approx. cost: $0.00 (in 0 tokens, out 0 tokens)
== minos run --generations 2
run eb4376c50c9a started; target generation 2
g00 32c3381a1314 baseline    scored                         n/a   0m02s
g00 32c3381a1314 baseline    scored                         n/a   0m02s
g00 32c3381a1314 baseline    scored                         n/a   0m02s
g00 noise verdict: ok (advisory; minos check explains)
g00 approx. cost: $0.00 (in 0 tokens, out 0 tokens)
g01 proposed single_shot admitted 1 dropped 0
g01 8acd42f0d7f1 single_shot scored                     +231.0%   0m02s
g01 selected improved 8acd42f0d7f1
g01 approx. cost: $0.01 (in 1000 tokens, out 2000 tokens)
g02 proposed single_shot admitted 1 dropped 0
g02 7f356fa5d6ee single_shot scored                       +0.0%   0m02s
g02 selected kept 8acd42f0d7f1
g02 approx. cost: $0.02 (in 2000 tokens, out 4000 tokens)
run eb4376c50c9a reached generation 2
== minos report
minos report v1
  run: eb4376c50c9a4f7932a6aff1ee5933d5d22540f366c391453e102d2be35ef0ec
  experiment: 8acd42f0d7f1c3a3f1444eb5f9981d91a6769e329d99f1dccdc7e8e3be84c873
  judge: a920dc2de1a972479355fa013f4a085e0f64bed89ebc953d18cf6528a054643e
  base: fc4a25d807d18f1b2787b83f567414a6e8465914ffc69a7384c42e54ae8762e9

summary
  objective accuracy: baseline 0.3020833333333333 (0.3020833333333333 to 0.3020833333333333, 3 trials), winner 1 (1 trials), delta +0.6979166666666667 (+231.03448275862073%), clears min_improvement 5%: yes, clears noise: n/a
  gate rows_match: baseline 1 (1 to 1, 3 trials), winner 1 (1 trials), delta 0 (0%), clears noise: n/a
  held-out accuracy_held_out: baseline 0.3020833333333333 (0.3020833333333333 to 0.3020833333333333, 3 trials), winner 1 (1 trials), delta +0.6979166666666667 (+231.03448275862073%), clears noise: n/a

flags
  literal match: src/parse.rs:45 adds latency_ms=, which also appears in a visible surface file
  literal match: src/parse.rs:148 adds WARNING, which also appears in a visible surface file
  literal match: src/parse.rs:162 adds 2026-10-28T14:03:40Z  Err  billing:  refund failed  latency_ms=910, which also appears in a visible surface file
  literal match: src/parse.rs:174 adds Nov  3 09:15:02 web01 cron[77]: job nightly-report started 120ms, which also appears in a visible surface file
  literal match: src/parse.rs:186 adds [2026/10/28 14:03:21] warning db: slow query took 1250 ms, which also appears in a visible surface file

provenance
  isolation: hardware virtualization
  reduced hardening: dedicated_user
  judge: a920dc2de1a972479355fa013f4a085e0f64bed89ebc953d18cf6528a054643e
  backends: krun arm64
  root images: 47340764c306ad0b1d20513e46df60fb5373dcc8ed64560dd2f0937b1dd0ba18
  kernels: 59764391005357bbdd5218f3fd522dd8ab63722cbc96144752c7bddc2c8e5217
  models: claude-haiku-5-5
  prompts: 9e9c1e8d47c06691b931fd40b08e5ce9cb626754b0c263a17b429cbb3a1e29ee

lineage
  baseline 32c3381a1314b2aa614a9bd05cd4618aa0a4373039be7eafb795c008cf51d8a2
  generation 1 single_shot 8acd42f0d7f1c3a3f1444eb5f9981d91a6769e329d99f1dccdc7e8e3be84c873: Parse all three formats and normalize the level.

diff
diff --git a/src/parse.rs b/src/parse.rs
--- a/src/parse.rs
+++ b/src/parse.rs
@@ -1,39 +1,155 @@
 //! The parser: the one file a Minos proposer may change.
 //!
-//! This baseline is imperfect on purpose. It handles format A (`<ISO timestamp> <LEVEL> <component>: <message>`)
-//! only when its tokens are separated by single spaces and its level is already one of the four canonical
-//! spellings, and yields an empty record for every other line. `program.md` states the full spec.
+//! This is the improved parser for the `examples/logparse` sample, kept as a test fixture: the full-file content
+//! the Phase 8 VM test's scripted provider proposes in place of `src/parse.rs`. It is not compiled by the Minos
+//! workspace; it compiles only as `src/parse.rs` of the sample crate, under that crate's lints, and implements
+//! the whole output spec in the sample's `program.md`:
+//!
+//! - format A: `<YYYY-MM-DDTHH:MM:SSZ> <level> <component>: <message>`, latency from `latency_ms=<n>`;
+//! - format B: `<Mon> <D> <HH:MM:SS> <host> <component>[<pid>]: <message>`, year 2026, no level, latency from a
+//!   final `<n>ms` token;
+//! - format C: `[YYYY/MM/DD HH:MM:SS] <level> <component>: <message>`, latency from `took <n> ms`.
+//!
+//! Tokens may be separated by runs of spaces, and a line may carry leading or trailing spaces.
 
 use crate::record::Record;
 
-/// The canonical level names.
-const LEVELS: [&str; 4] = ["DEBUG", "INFO", "WARN", "ERROR"];
+/// The year a format B line carries implicitly.
+const SYSLOG_YEAR: &str = "2026";
+
+/// The three-letter month names format B uses, in calendar order.
+const MONTHS: [&str; 12] = [
+    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
+];
 
 /// Extracts the four fields from one input line.
 #[must_use]
 pub fn parse_line(line: &str) -> Record {
-    let mut tokens = line.split(' ');
-    let (Some(timestamp), Some(level), Some(component)) =
-        (tokens.next(), tokens.next(), tokens.next())
-    else {
-        return Record::default();
+    let tokens: Vec<&str> = line.split_whitespace().collect();
+    format_a(&tokens)
+        .or_else(|| format_c(&tokens))
+        .or_else(|| format_b(&tokens))
+        .unwrap_or_default()
+}
+
+/// `<YYYY-MM-DDTHH:MM:SSZ> <level> <component>: <message>`.
+fn format_a(tokens: &[&str]) -> Option<Record> {
+    let [stamp, level, component, message @ ..] = tokens else {
+        return None;
+    };
+    let (date, time) = stamp.strip_suffix('Z')?.split_once('T')?;
+    let timestamp = iso_timestamp(date, '-', time)?;
+    let component = component_name(component.strip_suffix(':')?)?;
+    let latency_ms = message
+        .iter()
+        .find_map(|token| token.strip_prefix("latency_ms="))
+        .and_then(number);
+    Some(Record::new(
+        Some(timestamp),
+        canonical_level(level),
+        Some(component),
+        latency_ms,
+    ))
+}
+
+/// `[YYYY/MM/DD HH:MM:SS] <level> <component>: <message>`.
+fn format_c(tokens: &[&str]) -> Option<Record> {
+    let [date, time, level, component, message @ ..] = tokens else {
+        return None;
     };
-    let Some(component) = component.strip_suffix(':') else {
-        return Record::default();
+    let date = date.strip_prefix('[')?;
+    let time = time.strip_suffix(']')?;
+    let timestamp = iso_timestamp(date, '/', time)?;
+    let component = component_name(component.strip_suffix(':')?)?;
+    let latency_ms = message.windows(3).find_map(|window| match window {
+        ["took", value, "ms"] => number(value),
+        _ => None,
+    });
+    Some(Record::new(
+        Some(timestamp),
+        canonical_level(level),
+        Some(component),
+        latency_ms,
+    ))
+}
+
+/// `<Mon> <D> <HH:MM:SS> <host> <component>[<pid>]: <message>`.
+fn format_b(tokens: &[&str]) -> Option<Record> {
+    let [month, day, time, _host, tag, message @ ..] = tokens else {
+        return None;
     };
-    if !timestamp.ends_with('Z') || component.is_empty() {
-        return Record::default();
+    let month = (1_u8..)
+        .zip(MONTHS)
+        .find_map(|(number, name)| (name == *month).then_some(number))?;
+    let day: u8 = day.parse().ok().filter(|day| (1..=31).contains(day))?;
+    let time = clock(time)?;
+    let (component, pid) = tag.strip_suffix("]:")?.split_once('[')?;
+    number(pid)?;
+    let component = component_name(component)?;
+    let latency_ms = message
+        .last()
+        .and_then(|token| token.strip_suffix("ms"))
+        .and_then(number);
+    Some(Record::new(
+        Some(format!("{SYSLOG_YEAR}-{month:02}-{day:02}T{time}Z")),
+        None,
+        Some(component),
+        latency_ms,
+    ))
+}
+
+/// `YYYY<sep>MM<sep>DD` and `HH:MM:SS` as `YYYY-MM-DDTHH:MM:SSZ`.
+fn iso_timestamp(date: &str, separator: char, time: &str) -> Option<String> {
+    let mut parts = date.split(separator);
+    let year = digits(parts.next()?, 4)?;
+    let month = digits(parts.next()?, 2)?;
+    let day = digits(parts.next()?, 2)?;
+    if parts.next().is_some() {
+        return None;
+    }
+    let time = clock(time)?;
+    Some(format!("{year}-{month}-{day}T{time}Z"))
+}
+
+/// `HH:MM:SS`, two digits each.
+fn clock(time: &str) -> Option<&str> {
+    let mut parts = time.split(':');
+    let all_two_digits = (0..3).all(|_| parts.next().and_then(|part| digits(part, 2)).is_some());
+    (all_two_digits && parts.next().is_none()).then_some(time)
+}
+
+/// `text` when it is exactly `count` ASCII digits.
+fn digits(text: &str, count: usize) -> Option<&str> {
+    (text.len() == count && text.bytes().all(|byte| byte.is_ascii_digit())).then_some(text)
+}
+
+/// A non-negative integer written in ASCII digits only.
+fn number(text: &str) -> Option<u64> {
+    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
+        return None;
     }
-    let level = LEVELS.contains(&level).then(|| level.to_owned());
-    let latency_ms = tokens
-        .find_map(|token| token.strip_prefix("latency_ms="))
-        .and_then(|value| value.parse().ok());
-    Record::new(
-        Some(timestamp.to_owned()),
-        level,
-        Some(component.to_owned()),
-        latency_ms,
-    )
+    text.parse().ok()
+}
+
+/// A component name: non-empty letters, digits, `-`, `_`, or `.`.
+fn component_name(text: &str) -> Option<String> {
+    let valid = !text.is_empty()
+        && text
+            .bytes()
+            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
+    valid.then(|| text.to_owned())
+}
+
+/// A level spelling, case-insensitive, as its canonical name; `warning` is `WARN` and `err` is `ERROR`.
+fn canonical_level(text: &str) -> Option<String> {
+    let canonical = match text.to_ascii_uppercase().as_str() {
+        "DEBUG" => "DEBUG",
+        "INFO" => "INFO",
+        "WARN" | "WARNING" => "WARN",
+        "ERROR" | "ERR" => "ERROR",
+        _ => return None,
+    };
+    Some(canonical.to_owned())
 }
 
 #[cfg(test)]
@@ -41,33 +157,45 @@
     use super::parse_line;
 
     #[test]
-    fn a_single_spaced_format_a_line_is_parsed() {
+    fn a_format_a_line_with_runs_of_spaces_is_parsed() {
+        // Arrange
+        let line = "2026-10-28T14:03:40Z  Err  billing:  refund failed  latency_ms=910";
+
+        // Act
+        let row = parse_line(line).to_tsv();
+
+        // Assert
+        assert_eq!(row, "2026-10-28T14:03:40Z\tERROR\tbilling\t910");
+    }
+
+    #[test]
+    fn a_format_b_line_has_no_level() {
         // Arrange
-        let line = "2026-10-28T14:03:11Z INFO auth: login ok user=alice latency_ms=12";
+        let line = "Nov  3 09:15:02 web01 cron[77]: job nightly-report started 120ms";
 
         // Act
         let row = parse_line(line).to_tsv();
 
         // Assert
-        assert_eq!(row, "2026-10-28T14:03:11Z\tINFO\tauth\t12");
+        assert_eq!(row, "2026-11-03T09:15:02Z\t-\tcron\t120");
     }
 
     #[test]
-    fn the_baseline_yields_nothing_for_format_b() {
+    fn a_format_c_line_takes_its_latency_from_took() {
         // Arrange
-        let line = "Oct 28 14:03:12 web01 nginx[2211]: GET /api/orders 200 15ms";
+        let line = "[2026/10/28 14:03:21] warning db: slow query took 1250 ms";
 
         // Act
         let row = parse_line(line).to_tsv();
 
         // Assert
-        assert_eq!(row, "-\t-\t-\t-");
+        assert_eq!(row, "2026-10-28T14:03:21Z\tWARN\tdb\t1250");
     }
 
     #[test]
-    fn the_baseline_yields_nothing_for_format_c() {
+    fn an_unrecognized_line_yields_an_empty_record() {
         // Arrange
-        let line = "[2026/10/28 14:03:13] INFO db: query took 42 ms";
+        let line = "not a log line";
 
         // Act
         let row = parse_line(line).to_tsv();

report hash: 4818bc1fc22a7d101ca506a7c3ed6285daa8cb54fe144ad403bf417164eceeb8
to adopt this experiment after review: minos adopt 8acd42f0d7f1 --run eb4376c50c9a --reviewed 4818bc1fc22a7d101ca506a7c3ed6285daa8cb54fe144ad403bf417164eceeb8
== minos adopt
acknowledged report 4818bc1fc22a for experiment 8acd42f0d7f1 (recorded)
patch: /private/tmp/msvbxyJ3X/logparse/minos-8acd42f0d7f1.patch
patch contents:
diff --git a/src/parse.rs b/src/parse.rs
--- a/src/parse.rs
+++ b/src/parse.rs
@@ -1,39 +1,155 @@
 //! The parser: the one file a Minos proposer may change.
 //!
-//! This baseline is imperfect on purpose. It handles format A (`<ISO timestamp> <LEVEL> <component>: <message>`)
-//! only when its tokens are separated by single spaces and its level is already one of the four canonical
-//! spellings, and yields an empty record for every other line. `program.md` states the full spec.
+//! This is the improved parser for the `examples/logparse` sample, kept as a test fixture: the full-file content
+//! the Phase 8 VM test's scripted provider proposes in place of `src/parse.rs`. It is not compiled by the Minos
+//! workspace; it compiles only as `src/parse.rs` of the sample crate, under that crate's lints, and implements
+//! the whole output spec in the sample's `program.md`:
+//!
+//! - format A: `<YYYY-MM-DDTHH:MM:SSZ> <level> <component>: <message>`, latency from `latency_ms=<n>`;
+//! - format B: `<Mon> <D> <HH:MM:SS> <host> <component>[<pid>]: <message>`, year 2026, no level, latency from a
+//!   final `<n>ms` token;
+//! - format C: `[YYYY/MM/DD HH:MM:SS] <level> <component>: <message>`, latency from `took <n> ms`.
+//!
+//! Tokens may be separated by runs of spaces, and a line may carry leading or trailing spaces.
 
 use crate::record::Record;
 
-/// The canonical level names.
-const LEVELS: [&str; 4] = ["DEBUG", "INFO", "WARN", "ERROR"];
+/// The year a format B line carries implicitly.
+const SYSLOG_YEAR: &str = "2026";
+
+/// The three-letter month names format B uses, in calendar order.
+const MONTHS: [&str; 12] = [
+    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
+];
 
 /// Extracts the four fields from one input line.
 #[must_use]
 pub fn parse_line(line: &str) -> Record {
-    let mut tokens = line.split(' ');
-    let (Some(timestamp), Some(level), Some(component)) =
-        (tokens.next(), tokens.next(), tokens.next())
-    else {
-        return Record::default();
+    let tokens: Vec<&str> = line.split_whitespace().collect();
+    format_a(&tokens)
+        .or_else(|| format_c(&tokens))
+        .or_else(|| format_b(&tokens))
+        .unwrap_or_default()
+}
+
+/// `<YYYY-MM-DDTHH:MM:SSZ> <level> <component>: <message>`.
+fn format_a(tokens: &[&str]) -> Option<Record> {
+    let [stamp, level, component, message @ ..] = tokens else {
+        return None;
+    };
+    let (date, time) = stamp.strip_suffix('Z')?.split_once('T')?;
+    let timestamp = iso_timestamp(date, '-', time)?;
+    let component = component_name(component.strip_suffix(':')?)?;
+    let latency_ms = message
+        .iter()
+        .find_map(|token| token.strip_prefix("latency_ms="))
+        .and_then(number);
+    Some(Record::new(
+        Some(timestamp),
+        canonical_level(level),
+        Some(component),
+        latency_ms,
+    ))
+}
+
+/// `[YYYY/MM/DD HH:MM:SS] <level> <component>: <message>`.
+fn format_c(tokens: &[&str]) -> Option<Record> {
+    let [date, time, level, component, message @ ..] = tokens else {
+        return None;
     };
-    let Some(component) = component.strip_suffix(':') else {
-        return Record::default();
+    let date = date.strip_prefix('[')?;
+    let time = time.strip_suffix(']')?;
+    let timestamp = iso_timestamp(date, '/', time)?;
+    let component = component_name(component.strip_suffix(':')?)?;
+    let latency_ms = message.windows(3).find_map(|window| match window {
+        ["took", value, "ms"] => number(value),
+        _ => None,
+    });
+    Some(Record::new(
+        Some(timestamp),
+        canonical_level(level),
+        Some(component),
+        latency_ms,
+    ))
+}
+
+/// `<Mon> <D> <HH:MM:SS> <host> <component>[<pid>]: <message>`.
+fn format_b(tokens: &[&str]) -> Option<Record> {
+    let [month, day, time, _host, tag, message @ ..] = tokens else {
+        return None;
     };
-    if !timestamp.ends_with('Z') || component.is_empty() {
-        return Record::default();
+    let month = (1_u8..)
+        .zip(MONTHS)
+        .find_map(|(number, name)| (name == *month).then_some(number))?;
+    let day: u8 = day.parse().ok().filter(|day| (1..=31).contains(day))?;
+    let time = clock(time)?;
+    let (component, pid) = tag.strip_suffix("]:")?.split_once('[')?;
+    number(pid)?;
+    let component = component_name(component)?;
+    let latency_ms = message
+        .last()
+        .and_then(|token| token.strip_suffix("ms"))
+        .and_then(number);
+    Some(Record::new(
+        Some(format!("{SYSLOG_YEAR}-{month:02}-{day:02}T{time}Z")),
+        None,
+        Some(component),
+        latency_ms,
+    ))
+}
+
+/// `YYYY<sep>MM<sep>DD` and `HH:MM:SS` as `YYYY-MM-DDTHH:MM:SSZ`.
+fn iso_timestamp(date: &str, separator: char, time: &str) -> Option<String> {
+    let mut parts = date.split(separator);
+    let year = digits(parts.next()?, 4)?;
+    let month = digits(parts.next()?, 2)?;
+    let day = digits(parts.next()?, 2)?;
+    if parts.next().is_some() {
+        return None;
+    }
+    let time = clock(time)?;
+    Some(format!("{year}-{month}-{day}T{time}Z"))
+}
+
+/// `HH:MM:SS`, two digits each.
+fn clock(time: &str) -> Option<&str> {
+    let mut parts = time.split(':');
+    let all_two_digits = (0..3).all(|_| parts.next().and_then(|part| digits(part, 2)).is_some());
+    (all_two_digits && parts.next().is_none()).then_some(time)
+}
+
+/// `text` when it is exactly `count` ASCII digits.
+fn digits(text: &str, count: usize) -> Option<&str> {
+    (text.len() == count && text.bytes().all(|byte| byte.is_ascii_digit())).then_some(text)
+}
+
+/// A non-negative integer written in ASCII digits only.
+fn number(text: &str) -> Option<u64> {
+    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
+        return None;
     }
-    let level = LEVELS.contains(&level).then(|| level.to_owned());
-    let latency_ms = tokens
-        .find_map(|token| token.strip_prefix("latency_ms="))
-        .and_then(|value| value.parse().ok());
-    Record::new(
-        Some(timestamp.to_owned()),
-        level,
-        Some(component.to_owned()),
-        latency_ms,
-    )
+    text.parse().ok()
+}
+
+/// A component name: non-empty letters, digits, `-`, `_`, or `.`.
+fn component_name(text: &str) -> Option<String> {
+    let valid = !text.is_empty()
+        && text
+            .bytes()
+            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
+    valid.then(|| text.to_owned())
+}
+
+/// A level spelling, case-insensitive, as its canonical name; `warning` is `WARN` and `err` is `ERROR`.
+fn canonical_level(text: &str) -> Option<String> {
+    let canonical = match text.to_ascii_uppercase().as_str() {
+        "DEBUG" => "DEBUG",
+        "INFO" => "INFO",
+        "WARN" | "WARNING" => "WARN",
+        "ERROR" | "ERR" => "ERROR",
+        _ => return None,
+    };
+    Some(canonical.to_owned())
 }
 
 #[cfg(test)]
@@ -41,33 +157,45 @@
     use super::parse_line;
 
     #[test]
-    fn a_single_spaced_format_a_line_is_parsed() {
+    fn a_format_a_line_with_runs_of_spaces_is_parsed() {
+        // Arrange
+        let line = "2026-10-28T14:03:40Z  Err  billing:  refund failed  latency_ms=910";
+
+        // Act
+        let row = parse_line(line).to_tsv();
+
+        // Assert
+        assert_eq!(row, "2026-10-28T14:03:40Z\tERROR\tbilling\t910");
+    }
+
+    #[test]
+    fn a_format_b_line_has_no_level() {
         // Arrange
-        let line = "2026-10-28T14:03:11Z INFO auth: login ok user=alice latency_ms=12";
+        let line = "Nov  3 09:15:02 web01 cron[77]: job nightly-report started 120ms";
 
         // Act
         let row = parse_line(line).to_tsv();
 
         // Assert
-        assert_eq!(row, "2026-10-28T14:03:11Z\tINFO\tauth\t12");
+        assert_eq!(row, "2026-11-03T09:15:02Z\t-\tcron\t120");
     }
 
     #[test]
-    fn the_baseline_yields_nothing_for_format_b() {
+    fn a_format_c_line_takes_its_latency_from_took() {
         // Arrange
-        let line = "Oct 28 14:03:12 web01 nginx[2211]: GET /api/orders 200 15ms";
+        let line = "[2026/10/28 14:03:21] warning db: slow query took 1250 ms";
 
         // Act
         let row = parse_line(line).to_tsv();
 
         // Assert
-        assert_eq!(row, "-\t-\t-\t-");
+        assert_eq!(row, "2026-10-28T14:03:21Z\tWARN\tdb\t1250");
     }
 
     #[test]
-    fn the_baseline_yields_nothing_for_format_c() {
+    fn an_unrecognized_line_yields_an_empty_record() {
         // Arrange
-        let line = "[2026/10/28 14:03:13] INFO db: query took 42 ms";
+        let line = "not a log line";
 
         // Act
         let row = parse_line(line).to_tsv();
apply it from the project directory with either:
  patch -p1 -i minos-8acd42f0d7f1.patch
  git apply --directory="$(git rev-parse --show-prefix)" minos-8acd42f0d7f1.patch
plain git apply run inside a subdirectory of a repository skips every file and still exits 0; minos never applies a patch
== patch -p1 stdout
patching file 'src/parse.rs'
== patch -p1 stderr
ok
[... the measurement test's output, recorded in the sections below ...]

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 408.44s

exit: 0
```

## RLIMIT_AS bisection (D-02)

From the run above (`sample_measurements_on_real_vms`). Each row is trial 0 of the baseline on a copy of the sample
whose `[environment]` is `memory_mib = 512 + allowance` and `scratch_mib = 256`, so the 256 MiB guest reserve and the
scratch leave exactly the allowance as the build stage's `RLIMIT_AS`. `candidate_ms` and `judge_ms` are the
host-measured VM wall times (vCPU milliseconds over the runner's 2 vCPUs). The output of `env build`, `judge push`,
the five rows, and the 384 MiB row's build log tail, verbatim:

```text
test sample_measurements_on_real_vms ... == env build: root image 47340764c306ad0b1d20513e46df60fb5373dcc8ed64560dd2f0937b1dd0ba18, lock written true, 5727 files, 649 directories, 1657 symlinks in the image, 1 base images
== judge push judge: label 1, tree 8e157032719c3f33eefc3525b5ecf381d31f2ce401005fa37d649fafe203c3ab, 5 files, 20329 bytes, judge version a920dc2de1a972479355fa013f4a085e0f64bed89ebc953d18cf6528a054643e
== RLIMIT_AS bisection (D-02)
allowance_mib=384 memory_mib=896 scratch_mib=256 outcome=rejected:build_failed candidate_ms=1007 judge_ms=n/a accuracy=n/a
-- build log tail (1380 bytes shown, head dropped: false):
   Compiling logparse v0.1.0 (/scratch/src)
Fatal glibc error: failed to register TLS destructor: out of memory
memory allocation of 1048576 bytes failed
stack backtrace:
error: could not compile `logparse` (bin "logparse")

Caused by:
  process didn't exit successfully: `rustc --crate-name logparse --edition=2024 src/main.rs --error-format=json --json=diagnostic-rendered-ansi,artifacts,future-incompat --crate-type bin --emit=dep-info,link -C opt-level=3 -C embed-bitcode=no '--warn=clippy::suspicious' '--warn=clippy::style' '--warn=clippy::perf' '--warn=clippy::pedantic' '--deny=clippy::correctness' '--warn=clippy::complexity' '--warn=clippy::cargo' '--forbid=clippy::unwrap_used' --forbid=unsafe_code '--deny=clippy::undocumented_unsafe_blocks' '--allow=clippy::too_many_lines' '--allow=clippy::struct_excessive_bools' '--allow=clippy::similar_names' '--allow=clippy::multiple_crate_versions' --warn=missing_docs '--forbid=clippy::indexing_slicing' '--forbid=clippy::expect_used' '--deny=clippy::allow_attributes_without_reason' '--deny=clippy::allow_attributes' -C overflow-checks=on --check-cfg 'cfg(docsrs,test)' --check-cfg 'cfg(feature, values())' -C metadata=fe36be44786ed9ba -C extra-filename=-aa91aeb7da64fad2 --out-dir /scratch/src/target/release/deps -C strip=debuginfo -L dependency=/scratch/src/target/release/deps` (signal: 6, SIGABRT: process abort signal)

allowance_mib=448 memory_mib=960 scratch_mib=256 outcome=scored candidate_ms=1178 judge_ms=1264 accuracy=0.3020833333333333
allowance_mib=512 memory_mib=1024 scratch_mib=256 outcome=scored candidate_ms=1220 judge_ms=1289 accuracy=0.3020833333333333
allowance_mib=640 memory_mib=1152 scratch_mib=256 outcome=scored candidate_ms=1239 judge_ms=1256 accuracy=0.3020833333333333
allowance_mib=768 memory_mib=1280 scratch_mib=256 outcome=scored candidate_ms=1232 judge_ms=1286 accuracy=0.3020833333333333
```

| allowance_mib | memory_mib | outcome | candidate_ms | judge_ms |
|---|---|---|---|---|
| 384 | 896 | rejected:build_failed (rustc SIGABRT, `failed to register TLS destructor: out of memory`) | 1007 | n/a |
| 448 | 960 | scored | 1178 | 1264 |
| 512 | 1024 | scored | 1220 | 1289 |
| 640 | 1152 | scored | 1239 | 1256 |
| 768 | 1280 | scored | 1232 | 1286 |

Reading: the smallest passing allowance is 448 MiB (384 MiB fails); the default allowance of 768 MiB (the 2048 and
1024 defaults) passes, 320 MiB above the smallest passing step, and the five default-configuration boots below all
scored too. `DEFAULT_MEMORY_MIB` and `DEFAULT_SCRATCH_MIB` are unchanged, and the sample keeps them. The judge also
compiled and scored with a 256 MiB scratch in every passing row; the judge's in-VM target size was not measured
separately (08-RESEARCH Pitfall 6).

## Judge compile time (D-04)

From the five boots at the default configuration (memory 2048 MiB, scratch 1024 MiB, allowance 768 MiB), with the
budgets in the sample before this plan (`build = "5m"`, `run = "1m"`, `judge = "5m"`):

```text
== default configuration: memory_mib=2048 scratch_mib=1024 allowance_mib=768; budget build 300000 ms, run 60000 ms, judge 300000 ms
boot=0 outcome=scored accuracy=0.3020833333333333 accuracy_held_out=0.3020833333333333 candidate_ms=1237 judge_ms=1369
boot=1 outcome=scored accuracy=0.3020833333333333 accuracy_held_out=0.3020833333333333 candidate_ms=1358 judge_ms=1434
boot=2 outcome=scored accuracy=0.3020833333333333 accuracy_held_out=0.3020833333333333 candidate_ms=1204 judge_ms=1264
boot=3 outcome=scored accuracy=0.3020833333333333 accuracy_held_out=0.3020833333333333 candidate_ms=1189 judge_ms=1259
boot=4 outcome=scored accuracy=0.3020833333333333 accuracy_held_out=0.3020833333333333 candidate_ms=1203 judge_ms=1262
```

The judge VM's host-measured time, boot through judge compile and scoring, was 1259 to 1434 ms against
`[budget] judge` of 300000 ms (5m). The candidate VM's, boot through the release build and the run, was 1189 to
1358 ms. Step 5 sets the sample's `[budget] build` and `judge` to the smallest whole minute that is at least ten
times the larger of those maxima and at least `1m`: `1m` (60000 ms, which is more than 44 times 1358 ms and
more than 41 times 1434 ms). `run` stays `1m`. Each budget bounds one stage inside its VM, and each VM time includes the boot, so the
stage times are below these VM times.

## Boot-to-boot variance (D-07)

The same five boots of the baseline at the sample's own configuration (above):

| boot | accuracy | accuracy_held_out | candidate_ms | judge_ms |
|---|---|---|---|---|
| 0 | 0.3020833333333333 | 0.3020833333333333 | 1237 | 1369 |
| 1 | 0.3020833333333333 | 0.3020833333333333 | 1358 | 1434 |
| 2 | 0.3020833333333333 | 0.3020833333333333 | 1204 | 1264 |
| 3 | 0.3020833333333333 | 0.3020833333333333 | 1189 | 1259 |
| 4 | 0.3020833333333333 | 0.3020833333333333 | 1203 | 1262 |

The objective's spread is 0 by construction: the judge's score is a pure function of the candidate's output file,
so every boot of the same tree scores the same (08-RESEARCH Pitfall 9), as do the three `check` trials above
(`noise floor: median 0.3020833333333333, spread 0 over 3 trials`). The per-boot times are host-measured VM wall
time, not an objective, and are the only boot-to-boot variance this sample measures: candidate 1189 to 1358 ms
(range 169 ms), judge 1259 to 1434 ms (range 175 ms). The default of 3 trials, and the sample's `trials` (the
default), stay a recorded assumption until a project with a timing objective exists.

**Decision for the user:** D-07 asked for the default baseline trial count to be set from measured boot-to-boot
variance. It was met partially. The sample's objective is deterministic, so its spread is 0 by construction and it
gives no objective variance to set a trial count from. The default of 3 trials and the sample's `trials` are
unchanged and remain a recorded assumption. The boot-to-boot variance recorded instead is VM wall time: candidate VM
1237, 1358, 1204, 1189, and 1203 ms (range 1189 to 1358 ms) and judge VM 1369, 1434, 1264, 1259, and 1262 ms (range
1259 to 1434 ms) over five boots. The options are to accept this outcome for the milestone, or to ask for a
measurement-only timing probe in a later phase. The STATE.md Phase 8 blocker is not closed by this data.

User answer: pending (Task 3 checkpoint)

## Sample end to end with the final budgets (08-13)

Rerun of the end-to-end test alone after step 5, on the sample carrying its final budgets:

```toml
[budget]
build = "1m"
run = "1m"
judge = "1m"
spend_usd = 5
```

No cargo command since the signing relinked `minos-vmm`: `cargo test -p minos-cli --test sample_vm --no-run`
finished in 0.36 s, and `codesign --display --entitlements - target/debug/minos-vmm | grep -c
com.apple.security.hypervisor` printed 1 immediately before the run. The manifest and base IDs differ from the
first run because `minos.toml` is part of the snapshot. By the shell's clock the command ran from 20:06:03 to
20:09:29 UTC.

Command: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest MINOS_KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib cargo test -p minos-cli --test sample_vm -- --ignored --nocapture --test-threads=1 sample_end_to_end_on_real_vms`

Exit code: 0

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running tests/sample_vm.rs (target/debug/deps/sample_vm-df5368a586a7268c)

running 1 test
test sample_end_to_end_on_real_vms ... == env build: root image 47340764c306ad0b1d20513e46df60fb5373dcc8ed64560dd2f0937b1dd0ba18, lock written true, 5727 files, 649 directories, 1657 symlinks in the image, 1 base images
== judge push judge: label 1, tree 8e157032719c3f33eefc3525b5ecf381d31f2ce401005fa37d649fafe203c3ab, 5 files, 20329 bytes, judge version a920dc2de1a972479355fa013f4a085e0f64bed89ebc953d18cf6528a054643e
== minos check
project: logparse (manifest 64bb07c178ba)
base: tree 0ae41f50a714, 12 files, 24190 bytes packed
root image: 47340764c306
judge: version a920dc2de1a9, label 1, 5 files, 20329 bytes
memory: 2048 MiB = guest reserve 256 MiB + scratch 1024 MiB + candidate allowance 768 MiB
judge scratch: needs 37 MiB of 1024 MiB
routes:
  0 single_shot haiku: eligible
provider: scripted test provider
baseline: run 8e50b1520e65, 3 trials (each boots a candidate VM and then a judge VM)
trial 0: scored accuracy 0.3020833333333333
trial 1: scored accuracy 0.3020833333333333
trial 2: scored accuracy 0.3020833333333333
noise floor: median 0.3020833333333333, spread 0 over 3 trials (0 skipped)
min_improvement: 5% (threshold 0.015104166666666667 at the median)
noise verdict: ok
approx. cost: $0.00 (in 0 tokens, out 0 tokens)
== minos check (again)
project: logparse (manifest 64bb07c178ba)
base: tree 0ae41f50a714, 12 files, 24190 bytes packed
root image: 47340764c306
judge: version a920dc2de1a9, label 1, 5 files, 20329 bytes
memory: 2048 MiB = guest reserve 256 MiB + scratch 1024 MiB + candidate allowance 768 MiB
judge scratch: needs 37 MiB of 1024 MiB
routes:
  0 single_shot haiku: eligible
provider: scripted test provider
baseline: reused run 8e50b1520e65 (no VM launched)
trial 0: scored accuracy 0.3020833333333333
trial 1: scored accuracy 0.3020833333333333
trial 2: scored accuracy 0.3020833333333333
noise floor: median 0.3020833333333333, spread 0 over 3 trials (0 skipped)
min_improvement: 5% (threshold 0.015104166666666667 at the median)
noise verdict: ok
approx. cost: $0.00 (in 0 tokens, out 0 tokens)
== minos run --generations 2
run f2ca2a2c45b3 started; target generation 2
g00 77068d7e62b3 baseline    scored                         n/a   0m02s
g00 77068d7e62b3 baseline    scored                         n/a   0m02s
g00 77068d7e62b3 baseline    scored                         n/a   0m02s
g00 noise verdict: ok (advisory; minos check explains)
g00 approx. cost: $0.00 (in 0 tokens, out 0 tokens)
g01 proposed single_shot admitted 1 dropped 0
g01 7f1a4b38bd38 single_shot scored                     +231.0%   0m02s
g01 selected improved 7f1a4b38bd38
g01 approx. cost: $0.01 (in 1000 tokens, out 2000 tokens)
g02 proposed single_shot admitted 1 dropped 0
g02 694f5868358e single_shot scored                       +0.0%   0m02s
g02 selected kept 7f1a4b38bd38
g02 approx. cost: $0.02 (in 2000 tokens, out 4000 tokens)
run f2ca2a2c45b3 reached generation 2
== minos report
minos report v1
  run: f2ca2a2c45b38f6d6415e8716db6be26540a42dac4567b3e83b3454c6696709e
  experiment: 7f1a4b38bd38233083b43aba247e77fc6e91ebbd551690b0670d868c32078d81
  judge: a920dc2de1a972479355fa013f4a085e0f64bed89ebc953d18cf6528a054643e
  base: 0ae41f50a714fab89b7cdeeaeed8e4ad112ac398805361a4930dc4d365e893d4

summary
  objective accuracy: baseline 0.3020833333333333 (0.3020833333333333 to 0.3020833333333333, 3 trials), winner 1 (1 trials), delta +0.6979166666666667 (+231.03448275862073%), clears min_improvement 5%: yes, clears noise: n/a
  gate rows_match: baseline 1 (1 to 1, 3 trials), winner 1 (1 trials), delta 0 (0%), clears noise: n/a
  held-out accuracy_held_out: baseline 0.3020833333333333 (0.3020833333333333 to 0.3020833333333333, 3 trials), winner 1 (1 trials), delta +0.6979166666666667 (+231.03448275862073%), clears noise: n/a

flags
  literal match: src/parse.rs:45 adds latency_ms=, which also appears in a visible surface file
  literal match: src/parse.rs:148 adds WARNING, which also appears in a visible surface file
  literal match: src/parse.rs:162 adds 2026-10-28T14:03:40Z  Err  billing:  refund failed  latency_ms=910, which also appears in a visible surface file
  literal match: src/parse.rs:174 adds Nov  3 09:15:02 web01 cron[77]: job nightly-report started 120ms, which also appears in a visible surface file
  literal match: src/parse.rs:186 adds [2026/10/28 14:03:21] warning db: slow query took 1250 ms, which also appears in a visible surface file

provenance
  isolation: hardware virtualization
  reduced hardening: dedicated_user
  judge: a920dc2de1a972479355fa013f4a085e0f64bed89ebc953d18cf6528a054643e
  backends: krun arm64
  root images: 47340764c306ad0b1d20513e46df60fb5373dcc8ed64560dd2f0937b1dd0ba18
  kernels: 59764391005357bbdd5218f3fd522dd8ab63722cbc96144752c7bddc2c8e5217
  models: claude-haiku-5-5
  prompts: 9e9c1e8d47c06691b931fd40b08e5ce9cb626754b0c263a17b429cbb3a1e29ee

lineage
  baseline 77068d7e62b3e3a138bed56e07f62a30865b4374794f265059cb88b5f81d715c
  generation 1 single_shot 7f1a4b38bd38233083b43aba247e77fc6e91ebbd551690b0670d868c32078d81: Parse all three formats and normalize the level.

diff
diff --git a/src/parse.rs b/src/parse.rs
--- a/src/parse.rs
+++ b/src/parse.rs
@@ -1,39 +1,155 @@
 //! The parser: the one file a Minos proposer may change.
 //!
-//! This baseline is imperfect on purpose. It handles format A (`<ISO timestamp> <LEVEL> <component>: <message>`)
-//! only when its tokens are separated by single spaces and its level is already one of the four canonical
-//! spellings, and yields an empty record for every other line. `program.md` states the full spec.
+//! This is the improved parser for the `examples/logparse` sample, kept as a test fixture: the full-file content
+//! the Phase 8 VM test's scripted provider proposes in place of `src/parse.rs`. It is not compiled by the Minos
+//! workspace; it compiles only as `src/parse.rs` of the sample crate, under that crate's lints, and implements
+//! the whole output spec in the sample's `program.md`:
+//!
+//! - format A: `<YYYY-MM-DDTHH:MM:SSZ> <level> <component>: <message>`, latency from `latency_ms=<n>`;
+//! - format B: `<Mon> <D> <HH:MM:SS> <host> <component>[<pid>]: <message>`, year 2026, no level, latency from a
+//!   final `<n>ms` token;
+//! - format C: `[YYYY/MM/DD HH:MM:SS] <level> <component>: <message>`, latency from `took <n> ms`.
+//!
+//! Tokens may be separated by runs of spaces, and a line may carry leading or trailing spaces.
 
 use crate::record::Record;
 
-/// The canonical level names.
-const LEVELS: [&str; 4] = ["DEBUG", "INFO", "WARN", "ERROR"];
+/// The year a format B line carries implicitly.
+const SYSLOG_YEAR: &str = "2026";
+
+/// The three-letter month names format B uses, in calendar order.
+const MONTHS: [&str; 12] = [
+    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
+];
 
 /// Extracts the four fields from one input line.
 #[must_use]
 pub fn parse_line(line: &str) -> Record {
-    let mut tokens = line.split(' ');
-    let (Some(timestamp), Some(level), Some(component)) =
-        (tokens.next(), tokens.next(), tokens.next())
-    else {
-        return Record::default();
+    let tokens: Vec<&str> = line.split_whitespace().collect();
+    format_a(&tokens)
+        .or_else(|| format_c(&tokens))
+        .or_else(|| format_b(&tokens))
+        .unwrap_or_default()
+}
+
+/// `<YYYY-MM-DDTHH:MM:SSZ> <level> <component>: <message>`.
+fn format_a(tokens: &[&str]) -> Option<Record> {
+    let [stamp, level, component, message @ ..] = tokens else {
+        return None;
+    };
+    let (date, time) = stamp.strip_suffix('Z')?.split_once('T')?;
+    let timestamp = iso_timestamp(date, '-', time)?;
+    let component = component_name(component.strip_suffix(':')?)?;
+    let latency_ms = message
+        .iter()
+        .find_map(|token| token.strip_prefix("latency_ms="))
+        .and_then(number);
+    Some(Record::new(
+        Some(timestamp),
+        canonical_level(level),
+        Some(component),
+        latency_ms,
+    ))
+}
+
+/// `[YYYY/MM/DD HH:MM:SS] <level> <component>: <message>`.
+fn format_c(tokens: &[&str]) -> Option<Record> {
+    let [date, time, level, component, message @ ..] = tokens else {
+        return None;
     };
-    let Some(component) = component.strip_suffix(':') else {
-        return Record::default();
+    let date = date.strip_prefix('[')?;
+    let time = time.strip_suffix(']')?;
+    let timestamp = iso_timestamp(date, '/', time)?;
+    let component = component_name(component.strip_suffix(':')?)?;
+    let latency_ms = message.windows(3).find_map(|window| match window {
+        ["took", value, "ms"] => number(value),
+        _ => None,
+    });
+    Some(Record::new(
+        Some(timestamp),
+        canonical_level(level),
+        Some(component),
+        latency_ms,
+    ))
+}
+
+/// `<Mon> <D> <HH:MM:SS> <host> <component>[<pid>]: <message>`.
+fn format_b(tokens: &[&str]) -> Option<Record> {
+    let [month, day, time, _host, tag, message @ ..] = tokens else {
+        return None;
     };
-    if !timestamp.ends_with('Z') || component.is_empty() {
-        return Record::default();
+    let month = (1_u8..)
+        .zip(MONTHS)
+        .find_map(|(number, name)| (name == *month).then_some(number))?;
+    let day: u8 = day.parse().ok().filter(|day| (1..=31).contains(day))?;
+    let time = clock(time)?;
+    let (component, pid) = tag.strip_suffix("]:")?.split_once('[')?;
+    number(pid)?;
+    let component = component_name(component)?;
+    let latency_ms = message
+        .last()
+        .and_then(|token| token.strip_suffix("ms"))
+        .and_then(number);
+    Some(Record::new(
+        Some(format!("{SYSLOG_YEAR}-{month:02}-{day:02}T{time}Z")),
+        None,
+        Some(component),
+        latency_ms,
+    ))
+}
+
+/// `YYYY<sep>MM<sep>DD` and `HH:MM:SS` as `YYYY-MM-DDTHH:MM:SSZ`.
+fn iso_timestamp(date: &str, separator: char, time: &str) -> Option<String> {
+    let mut parts = date.split(separator);
+    let year = digits(parts.next()?, 4)?;
+    let month = digits(parts.next()?, 2)?;
+    let day = digits(parts.next()?, 2)?;
+    if parts.next().is_some() {
+        return None;
+    }
+    let time = clock(time)?;
+    Some(format!("{year}-{month}-{day}T{time}Z"))
+}
+
+/// `HH:MM:SS`, two digits each.
+fn clock(time: &str) -> Option<&str> {
+    let mut parts = time.split(':');
+    let all_two_digits = (0..3).all(|_| parts.next().and_then(|part| digits(part, 2)).is_some());
+    (all_two_digits && parts.next().is_none()).then_some(time)
+}
+
+/// `text` when it is exactly `count` ASCII digits.
+fn digits(text: &str, count: usize) -> Option<&str> {
+    (text.len() == count && text.bytes().all(|byte| byte.is_ascii_digit())).then_some(text)
+}
+
+/// A non-negative integer written in ASCII digits only.
+fn number(text: &str) -> Option<u64> {
+    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
+        return None;
     }
-    let level = LEVELS.contains(&level).then(|| level.to_owned());
-    let latency_ms = tokens
-        .find_map(|token| token.strip_prefix("latency_ms="))
-        .and_then(|value| value.parse().ok());
-    Record::new(
-        Some(timestamp.to_owned()),
-        level,
-        Some(component.to_owned()),
-        latency_ms,
-    )
+    text.parse().ok()
+}
+
+/// A component name: non-empty letters, digits, `-`, `_`, or `.`.
+fn component_name(text: &str) -> Option<String> {
+    let valid = !text.is_empty()
+        && text
+            .bytes()
+            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
+    valid.then(|| text.to_owned())
+}
+
+/// A level spelling, case-insensitive, as its canonical name; `warning` is `WARN` and `err` is `ERROR`.
+fn canonical_level(text: &str) -> Option<String> {
+    let canonical = match text.to_ascii_uppercase().as_str() {
+        "DEBUG" => "DEBUG",
+        "INFO" => "INFO",
+        "WARN" | "WARNING" => "WARN",
+        "ERROR" | "ERR" => "ERROR",
+        _ => return None,
+    };
+    Some(canonical.to_owned())
 }
 
 #[cfg(test)]
@@ -41,33 +157,45 @@
     use super::parse_line;
 
     #[test]
-    fn a_single_spaced_format_a_line_is_parsed() {
+    fn a_format_a_line_with_runs_of_spaces_is_parsed() {
+        // Arrange
+        let line = "2026-10-28T14:03:40Z  Err  billing:  refund failed  latency_ms=910";
+
+        // Act
+        let row = parse_line(line).to_tsv();
+
+        // Assert
+        assert_eq!(row, "2026-10-28T14:03:40Z\tERROR\tbilling\t910");
+    }
+
+    #[test]
+    fn a_format_b_line_has_no_level() {
         // Arrange
-        let line = "2026-10-28T14:03:11Z INFO auth: login ok user=alice latency_ms=12";
+        let line = "Nov  3 09:15:02 web01 cron[77]: job nightly-report started 120ms";
 
         // Act
         let row = parse_line(line).to_tsv();
 
         // Assert
-        assert_eq!(row, "2026-10-28T14:03:11Z\tINFO\tauth\t12");
+        assert_eq!(row, "2026-11-03T09:15:02Z\t-\tcron\t120");
     }
 
     #[test]
-    fn the_baseline_yields_nothing_for_format_b() {
+    fn a_format_c_line_takes_its_latency_from_took() {
         // Arrange
-        let line = "Oct 28 14:03:12 web01 nginx[2211]: GET /api/orders 200 15ms";
+        let line = "[2026/10/28 14:03:21] warning db: slow query took 1250 ms";
 
         // Act
         let row = parse_line(line).to_tsv();
 
         // Assert
-        assert_eq!(row, "-\t-\t-\t-");
+        assert_eq!(row, "2026-10-28T14:03:21Z\tWARN\tdb\t1250");
     }
 
     #[test]
-    fn the_baseline_yields_nothing_for_format_c() {
+    fn an_unrecognized_line_yields_an_empty_record() {
         // Arrange
-        let line = "[2026/10/28 14:03:13] INFO db: query took 42 ms";
+        let line = "not a log line";
 
         // Act
         let row = parse_line(line).to_tsv();

report hash: 20df423299a58bfe3da11a6a25d494898675724debde323408617c0011e5dbc5
to adopt this experiment after review: minos adopt 7f1a4b38bd38 --run f2ca2a2c45b3 --reviewed 20df423299a58bfe3da11a6a25d494898675724debde323408617c0011e5dbc5
== minos adopt
acknowledged report 20df423299a5 for experiment 7f1a4b38bd38 (recorded)
patch: /private/tmp/msvDaWeth/logparse/minos-7f1a4b38bd38.patch
patch contents:
diff --git a/src/parse.rs b/src/parse.rs
--- a/src/parse.rs
+++ b/src/parse.rs
@@ -1,39 +1,155 @@
 //! The parser: the one file a Minos proposer may change.
 //!
-//! This baseline is imperfect on purpose. It handles format A (`<ISO timestamp> <LEVEL> <component>: <message>`)
-//! only when its tokens are separated by single spaces and its level is already one of the four canonical
-//! spellings, and yields an empty record for every other line. `program.md` states the full spec.
+//! This is the improved parser for the `examples/logparse` sample, kept as a test fixture: the full-file content
+//! the Phase 8 VM test's scripted provider proposes in place of `src/parse.rs`. It is not compiled by the Minos
+//! workspace; it compiles only as `src/parse.rs` of the sample crate, under that crate's lints, and implements
+//! the whole output spec in the sample's `program.md`:
+//!
+//! - format A: `<YYYY-MM-DDTHH:MM:SSZ> <level> <component>: <message>`, latency from `latency_ms=<n>`;
+//! - format B: `<Mon> <D> <HH:MM:SS> <host> <component>[<pid>]: <message>`, year 2026, no level, latency from a
+//!   final `<n>ms` token;
+//! - format C: `[YYYY/MM/DD HH:MM:SS] <level> <component>: <message>`, latency from `took <n> ms`.
+//!
+//! Tokens may be separated by runs of spaces, and a line may carry leading or trailing spaces.
 
 use crate::record::Record;
 
-/// The canonical level names.
-const LEVELS: [&str; 4] = ["DEBUG", "INFO", "WARN", "ERROR"];
+/// The year a format B line carries implicitly.
+const SYSLOG_YEAR: &str = "2026";
+
+/// The three-letter month names format B uses, in calendar order.
+const MONTHS: [&str; 12] = [
+    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
+];
 
 /// Extracts the four fields from one input line.
 #[must_use]
 pub fn parse_line(line: &str) -> Record {
-    let mut tokens = line.split(' ');
-    let (Some(timestamp), Some(level), Some(component)) =
-        (tokens.next(), tokens.next(), tokens.next())
-    else {
-        return Record::default();
+    let tokens: Vec<&str> = line.split_whitespace().collect();
+    format_a(&tokens)
+        .or_else(|| format_c(&tokens))
+        .or_else(|| format_b(&tokens))
+        .unwrap_or_default()
+}
+
+/// `<YYYY-MM-DDTHH:MM:SSZ> <level> <component>: <message>`.
+fn format_a(tokens: &[&str]) -> Option<Record> {
+    let [stamp, level, component, message @ ..] = tokens else {
+        return None;
+    };
+    let (date, time) = stamp.strip_suffix('Z')?.split_once('T')?;
+    let timestamp = iso_timestamp(date, '-', time)?;
+    let component = component_name(component.strip_suffix(':')?)?;
+    let latency_ms = message
+        .iter()
+        .find_map(|token| token.strip_prefix("latency_ms="))
+        .and_then(number);
+    Some(Record::new(
+        Some(timestamp),
+        canonical_level(level),
+        Some(component),
+        latency_ms,
+    ))
+}
+
+/// `[YYYY/MM/DD HH:MM:SS] <level> <component>: <message>`.
+fn format_c(tokens: &[&str]) -> Option<Record> {
+    let [date, time, level, component, message @ ..] = tokens else {
+        return None;
     };
-    let Some(component) = component.strip_suffix(':') else {
-        return Record::default();
+    let date = date.strip_prefix('[')?;
+    let time = time.strip_suffix(']')?;
+    let timestamp = iso_timestamp(date, '/', time)?;
+    let component = component_name(component.strip_suffix(':')?)?;
+    let latency_ms = message.windows(3).find_map(|window| match window {
+        ["took", value, "ms"] => number(value),
+        _ => None,
+    });
+    Some(Record::new(
+        Some(timestamp),
+        canonical_level(level),
+        Some(component),
+        latency_ms,
+    ))
+}
+
+/// `<Mon> <D> <HH:MM:SS> <host> <component>[<pid>]: <message>`.
+fn format_b(tokens: &[&str]) -> Option<Record> {
+    let [month, day, time, _host, tag, message @ ..] = tokens else {
+        return None;
     };
-    if !timestamp.ends_with('Z') || component.is_empty() {
-        return Record::default();
+    let month = (1_u8..)
+        .zip(MONTHS)
+        .find_map(|(number, name)| (name == *month).then_some(number))?;
+    let day: u8 = day.parse().ok().filter(|day| (1..=31).contains(day))?;
+    let time = clock(time)?;
+    let (component, pid) = tag.strip_suffix("]:")?.split_once('[')?;
+    number(pid)?;
+    let component = component_name(component)?;
+    let latency_ms = message
+        .last()
+        .and_then(|token| token.strip_suffix("ms"))
+        .and_then(number);
+    Some(Record::new(
+        Some(format!("{SYSLOG_YEAR}-{month:02}-{day:02}T{time}Z")),
+        None,
+        Some(component),
+        latency_ms,
+    ))
+}
+
+/// `YYYY<sep>MM<sep>DD` and `HH:MM:SS` as `YYYY-MM-DDTHH:MM:SSZ`.
+fn iso_timestamp(date: &str, separator: char, time: &str) -> Option<String> {
+    let mut parts = date.split(separator);
+    let year = digits(parts.next()?, 4)?;
+    let month = digits(parts.next()?, 2)?;
+    let day = digits(parts.next()?, 2)?;
+    if parts.next().is_some() {
+        return None;
+    }
+    let time = clock(time)?;
+    Some(format!("{year}-{month}-{day}T{time}Z"))
+}
+
+/// `HH:MM:SS`, two digits each.
+fn clock(time: &str) -> Option<&str> {
+    let mut parts = time.split(':');
+    let all_two_digits = (0..3).all(|_| parts.next().and_then(|part| digits(part, 2)).is_some());
+    (all_two_digits && parts.next().is_none()).then_some(time)
+}
+
+/// `text` when it is exactly `count` ASCII digits.
+fn digits(text: &str, count: usize) -> Option<&str> {
+    (text.len() == count && text.bytes().all(|byte| byte.is_ascii_digit())).then_some(text)
+}
+
+/// A non-negative integer written in ASCII digits only.
+fn number(text: &str) -> Option<u64> {
+    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
+        return None;
     }
-    let level = LEVELS.contains(&level).then(|| level.to_owned());
-    let latency_ms = tokens
-        .find_map(|token| token.strip_prefix("latency_ms="))
-        .and_then(|value| value.parse().ok());
-    Record::new(
-        Some(timestamp.to_owned()),
-        level,
-        Some(component.to_owned()),
-        latency_ms,
-    )
+    text.parse().ok()
+}
+
+/// A component name: non-empty letters, digits, `-`, `_`, or `.`.
+fn component_name(text: &str) -> Option<String> {
+    let valid = !text.is_empty()
+        && text
+            .bytes()
+            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
+    valid.then(|| text.to_owned())
+}
+
+/// A level spelling, case-insensitive, as its canonical name; `warning` is `WARN` and `err` is `ERROR`.
+fn canonical_level(text: &str) -> Option<String> {
+    let canonical = match text.to_ascii_uppercase().as_str() {
+        "DEBUG" => "DEBUG",
+        "INFO" => "INFO",
+        "WARN" | "WARNING" => "WARN",
+        "ERROR" | "ERR" => "ERROR",
+        _ => return None,
+    };
+    Some(canonical.to_owned())
 }
 
 #[cfg(test)]
@@ -41,33 +157,45 @@
     use super::parse_line;
 
     #[test]
-    fn a_single_spaced_format_a_line_is_parsed() {
+    fn a_format_a_line_with_runs_of_spaces_is_parsed() {
+        // Arrange
+        let line = "2026-10-28T14:03:40Z  Err  billing:  refund failed  latency_ms=910";
+
+        // Act
+        let row = parse_line(line).to_tsv();
+
+        // Assert
+        assert_eq!(row, "2026-10-28T14:03:40Z\tERROR\tbilling\t910");
+    }
+
+    #[test]
+    fn a_format_b_line_has_no_level() {
         // Arrange
-        let line = "2026-10-28T14:03:11Z INFO auth: login ok user=alice latency_ms=12";
+        let line = "Nov  3 09:15:02 web01 cron[77]: job nightly-report started 120ms";
 
         // Act
         let row = parse_line(line).to_tsv();
 
         // Assert
-        assert_eq!(row, "2026-10-28T14:03:11Z\tINFO\tauth\t12");
+        assert_eq!(row, "2026-11-03T09:15:02Z\t-\tcron\t120");
     }
 
     #[test]
-    fn the_baseline_yields_nothing_for_format_b() {
+    fn a_format_c_line_takes_its_latency_from_took() {
         // Arrange
-        let line = "Oct 28 14:03:12 web01 nginx[2211]: GET /api/orders 200 15ms";
+        let line = "[2026/10/28 14:03:21] warning db: slow query took 1250 ms";
 
         // Act
         let row = parse_line(line).to_tsv();
 
         // Assert
-        assert_eq!(row, "-\t-\t-\t-");
+        assert_eq!(row, "2026-10-28T14:03:21Z\tWARN\tdb\t1250");
     }
 
     #[test]
-    fn the_baseline_yields_nothing_for_format_c() {
+    fn an_unrecognized_line_yields_an_empty_record() {
         // Arrange
-        let line = "[2026/10/28 14:03:13] INFO db: query took 42 ms";
+        let line = "not a log line";
 
         // Act
         let row = parse_line(line).to_tsv();
apply it from the project directory with either:
  patch -p1 -i minos-7f1a4b38bd38.patch
  git apply --directory="$(git rev-parse --show-prefix)" minos-7f1a4b38bd38.patch
plain git apply run inside a subdirectory of a repository skips every file and still exits 0; minos never applies a patch
== patch -p1 stdout
patching file 'src/parse.rs'
== patch -p1 stderr
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 205.86s

```
