//! `minos_cli::init::init` in temporary directories only, never `$HOME` (CLI-01, COST-02).
//!
//! - Each kind's manifest template parses with `Manifest::parse_toml` and names its own kind, independent of the
//!   filesystem.
//! - `init` with each kind in an empty directory writes exactly `minos.toml`, `program.md`, `minos.Dockerfile`, and
//!   `.gitignore`; the manifest parses with that kind, `base_images` accepts the Dockerfile (whose only base is
//!   pinned by digest alone), and `.gitignore` ignores `/.minos/` and `/minos-*.patch` but never the lock.
//! - A second `init` is `InitError::Exists` listing all four paths, and every file's bytes are unchanged.
//! - With only `program.md` present, `init` refuses naming it and writes none of the other three.
//! - A symlink at a scaffold path counts as present: `init` refuses and never writes through it.
//! - With no price table, `init` writes `PRICES_TEMPLATE`, creating missing parents with mode 0700 and the file
//!   with mode 0600, and reports it written; the template parses with non-zero compute rates and no model.
//! - With a price table present, its bytes are unchanged and the report says it was kept.
//!
//! Every file `init` writes, `PRICES_TEMPLATE`, and the text of every provoked error pass
//! [`assert_no_banned_words`]: no `estimate`, and no whole word `exact` (COST-02). `init` prints nothing itself;
//! 08-12's binary test checks the lines `minos init` prints.

use std::error::Error;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use minos_cli::init::{
    DOCKERFILE_TEMPLATE, GITIGNORE_TEMPLATE, InitError, InitKind, InitOptions, PRICES_TEMPLATE,
    PricesFile, init, manifest_template, program_template,
};
use minos_core::image::base_images;
use minos_core::manifest::{Manifest, ProjectKind};
use minos_core::money::Prices;
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn Error>>;
type Fallible<T> = Result<T, Box<dyn Error>>;

/// The four files `init` scaffolds, in the order it reports them.
const SCAFFOLD: [&str; 4] = ["minos.toml", "program.md", "minos.Dockerfile", ".gitignore"];

/// Every kind, beside the project kind its manifest names.
const KINDS: [(InitKind, ProjectKind); 3] = [
    (InitKind::Tune, ProjectKind::Tune),
    (InitKind::Optimize, ProjectKind::Optimize),
    (InitKind::Explore, ProjectKind::Explore),
];

/// A temporary directory holding an empty project directory and a price table path under missing parents.
struct Fixture {
    _dir: TempDir,
    project: PathBuf,
    prices: PathBuf,
}

impl Fixture {
    fn new() -> Fallible<Self> {
        let dir = tempfile::tempdir()?;
        let root = fs::canonicalize(dir.path())?;
        let project = root.join("project");
        fs::create_dir(&project)?;
        let prices = root.join("config").join("minos").join("prices.toml");
        Ok(Self {
            _dir: dir,
            project,
            prices,
        })
    }

    /// `init` of `kind` on the fixture.
    fn init(&self, kind: InitKind) -> Result<minos_cli::init::InitReport, InitError> {
        init(&self.project, &self.prices, InitOptions::new(kind))
    }

    /// The scaffold file `name` in the project.
    fn file(&self, name: &str) -> PathBuf {
        self.project.join(name)
    }
}

/// No `estimate` anywhere, and no `exact` as a whole word (`exactly` passes), in `text` lowercased (COST-02).
fn assert_no_banned_words(text: &str) -> TestResult {
    let lower = text.to_lowercase();
    if lower.contains("estimate") {
        return Err(format!("the text contains the word estimate: {text}").into());
    }
    if lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .any(|word| word == "exact")
    {
        return Err(format!("the text contains the word exact: {text}").into());
    }
    Ok(())
}

/// The names in `dir`, sorted.
fn names_in(dir: &Path) -> Fallible<Vec<String>> {
    let mut names = Vec::new();
    for entry in fs::read_dir(dir)? {
        names.push(entry?.file_name().to_string_lossy().into_owned());
    }
    names.sort();
    Ok(names)
}

