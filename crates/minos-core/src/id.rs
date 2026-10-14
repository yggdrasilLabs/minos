//! Content IDs and the single hashing builder for composite IDs.
//!
//! Two kinds of ID live here (ARCHITECTURE.md A10):
//!
//! - **Plain digests** ([`BlobId`], [`TreeId`], [`ManifestId`], [`PricesId`], [`ImageDigest`]) are the bare
//!   SHA-256 of content, with no domain, so they equal the checksum S3 verifies natively.
//! - **Composite IDs** ([`ExperimentId`], [`RunId`], [`PromptHash`], [`ReportHash`], and the IDs later modules
//!   define) are hashed only through [`IdHasher`]: a NUL-terminated domain string, then each field as a fixed
//!   32-byte digest, a `u64` big-endian integer, or a `u64` big-endian length prefix followed by bytes. One builder
//!   means no constructor can forget a length prefix or encode an integer differently.
//!
//! Every ID prints and serializes as 64 lowercase hex characters and parses only from exactly that form.
//! [`MetricName`] and [`ModelId`] are the validated names that sit beside the IDs in events and manifests.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::diff::Diff;
use crate::limits::{MAX_MODEL_ID_BYTES, MAX_NAME_BYTES};

/// Domain of [`ExperimentId`].
const EXPERIMENT_DOMAIN: &str = "minos/experiment/v1";
/// Domain of [`RunId`].
const RUN_DOMAIN: &str = "minos/run/v1";
/// Domain of [`PromptHash`].
const PROMPT_DOMAIN: &str = "minos/prompt/v1";
/// Domain of [`ReportHash`].
const REPORT_DOMAIN: &str = "minos/report/v1";

/// Length of an ID written as lowercase hex.
const HEX_LEN: usize = 64;

/// The only way composite IDs are hashed.
///
/// [`IdHasher::new`] writes the domain followed by one NUL byte. Each field is then written as a fixed 32-byte
/// digest ([`IdHasher::digest`]), a `u64` big-endian integer ([`IdHasher::u64`]; narrower integers are widened
/// with `u64::from`, so there is one integer encoding), or a `u64` big-endian length followed by the bytes
/// ([`IdHasher::bytes`]). Fixed-width fields and length prefixes make every encoding unambiguous. The builder is
/// public because the launch key in `minos-sandbox` hashes through it too.
pub struct IdHasher(Sha256);

impl IdHasher {
    /// Starts a hash in `domain`, which must contain no NUL byte (the NUL written after it ends the domain).
    ///
    /// # Panics
    ///
    /// If `domain` contains a NUL byte: the terminator would then be ambiguous and two domains could collide.
    /// Domains are `&'static str` constants, so this is a programming error caught by the first test that hashes
    /// in the domain, never a runtime condition.
    #[must_use]
    pub fn new(domain: &'static str) -> Self {
        assert!(
            !domain.as_bytes().contains(&0),
            "an IdHasher domain contains no NUL byte"
        );
        let mut hasher = Sha256::new();
        hasher.update(domain.as_bytes());
        hasher.update([0_u8]);
        Self(hasher)
    }

    /// Writes a 32-byte digest, unprefixed: its width is fixed.
    #[must_use]
    pub fn digest(mut self, digest: &[u8; 32]) -> Self {
        self.0.update(digest);
        self
    }

    /// Writes an integer as eight bytes, big-endian.
    #[must_use]
    pub fn u64(mut self, value: u64) -> Self {
        self.0.update(value.to_be_bytes());
        self
    }

    /// Writes the byte length as a `u64` big-endian prefix, then the bytes.
    #[must_use]
    pub fn bytes(mut self, bytes: &[u8]) -> Self {
        // usize is 64 bits on every supported target, so this widening never truncates (03-RESEARCH Pattern 2).
        self.0.update((bytes.len() as u64).to_be_bytes());
        self.0.update(bytes);
        self
    }

    /// Finishes the hash.
    #[must_use]
    pub fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

/// Why an ID did not parse. It carries no input text, because IDs arrive from the ledger and the command line.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum IdParseError {
    /// The text is not exactly 64 lowercase hex characters.
    #[error("an ID is 64 lowercase hex characters")]
    Malformed,
}

/// Decodes exactly 64 lowercase hex characters into a digest. Used by every [`digest_id!`] expansion.
pub(crate) fn decode_hex_digest(text: &str) -> Result<[u8; 32], IdParseError> {
    let lower_hex = text.len() == HEX_LEN
        && text
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'));
    let mut digest = [0_u8; 32];
    if lower_hex && base16ct::lower::decode(text, &mut digest).is_ok() {
        Ok(digest)
    } else {
        Err(IdParseError::Malformed)
    }
}

