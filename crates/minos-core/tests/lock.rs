//! `minos.lock`: the golden wire form, its parse refusals, and the check and replace operations (CLI-02;
//! 05-CONTEXT D-10, D-11).
//!
//! The golden text is an insta snapshot under `tests/snapshots/`. `.config/insta.yaml` sets `update: no`, so the
//! plain gate never writes or accepts it, and because insta trims trailing whitespace and normalizes line endings
//! when it compares, `lock_golden_v1` also pins the exact bytes with one SHA-256 literal.

use std::error::Error;
use std::str::FromStr;

use minos_core::id::{BlobId, ImageDigest};
use minos_core::image::ImageRef;
use minos_core::ledger::BackendKind;
use minos_core::lock::{
    BuilderId, BuilderText, LOCK_VERSION, Lock, LockError, LockMismatch, LockedImage,
    MAX_BUILDER_TEXT_BYTES, MAX_LOCK_BYTES,
};
use minos_core::manifest::Arch;
use sha2::{Digest, Sha256};

type TestResult = Result<(), Box<dyn Error>>;

const ALPINE: &str = "docker.io/library/alpine@sha256:1111111111111111111111111111111111111111111111111111111111111111";
const RUST: &str = "docker.io/library/rust@sha256:2222222222222222222222222222222222222222222222222222222222222222";

fn sha256_hex(bytes: &[u8]) -> String {
    base16ct::lower::encode_string(&Sha256::digest(bytes))
}

fn builder(name: &str, version: &str) -> Result<BuilderId, LockError> {
    Ok(BuilderId::new(
        BuilderText::try_from(name.to_owned())?,
        BuilderText::try_from(version.to_owned())?,
    ))
}

fn image(arch: Arch, dockerfile: &[u8], guest: &[u8]) -> Result<LockedImage, Box<dyn Error>> {
    // Out of order on purpose: the constructor sorts.
    let base_images = vec![ImageRef::from_str(RUST)?, ImageRef::from_str(ALPINE)?];
    Ok(LockedImage::new(
        BackendKind::Krun,
        arch,
        ImageDigest::of(b"root image"),
        BlobId::of(dockerfile),
        base_images,
        BlobId::of(guest),
        builder("docker-buildx", "v0.35.0-desktop.2")?,
    ))
}

fn golden_lock() -> Result<Lock, Box<dyn Error>> {
    Ok(Lock::empty().with_image(image(Arch::Arm64, b"FROM alpine\n", b"minos-guest")?))
}

/// The golden text with one edit applied, for refusal tests.
fn edited(from: &str, to: &str) -> Result<String, Box<dyn Error>> {
    let text = golden_lock()?.to_toml()?;
    assert!(text.contains(from), "the golden text has no {from:?}");
    Ok(text.replacen(from, to, 1))
}

#[test]
fn lock_golden_v1() -> TestResult {
    let text = golden_lock()?.to_toml()?;
    insta::assert_snapshot!("minos_lock_v1", text);
    // The pin was computed outside the code under test, from the accepted snapshot body (the text after the
    // file's second `---` line):
    //
    //   awk 'f;/^---$/{c++; if(c==2) f=1}' crates/minos-core/tests/snapshots/lock__minos_lock_v1.snap \
    //     | shasum -a 256
    //
    // awk prints each body line with one newline, and the serialized lock ends with exactly one newline.
    assert_eq!(
        sha256_hex(text.as_bytes()),
        "a495cdbae3f6eb7bc57a26eba8b08c0c8eda5c3245904267ff30c263d343dfdb"
    );
    Ok(())
}

#[test]
fn lock_golden_is_v1_with_one_image() -> TestResult {
    let text = golden_lock()?.to_toml()?;
    assert_eq!(LOCK_VERSION, 1);
    assert!(text.starts_with("v = 1\n"));
    assert_eq!(text.matches("[[image]]").count(), 1);
    assert!(text.ends_with('\n') && !text.ends_with("\n\n"));
    Ok(())
}

#[test]
fn lock_round_trips() -> TestResult {
    let lock = golden_lock()?.with_image(image(Arch::X86_64, b"FROM x\n", b"guest x86")?);
    let text = lock.to_toml()?;
    assert_eq!(Lock::parse_toml(&text)?, lock);
    assert_eq!(text.matches("[[image]]").count(), 2);
    let empty = Lock::empty();
    assert_eq!(Lock::parse_toml(&empty.to_toml()?)?, empty);
    Ok(())
}