/// The permission bits of `path`.
fn mode_of(path: &Path) -> Fallible<u32> {
    Ok(fs::metadata(path)?.permissions().mode() & 0o777)
}

#[test]
fn every_manifest_template_parses_with_its_own_kind() -> TestResult {
    for (kind, project_kind) in KINDS {
        // Act
        let manifest = Manifest::parse_toml(manifest_template(kind))?;

        // Assert
        assert_eq!(manifest.kind(), project_kind);
        assert_no_banned_words(manifest_template(kind))?;
        assert_no_banned_words(program_template(kind))?;
    }
    Ok(())
}

#[test]
fn kinds_parse_from_their_names() -> TestResult {
    assert_eq!(InitKind::from_str("tune")?, InitKind::Tune);
    assert_eq!(InitKind::from_str("optimize")?, InitKind::Optimize);
    assert_eq!(InitKind::from_str("explore")?, InitKind::Explore);
    let Err(error) = InitKind::from_str("design") else {
        return Err("an unknown kind parsed".into());
    };
    assert_no_banned_words(&error.to_string())?;
    Ok(())
}

#[test]
fn init_scaffolds_four_valid_files_for_each_kind() -> TestResult {
    for (kind, project_kind) in KINDS {
        // Arrange
        let fixture = Fixture::new()?;

        // Act
        let report = fixture.init(kind)?;

        // Assert: exactly the four files, and each is valid.
        let mut expected: Vec<String> = SCAFFOLD.iter().map(|name| (*name).to_owned()).collect();
        expected.sort();
        assert_eq!(names_in(&fixture.project)?, expected);
        let written: Vec<PathBuf> = SCAFFOLD.iter().map(|name| fixture.file(name)).collect();
        assert_eq!(report.written(), written.as_slice());

        let manifest_text = fs::read_to_string(fixture.file("minos.toml"))?;
        assert_eq!(manifest_text, manifest_template(kind));
        assert_eq!(Manifest::parse_toml(&manifest_text)?.kind(), project_kind);

        let dockerfile = fs::read_to_string(fixture.file("minos.Dockerfile"))?;
        assert_eq!(dockerfile, DOCKERFILE_TEMPLATE);
        let images = base_images(&dockerfile)?;
        assert_eq!(images.len(), 1);

        let program = fs::read_to_string(fixture.file("program.md"))?;
        assert_eq!(program, program_template(kind));

        let gitignore = fs::read_to_string(fixture.file(".gitignore"))?;
        assert_eq!(gitignore, GITIGNORE_TEMPLATE);
        let entries: Vec<&str> = gitignore
            .lines()
            .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
            .collect();
        assert!(entries.contains(&"/.minos/"), "{gitignore}");
        assert!(entries.contains(&"/minos-*.patch"), "{gitignore}");
        assert!(
            !entries.iter().any(|entry| entry.contains("minos.lock")),
            "{gitignore}"
        );

        for name in SCAFFOLD {
            assert_no_banned_words(&fs::read_to_string(fixture.file(name))?)?;
        }
    }
    Ok(())
}

#[test]
fn a_second_init_refuses_naming_every_file_and_changes_nothing() -> TestResult {
    // Arrange
    let fixture = Fixture::new()?;
    let _first = fixture.init(InitKind::Optimize)?;
    let mut before = Vec::new();
    for name in SCAFFOLD {
        before.push(fs::read(fixture.file(name))?);
    }

    // Act
    let result = fixture.init(InitKind::Tune);

    // Assert
    let Err(error) = result else {
        return Err("a second init overwrote the scaffold".into());
    };
    assert!(matches!(&error, InitError::Exists { .. }), "{error}");
    let text = error.to_string();
    for name in SCAFFOLD {
        assert!(text.contains(name), "{text}");
    }
    assert!(text.contains("never overwrites"), "{text}");
    assert_no_banned_words(&text)?;
    let mut after = Vec::new();
    for name in SCAFFOLD {
        after.push(fs::read(fixture.file(name))?);
    }
    assert_eq!(after, before);
    Ok(())
}

