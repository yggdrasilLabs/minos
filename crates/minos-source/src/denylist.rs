//! The hard denylist (04-CONTEXT D-07, CLI-11).
//!
//! The denylist is the fourth exclusion source of the snapshot (D-04), next to `.gitignore`, `.minosignore`, and
//! the manifest's `[source] exclude`. It is fixed in code, applied to every walked entry name (files and
//! directories) before every other rule, and nothing overrides it: no ignore file, no negation, no manifest key,
//! and no CLI flag. Every drop is reported in the pack report (plan 04-07), so nothing is dropped silently.
//!
//! Names match ASCII case-insensitively, because APFS is case-insensitive by default: `.ENV` is the same file as
//! `.env` there. Each rule carries the reason it exists (PITFALLS.md Pitfall 28).

/// One rule of the hard denylist. [`DenyRule::pattern`] says what it matches and [`DenyRule::reason`] why.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum DenyRule {
    /// `.git`: the repository.
    GitDir,
    /// `.minos`: the project's ledger and blobs.
    MinosDir,
    /// `.claude`: agent settings and hooks.
    ClaudeDir,
    /// `.aws`: cloud credentials.
    AwsDir,
    /// `.ssh`: SSH keys and config.
    SshDir,
    /// `.env*`: environment files.
    EnvFile,
    /// `*.pem`: keys and certificates.
    PemFile,
    /// `id_*`: SSH private keys by naming convention.
    SshKey,
    /// `minos.lock`: the environment's image digests.
    MinosLock,
    /// A path the judge command names. Applied by the walk from the manifest, not by name.
    JudgePath,
}

/// How a rule matches one path component, compared ASCII case-insensitively.
#[derive(Clone, Copy, Debug)]
enum NamePattern {
    /// The whole name.
    Exact(&'static str),
    /// The leading bytes of the name.
    Prefix(&'static str),
    /// The trailing bytes of the name.
    Suffix(&'static str),
}

impl NamePattern {
    /// Whether `name` matches, ASCII case-insensitively.
    fn matches(self, name: &[u8]) -> bool {
        match self {
            Self::Exact(text) => name.eq_ignore_ascii_case(text.as_bytes()),
            Self::Prefix(text) => name
                .get(..text.len())
                .is_some_and(|head| head.eq_ignore_ascii_case(text.as_bytes())),
            Self::Suffix(text) => name
                .len()
                .checked_sub(text.len())
                .and_then(|start| name.get(start..))
                .is_some_and(|tail| tail.eq_ignore_ascii_case(text.as_bytes())),
        }
    }
}

impl DenyRule {
    /// What the rule matches, as shown to the user.
    #[must_use]
    pub const fn pattern(self) -> &'static str {
        match self {
            Self::GitDir => ".git",
            Self::MinosDir => ".minos",
            Self::ClaudeDir => ".claude",
            Self::AwsDir => ".aws",
            Self::SshDir => ".ssh",
            Self::EnvFile => ".env*",
            Self::PemFile => "*.pem",
            Self::SshKey => "id_*",
            Self::MinosLock => "minos.lock",
            Self::JudgePath => "a path the judge command names",
        }
    }

    /// Why the rule exists.
    #[must_use]
    pub const fn reason(self) -> &'static str {
        match self {
            Self::GitDir => {
                "holds the repository, its hooks, and its config; git must never run on a candidate's tree"
            }
            Self::MinosDir => {
                "holds the project's ledger and blobs, including pushed judge data (04-CONTEXT D-10)"
            }
            Self::ClaudeDir => "holds agent settings and hooks",
            Self::AwsDir => "holds cloud credentials",
            Self::SshDir => "holds SSH keys and config",
            Self::EnvFile => "environment files hold secrets",
            Self::PemFile => "holds keys and certificates",
            Self::SshKey => "the SSH private key naming convention",
            Self::MinosLock => {
                "records the environment's image digests and is never packed (04-CONTEXT D-04)"
            }
            Self::JudgePath => {
                "keeps the judge program out of the candidate's tree even if [source] exclude were wrong"
            }
        }
    }

    /// The name pattern of a name rule; `None` for [`DenyRule::JudgePath`], which the walk applies by path.
    const fn name_pattern(self) -> Option<NamePattern> {
        match self {
            Self::GitDir => Some(NamePattern::Exact(".git")),
            Self::MinosDir => Some(NamePattern::Exact(".minos")),
            Self::ClaudeDir => Some(NamePattern::Exact(".claude")),
            Self::AwsDir => Some(NamePattern::Exact(".aws")),
            Self::SshDir => Some(NamePattern::Exact(".ssh")),
            Self::EnvFile => Some(NamePattern::Prefix(".env")),
            Self::PemFile => Some(NamePattern::Suffix(".pem")),
            Self::SshKey => Some(NamePattern::Prefix("id_")),
            Self::MinosLock => Some(NamePattern::Exact("minos.lock")),
            Self::JudgePath => None,
        }
    }
}

/// The nine rules that match by name, in the order [`deny_component`] tries them. [`DenyRule::JudgePath`] is not
/// here: the walk applies it to the paths the manifest's judge command names.
pub const DENYLIST: [DenyRule; 9] = [
    DenyRule::GitDir,
    DenyRule::MinosDir,
    DenyRule::ClaudeDir,
    DenyRule::AwsDir,
    DenyRule::SshDir,
    DenyRule::EnvFile,
    DenyRule::PemFile,
    DenyRule::SshKey,
    DenyRule::MinosLock,
];

/// The first [`DENYLIST`] rule that matches one path component `name` (a file or directory name, not a path),
/// compared ASCII case-insensitively, or `None`.
#[must_use]
pub fn deny_component(name: &[u8]) -> Option<DenyRule> {
    DENYLIST.into_iter().find(|rule| {
        rule.name_pattern()
            .is_some_and(|pattern| pattern.matches(name))
    })
}