#[test]
fn lock_sorts_base_images() -> TestResult {
    let lock = golden_lock()?;
    let locked = lock
        .image(BackendKind::Krun, Arch::Arm64)
        .ok_or("no image")?;
    let names: Vec<String> = locked
        .base_images()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(names, vec![ALPINE.to_owned(), RUST.to_owned()]);
    // A repeated base image is kept once.
    let repeated = LockedImage::new(
        BackendKind::Krun,
        Arch::Arm64,
        ImageDigest::of(b"r"),
        BlobId::of(b"d"),
        vec![ImageRef::from_str(ALPINE)?, ImageRef::from_str(ALPINE)?],
        BlobId::of(b"g"),
        builder("b", "1")?,
    );
    assert_eq!(repeated.base_images().len(), 1);
    Ok(())
}

#[test]
fn lock_refuses_unknown_keys() -> TestResult {
    let top = format!("extra = 1\n{}", golden_lock()?.to_toml()?);
    assert!(matches!(Lock::parse_toml(&top), Err(LockError::Toml(_))));
    let in_image = edited("backend = \"krun\"", "backend = \"krun\"\nextra = 1")?;
    assert!(matches!(
        Lock::parse_toml(&in_image),
        Err(LockError::Toml(_))
    ));
    let in_builder = edited(
        "name = \"docker-buildx\"",
        "name = \"docker-buildx\"\nengine = \"x\"",
    )?;
    assert!(matches!(
        Lock::parse_toml(&in_builder),
        Err(LockError::Toml(_))
    ));
    Ok(())
}

#[test]
fn lock_refuses_missing_keys() -> TestResult {
    let no_guest = edited("guest_sha256 = ", "unused_guest = ")?;
    assert!(matches!(
        Lock::parse_toml(&no_guest),
        Err(LockError::Toml(_))
    ));
    let no_version = edited("v = 1\n", "")?;
    assert!(matches!(
        Lock::parse_toml(&no_version),
        Err(LockError::Toml(_))
    ));
    Ok(())
}

#[test]
fn lock_refuses_version() -> TestResult {
    let text = edited("v = 1\n", "v = 2\n")?;
    assert!(matches!(Lock::parse_toml(&text), Err(LockError::Version)));
    Ok(())
}

#[test]
fn lock_refuses_duplicate_image() -> TestResult {
    let text = golden_lock()?.to_toml()?;
    let start = text.find("[[image]]").ok_or("no image table")?;
    let doubled = format!("{text}\n{}", text.get(start..).ok_or("bad offset")?);
    assert!(matches!(
        Lock::parse_toml(&doubled),
        Err(LockError::DuplicateImage {
            backend: BackendKind::Krun,
            arch: Arch::Arm64
        })
    ));
    Ok(())
}

#[test]
fn lock_refuses_unsorted_base_images() -> TestResult {
    let sorted = format!("base_images = [\"{ALPINE}\", \"{RUST}\"]");
    let unsorted = edited(
        &sorted,
        &format!("base_images = [\"{RUST}\", \"{ALPINE}\"]"),
    )?;
    assert!(matches!(
        Lock::parse_toml(&unsorted),
        Err(LockError::BaseImagesNotCanonical)
    ));
    let repeated = edited(
        &sorted,
        &format!("base_images = [\"{ALPINE}\", \"{ALPINE}\"]"),
    )?;
    assert!(matches!(
        Lock::parse_toml(&repeated),
        Err(LockError::BaseImagesNotCanonical)
    ));
    // A tag-pinned base image is not an ImageRef at all.
    let tagged = edited(&sorted, "base_images = [\"alpine:3.20\"]")?;
    assert!(matches!(Lock::parse_toml(&tagged), Err(LockError::Toml(_))));
    Ok(())
}