#[test]
fn one_present_file_refuses_before_anything_is_written() -> TestResult {
    // Arrange: only program.md exists.
    let fixture = Fixture::new()?;
    fs::write(fixture.file("program.md"), b"my own instructions\n")?;

    // Act
    let result = fixture.init(InitKind::Explore);

    // Assert
    let Err(error) = result else {
        return Err("init wrote beside a present file".into());
    };
    let text = error.to_string();
    assert!(text.contains("program.md"), "{text}");
    assert!(!text.contains("minos.toml"), "{text}");
    assert_no_banned_words(&text)?;
    assert_eq!(names_in(&fixture.project)?, ["program.md"]);
    assert_eq!(
        fs::read(fixture.file("program.md"))?,
        b"my own instructions\n"
    );
    assert!(!fixture.prices.exists());
    Ok(())
}

#[test]
fn a_symlink_at_a_scaffold_path_is_present_and_never_written_through() -> TestResult {
    // Arrange: `.gitignore` is a dangling symlink to a file outside the project.
    let fixture = Fixture::new()?;
    let target = fixture.project.with_file_name("outside.txt");
    symlink(&target, fixture.file(".gitignore"))?;

    // Act
    let result = fixture.init(InitKind::Optimize);

    // Assert
    let Err(error) = result else {
        return Err("init wrote through a symlink".into());
    };
    let text = error.to_string();
    assert!(text.contains(".gitignore"), "{text}");
    assert_no_banned_words(&text)?;
    assert!(!target.exists());
    assert_eq!(names_in(&fixture.project)?, [".gitignore"]);
    Ok(())
}

#[test]
fn the_prices_template_is_written_when_none_exists_and_fails_closed() -> TestResult {
    // Arrange
    let fixture = Fixture::new()?;

    // Act
    let report = fixture.init(InitKind::Optimize)?;

    // Assert: the template was written, privately, under new private parents.
    assert_eq!(
        report.prices(),
        &PricesFile::Written(fixture.prices.clone())
    );
    assert_eq!(fs::read_to_string(&fixture.prices)?, PRICES_TEMPLATE);
    assert_eq!(mode_of(&fixture.prices)?, 0o600);
    let parent = fixture
        .prices
        .parent()
        .ok_or("the prices path has no parent")?;
    assert_eq!(mode_of(parent)?, 0o700);
    let grandparent = parent
        .parent()
        .ok_or("the prices path has no grandparent")?;
    assert_eq!(mode_of(grandparent)?, 0o700);

    // The template parses, prices compute above zero, and prices no model, so a model proposer fails closed.
    let prices = Prices::parse_toml(PRICES_TEMPLATE)?;
    assert!(prices.compute().vcpu_second().pico() > 0);
    assert!(prices.compute().gib_second().pico() > 0);
    assert_eq!(prices.models().count(), 0);
    let manifest = Manifest::parse_toml(manifest_template(InitKind::Optimize))?;
    assert!(!prices.missing_models(&manifest).is_empty());
    assert_no_banned_words(PRICES_TEMPLATE)?;
    Ok(())
}

#[test]
fn a_present_price_table_is_kept_unchanged() -> TestResult {
    // Arrange
    let fixture = Fixture::new()?;
    let parent = fixture
        .prices
        .parent()
        .ok_or("the prices path has no parent")?;
    fs::create_dir_all(parent)?;
    fs::write(&fixture.prices, b"my own rates\n")?;

    // Act
    let report = fixture.init(InitKind::Tune)?;

    // Assert
    assert_eq!(report.prices(), &PricesFile::Kept(fixture.prices.clone()));
    assert_eq!(fs::read(&fixture.prices)?, b"my own rates\n");
    Ok(())
}