/// Writes a digest as 64 lowercase hex characters without allocating. Used by every [`digest_id!`] expansion.
pub(crate) fn write_hex_digest(digest: &[u8; 32], f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let mut buf = [0_u8; HEX_LEN];
    let text = base16ct::lower::encode_str(digest, &mut buf).map_err(|_| fmt::Error)?;
    f.write_str(text)
}

/// Defines a SHA-256 ID newtype: 32 private bytes, lowercase hex in `Display` and serde, and strict parsing.
///
/// The private `from_digest` constructor is visible only in the invoking module, so only that module decides how
/// the digest is computed.
macro_rules! digest_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 32]);

        impl $name {
            /// The 32 digest bytes.
            #[must_use]
            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }

            /// The digest as 64 lowercase hex characters.
            #[must_use]
            pub fn to_hex(&self) -> String {
                ::base16ct::lower::encode_string(&self.0)
            }

            /// Wraps a digest this module computed.
            const fn from_digest(digest: [u8; 32]) -> Self {
                Self(digest)
            }
        }

        impl ::std::fmt::Debug for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.write_str(concat!(stringify!($name), "("))?;
                ::std::fmt::Display::fmt(self, f)?;
                f.write_str(")")
            }
        }

        impl ::std::fmt::Display for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                $crate::id::write_hex_digest(&self.0, f)
            }
        }

        impl ::std::str::FromStr for $name {
            type Err = $crate::id::IdParseError;

            fn from_str(text: &str) -> Result<Self, Self::Err> {
                $crate::id::decode_hex_digest(text).map(Self)
            }
        }

        impl ::serde::Serialize for $name {
            fn serialize<S: ::serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.collect_str(self)
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D: ::serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let text = <String as ::serde::Deserialize>::deserialize(deserializer)?;
                text.parse().map_err(::serde::de::Error::custom)
            }
        }
    };
}

pub(crate) use digest_id;

/// Plain SHA-256 of content bytes.
fn plain(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

self::digest_id! {
    /// A content-addressed blob: the plain SHA-256 of its bytes, equal to S3's `ChecksumSHA256`.
    BlobId
}

impl BlobId {
    /// The ID of `bytes`.
    #[must_use]
    pub fn of(bytes: &[u8]) -> Self {
        Self::from_digest(plain(bytes))
    }
}

self::digest_id! {
    /// A source tree: the plain SHA-256 of its deterministic tarball.
    TreeId
}

impl TreeId {
    /// The ID of the source tarball `tarball`.
    #[must_use]
    pub fn of(tarball: &[u8]) -> Self {
        Self::from_digest(plain(tarball))
    }

    /// The blob that stores this tree's tarball: the same digest.
    #[must_use]
    pub const fn to_blob_id(&self) -> BlobId {
        BlobId::from_digest(self.0)
    }
}

self::digest_id! {
    /// A manifest version: the plain SHA-256 of the stored manifest bytes.
    ManifestId
}

impl ManifestId {
    /// The ID of `manifest_bytes`.
    #[must_use]
    pub fn of(manifest_bytes: &[u8]) -> Self {
        Self::from_digest(plain(manifest_bytes))
    }
}

self::digest_id! {
    /// A price table version: the plain SHA-256 of the stored `prices.toml` bytes.
    PricesId
}

impl PricesId {
    /// The ID of `prices_bytes`.
    #[must_use]
    pub fn of(prices_bytes: &[u8]) -> Self {
        Self::from_digest(plain(prices_bytes))
    }
}

self::digest_id! {
    /// A root image or OCI digest: the plain SHA-256 of the image bytes.
    ImageDigest
}

impl ImageDigest {
    /// The digest of `bytes`.
    #[must_use]
    pub fn of(bytes: &[u8]) -> Self {
        Self::from_digest(plain(bytes))
    }
}

self::digest_id! {
    /// An experiment: the base tree plus the canonical cumulative diff, under the domain `minos/experiment/v1`.
    ExperimentId
}

impl ExperimentId {
    /// Hashes the domain, the base tree digest, and the length-prefixed canonical diff bytes.
    fn hash(base: &TreeId, canonical: &[u8]) -> Self {
        Self::from_digest(
            IdHasher::new(EXPERIMENT_DOMAIN)
                .digest(base.as_bytes())
                .bytes(canonical)
                .finish(),
        )
    }

    /// The baseline of a run: the base tree with a zero-length diff (03-CONTEXT D-04).
    ///
    /// A parsed diff is never empty, so no experiment built from a diff can share this ID.
    #[must_use]
    pub fn baseline(base: &TreeId) -> Self {
        Self::hash(base, &[])
    }

    /// An experiment: the base tree and its diff, cumulative from the run's base tree (03-CONTEXT D-04
    /// amendment, 2026-10-14).
    ///
    /// It hashes the diff's canonical bytes, never the text a model wrote, so two texts that canonicalize
    /// identically get the same ID.
    #[must_use]
    pub fn of(base: &TreeId, diff: &Diff) -> Self {
        Self::hash(base, diff.as_bytes())
    }
}

self::digest_id! {
    /// A run: the manifest, the base tree, and the start time, under the domain `minos/run/v1`.
    RunId
}

impl RunId {
    /// The ID of a run of `manifest` on `base` started at `started_at` (Unix seconds, read by the shell; the
    /// core never reads a clock, 03-CONTEXT D-06).
    #[must_use]
    pub fn of(manifest: &ManifestId, base: &TreeId, started_at: u64) -> Self {
        Self::from_digest(
            IdHasher::new(RUN_DOMAIN)
                .digest(manifest.as_bytes())
                .digest(base.as_bytes())
                .u64(started_at)
                .finish(),
        )
    }
}

self::digest_id! {
    /// An assembled prompt, under the domain `minos/prompt/v1`.
    PromptHash
}

impl PromptHash {
    /// The hash of the exact prompt bytes sent to the model.
    #[must_use]
    pub fn of(prompt: &[u8]) -> Self {
        Self::from_digest(IdHasher::new(PROMPT_DOMAIN).bytes(prompt).finish())
    }
}

self::digest_id! {
    /// A rendered report, under the domain `minos/report/v1`. It covers the exact bytes the user reviews.
    ReportHash
}

impl ReportHash {
    /// The hash of the rendered report bytes.
    #[must_use]
    pub fn of(rendered: &[u8]) -> Self {
        Self::from_digest(IdHasher::new(REPORT_DOMAIN).bytes(rendered).finish())
    }
}

/// Why a name did not validate. It carries no input text.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum NameError {
    /// The name is empty.
    #[error("a name cannot be empty")]
    Empty,
    /// The name is longer than its limit.
    #[error("a name is at most {max} bytes")]
    TooLong {
        /// The limit, in bytes.
        max: usize,
    },
    /// A byte after the first is outside the name's grammar.
    #[error("the byte at index {index} is not allowed in a name")]
    BadByte {
        /// Zero-based byte index of the first offending byte.
        index: usize,
    },
    /// The first byte is not allowed to start a name.
    #[error("a name cannot start with that character")]
    BadStart,
}

