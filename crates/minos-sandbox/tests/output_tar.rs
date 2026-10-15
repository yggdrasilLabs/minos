//! The hostile output tar reader (SBX-05, D-16): stream checks, end-of-archive checks, fault mapping, and the
//! `Finished` wrapper a backend returns.
//!
//! Every stream here is built header by header in memory with `tar::Header`; nothing shells out to a system `tar`.
//!
//! # Corpus
//!
//! The 20 files under `tests/fixtures/output_tar/` are the small hostile shapes (plus one positive control), each
//! produced by the builder function named in [`CORPUS`] and pinned there by its SHA-256. They were written once by
//! the ignored test `write_corpus`:
//!
//! ```text
//! cargo test -p minos-sandbox --test output_tar -- --ignored write_corpus
//! shasum -a 256 crates/minos-sandbox/tests/fixtures/output_tar/*.tar
//! ```
//!
//! and the hashes were pasted into [`CORPUS`]. The gate never runs the writer: `corpus_is_pinned` checks the bytes
//! on disk and `corpus_matches_builders` ties each file to its builder, so neither a fixture nor a builder can
//! change unnoticed. The large cases (an entry-count bomb, a million-entry stream, content over the total cap) are
//! generated at run time and never committed.
//!
//! # Allocation bound (04-RESEARCH Pitfall 9)
//!
//! "The reader never allocates past its caps" cannot be measured here: counting allocations needs a global
//! allocator, which needs `unsafe` code, which the workspace forbids. The bound is structural, as
//! `minos_sandbox::output` documents: the slice is refused above `MAX_OUTPUT_STREAM_BYTES` before parsing; raw mode
//! never buffers an extension record; and each entry is read through `take(size)` after the per-entry cap, with at
//! most 64 KiB preallocated. The properties below check the outcome: nothing panics, and every accepted output
//! respects the entry-count and content caps.

use std::collections::BTreeSet;
use std::error::Error;
use std::str::FromStr;

use minos_core::id::{BlobId, ImageDigest, TreeId};
use minos_core::job::{JobBudget, JobRequest};
use minos_core::ledger::Isolation;
use minos_core::manifest::{Arch, JudgeVersion};
use minos_core::outcome::Reason;
use minos_sandbox::output::{
    MAX_OUTPUT_CONTENT_BYTES, MAX_OUTPUT_ENTRIES, MAX_OUTPUT_ENTRY_BYTES, MAX_OUTPUT_STREAM_BYTES,
};
use minos_sandbox::{
    Candidate, Capabilities, Cleared, Ended, Finished, GuestOutput, InputDisk, Judge, JudgeData,
    Machine, OutputCap, OutputFault, OutputInvalid, Role, SandboxSpec, Untrusted, preflight,
};
use proptest::prelude::*;
use proptest::sample::Index;
use tar::Header;

type TestResult = Result<(), Box<dyn Error>>;

// ---------------------------------------------------------------------------------------------------------------
// Cleared plans
// ---------------------------------------------------------------------------------------------------------------

/// A baseline job: build 60 s, run 120 s, judge 90 s.
fn job() -> Result<JobRequest, Box<dyn Error>> {
    let budget = JobBudget::new(60_000, 120_000, 90_000)?;
    let judge = JudgeVersion::from_str(&"ab".repeat(32))?;
    Ok(JobRequest::baseline(TreeId::of(b"t"), judge, 0, budget)?)
}

/// An arm64 Hardware host with room for every test plan and no host refusals.
fn host() -> Result<Capabilities, Box<dyn Error>> {
    Ok(Capabilities::new(
        Arch::Arm64,
        Machine::new(8, 16_384)?,
        3_600_000,
        Isolation::Hardware,
        BTreeSet::new(),
    ))
}

/// A spec of role `R` for the baseline job.
fn spec<R: Role>() -> Result<SandboxSpec<R>, Box<dyn Error>> {
    Ok(SandboxSpec::<R>::new(
        &job()?,
        ImageDigest::of(b"root image"),
        InputDisk::new(BlobId::of(b"input disk")),
        Machine::new(2, 512)?,
        Arch::Arm64,
    )?)
}