#[test]
fn lock_refuses_bad_builder_text() -> TestResult {
    assert!(matches!(
        BuilderText::try_from(String::new()),
        Err(LockError::BuilderText)
    ));
    assert!(matches!(
        BuilderText::try_from("v".repeat(MAX_BUILDER_TEXT_BYTES + 1)),
        Err(LockError::BuilderText)
    ));
    assert!(BuilderText::try_from("v".repeat(MAX_BUILDER_TEXT_BYTES)).is_ok());
    for bad in ["v 1", "v\t1", "v\u{7}1", "v\u{7f}1", "vé"] {
        assert!(
            matches!(
                BuilderText::try_from(bad.to_owned()),
                Err(LockError::BuilderText)
            ),
            "{bad:?}"
        );
    }
    let empty_name = edited("name = \"docker-buildx\"", "name = \"\"")?;
    assert!(matches!(
        Lock::parse_toml(&empty_name),
        Err(LockError::BuilderText)
    ));
    let long = "x".repeat(MAX_BUILDER_TEXT_BYTES + 1);
    let long_version = edited(
        "version = \"v0.35.0-desktop.2\"",
        &format!("version = \"{long}\""),
    )?;
    assert!(matches!(
        Lock::parse_toml(&long_version),
        Err(LockError::BuilderText)
    ));
    let spaced = edited(
        "version = \"v0.35.0-desktop.2\"",
        "version = \"v0.35 desktop\"",
    )?;
    assert!(matches!(
        Lock::parse_toml(&spaced),
        Err(LockError::BuilderText)
    ));
    let control = edited(
        "version = \"v0.35.0-desktop.2\"",
        "version = \"v0.35\\u001b\"",
    )?;
    assert!(matches!(
        Lock::parse_toml(&control),
        Err(LockError::BuilderText)
    ));
    Ok(())
}

#[test]
fn lock_refuses_oversize_text() {
    let text = format!("v = 1\n#{}\n", "x".repeat(MAX_LOCK_BYTES));
    assert!(matches!(Lock::parse_toml(&text), Err(LockError::TooLarge)));
}

#[test]
fn lock_check() -> TestResult {
    let lock = golden_lock()?;
    let dockerfile = BlobId::of(b"FROM alpine\n");
    let guest = BlobId::of(b"minos-guest");
    let locked = lock.check(BackendKind::Krun, Arch::Arm64, &dockerfile, &guest)?;
    assert_eq!(locked.root_image(), &ImageDigest::of(b"root image"));
    assert_eq!(locked.dockerfile_sha256(), &dockerfile);
    assert_eq!(locked.guest_sha256(), &guest);
    assert_eq!(locked.builder().name().as_str(), "docker-buildx");
    assert_eq!(locked.builder().version().as_str(), "v0.35.0-desktop.2");

    assert_eq!(
        lock.check(BackendKind::Krun, Arch::X86_64, &dockerfile, &guest),
        Err(LockMismatch::NoImage {
            backend: BackendKind::Krun,
            arch: Arch::X86_64
        })
    );
    let changed = BlobId::of(b"FROM alpine\nRUN true\n");
    let mismatch = lock.check(BackendKind::Krun, Arch::Arm64, &changed, &guest);
    assert_eq!(
        mismatch,
        Err(LockMismatch::Dockerfile {
            locked: dockerfile,
            found: changed
        })
    );
    let message = mismatch.err().map(|m| m.to_string()).unwrap_or_default();
    assert!(message.contains(&dockerfile.to_hex()) && message.contains(&changed.to_hex()));
    assert!(message.contains("minos env build"));

    let rebuilt = BlobId::of(b"minos-guest v2");
    let mismatch = lock.check(BackendKind::Krun, Arch::Arm64, &dockerfile, &rebuilt);
    assert_eq!(
        mismatch,
        Err(LockMismatch::Guest {
            locked: guest,
            found: rebuilt
        })
    );
    let message = mismatch.err().map(|m| m.to_string()).unwrap_or_default();
    assert!(message.contains(&guest.to_hex()) && message.contains(&rebuilt.to_hex()));
    Ok(())
}

#[test]
fn lock_with_image_replaces() -> TestResult {
    let lock = golden_lock()?.with_image(image(Arch::X86_64, b"x86", b"guest x86")?);
    let replaced =
        lock.clone()
            .with_image(image(Arch::Arm64, b"FROM alpine:new\n", b"minos-guest")?);
    let arm = replaced
        .image(BackendKind::Krun, Arch::Arm64)
        .ok_or("no arm64 image")?;
    assert_eq!(arm.dockerfile_sha256(), &BlobId::of(b"FROM alpine:new\n"));
    assert_eq!(
        replaced.image(BackendKind::Krun, Arch::X86_64),
        lock.image(BackendKind::Krun, Arch::X86_64)
    );
    assert_eq!(replaced.to_toml()?.matches("[[image]]").count(), 2);
    Ok(())
}

#[test]
fn lock_deserializes_through_validation() -> TestResult {
    // Serde on the validated type runs the same checks as parse_toml.
    let text = edited("v = 1\n", "v = 2\n")?;
    assert!(toml::from_str::<Lock>(&text).is_err());
    let good = golden_lock()?;
    assert_eq!(toml::from_str::<Lock>(&good.to_toml()?)?, good);
    Ok(())
}