/// Checks a name against a length limit, a first-byte class, and a class for every later byte.
fn check_name(
    text: &str,
    max: usize,
    start: fn(u8) -> bool,
    rest: fn(u8) -> bool,
) -> Result<(), NameError> {
    let mut bytes = text.bytes();
    let Some(first) = bytes.next() else {
        return Err(NameError::Empty);
    };
    if text.len() > max {
        return Err(NameError::TooLong { max });
    }
    if !start(first) {
        return Err(NameError::BadStart);
    }
    match bytes.position(|byte| !rest(byte)) {
        Some(offset) => Err(NameError::BadByte { index: offset + 1 }),
        None => Ok(()),
    }
}

/// Defines a validated name type over a private `String`, parsed and deserialized through one checker.
macro_rules! name_type {
    ($(#[$meta:meta])* $name:ident, $max:expr, $start:expr, $rest:expr) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            /// The name as text.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = NameError;

            fn try_from(text: String) -> Result<Self, Self::Error> {
                check_name(&text, $max, $start, $rest)?;
                Ok(Self(text))
            }
        }

        impl FromStr for $name {
            type Err = NameError;

            fn from_str(text: &str) -> Result<Self, Self::Err> {
                Self::try_from(text.to_owned())
            }
        }

        impl From<$name> for String {
            fn from(name: $name) -> Self {
                name.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

name_type! {
    /// A metric name, such as `throughput_mb_s`: a lowercase letter, then lowercase letters, digits, and
    /// underscores, at most [`MAX_NAME_BYTES`] bytes.
    MetricName,
    MAX_NAME_BYTES,
    |byte| byte.is_ascii_lowercase(),
    |byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'
}

name_type! {
    /// A model identifier, such as `haiku` or a Bedrock model ID: an ASCII letter or digit, then letters, digits,
    /// `.`, `_`, `:`, and `-`, at most [`MAX_MODEL_ID_BYTES`] bytes.
    ModelId,
    MAX_MODEL_ID_BYTES,
    |byte| byte.is_ascii_alphanumeric(),
    |byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-')
}

#[cfg(test)]
mod tests {
    use super::{EXPERIMENT_DOMAIN, PROMPT_DOMAIN, REPORT_DOMAIN, RUN_DOMAIN};

    #[test]
    fn domains_contain_no_nul() {
        for domain in [EXPERIMENT_DOMAIN, RUN_DOMAIN, PROMPT_DOMAIN, REPORT_DOMAIN] {
            assert!(!domain.as_bytes().contains(&0));
        }
    }
}