/// A cleared Judge plan carrying judge data.
fn judge_cleared() -> Result<Cleared<Judge>, Box<dyn Error>> {
    let plan = spec::<Judge>()?
        .secrets(JudgeData::new(BlobId::of(b"judge data")))
        .seal();
    preflight(&host()?, plan).map_err(|refusals| format!("{refusals:?}").into())
}

/// A cleared Candidate plan.
fn candidate_cleared() -> Result<Cleared<Candidate>, Box<dyn Error>> {
    preflight(&host()?, spec::<Candidate>()?.seal())
        .map_err(|refusals| format!("{refusals:?}").into())
}

/// What reading a judge stream yields.
type ReadResult = Result<Untrusted<GuestOutput<Judge>>, OutputFault>;

/// Reads `stream` as the output of an exited Judge launch.
fn read(stream: Vec<u8>) -> Result<ReadResult, Box<dyn Error>> {
    Ok(Finished::new(&judge_cleared()?, Ended::Exited, stream).into_output())
}

/// The fault `stream` is refused with, or an error when it was accepted.
fn fault(stream: Vec<u8>) -> Result<OutputFault, Box<dyn Error>> {
    match read(stream)? {
        Ok(_) => Err("the stream was accepted".into()),
        Err(fault) => Ok(fault),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Block builder
// ---------------------------------------------------------------------------------------------------------------

/// Bytes in one tar block.
const BLOCK: usize = 512;

/// Which header format a block uses.
#[derive(Clone, Copy, Debug)]
enum Magic {
    /// POSIX ustar: magic `ustar\0`, version `00`.
    Ustar,
    /// GNU: magic `ustar `, version ` \0`.
    Gnu,
    /// Pre-POSIX v7: no magic.
    Old,
}

/// A header of `magic` named by the raw bytes `name` (copied into the 100-byte name field, so hostile names that
/// `Header::set_path` refuses still land), with typeflag `typeflag`, a size field of `size`, mode 0644, owner 0,
/// mtime 0, and a valid checksum.
fn header(magic: Magic, name: &[u8], typeflag: u8, size: u64) -> Header {
    let mut header = match magic {
        Magic::Ustar => Header::new_ustar(),
        Magic::Gnu => Header::new_gnu(),
        Magic::Old => Header::new_old(),
    };
    for (slot, byte) in header.as_old_mut().name.iter_mut().zip(name) {
        *slot = *byte;
    }
    header.as_old_mut().linkflag = [typeflag];
    header.set_size(size);
    header.set_mode(0o644);
    header.set_uid(0);
    header.set_gid(0);
    header.set_mtime(0);
    header.set_cksum();
    header
}

/// One header block followed by `data`, zero-padded to a whole number of blocks. `data` need not match the
/// header's size field.
fn entry(header: &Header, data: &[u8]) -> Vec<u8> {
    let mut out = header.as_bytes().to_vec();
    out.extend_from_slice(data);
    let padding = (BLOCK - data.len() % BLOCK) % BLOCK;
    out.resize(out.len() + padding, 0);
    out
}

/// A ustar regular-file entry named `name` holding `data`.
fn regular(name: &str, data: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    let size = u64::try_from(data.len())?;
    Ok(entry(
        &header(Magic::Ustar, name.as_bytes(), b'0', size),
        data,
    ))
}

/// The two zero blocks that end an archive.
fn end_blocks() -> Vec<u8> {
    vec![0; 2 * BLOCK]
}

/// The scorecard a valid judge writes.
const SCORECARD: &[u8] =
    br#"{"v":1,"metrics":[{"name":"throughput_mb_s","value":12.5,"held_out":false}]}"#;
/// The judge's log.
const JUDGE_LOG: &[u8] = b"judge ok\n";
/// A judge artifact.
const RESULT: &[u8] = b"{\"ok\":true}\n";

/// A valid judge output: the scorecard, the judge log, one artifact, and two end blocks.
fn valid_judge_output() -> Result<Vec<u8>, Box<dyn Error>> {
    let mut out = regular(".minos/scorecard.json", SCORECARD)?;
    out.extend(regular(".minos/judge.log", JUDGE_LOG)?);
    out.extend(regular("out/result.json", RESULT)?);
    out.extend(end_blocks());
    Ok(out)
}

// ---------------------------------------------------------------------------------------------------------------
// Behavior
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn valid_stream_reads_with_counts() -> TestResult {
    let output = read(valid_judge_output()?)??;
    assert_eq!(output.entry_count(), 3);
    let total = SCORECARD.len() + JUDGE_LOG.len() + RESULT.len();
    assert_eq!(output.content_bytes(), u64::try_from(total)?);
    Ok(())
}

#[test]
fn stream_over_its_cap_is_refused_before_parsing() -> TestResult {
    // One byte over the cap and not block-aligned: the length check comes first.
    let len = usize::try_from(MAX_OUTPUT_STREAM_BYTES)?.saturating_add(1);
    assert_eq!(
        fault(vec![0; len])?,
        OutputFault::TooLarge {
            cap: OutputCap::Stream
        }
    );
    Ok(())
}

#[test]
fn unaligned_stream_is_refused() -> TestResult {
    assert_eq!(
        fault(vec![0; BLOCK + 1])?,
        OutputFault::Invalid {
            why: OutputInvalid::NotBlockAligned
        }
    );
    Ok(())
}

#[test]
fn empty_archive_reads_as_no_entries() -> TestResult {
    let output = read(end_blocks())??;
    assert_eq!(output.entry_count(), 0);
    assert_eq!(output.content_bytes(), 0);
    Ok(())
}

#[test]
fn empty_slice_has_no_end_marker() -> TestResult {
    assert_eq!(
        fault(Vec::new())?,
        OutputFault::Invalid {
            why: OutputInvalid::NoEndMarker
        }
    );
    Ok(())
}

#[test]
fn faults_map_to_candidate_reasons() {
    for cap in [
        OutputCap::Stream,
        OutputCap::Entries,
        OutputCap::Entry,
        OutputCap::Total,
        OutputCap::DiffCount,
        OutputCap::DiffBytes,
    ] {
        assert_eq!(
            OutputFault::TooLarge { cap }.reason(),
            Reason::OutputTooLarge
        );
    }
    for why in [
        OutputInvalid::NotBlockAligned,
        OutputInvalid::NotUstar,
        OutputInvalid::EntryType,
        OutputInvalid::Path,
        OutputInvalid::Truncated,
        OutputInvalid::Duplicate,
        OutputInvalid::NoEndMarker,
        OutputInvalid::TrailingData,
        OutputInvalid::Malformed,
        OutputInvalid::DiffNotText,
    ] {
        assert_eq!(OutputFault::Invalid { why }.reason(), Reason::OutputInvalid);
    }
}

#[test]
fn finished_reports_its_end_and_key() -> TestResult {
    let cleared = judge_cleared()?;
    let finished = Finished::new(&cleared, Ended::TimedOut, end_blocks());
    assert_eq!(finished.ended(), Ended::TimedOut);
    assert_eq!(finished.key(), cleared.plan().key());

    let candidate = candidate_cleared()?;
    let finished = Finished::new(&candidate, Ended::Exited, valid_judge_output()?);
    assert_eq!(finished.key(), candidate.plan().key());
    assert_eq!(finished.into_output()?.entry_count(), 3);
    Ok(())
}

#[test]
fn gnu_and_v7_headers_are_not_ustar() -> TestResult {
    for magic in [Magic::Gnu, Magic::Old] {
        let mut stream = entry(&header(magic, b"out/a", b'0', 4), b"data");
        stream.extend(end_blocks());
        assert_eq!(
            fault(stream)?,
            OutputFault::Invalid {
                why: OutputInvalid::NotUstar
            },
            "{magic:?}"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Corpus builders
// ---------------------------------------------------------------------------------------------------------------

/// The bytes of one fixture.
type Built = Result<Vec<u8>, Box<dyn Error>>;

/// A builder of one fixture.
type Builder = fn() -> Built;

/// `stream` followed by the two end blocks.
fn with_end(mut stream: Vec<u8>) -> Vec<u8> {
    stream.extend(end_blocks());
    stream
}

/// A one-entry archive: a ustar header named by the raw bytes `name`, with typeflag `typeflag`, over `data`, then
/// the end blocks.
fn single(name: &[u8], typeflag: u8, data: &[u8]) -> Built {
    let size = u64::try_from(data.len())?;
    Ok(with_end(entry(
        &header(Magic::Ustar, name, typeflag, size),
        data,
    )))
}

/// A one-entry archive of a zero-size ustar link entry of typeflag `typeflag` pointing at `/etc/passwd`.
fn link(typeflag: u8) -> Vec<u8> {
    let mut link = header(Magic::Ustar, b"out/link", typeflag, 0);
    for (slot, byte) in link.as_old_mut().linkname.iter_mut().zip(b"/etc/passwd") {
        *slot = *byte;
    }
    link.set_cksum();
    with_end(entry(&link, &[]))
}

/// One PAX extended-header record, `"<len> <key>=<value>\n"`, where `<len>` counts the whole record.
fn pax_record(key_value: &str) -> String {
    let body = format!(" {key_value}\n");
    let mut len = body.len() + 1;
    while format!("{len}{body}").len() != len {
        len += 1;
    }
    format!("{len}{body}")
}

/// A ustar PAX header (typeflag `b'x'`) carrying `record`.
fn pax_entry(record: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let size = u64::try_from(record.len())?;
    Ok(entry(
        &header(Magic::Ustar, b"PaxHeaders/a", b'x', size),
        record.as_bytes(),
    ))
}

fn build_path_traversal() -> Built {
    single(b"../escape", b'0', b"data")
}

fn build_absolute_path() -> Built {
    single(b"/etc/passwd", b'0', b"data")
}

fn build_non_grammar_name() -> Built {
    single(b"a b.txt", b'0', b"data")
}

fn build_symlink() -> Vec<u8> {
    link(b'2')
}

fn build_hardlink() -> Vec<u8> {
    link(b'1')
}

fn build_char_device() -> Built {
    single(b"out/tty", b'3', &[])
}

fn build_fifo() -> Built {
    single(b"out/fifo", b'6', &[])
}

fn build_directory() -> Built {
    single(b"out/dir", b'5', &[])
}

fn build_legacy_regular() -> Built {
    single(b"out/a", b'\0', b"data")
}

/// A GNU long-name header (GNU magic, typeflag `b'L'`) naming `out/a`, then the regular entry it would rename.
fn build_gnu_long_name() -> Built {
    let long = b"out/a\0";
    let size = u64::try_from(long.len())?;
    let mut stream = entry(&header(Magic::Gnu, b"././@LongLink", b'L', size), long);
    stream.extend(regular("out/a", b"data")?);
    Ok(with_end(stream))
}

/// A PAX header whose `path` record disagrees with the next entry's ustar name.
fn build_pax_path_override() -> Built {
    let mut stream = pax_entry(&pax_record("path=out/evil"))?;
    stream.extend(regular("out/a", b"data")?);
    Ok(with_end(stream))
}

/// A PAX header setting `size=0`, then a regular entry whose data holds a forged header: a parser that honored
/// the PAX size would read the forged header as the next entry.
fn build_pax_size_desync() -> Built {
    let mut stream = pax_entry(&pax_record("size=0"))?;
    let mut forged = entry(&header(Magic::Ustar, b"out/forged", b'0', 4), b"evil");
    forged.resize(2 * BLOCK, 0);
    stream.extend(regular("out/a", &forged)?);
    Ok(with_end(stream))
}

fn build_v7_header() -> Vec<u8> {
    with_end(entry(&header(Magic::Old, b"out/a", b'0', 4), b"data"))
}

/// A ustar regular entry claiming 8,000,000,000 bytes, with no data and no end blocks.
fn build_size_8gb() -> Vec<u8> {
    header(Magic::Ustar, b"out/big", b'0', 8_000_000_000)
        .as_bytes()
        .to_vec()
}

/// A ustar regular entry of size 1000 with only 512 data bytes and no end blocks.
fn build_truncated_data() -> Vec<u8> {
    entry(&header(Magic::Ustar, b"out/a", b'0', 1000), &[b'x'; BLOCK])
}

/// One valid entry, then the end of the stream.
fn build_no_end_marker() -> Built {
    regular("out/a", b"data")
}

/// One valid entry, one zero block, one non-zero block, one zero block.
fn build_trailing_data() -> Built {
    let mut stream = regular("out/a", b"data")?;
    stream.extend([0; BLOCK]);
    stream.extend([b'A'; BLOCK]);
    stream.extend([0; BLOCK]);
    Ok(stream)
}

fn build_duplicate_name() -> Built {
    let mut stream = regular("out/a", b"data")?;
    stream.extend(regular("out/a", b"more")?);
    Ok(with_end(stream))
}

/// One ustar entry whose checksum field holds a valid octal value one more than the true sum.
fn build_bad_checksum() -> Built {
    let mut bad = header(Magic::Ustar, b"out/a", b'0', 4);
    let wrong = format!("{:06o}\0 ", bad.cksum()?.saturating_add(1));
    for (slot, byte) in bad.as_old_mut().cksum.iter_mut().zip(wrong.as_bytes()) {
        *slot = *byte;
    }
    Ok(with_end(entry(&bad, b"data")))
}

/// The committed corpus: file name, builder, and the SHA-256 of the committed bytes.
const CORPUS: [(&str, Builder, &str); 20] = [
    (
        "path_traversal.tar",
        build_path_traversal,
        "2b2ad23b90f110add8555148b3b4830c6e736645513e09e98df53b7b2bfed5ad",
    ),
    (
        "absolute_path.tar",
        build_absolute_path,
        "ce1bb127ec908e7e30fcb051628b3dbfaf14bd10e8cf0012fae27383c99dd20e",
    ),
    (
        "non_grammar_name.tar",
        build_non_grammar_name,
        "a8ea998d998974ca562010e18aefb45b5edc3f60c21493dbbdb526d3d60333ad",
    ),
    (
        "symlink.tar",
        || Ok(build_symlink()),
        "271b43475022946186dda0149d67d292ea37a6676136013b156c3f0b294a84cf",
    ),
    (
        "hardlink.tar",
        || Ok(build_hardlink()),
        "b2d8d27aedf42c43ac93f192b17a7d0e9f3f8519207a1f1f97a62b10800a09da",
    ),
    (
        "char_device.tar",
        build_char_device,
        "aaae1446099bb123e9915a1dcd85f7304be05c6963be2d53390bc70fa4d2a66f",
    ),
    (
        "fifo.tar",
        build_fifo,
        "69e18788da9e819edbd5d8f653f8f3bc06ffa6a5449137720c9574042adc6a4a",
    ),
    (
        "directory.tar",
        build_directory,
        "663345023a2ac83544ba25190cd9be9dc7ed1991607f694cf354525184c71ee6",
    ),
    (
        "legacy_regular.tar",
        build_legacy_regular,
        "9725f0e27110283dfec557b4a75247e73465f2187821274bcb01eb6d3fa48725",
    ),
    (
        "gnu_long_name.tar",
        build_gnu_long_name,
        "df2733b4e149a4079dcfe72f40b698829998123baa3fa92923a5306279466d70",
    ),
    (
        "pax_path_override.tar",
        build_pax_path_override,
        "9ac5a02201e3fc1a58df19e563111cd7b933a57b1ac1a34abe19d8b58e12d9b4",
    ),
    (
        "pax_size_desync.tar",
        build_pax_size_desync,
        "9f6936c5d559c0501ec5797dd87485b4cb6cef2a445db7cbeba021c7c79019bc",
    ),
    (
        "v7_header.tar",
        || Ok(build_v7_header()),
        "7024e75af94581249ee1de502cba4d9ec27f18a02bc7ee06fc6e090e801f8622",
    ),
    (
        "size_8gb.tar",
        || Ok(build_size_8gb()),
        "ff49ee97928b1266bfae14d144376adfb9eec791471fcf0acb77c8c7c5eac64a",
    ),
    (
        "truncated_data.tar",
        || Ok(build_truncated_data()),
        "e786c0cd92292965d810d430ccbf8823d1a3489b0cbd9142c19502582ec6507e",
    ),
    (
        "no_end_marker.tar",
        build_no_end_marker,
        "5c52fbabcc462ef5b55142a476e5f28f4f5dfd9ea5f86f0e1543f5b557efa6d7",
    ),
    (
        "trailing_data.tar",
        build_trailing_data,
        "833df30eddd94e95717b55933eec4696c07899b3236a2835a0225a9273a05144",
    ),
    (
        "duplicate_name.tar",
        build_duplicate_name,
        "2617596d0d37ca15e8189f98340ae6992dd47d23fb7fb99cde8ba29ee29d37ad",
    ),
    (
        "bad_checksum.tar",
        build_bad_checksum,
        "b75028a3716ff05e2efc6aa590686945b19933ff2ae9798b4b5ba9d1f1cac17d",
    ),
    (
        "valid_judge_output.tar",
        valid_judge_output,
        "37f1f10657a1d069fa59ff163d374fa146e8b3f0c8e6cc3e2bb2d6bf3e683a0e",
    ),
];

/// The directory of the committed corpus.
const CORPUS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/output_tar");

/// The committed bytes of fixture `name`.
fn fixture(name: &str) -> Built {
    Ok(std::fs::read(format!("{CORPUS_DIR}/{name}"))?)
}

/// Writes the corpus. Run by hand only (see the module docs); the gate never writes a fixture.
#[test]
#[ignore = "writes the committed corpus; run by hand, then re-pin"]
fn write_corpus() -> TestResult {
    std::fs::create_dir_all(CORPUS_DIR)?;
    for (name, build, _) in CORPUS {
        std::fs::write(format!("{CORPUS_DIR}/{name}"), build()?)?;
    }
    Ok(())
}

#[test]
fn corpus_is_pinned() -> TestResult {
    for (name, _, pin) in CORPUS {
        assert_eq!(BlobId::of(&fixture(name)?).to_hex(), pin, "{name}");
    }
    Ok(())
}

#[test]
fn corpus_matches_builders() -> TestResult {
    for (name, build, _) in CORPUS {
        assert!(fixture(name)? == build()?, "{name}");
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Corpus replay: each fixture's exact fault
// ---------------------------------------------------------------------------------------------------------------

/// A test that the committed fixture `$file` is refused with exactly `$fault`.
macro_rules! rejects {
    ($test:ident, $file:literal, $fault:expr) => {
        #[test]
        fn $test() -> TestResult {
            assert_eq!(fault(fixture($file)?)?, $fault);
            Ok(())
        }
    };
}

const fn invalid(why: OutputInvalid) -> OutputFault {
    OutputFault::Invalid { why }
}

rejects!(
    rejects_path_traversal,
    "path_traversal.tar",
    invalid(OutputInvalid::Path)
);
rejects!(
    rejects_absolute_path,
    "absolute_path.tar",
    invalid(OutputInvalid::Path)
);
rejects!(
    rejects_non_grammar_name,
    "non_grammar_name.tar",
    invalid(OutputInvalid::Path)
);
rejects!(
    rejects_symlink,
    "symlink.tar",
    invalid(OutputInvalid::EntryType)
);
rejects!(
    rejects_hardlink,
    "hardlink.tar",
    invalid(OutputInvalid::EntryType)
);
rejects!(
    rejects_char_device,
    "char_device.tar",
    invalid(OutputInvalid::EntryType)
);
rejects!(rejects_fifo, "fifo.tar", invalid(OutputInvalid::EntryType));
rejects!(
    rejects_directory,
    "directory.tar",
    invalid(OutputInvalid::EntryType)
);
rejects!(
    rejects_legacy_regular,
    "legacy_regular.tar",
    invalid(OutputInvalid::EntryType)
);
rejects!(
    rejects_gnu_long_name,
    "gnu_long_name.tar",
    invalid(OutputInvalid::NotUstar)
);
rejects!(
    rejects_pax_path_override,
    "pax_path_override.tar",
    invalid(OutputInvalid::EntryType)
);
rejects!(
    rejects_pax_size_desync,
    "pax_size_desync.tar",
    invalid(OutputInvalid::EntryType)
);
rejects!(
    rejects_v7_header,
    "v7_header.tar",
    invalid(OutputInvalid::NotUstar)
);
rejects!(
    rejects_size_8gb,
    "size_8gb.tar",
    OutputFault::TooLarge {
        cap: OutputCap::Entry
    }
);
rejects!(
    rejects_truncated_data,
    "truncated_data.tar",
    invalid(OutputInvalid::Truncated)
);
rejects!(
    rejects_no_end_marker,
    "no_end_marker.tar",
    invalid(OutputInvalid::NoEndMarker)
);
rejects!(
    rejects_trailing_data,
    "trailing_data.tar",
    invalid(OutputInvalid::TrailingData)
);
rejects!(
    rejects_duplicate_name,
    "duplicate_name.tar",
    invalid(OutputInvalid::Duplicate)
);
rejects!(
    rejects_bad_checksum,
    "bad_checksum.tar",
    invalid(OutputInvalid::Malformed)
);

#[test]
fn accepts_valid_judge_output() -> TestResult {
    let output = read(fixture("valid_judge_output.tar")?)??;
    assert_eq!(output.entry_count(), 3);
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Generated bombs (never committed)
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn entry_count_bomb() -> TestResult {
    let mut stream = Vec::new();
    for index in 0..=MAX_OUTPUT_ENTRIES {
        stream.extend(regular(&format!("out/{index}"), &[])?);
    }
    assert_eq!(
        fault(with_end(stream))?,
        OutputFault::TooLarge {
            cap: OutputCap::Entries
        }
    );
    Ok(())
}

/// D-16's million-entry stream. Every stream of a million entries is at least 512,000,000 bytes, which the const
/// assertion proves is over `MAX_OUTPUT_STREAM_BYTES`, so the length check refuses all of them before any parse.
/// The test therefore builds only the first `MAX_OUTPUT_STREAM_BYTES / 512 + 1` blocks of such a stream (each the
/// same valid zero-size ustar header), the shortest prefix the same check refuses, and never allocates 512 MB.
#[test]
fn million_entry_stream() -> TestResult {
    const { assert!(1_000_000 * 512 > MAX_OUTPUT_STREAM_BYTES) };
    let blocks = usize::try_from(MAX_OUTPUT_STREAM_BYTES / 512 + 1)?;
    assert_eq!(blocks, 34_821);
    let stream = header(Magic::Ustar, b"out/a", b'0', 0)
        .as_bytes()
        .repeat(blocks);
    assert_eq!(stream.len(), 17_828_352);
    assert_eq!(
        fault(stream)?,
        OutputFault::TooLarge {
            cap: OutputCap::Stream
        }
    );
    Ok(())
}

#[test]
fn total_over_cap() -> TestResult {
    let big = vec![b'z'; usize::try_from(MAX_OUTPUT_ENTRY_BYTES)?];
    let mut stream = regular("out/a", &big)?;
    stream.extend(regular("out/b", &big)?);
    stream.extend(regular("out/c", &[b'z'; BLOCK])?);
    let stream = with_end(stream);
    assert!(u64::try_from(stream.len())? <= MAX_OUTPUT_STREAM_BYTES);
    assert_eq!(
        fault(stream)?,
        OutputFault::TooLarge {
            cap: OutputCap::Total
        }
    );
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Never-panic properties
// ---------------------------------------------------------------------------------------------------------------

/// One generated header and its data.
#[derive(Clone, Debug)]
struct EntrySpec {
    magic: Magic,
    name: Vec<u8>,
    typeflag: u8,
    size: u64,
    data_len: usize,
    fill: u8,
}

/// Headers of every format, with names that are either valid (and may repeat) or drawn from a small alphabet that
/// includes separators, a backslash, and a non-ASCII byte, typeflags weighted toward the interesting ones, and sizes that are small, just over a cap, or
/// huge. When `honest` is drawn and the size is small, the data length matches the size, so accepted outputs
/// occur; otherwise the data length is independent of the size.
fn entry_spec() -> impl Strategy<Value = EntrySpec> {
    (
        prop_oneof![
            3 => Just(Magic::Ustar),
            1 => Just(Magic::Gnu),
            1 => Just(Magic::Old),
        ],
        prop_oneof![
            1 => prop::sample::select(vec![b"out/a".to_vec(), b"out/b".to_vec(), b"diffs/0.diff".to_vec()]),
            2 => proptest::collection::vec(
                prop::sample::select(vec![b'a', b'b', b'0', b'/', b'.', b'\\', b'-', 0x80]),
                0..12,
            ),
        ],
        prop_oneof![
            4 => prop::sample::select(vec![b'0', b'\0', b'1', b'2', b'5', b'x', b'L', b'S']),
            1 => any::<u8>(),
        ],
        prop_oneof![
            4 => 0..=4096_u64,
            1 => Just(MAX_OUTPUT_ENTRY_BYTES + 1),
            1 => Just(8_000_000_000_u64),
            1 => any::<u64>(),
        ],
        0..=4096_usize,
        any::<u8>(),
        any::<bool>(),
    )
        .prop_map(|(magic, name, typeflag, size, data_len, fill, honest)| {
            let data_len = if honest {
                usize::try_from(size)
                    .ok()
                    .filter(|len| *len <= 4096)
                    .unwrap_or(data_len)
            } else {
                data_len
            };
            EntrySpec {
                magic,
                name,
                typeflag,
                size,
                data_len,
                fill,
            }
        })
}

/// How a generated stream ends.
#[derive(Clone, Copy, Debug)]
enum Tail {
    /// Two zero blocks.
    End,
    /// Nothing.
    Bare,
    /// One zero block.
    OneZero,
    /// Two zero blocks, then a non-zero block.
    Trailing,
}

/// Checks the outcome invariants for `stream`: nothing panicked (the call returned); an accepted output respects
/// the entry-count and content caps; a refusal maps to a candidate output reason.
fn check(stream: Vec<u8>) -> Result<(), TestCaseError> {
    let fail = |error: Box<dyn Error>| TestCaseError::fail(error.to_string());
    match read(stream).map_err(fail)? {
        Ok(output) => {
            prop_assert!(output.entry_count() <= MAX_OUTPUT_ENTRIES);
            prop_assert!(output.content_bytes() <= MAX_OUTPUT_CONTENT_BYTES);
        }
        Err(refused) => prop_assert!(matches!(
            refused.reason(),
            Reason::OutputInvalid | Reason::OutputTooLarge
        )),
    }
    Ok(())
}

proptest! {
    #[test]
    fn structured_tars_never_panic(
        specs in proptest::collection::vec(entry_spec(), 0..=8),
        tail in prop_oneof![Just(Tail::End), Just(Tail::Bare), Just(Tail::OneZero), Just(Tail::Trailing)],
        cut in proptest::option::of(any::<Index>()),
    ) {
        let mut stream = Vec::new();
        for spec in &specs {
            let data = vec![spec.fill; spec.data_len];
            stream.extend(entry(&header(spec.magic, &spec.name, spec.typeflag, spec.size), &data));
        }
        match tail {
            Tail::End => stream.extend(end_blocks()),
            Tail::Bare => {}
            Tail::OneZero => stream.extend([0; BLOCK]),
            Tail::Trailing => {
                stream.extend(end_blocks());
                stream.extend([b'T'; BLOCK]);
            }
        }
        if let Some(cut) = cut {
            let keep = cut.index(stream.len().saturating_add(1));
            stream.truncate(keep);
        }
        check(stream)?;
    }

    #[test]
    fn raw_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..=8192)) {
        let aligned = bytes.len() / BLOCK * BLOCK;
        let mut whole = bytes.clone();
        whole.truncate(aligned);
        check(bytes)?;
        check(whole)?;
    }
}
