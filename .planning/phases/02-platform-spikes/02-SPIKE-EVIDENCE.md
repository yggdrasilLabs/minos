# Phase 2: Spike Evidence (D-06)

This file holds the verbatim evidence for every Phase 2 spike. Each plan wrote its own fragment so that plans running in parallel never edited the same file; 02-10 concatenated them here unchanged, in plan order, between the two markers below (02-05 wrote no fragment). Each fragment states its own redactions at the top. Conclusions and recipes live in CLAUDE.md; this file holds the commands and their output.

Some section IDs occur in more than one fragment (`DEFECT-01` in 02-07 and 02-08; the gate checks `C-03c` and `C-03d` beside 02-02's `C-01` to `C-14`). The index names the plan wherever an ID is ambiguous.

| Requirement | Evidence |
|---|---|
| DOCS-05 | the Boot-path decision and Research claims overturned sections below; 02-11's final gate section |
| SPIKE-01 | H-01, H-02, G-GUEST, V-EXIT, B-06, S01-ROOTDISK, S01-CMDLINE, S01-FRAME, S01-NETIFACES, S01-INITRAMFS; K7-00 to K7-11, K7-07b, 02-07 DEFECT-01, K7 table, Classification rule for Phase 5 (K7 is folded into SPIKE-01, D-05) |
| SPIKE-02 | S02-EGRESS, D-PROBE, S01-NETIFACES, SPIKE-02 and SPIKE-03 answers |
| SPIKE-03 | S03-WITHIN, S03-OVER, K4-THROUGHPUT, 02-08 DEFECT-01, DIAG-01, SPIKE-02 and SPIKE-03 answers, S03-ANSWER. A recorded limit, not a pass: one connection carries at most 8192 bytes guest to host on the pinned VMM, and the user chose a chunked protocol for Phase 5; REQUIREMENTS.md leaves SPIKE-03 Pending |
| SPIKE-04 | B-06, S04-UNSIGNED, S04-SIGN, K5 |
| SPIKE-05 | 09-00, S05-ITER-1 to S05-ITER-4, S05-CTRL-SOCK, S05-BOOT, S05-DENY-NET, S05-DENY-FILE, S05-RECIPE, SPIKE-05 answer |
| SPIKE-06 | U-PACK, IR-01, RI-01 to RI-04, S06-BOOT |
| SPIKE-07 | C-01 to C-14, SPIKE-07 contract, Research claims checked |
| SPIKE-08 | S08, and the Boot-path decision section below |

<!-- fragments-start -->
## Plan 02-01: foundation and guest

Date: 2026-10-10

- `rustc --version`: rustc 1.97.1 (8bab26f4f 2026-07-14)
- `cargo --version`: cargo 1.97.1 (c980f4866 2026-06-30)
- `cargo clippy --version`: clippy 0.1.97 (8bab26f4f6 2026-07-14)
- `cargo deny --version`: cargo-deny 0.20.2
- macOS: 27.0, arm64
- HEAD at start: `dbd1a87491d155fb6c4685ee4a30ce7a42d937c4`

Redactions: the absolute home directory path is shown as `~`. Nothing else is altered. Cargo progress lines are removed.

### TC-01: musl target in the pinned toolchain

Requirement: SPIKE-01 (D-03)

Edit: rust-toolchain.toml replaced with E-1 (adds `targets = ["aarch64-unknown-linux-musl"]` and the D-03 reason).

Command: `rustup show active-toolchain`

Exit code: 0

```text
1.97.1-aarch64-apple-darwin (overridden by '~/Repos/minos/rust-toolchain.toml')
```

Command: `rustup target list --installed --toolchain 1.97.1`

Exit code: 0

```text
aarch64-apple-darwin
aarch64-unknown-linux-gnu
aarch64-unknown-linux-musl
```

### B-MUSL: forbid backstop reaches minos-guest on the musl target

Requirement: SPIKE-01 (D-03), T-02-01

Edit: .cargo/config.toml gained E-2 (`[target.aarch64-unknown-linux-musl]` with `linker = "rust-lld"` only). No forbidden construct is written; the proof inspects the clippy-driver command line with `-v`. The second block below holds only the `Running` lines of the verbose output that contain `--crate-name minos_guest` (the other lines are dependency and progress lines), verbatim.

Command: `cargo clean -p minos-guest --target aarch64-unknown-linux-musl`

Exit code: 0

```text
     Removed 8 files, 1.4KiB total
```

Command: `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -v -- -D warnings`

Exit code: 0

```text
     Running `~/.rustup/toolchains/1.97.1-aarch64-apple-darwin/bin/clippy-driver ~/.rustup/toolchains/1.97.1-aarch64-apple-darwin/bin/rustc --crate-name minos_guest --edition=2024 crates/minos-guest/src/main.rs --error-format=json --json=diagnostic-rendered-ansi,artifacts,future-incompat --crate-type bin --emit=dep-info,metadata -C embed-bitcode=no -C debuginfo=2 '--warn=clippy::suspicious' '--warn=clippy::style' '--warn=clippy::perf' '--warn=clippy::pedantic' '--deny=clippy::correctness' '--warn=clippy::complexity' '--warn=clippy::cargo' '--forbid=clippy::unwrap_used' --forbid=unsafe_code '--deny=clippy::undocumented_unsafe_blocks' '--allow=clippy::too_many_lines' '--allow=clippy::struct_excessive_bools' '--allow=clippy::similar_names' '--allow=clippy::multiple_crate_versions' --warn=missing_docs '--forbid=clippy::indexing_slicing' '--forbid=clippy::expect_used' '--deny=rustdoc::broken_intra_doc_links' '--deny=clippy::allow_attributes_without_reason' '--deny=clippy::allow_attributes' --check-cfg 'cfg(docsrs,test)' --check-cfg 'cfg(feature, values())' -C metadata=f28b0019c79433e2 -C extra-filename=-9d0ce105ad1b29ee --out-dir ~/Repos/minos/target/aarch64-unknown-linux-musl/debug/deps --target aarch64-unknown-linux-musl -C linker=rust-lld -C incremental=~/Repos/minos/target/aarch64-unknown-linux-musl/debug/incremental -L dependency=~/Repos/minos/target/aarch64-unknown-linux-musl/debug/deps -L dependency=~/Repos/minos/target/debug/deps --extern minos_core=~/Repos/minos/target/aarch64-unknown-linux-musl/debug/deps/libminos_core-282d4dd05d9c0b2e.rmeta --extern rustix=~/Repos/minos/target/aarch64-unknown-linux-musl/debug/deps/librustix-2f0a2741cd4a511b.rmeta --extern tar=~/Repos/minos/target/aarch64-unknown-linux-musl/debug/deps/libtar-6c9d9a306a244bba.rmeta --extern thiserror=~/Repos/minos/target/aarch64-unknown-linux-musl/debug/deps/libthiserror-72f8d7cf57fee045.rmeta --extern vsock=~/Repos/minos/target/aarch64-unknown-linux-musl/debug/deps/libvsock-077fa49f40ee31f1.rmeta -Funsafe_code '-Fclippy::unwrap_used' '-Fclippy::expect_used' '-Fclippy::indexing_slicing'`
     Running `~/.rustup/toolchains/1.97.1-aarch64-apple-darwin/bin/clippy-driver ~/.rustup/toolchains/1.97.1-aarch64-apple-darwin/bin/rustc --crate-name minos_guest --edition=2024 crates/minos-guest/src/main.rs --error-format=json --json=diagnostic-rendered-ansi,artifacts,future-incompat --emit=dep-info,metadata -C embed-bitcode=no -C debuginfo=2 '--warn=clippy::suspicious' '--warn=clippy::style' '--warn=clippy::perf' '--warn=clippy::pedantic' '--deny=clippy::correctness' '--warn=clippy::complexity' '--warn=clippy::cargo' '--forbid=clippy::unwrap_used' --forbid=unsafe_code '--deny=clippy::undocumented_unsafe_blocks' '--allow=clippy::too_many_lines' '--allow=clippy::struct_excessive_bools' '--allow=clippy::similar_names' '--allow=clippy::multiple_crate_versions' --warn=missing_docs '--forbid=clippy::indexing_slicing' '--forbid=clippy::expect_used' '--deny=rustdoc::broken_intra_doc_links' '--deny=clippy::allow_attributes_without_reason' '--deny=clippy::allow_attributes' --test --check-cfg 'cfg(docsrs,test)' --check-cfg 'cfg(feature, values())' -C metadata=a69897bf3c254734 -C extra-filename=-69f9774022e47eb4 --out-dir ~/Repos/minos/target/aarch64-unknown-linux-musl/debug/deps --target aarch64-unknown-linux-musl -C linker=rust-lld -C incremental=~/Repos/minos/target/aarch64-unknown-linux-musl/debug/incremental -L dependency=~/Repos/minos/target/aarch64-unknown-linux-musl/debug/deps -L dependency=~/Repos/minos/target/debug/deps --extern minos_core=~/Repos/minos/target/aarch64-unknown-linux-musl/debug/deps/libminos_core-282d4dd05d9c0b2e.rmeta --extern rustix=~/Repos/minos/target/aarch64-unknown-linux-musl/debug/deps/librustix-2f0a2741cd4a511b.rmeta --extern tar=~/Repos/minos/target/aarch64-unknown-linux-musl/debug/deps/libtar-6c9d9a306a244bba.rmeta --extern thiserror=~/Repos/minos/target/aarch64-unknown-linux-musl/debug/deps/libthiserror-72f8d7cf57fee045.rmeta --extern vsock=~/Repos/minos/target/aarch64-unknown-linux-musl/debug/deps/libvsock-077fa49f40ee31f1.rmeta -Funsafe_code '-Fclippy::unwrap_used' '-Fclippy::expect_used' '-Fclippy::indexing_slicing'`
```

### L-MUSL: the musl guest links statically with rust-lld

Requirement: SPIKE-01 (D-03)

Setup: E-1 and E-2 in place; guest is still the Phase 1 stub `main`.

Command: `cargo build -p minos-guest --release --target aarch64-unknown-linux-musl`

Exit code: 0

```text
    Finished `release` profile [optimized] target(s) in 3.13s
```

Command: `file target/aarch64-unknown-linux-musl/release/minos-guest`

Exit code: 0

```text
target/aarch64-unknown-linux-musl/release/minos-guest: ELF 64-bit LSB executable, ARM aarch64, version 1 (SYSV), statically linked, not stripped
```

### P-PANIC: Cargo and a per-package panic override

Requirement: SPIKE-01 (D-03)

Edit (temporary, restored with `git checkout -- Cargo.toml` right after): appended to Cargo.toml the table `[profile.release.package.minos-guest]` with the single key `panic = "abort"`. No Rust was written. The block for `cargo metadata` holds its stderr; its stdout was discarded.

Command: `git diff --quiet -- Cargo.toml`

Exit code: 0

```text
```

Command: `cargo metadata --format-version 1 --no-deps --offline`

Exit code: 101

```text
error: failed to parse manifest at `~/Repos/minos/Cargo.toml`

Caused by:
  `panic` may not be specified in a `package` profile
```

### LK-01: lock gains edges only

Requirement: SPIKE-04 (wave-2 plans must not touch Cargo.lock), T-02-05

Edit: E-5 (`minos-guest`: `serde`, `serde_json`), E-6 (`minos-vmm`: `sha2`, `base16ct`), E-7 (`minos-backend-krun`: `tar`, `sha2`, `base16ct`, `serde`), each as a `workspace = true` line.

Before the manifest edits:

Command: `grep -c '^name = ' Cargo.lock`

Exit code: 0

```text
205
```

After the manifest edits:

Command: `cargo check --workspace --offline`

Exit code: 0

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.30s
```

Command: `git diff --numstat -- Cargo.lock`

Exit code: 0

```text
8	0	Cargo.lock
```

Command: `grep -c '^name = ' Cargo.lock`

Exit code: 0

```text
205
```

### H-01: libkrunfw from the libkrun/krun tap

Requirement: SPIKE-01, SPIKE-04 (D-10), T-02-06

Setup: host change allowed by D-10. Only libkrunfw is installed from the tap. `brew install` and `brew info` ran with `HOMEBREW_NO_AUTO_UPDATE=1` because the `brew tap` call had already run the auto-update. The install poured a bottle (`arm64_tahoe`); nothing was built from source.

Command: `brew tap libkrun/krun`

Exit code: 0

```text
==> Auto-updating Homebrew...
Adjust how often this is run with `$HOMEBREW_AUTO_UPDATE_SECS` or disable with
`$HOMEBREW_NO_AUTO_UPDATE=1`. Hide these hints with `$HOMEBREW_NO_ENV_HINTS=1` (see `man brew`).
==> Auto-updated Homebrew!
Updated 3 taps (hashicorp/tap, homebrew/core and homebrew/cask).
==> New Formulae
all-smi: GPU monitoring tool for NVIDIA/Jetson/Apple Silicon/Tenstorrent
apkid: Android application identifier for compilers, packers, and obfuscators
arcane-cli: Command-line client for the Arcane Docker management platform
baysor: Bayesian segmentation of imaging-based spatial transcriptomics data
chisel-ubuntu: Carve and cut Debian packages into slices
ctcache: Cache for clang-tidy static analysis results
cymbal: Language-agnostic code navigation CLI powered by tree-sitter
ddns-updater: Lightweight universal DDNS Updater program
deja: Predictive ghost-text autosuggestions for zsh
doltlite: SQLite fork with Git-style version control via prolly trees
goleft: Tools for coverage and QC of BAM/CRAM files, including indexcov
licensee: Detect under what license a project is distributed
mailtrap-local: Local email sandbox with SMTP server, web UI, and JSON API
maki: Efficient AI coding agent extendable by neovim-like Lua plugins
mariadb@12.3: Drop-in replacement for MySQL
mcpc: Universal command-line client for the Model Context Protocol (MCP)
modflow6: USGS modular hydrologic model
morphe: Desktop app and CLI for patching Android apps with Morphe
openshell: Safe, private runtime for autonomous AI agents
packslip: Signed release manifest for vendor binaries
pg_activity: Top-like interactive monitoring tool for PostgreSQL server activity
pypy3.12: Implementation of Python 3 in Python
python-gdbm@3.15: Python interface to gdbm
python-tk@3.15: Python interface to Tcl/Tk
python@3.15: Interpreted, interactive, object-oriented programming language
regenie: Whole genome regression modelling of large genome-wide association studies
rustqc: Fast genomics quality control tools for sequencing data
sac: Seismic Analysis Code, Community Edition
sdrmm: Modular, client-server software-defined radio
sniffles: Structural variant caller for long-read sequencing data
tfswitch: Command-line tool to switch between Terraform versions
typdiff: Diff tool that generates Typst documents highlighting differences between inputs
unixcw: Morse code tutor and command-line tools
vcfexpress: Filter and format VCF/BCF files with Lua expressions
vortix: Terminal UI for WireGuard and OpenVPN with live telemetry and leak guarding
wavemon: Ncurses-based monitoring application for wireless network devices on Linux
zig@0.16: Programming language designed for robustness, optimality, and clarity
==> New Casks
argo-books: Accounting software for small businesses
auto-tune-central: Software download manager for Antares products
azure-cli: Microsoft Azure CLI 2.0
buildin: Collaborative workspace for notes, documents and wikis
crest: Notch utility with widgets
cuadriver: Background desktop automation driver for AI agents
deckcraft: Presentation editor
deepseek-harness: Plugin-based AI agent desktop application
designcraft: Page layout and publishing tool
effectcraft: Motion graphics and visual effects compositor
filmcraft: Video editor
fl-studio: Digital audio production application
fluxer: Chat for friends, groups, and communities with text, voice, and video
font-aptos
font-lexica-ultralegible
genoffice: Open-source AI office suite
gridcraft: Spreadsheet editor
hagimimonitordirect: Menu bar monitor for system performance and hardware
holst: Online whiteboard for team collaboration
ideashell: AI partner for notes, voice, and to-dos
ideashell-cn: AI partner for notes, voice, and to-dos (China edition)
keyecho: Mechanical keyboard sounds for every keystroke
lightcraft: Photo library manager and raw image developer
littlebird: AI assistant that captures meetings and work context
lody: Share coding agent sessions across desktop and mobile
manus-cue: Personal AI agents with their own identities
markpad: Markdown viewer and editor
mcplinker: Manage and sync MCP server configurations across AI clients
mdhero: Markdown viewer and editor
napari: Multi-dimensional image viewer for Python
nodeterm: Node-based terminal manager for terminals and coding agents on a canvas
pdfcraft: PDF editor
photocraft: Image editor
picsart-ai-playground: Workspace for AI image, video and audio creation
plane: Project management tool for tasks, sprints and docs
ruffle: Flash Player emulator
sandisk: Managing SanDisk external drives
sdrmm-app: Modular, client-server software-defined radio
solidtime: Open-source time tracker
sononym: AI-powered sample browser for exploring audio samples
soundcraft: Digital audio workstation
stashcat: Secure messenger for organisations
swiftcord: Native Discord client
threat-dragon: Threat modeling tool
vectorcraft: Vector editor
volumehud: Bring back the pre-Tahoe volume and brightness HUDs
wheelwizard: Mod manager for Mario Kart Wii and Retro Rewind
wordcraft: Word processor
zed-delta: Multiplayer environment for coding with agents

You have 79 outdated formulae and 3 outdated casks installed.

==> Tapping libkrun/krun
Cloning into '/opt/homebrew/Library/Taps/libkrun/homebrew-krun'...
Tapped 8 formulae (37 files, 468.3MB).
```

Command: `brew trust libkrun/krun`

Exit code: 0

```text
Trusted tap: libkrun/krun
```

Command: `HOMEBREW_NO_AUTO_UPDATE=1 brew install libkrunfw`

Exit code: 0

```text
Warning: The following taps are not trusted:
  cargo-lambda/tap
  derailed/k9s
  filosottile/musl-cross
  hashicorp/tap
  italomandara/cxpatcher
  kdash-rs/kdash
  manaflow-ai/cmux

Homebrew is currently ignoring formulae, casks and commands
from these taps because tap trust is required.
Prefer trusting only the specific formulae, casks or commands you need.
Trust installed formulae from these taps with:
  brew trust --formula cargo-lambda/tap/cargo-lambda
  brew trust --formula derailed/k9s/k9s
  brew trust --formula filosottile/musl-cross/musl-cross
  brew trust --formula hashicorp/tap/terraform
  brew trust --formula kdash-rs/kdash/kdash
Trust installed casks from these taps with:
  brew trust --cask italomandara/cxpatcher/cxpatcher
Trust other specific commands with:
  brew trust --command <user>/<tap>/<command>
Whole-tap trust is broader and includes all current and future formulae,
casks and commands from the listed taps. Trust whole taps with:
  brew trust cargo-lambda/tap derailed/k9s filosottile/musl-cross hashicorp/tap italomandara/cxpatcher kdash-rs/kdash manaflow-ai/cmux
Untap them with:
  brew untap cargo-lambda/tap derailed/k9s filosottile/musl-cross hashicorp/tap italomandara/cxpatcher kdash-rs/kdash manaflow-ai/cmux
For more information, see:
  https://docs.brew.sh/Tap-Trust
==> Fetching downloads for: libkrunfw
✔︎ Bottle libkrunfw (5.6.2)
==> Installing libkrunfw from libkrun/krun
==> Pouring libkrunfw-5.6.2.arm64_tahoe.bottle.tar.gz
🍺  /opt/homebrew/Cellar/libkrunfw/5.6.2: 7 files, 24.9MB
```

Command: `HOMEBREW_NO_AUTO_UPDATE=1 brew info libkrunfw`

Exit code: 0

```text
==> libkrun/krun/libkrunfw: stable 5.6.2 (bottled)
Dynamic library bundling a Linux kernel in a convenient storage format
https://github.com/containers/libkrunfw
Installed (on request)
From: https://github.com/libkrun/homebrew-krun/blob/HEAD/Formula/libkrunfw.rb
Tap: libkrun/krun
License: GPL-2.0-only AND LGPL-2.1-only
==> Installed Versions
libkrun/krun/libkrunfw 5.6.2 (7 files, 24.9MB) [Linked]
==> Requirements
Required: arm64 architecture
```

Command: `ls -l /opt/homebrew/lib/libkrunfw*`

Exit code: 0

```text
lrwxr-xr-x@ 1 seanmcauliffe  admin  47 Oct 10 08:44 /opt/homebrew/lib/libkrunfw.5.dylib -> ../Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib
lrwxr-xr-x@ 1 seanmcauliffe  admin  45 Oct 10 08:44 /opt/homebrew/lib/libkrunfw.dylib -> ../Cellar/libkrunfw/5.6.2/lib/libkrunfw.dylib
```

Command: `realpath /opt/homebrew/lib/libkrunfw.5.dylib`

Exit code: 0

```text
/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib
```

Command: `shasum -a 256 /opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib`

Exit code: 0

```text
59764391005357bbdd5218f3fd522dd8ab63722cbc96144752c7bddc2c8e5217  /opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib
```

Command: `brew list --formula | grep -cE '^(krunkit|krunvm|libkrun)$'`

Exit code: 1

```text
0
```

### H-02: libkrunfw v5.6.2 kernel config facts behind D-18

Requirement: SPIKE-01 (D-18)

Setup: the installed version is 5.6.2 (H-01), so the v5.6.2 tag is the matching config. The file was fetched to the session scratch directory, shown below as `<scratch>`; it is not committed.

Command: `curl -fsSL https://raw.githubusercontent.com/containers/libkrunfw/v5.6.2/config-libkrunfw_aarch64 -o <scratch>/config-libkrunfw_aarch64`

Exit code: 0

```text
```

Command: `grep -nE 'BLK_DEV_INITRD|CONFIG_SQUASHFS|SQUASHFS_ZLIB|DEVTMPFS_MOUNT|CONFIG_VIRTIO_BLK|VIRTIO_VSOCKETS|CONFIG_TSI|^CONFIG_INET=|^CONFIG_NET=|^CONFIG_LOCALVERSION|Linux/arm64' <scratch>/config-libkrunfw_aarch64`

Exit code: 0

```text
3:# Linux/arm64 6.12.91 Kernel Configuration
39:CONFIG_LOCALVERSION=""
210:# CONFIG_BLK_DEV_INITRD is not set
971:CONFIG_NET=y
990:CONFIG_INET=y
1178:CONFIG_VIRTIO_VSOCKETS=y
1179:CONFIG_VIRTIO_VSOCKETS_COMMON=y
1188:CONFIG_TSI=y
1252:CONFIG_DEVTMPFS_MOUNT=y
1485:CONFIG_VIRTIO_BLK=y
2939:CONFIG_SQUASHFS=y
2940:CONFIG_SQUASHFS_FILE_CACHE=y
2941:# CONFIG_SQUASHFS_FILE_DIRECT is not set
2942:CONFIG_SQUASHFS_DECOMP_SINGLE=y
2943:# CONFIG_SQUASHFS_CHOICE_DECOMP_BY_MOUNT is not set
2944:CONFIG_SQUASHFS_COMPILE_DECOMP_SINGLE=y
2945:# CONFIG_SQUASHFS_COMPILE_DECOMP_MULTI is not set
2946:# CONFIG_SQUASHFS_COMPILE_DECOMP_MULTI_PERCPU is not set
2947:# CONFIG_SQUASHFS_XATTR is not set
2948:CONFIG_SQUASHFS_ZLIB=y
2949:# CONFIG_SQUASHFS_LZ4 is not set
2950:# CONFIG_SQUASHFS_LZO is not set
2951:# CONFIG_SQUASHFS_XZ is not set
2952:CONFIG_SQUASHFS_ZSTD=y
2953:# CONFIG_SQUASHFS_4K_DEVBLK_SIZE is not set
2954:# CONFIG_SQUASHFS_EMBEDDED is not set
2955:CONFIG_SQUASHFS_FRAGMENT_CACHE_SIZE=3
```

### H-03: host tools, read-only

Requirement: SPIKE-06, SPIKE-07 (D-10)

Setup: read-only queries. The Docker daemon was already running; nothing was started, installed, or created. `command -v claude` resolves to a terminal-multiplexer shim on this PATH (first block below); the canonical CLI is the `~/.local/bin/claude` target.

Command: `docker version --format '{{.Server.Version}}'`

Exit code: 0

```text
29.6.1
```

Command: `docker buildx version`

Exit code: 0

```text
github.com/docker/buildx v0.35.0-desktop.2 b554ce1decd8b509893b1e7c6227eabfb923d094
```

Command: `command -v claude`

Exit code: 0

```text
/var/folders/_p/cht4w96x7kddklkb8f3v304w0000gn/T/cmux-cli-shims/A4921A1B-78D1-4102-A48E-CE512ADB1555/claude
```

Command: `realpath ~/.local/bin/claude`

Exit code: 0

```text
~/.local/share/claude/versions/2.1.296
```

### G-GUEST: static guest binary

Requirement: SPIKE-01 (D-01, D-03)

Setup: `crates/minos-guest/src/main.rs` gates `mod linux;` on Linux; `src/linux.rs` holds the PID 1 path (boot marker, `/proc` and `/sys` mounts, job from `/dev/vdb`, the E-9 steps, one MINOSST1 frame to CID 2, power off or return). Built from the working tree before the Task 4 commit.

Command: `cargo build -p minos-guest --release --target aarch64-unknown-linux-musl`

Exit code: 0

```text
    Finished `release` profile [optimized] target(s) in 0.05s
```

Command: `file target/aarch64-unknown-linux-musl/release/minos-guest`

Exit code: 0

```text
target/aarch64-unknown-linux-musl/release/minos-guest: ELF 64-bit LSB executable, ARM aarch64, version 1 (SYSV), statically linked, not stripped
```

Command: `shasum -a 256 target/aarch64-unknown-linux-musl/release/minos-guest`

Exit code: 0

```text
10e59803c53ce60bb53445f95b86e07652a5029c5601170823f1fe156e4b3a89  target/aarch64-unknown-linux-musl/release/minos-guest
```

Command: `ls -l target/aarch64-unknown-linux-musl/release/minos-guest`

Exit code: 0

```text
-rwxr-xr-x@ 1 seanmcauliffe  staff  1022016 Oct 10 08:52 target/aarch64-unknown-linux-musl/release/minos-guest
```

## Plan 02-02: `claude -p` contract (SPIKE-07)

Date: 2026-10-10

- `{CLAUDE_BIN}`: `~/.local/share/claude/versions/2.1.296`
- `{CLAUDE_BIN} --version`: 2.1.296 (Claude Code)
- Model alias for every call: `haiku`
- Environment for every call: `env -i HOME PATH=/usr/bin:/bin:/usr/sbin:/sbin USER LOGNAME TMPDIR LANG=en_US.UTF-8`
- Work directory for every call: a fresh `mktemp -d` directory outside any repository

Redactions, applied with `sed -E -e "s#${HOME}#~#g" -e 's#"(total_cost_usd|costUSD)": *[-0-9.eE+]+#"\1":"<omitted: client-side estimate>"#g' -e 's#"(email|orgId|orgName|[A-Za-z_]*([Aa][Cc][Cc][Oo][Uu][Nn][Tt]|[Uu][Uu][Ii][Dd]|[Ee][Mm][Aa][Ii][Ll])[A-Za-z_]*)": *"[^"]*"#"\1": "<redacted>"#g'` before writing: the home path is shown as `~`; `total_cost_usd` and `costUSD` values are replaced with `"<omitted: client-side estimate>"` (they are estimates and never enter the record); in `claude auth status` output, email, org, account, and uuid values are `"<redacted>"`. The same program runs on every output, so the `uuid` value in each `result` and `system`/`init` line is also `"<redacted>"`. For stream-json calls only the `system`/`init` and `result` lines are kept. Nothing else is altered.

Raw output went to files in the session scratch directory; only the redacted copy was appended here. In every `Command:` line, `$HOME`, `$USER`, `$LOGNAME`, and `$TMPDIR` are the shell's own values passed through the allowlist, and the `cd` target is the directory `mktemp -d` printed for that call. Stdout and stderr were captured separately.

### C-01: Binary identity

Requirement: SPIKE-07 (T-02-08, Pitfall 9). No model call.

Command: `cd /var/folders/_p/cht4w96x7kddklkb8f3v304w0000gn/T/tmp.ZBUaYMExZD && { command -v claude; realpath ~/.local/bin/claude; ~/.local/share/claude/versions/2.1.296 --version; head -c 300 "$(command -v claude)"; }`

Exit code: 0

```text
/var/folders/_p/cht4w96x7kddklkb8f3v304w0000gn/T/cmux-cli-shims/A4921A1B-78D1-4102-A48E-CE512ADB1555/claude
~/.local/share/claude/versions/2.1.296
2.1.296 (Claude Code)
#!/usr/bin/env bash
cmux_wrapper="/Applications/cmux.app/Contents/Resources/bin/cmux-claude-wrapper"
if [[ ! -x "$cmux_wrapper" && -n "${CMUX_BUNDLED_CLI_PATH:-}" ]]; then
    cmux_candidate="$(dirname "$CMUX_BUNDLED_CLI_PATH")/cmux-claude-wrapper"
    if [[ -x "$cmux_candidate" ]]; then
        cmu
```

- The first `claude` on the inherited PATH is a bash script under `$TMPDIR/cmux-cli-shims/` that hands off to `/Applications/cmux.app/Contents/Resources/bin/cmux-claude-wrapper` (the shim, Pitfall 9).
- `~/.local/bin/claude` resolves to `~/.local/share/claude/versions/2.1.296`, version `2.1.296 (Claude Code)`. Every later call uses this path.

### C-02: Auth under the six-variable environment

Requirement: SPIKE-07 (T-02-10). No model call.

Command: `cd /var/folders/_p/cht4w96x7kddklkb8f3v304w0000gn/T/tmp.qRHtYzBR5H && env -i HOME="$HOME" PATH=/usr/bin:/bin:/usr/sbin:/sbin USER="$USER" LOGNAME="$LOGNAME" TMPDIR="$TMPDIR" LANG=en_US.UTF-8 ~/.local/share/claude/versions/2.1.296 auth status`

Exit code: 0

```text
{
  "loggedIn": true,
  "authMethod": "claude.ai",
  "apiProvider": "firstParty",
  "analyticsDisabled": false,
  "projectsDirectory": "~/.claude/projects",
  "configDirectory": "~/.claude",
  "email": "<redacted>",
  "orgId": "<redacted>",
  "orgName": "<redacted>",
  "subscriptionType": "max"
}
```

Stderr: empty (0 bytes).

- `loggedIn`: `true`; `authMethod`: `"claude.ai"`; `apiProvider`: `"firstParty"`; `subscriptionType`: `"max"`. Subscription login is reachable with only the six allowlisted variables (no `ANTHROPIC_*` or `CLAUDE_CODE_USE_*` variable present).

### C-03: Base contract (`--tools ""` with `--json-schema`, `--output-format json`)

Requirement: SPIKE-07. Model call 1 of at most 10.

Command: `cd /var/folders/_p/cht4w96x7kddklkb8f3v304w0000gn/T/tmp.NMIH2pskvK && printf 'Reply with the number 7.' | env -i HOME="$HOME" PATH=/usr/bin:/bin:/usr/sbin:/sbin USER="$USER" LOGNAME="$LOGNAME" TMPDIR="$TMPDIR" LANG=en_US.UTF-8 ~/.local/share/claude/versions/2.1.296 -p --model haiku --tools "" --output-format json --json-schema '{"type":"object","properties":{"n":{"type":"integer"}},"required":["n"],"additionalProperties":false}' --no-session-persistence --strict-mcp-config --mcp-config '{"mcpServers":{}}' --safe-mode --permission-prompts none --max-turns 1`

Exit code: 0

```text
{"duration_api_ms":867,"stop_reason":"tool_use","session_id":"05be13f6-1dc8-46c8-aaeb-0819762d8476","total_cost_usd":"<omitted: client-side estimate>","usage":{"input_tokens":2,"cache_creation_input_tokens":3281,"cache_read_input_tokens":0,"output_tokens":51,"output_tokens_details":{"thinking_tokens":0},"server_tool_use":{"web_search_requests":0,"web_fetch_requests":0},"service_tier":"standard","cache_creation":{"ephemeral_1h_input_tokens":3281,"ephemeral_5m_input_tokens":0},"inference_geo":"global","iterations":[{"input_tokens":2,"output_tokens":51,"cache_read_input_tokens":0,"cache_creation_input_tokens":3281,"cache_creation":{"ephemeral_5m_input_tokens":0,"ephemeral_1h_input_tokens":3281},"type":"message"}],"speed":"standard","fallback_credit":null},"modelUsage":{"claude-haiku-5-5":{"inputTokens":2,"outputTokens":51,"cacheReadInputTokens":0,"cacheCreationInputTokens":3281,"webSearchRequests":0,"costUSD":"<omitted: client-side estimate>","contextWindow":1000000,"maxOutputTokens":128000,"thinkingTokens":0,"canonicalModel":"claude-haiku-5-5","provider":"firstParty","costBasis":"list"}},"permission_denials":[],"terminal_reason":"completed","fast_mode_state":"off","fast_mode_disabled_reason":"sdk_opt_in_required","subagent_stats":{"spawned":0,"requested":{"background":0,"foreground":0,"unset":0},"started_in_background":0,"max_depth":0,"spawned_by_subagents":0,"completed":0,"failed":0,"killed":{"parent":0,"user":0,"system":0},"refused":{"depth_limit":0,"concurrency_limit":0,"budget":0},"by_type":{}},"safety_stops":0,"is_error":false,"num_turns":2,"subtype":"success","api_error_status":null,"result":"{\"n\":7}","structured_output":{"n":7},"ttft_ms":877,"type":"result","duration_ms":911,"uuid": "<redacted>","ttft_stream_ms":653,"time_to_request_ms":44,"first_content_frame_ms":654,"queued_turn_count":0,"result_index":0}
```

Stderr: empty (0 bytes).

Read off the block above:

- `subtype`: `"success"`; `is_error`: `false`; `type`: `"result"`.
- `structured_output`: `{"n":7}` (an object matching `{S1}`); `result`: `"{\"n\":7}"` (the same object as a JSON string).
- `num_turns`: `2` under `--max-turns 1`.
- `stop_reason`: `"tool_use"`; `terminal_reason`: `"completed"`; `permission_denials`: `[]`; `api_error_status`: `null`.
- `usage` keys, with nesting: `input_tokens`, `cache_creation_input_tokens`, `cache_read_input_tokens`, `output_tokens`, `output_tokens_details.thinking_tokens`, `server_tool_use.{web_search_requests, web_fetch_requests}`, `service_tier`, `cache_creation.{ephemeral_1h_input_tokens, ephemeral_5m_input_tokens}`, `inference_geo`, `iterations[].{input_tokens, output_tokens, cache_read_input_tokens, cache_creation_input_tokens, cache_creation.{ephemeral_5m_input_tokens, ephemeral_1h_input_tokens}, type}`, `speed`, `fallback_credit`.
- `modelUsage` has one key, `claude-haiku-5-5` (the `haiku` alias resolved to it). Its keys: `inputTokens`, `outputTokens`, `cacheReadInputTokens`, `cacheCreationInputTokens`, `webSearchRequests`, `costUSD` (value omitted), `contextWindow`, `maxOutputTokens`, `thinkingTokens`, `canonicalModel`, `provider`, `costBasis`.
- Other top-level keys present: `duration_api_ms`, `session_id`, `total_cost_usd` (value omitted), `fast_mode_state`, `fast_mode_disabled_reason`, `subagent_stats`, `safety_stops`, `ttft_ms`, `duration_ms`, `uuid`, `ttft_stream_ms`, `time_to_request_ms`, `first_content_frame_ms`, `queued_turn_count`, `result_index`.

SPIKE-07's core question: answered yes by this call (exit 0, `subtype` `success`, `structured_output` `{"n":7}`).

### C-04: `system`/`init` and tools under `--tools ""`

Requirement: SPIKE-07. Model call 2 of at most 10. Same command as C-03 with `--output-format stream-json --verbose` in place of `--output-format json`. Truncation noted: the call printed 5 lines (`system`/`init`, `assistant`, `user`, `rate_limit_event`, `result`); only the `system`/`init` line and the `result` line are kept.

Command: `cd /var/folders/_p/cht4w96x7kddklkb8f3v304w0000gn/T/tmp.uSStOpkyJg && printf 'Reply with the number 7.' | env -i HOME="$HOME" PATH=/usr/bin:/bin:/usr/sbin:/sbin USER="$USER" LOGNAME="$LOGNAME" TMPDIR="$TMPDIR" LANG=en_US.UTF-8 ~/.local/share/claude/versions/2.1.296 -p --model haiku --tools "" --output-format stream-json --verbose --json-schema '{"type":"object","properties":{"n":{"type":"integer"}},"required":["n"],"additionalProperties":false}' --no-session-persistence --strict-mcp-config --mcp-config '{"mcpServers":{}}' --safe-mode --permission-prompts none --max-turns 1`

Exit code: 0

```text
{"type":"system","subtype":"init","cwd":"/private/var/folders/_p/cht4w96x7kddklkb8f3v304w0000gn/T/tmp.uSStOpkyJg","session_id":"62a4fb19-f7f2-4ee9-89b9-532bdd4fa50d","tools":["StructuredOutput"],"mcp_servers":[],"model":"claude-haiku-5-5","permissionMode":"default","slash_commands":["deep-research","design","design-sync","dataviz","update-config","verify","debug","code-review","simplify","batch","fewer-permission-prompts","doctor","loop","schedule","claude-api","workflow-authoring","run","run-skill-generator","plugin-authoring","advisor","agents","auto-mode-setup","autocompact","clear","color","compact","config","output-style","context","effort","fast","focus","heapdump","init","mcp","import","model","__remote-workflow","workflow-launch-exec","reload-plugins","reload-skills","rename","ultrareview","security-review","usage-credits","extra-usage","usage","insights","recap","skill-doctor","goal","design-consent","design-revoke","list-agents","team-onboarding"],"terminal_slash_commands":["doctor","color","focus","reload-plugins"],"apiKeySource":"none","claude_code_version":"2.1.296","output_style":"default","agents":["claude","Explore","general-purpose","Plan"],"skills":["deep-research","design","design-sync","dataviz","update-config","verify","debug","code-review","simplify","batch","fewer-permission-prompts","doctor","loop","schedule","claude-api","workflow-authoring","run","run-skill-generator","plugin-authoring"],"plugins":[{"name":"cc-plugin-agents-md","path":"builtin","source":"cc-plugin-agents-md@builtin"},{"name":"cc-plugin-telemetry","path":"builtin","source":"cc-plugin-telemetry@builtin"},{"name":"cc-plugin-plugin-authoring","path":"builtin","source":"cc-plugin-plugin-authoring@builtin"}],"capabilities":["interrupt_receipt_v1","interrupt_cancel_queued_v1","interrupt_send_now_v1","msg_lifecycle_v1","request_marker_lists_v1","sdk_mcp_tools_list_changed","sdk_mcp_manifests","mcp_read_resource_v1","mcp_tool_ui_meta_v1","ui_surface_v1"],"analytics_disabled":false,"product_feedback_disabled":false,"uuid": "<redacted>","messaging_socket_path":"/tmp/cc-socks/25253.sock","fast_mode_state":"off","fast_mode_disabled_reason":"sdk_opt_in_required","per_turn_effort_active":true,"view_mode":"default"}
{"duration_api_ms":832,"stop_reason":"tool_use","session_id":"62a4fb19-f7f2-4ee9-89b9-532bdd4fa50d","total_cost_usd":"<omitted: client-side estimate>","usage":{"input_tokens":2,"cache_creation_input_tokens":362,"cache_read_input_tokens":2919,"output_tokens":51,"output_tokens_details":{"thinking_tokens":0},"server_tool_use":{"web_search_requests":0,"web_fetch_requests":0},"service_tier":"standard","cache_creation":{"ephemeral_1h_input_tokens":362,"ephemeral_5m_input_tokens":0},"inference_geo":"global","iterations":[{"input_tokens":2,"output_tokens":51,"cache_read_input_tokens":2919,"cache_creation_input_tokens":362,"cache_creation":{"ephemeral_5m_input_tokens":0,"ephemeral_1h_input_tokens":362},"type":"message"}],"speed":"standard","fallback_credit":null},"modelUsage":{"claude-haiku-5-5":{"inputTokens":2,"outputTokens":51,"cacheReadInputTokens":2919,"cacheCreationInputTokens":362,"webSearchRequests":0,"costUSD":"<omitted: client-side estimate>","contextWindow":1000000,"maxOutputTokens":128000,"thinkingTokens":0,"canonicalModel":"claude-haiku-5-5","provider":"firstParty","costBasis":"list"}},"permission_denials":[],"terminal_reason":"completed","fast_mode_state":"off","fast_mode_disabled_reason":"sdk_opt_in_required","subagent_stats":{"spawned":0,"requested":{"background":0,"foreground":0,"unset":0},"started_in_background":0,"max_depth":0,"spawned_by_subagents":0,"completed":0,"failed":0,"killed":{"parent":0,"user":0,"system":0},"refused":{"depth_limit":0,"concurrency_limit":0,"budget":0},"by_type":{}},"safety_stops":0,"is_error":false,"num_turns":2,"subtype":"success","api_error_status":null,"result":"{\"n\":7}","structured_output":{"n":7},"ttft_ms":840,"type":"result","duration_ms":866,"uuid": "<redacted>","ttft_stream_ms":560,"time_to_request_ms":34,"first_content_frame_ms":560,"queued_turn_count":0,"result_index":0}
```

Stderr: empty (0 bytes).

Read off the `system`/`init` line:

- `tools`: `["StructuredOutput"]`. No Agent, Skill, Bash, Read, or other built-in tool is offered; structured output is delivered through an internal `StructuredOutput` tool, which is why `stop_reason` is `"tool_use"` and `num_turns` is `2`.
- `mcp_servers`: `[]`.
- `apiKeySource`: `"none"` (no API key in use; auth is the subscription login of C-02).
- `permissionMode`: `"default"`.
- `model`: `"claude-haiku-5-5"`.
- `slash_commands`: 55 entries, and `skills`: 19 entries, still listed. `agents`: `["claude","Explore","general-purpose","Plan"]`, still listed. With only `StructuredOutput` in `tools`, the model has no Skill or Agent tool to invoke them with.
- `plugins`: three builtin entries, `cc-plugin-agents-md`, `cc-plugin-telemetry`, `cc-plugin-plugin-authoring` (`"path":"builtin"`), loaded under `--safe-mode`.
- The `result` line has the same `subtype` (`success`), `structured_output` (`{"n":7}`), `num_turns` (`2`), and key set as C-03.

### C-05: `--disable-slash-commands`

Requirement: SPIKE-07. Model call 3 of at most 10. C-04 plus `--disable-slash-commands`. Truncation noted: the call printed 5 lines; only the `system`/`init` line and the `result` line are kept.

Command: `cd /var/folders/_p/cht4w96x7kddklkb8f3v304w0000gn/T/tmp.Xab8kwy82g && printf 'Reply with the number 7.' | env -i HOME="$HOME" PATH=/usr/bin:/bin:/usr/sbin:/sbin USER="$USER" LOGNAME="$LOGNAME" TMPDIR="$TMPDIR" LANG=en_US.UTF-8 ~/.local/share/claude/versions/2.1.296 -p --model haiku --tools "" --output-format stream-json --verbose --json-schema '{"type":"object","properties":{"n":{"type":"integer"}},"required":["n"],"additionalProperties":false}' --no-session-persistence --strict-mcp-config --mcp-config '{"mcpServers":{}}' --safe-mode --permission-prompts none --max-turns 1 --disable-slash-commands`

Exit code: 0

```text
{"type":"system","subtype":"init","cwd":"/private/var/folders/_p/cht4w96x7kddklkb8f3v304w0000gn/T/tmp.Xab8kwy82g","session_id":"6228bbd2-993c-45fb-8c75-cca1a649d01b","tools":["StructuredOutput"],"mcp_servers":[],"model":"claude-haiku-5-5","permissionMode":"default","slash_commands":[],"apiKeySource":"none","claude_code_version":"2.1.296","output_style":"default","agents":["claude","Explore","general-purpose","Plan"],"skills":[],"plugins":[{"name":"cc-plugin-agents-md","path":"builtin","source":"cc-plugin-agents-md@builtin"},{"name":"cc-plugin-telemetry","path":"builtin","source":"cc-plugin-telemetry@builtin"},{"name":"cc-plugin-plugin-authoring","path":"builtin","source":"cc-plugin-plugin-authoring@builtin"}],"capabilities":["interrupt_receipt_v1","interrupt_cancel_queued_v1","interrupt_send_now_v1","msg_lifecycle_v1","request_marker_lists_v1","sdk_mcp_tools_list_changed","sdk_mcp_manifests","mcp_read_resource_v1","mcp_tool_ui_meta_v1","ui_surface_v1"],"analytics_disabled":false,"product_feedback_disabled":false,"uuid": "<redacted>","messaging_socket_path":"/tmp/cc-socks/25772.sock","fast_mode_state":"off","fast_mode_disabled_reason":"sdk_opt_in_required","per_turn_effort_active":true,"view_mode":"default"}
{"duration_api_ms":1003,"stop_reason":"tool_use","session_id":"6228bbd2-993c-45fb-8c75-cca1a649d01b","total_cost_usd":"<omitted: client-side estimate>","usage":{"input_tokens":2,"cache_creation_input_tokens":362,"cache_read_input_tokens":2919,"output_tokens":51,"output_tokens_details":{"thinking_tokens":0},"server_tool_use":{"web_search_requests":0,"web_fetch_requests":0},"service_tier":"standard","cache_creation":{"ephemeral_1h_input_tokens":362,"ephemeral_5m_input_tokens":0},"inference_geo":"global","iterations":[{"input_tokens":2,"output_tokens":51,"cache_read_input_tokens":2919,"cache_creation_input_tokens":362,"cache_creation":{"ephemeral_5m_input_tokens":0,"ephemeral_1h_input_tokens":362},"type":"message"}],"speed":"standard","fallback_credit":null},"modelUsage":{"claude-haiku-5-5":{"inputTokens":2,"outputTokens":51,"cacheReadInputTokens":2919,"cacheCreationInputTokens":362,"webSearchRequests":0,"costUSD":"<omitted: client-side estimate>","contextWindow":1000000,"maxOutputTokens":128000,"thinkingTokens":0,"canonicalModel":"claude-haiku-5-5","provider":"firstParty","costBasis":"list"}},"permission_denials":[],"terminal_reason":"completed","fast_mode_state":"off","fast_mode_disabled_reason":"sdk_opt_in_required","subagent_stats":{"spawned":0,"requested":{"background":0,"foreground":0,"unset":0},"started_in_background":0,"max_depth":0,"spawned_by_subagents":0,"completed":0,"failed":0,"killed":{"parent":0,"user":0,"system":0},"refused":{"depth_limit":0,"concurrency_limit":0,"budget":0},"by_type":{}},"safety_stops":0,"is_error":false,"num_turns":2,"subtype":"success","api_error_status":null,"result":"{\"n\":7}","structured_output":{"n":7},"ttft_ms":1003,"type":"result","duration_ms":1036,"uuid": "<redacted>","ttft_stream_ms":652,"time_to_request_ms":31,"first_content_frame_ms":652,"queued_turn_count":0,"result_index":0}
```

Stderr: empty (0 bytes).

- `slash_commands`: `[]` (55 entries in C-04). `skills`: `[]` (19 entries in C-04). Both are empty now.
- `terminal_slash_commands`: the key is absent from the init line (present with 4 entries in C-04).
- `agents`: still `["claude","Explore","general-purpose","Plan"]`; `--disable-slash-commands` does not remove them. `tools` is still `["StructuredOutput"]`, so no Agent tool exists to invoke them.
- `plugins`: the same three builtin entries as C-04.
- Result: `subtype` `success`, `structured_output` `{"n":7}`, `num_turns` `2`.

### C-06: `--tools ""` without a schema

Requirement: SPIKE-07. Model call 4 of at most 10. Truncation noted: the call printed 4 lines (`system`/`init`, `assistant`, `rate_limit_event`, `result`); only the `system`/`init` line and the `result` line are kept.

Command: `cd /var/folders/_p/cht4w96x7kddklkb8f3v304w0000gn/T/tmp.zJR1E9vJuj && printf 'Reply with the word ok.' | env -i HOME="$HOME" PATH=/usr/bin:/bin:/usr/sbin:/sbin USER="$USER" LOGNAME="$LOGNAME" TMPDIR="$TMPDIR" LANG=en_US.UTF-8 ~/.local/share/claude/versions/2.1.296 -p --model haiku --tools "" --output-format stream-json --verbose --no-session-persistence --strict-mcp-config --mcp-config '{"mcpServers":{}}' --safe-mode --permission-prompts none --max-turns 1`

Exit code: 0

```text
{"type":"system","subtype":"init","cwd":"/private/var/folders/_p/cht4w96x7kddklkb8f3v304w0000gn/T/tmp.zJR1E9vJuj","session_id":"c95a7e09-1368-426c-92ba-65254493f4d2","tools":[],"mcp_servers":[],"model":"claude-haiku-5-5","permissionMode":"default","slash_commands":["deep-research","design","design-sync","dataviz","update-config","verify","debug","code-review","simplify","batch","fewer-permission-prompts","doctor","loop","schedule","claude-api","workflow-authoring","run","run-skill-generator","plugin-authoring","advisor","agents","auto-mode-setup","autocompact","clear","color","compact","config","output-style","context","effort","fast","focus","heapdump","init","mcp","import","model","__remote-workflow","workflow-launch-exec","reload-plugins","reload-skills","rename","ultrareview","security-review","usage-credits","extra-usage","usage","insights","recap","skill-doctor","goal","design-consent","design-revoke","list-agents","team-onboarding"],"terminal_slash_commands":["doctor","color","focus","reload-plugins"],"apiKeySource":"none","claude_code_version":"2.1.296","output_style":"default","agents":["claude","Explore","general-purpose","Plan"],"skills":["deep-research","design","design-sync","dataviz","update-config","verify","debug","code-review","simplify","batch","fewer-permission-prompts","doctor","loop","schedule","claude-api","workflow-authoring","run","run-skill-generator","plugin-authoring"],"plugins":[{"name":"cc-plugin-agents-md","path":"builtin","source":"cc-plugin-agents-md@builtin"},{"name":"cc-plugin-telemetry","path":"builtin","source":"cc-plugin-telemetry@builtin"},{"name":"cc-plugin-plugin-authoring","path":"builtin","source":"cc-plugin-plugin-authoring@builtin"}],"capabilities":["interrupt_receipt_v1","interrupt_cancel_queued_v1","interrupt_send_now_v1","msg_lifecycle_v1","request_marker_lists_v1","sdk_mcp_tools_list_changed","sdk_mcp_manifests","mcp_read_resource_v1","mcp_tool_ui_meta_v1","ui_surface_v1"],"analytics_disabled":false,"product_feedback_disabled":false,"uuid": "<redacted>","messaging_socket_path":"/tmp/cc-socks/25837.sock","fast_mode_state":"off","fast_mode_disabled_reason":"sdk_opt_in_required","per_turn_effort_active":true,"view_mode":"default"}
{"duration_api_ms":812,"stop_reason":"end_turn","session_id":"c95a7e09-1368-426c-92ba-65254493f4d2","total_cost_usd":"<omitted: client-side estimate>","usage":{"input_tokens":2,"cache_creation_input_tokens":2248,"cache_read_input_tokens":535,"output_tokens":4,"output_tokens_details":{"thinking_tokens":0},"server_tool_use":{"web_search_requests":0,"web_fetch_requests":0},"service_tier":"standard","cache_creation":{"ephemeral_1h_input_tokens":2248,"ephemeral_5m_input_tokens":0},"inference_geo":"global","iterations":[{"input_tokens":2,"output_tokens":4,"cache_read_input_tokens":535,"cache_creation_input_tokens":2248,"cache_creation":{"ephemeral_5m_input_tokens":0,"ephemeral_1h_input_tokens":2248},"type":"message"}],"speed":"standard","fallback_credit":null},"modelUsage":{"claude-haiku-5-5":{"inputTokens":2,"outputTokens":4,"cacheReadInputTokens":535,"cacheCreationInputTokens":2248,"webSearchRequests":0,"costUSD":"<omitted: client-side estimate>","contextWindow":1000000,"maxOutputTokens":128000,"thinkingTokens":0,"canonicalModel":"claude-haiku-5-5","provider":"firstParty","costBasis":"list"}},"permission_denials":[],"terminal_reason":"completed","fast_mode_state":"off","fast_mode_disabled_reason":"sdk_opt_in_required","subagent_stats":{"spawned":0,"requested":{"background":0,"foreground":0,"unset":0},"started_in_background":0,"max_depth":0,"spawned_by_subagents":0,"completed":0,"failed":0,"killed":{"parent":0,"user":0,"system":0},"refused":{"depth_limit":0,"concurrency_limit":0,"budget":0},"by_type":{}},"safety_stops":0,"is_error":false,"num_turns":1,"subtype":"success","api_error_status":null,"result":"ok","ttft_ms":837,"type":"result","duration_ms":847,"uuid": "<redacted>","ttft_stream_ms":836,"time_to_request_ms":34,"first_content_frame_ms":836,"queued_turn_count":0,"result_index":0}
```

Stderr: empty (0 bytes).

- `tools`: `[]`. With no schema there is no `StructuredOutput` tool either.
- `subtype`: `"success"`; `is_error`: `false`; `num_turns`: `1`; `stop_reason`: `"end_turn"`; `result`: `"ok"`. The `result` line has no `structured_output` key.
- A plain reply under `--max-turns 1` completes in one turn without an error.

### C-07: `--disallowedTools "mcp__*"` alongside `--tools ""`

Requirement: SPIKE-07. Model call 5 of at most 10.

Command: `cd /var/folders/_p/cht4w96x7kddklkb8f3v304w0000gn/T/tmp.3zZGOUmQKr && printf 'Reply with the number 7.' | env -i HOME="$HOME" PATH=/usr/bin:/bin:/usr/sbin:/sbin USER="$USER" LOGNAME="$LOGNAME" TMPDIR="$TMPDIR" LANG=en_US.UTF-8 ~/.local/share/claude/versions/2.1.296 -p --model haiku --tools "" --output-format json --json-schema '{"type":"object","properties":{"n":{"type":"integer"}},"required":["n"],"additionalProperties":false}' --no-session-persistence --strict-mcp-config --mcp-config '{"mcpServers":{}}' --safe-mode --permission-prompts none --max-turns 1 --disallowedTools "mcp__*"`

Exit code: 0

```text
{"duration_api_ms":890,"stop_reason":"tool_use","session_id":"9728928d-21a7-498a-8f9d-592ff39462b1","total_cost_usd":"<omitted: client-side estimate>","usage":{"input_tokens":2,"cache_creation_input_tokens":363,"cache_read_input_tokens":2919,"output_tokens":51,"output_tokens_details":{"thinking_tokens":0},"server_tool_use":{"web_search_requests":0,"web_fetch_requests":0},"service_tier":"standard","cache_creation":{"ephemeral_1h_input_tokens":363,"ephemeral_5m_input_tokens":0},"inference_geo":"global","iterations":[{"input_tokens":2,"output_tokens":51,"cache_read_input_tokens":2919,"cache_creation_input_tokens":363,"cache_creation":{"ephemeral_5m_input_tokens":0,"ephemeral_1h_input_tokens":363},"type":"message"}],"speed":"standard","fallback_credit":null},"modelUsage":{"claude-haiku-5-5":{"inputTokens":2,"outputTokens":51,"cacheReadInputTokens":2919,"cacheCreationInputTokens":363,"webSearchRequests":0,"costUSD":"<omitted: client-side estimate>","contextWindow":1000000,"maxOutputTokens":128000,"thinkingTokens":0,"canonicalModel":"claude-haiku-5-5","provider":"firstParty","costBasis":"list"}},"permission_denials":[],"terminal_reason":"completed","fast_mode_state":"off","fast_mode_disabled_reason":"sdk_opt_in_required","subagent_stats":{"spawned":0,"requested":{"background":0,"foreground":0,"unset":0},"started_in_background":0,"max_depth":0,"spawned_by_subagents":0,"completed":0,"failed":0,"killed":{"parent":0,"user":0,"system":0},"refused":{"depth_limit":0,"concurrency_limit":0,"budget":0},"by_type":{}},"safety_stops":0,"is_error":false,"num_turns":2,"subtype":"success","api_error_status":null,"result":"{\"n\":7}","structured_output":{"n":7},"ttft_ms":889,"type":"result","duration_ms":922,"uuid": "<redacted>","ttft_stream_ms":583,"time_to_request_ms":31,"first_content_frame_ms":583,"queued_turn_count":0,"result_index":0}
```

Stderr: empty (0 bytes).

- Accepted: exit 0, no stderr. `subtype` `success`, `is_error` `false`, `structured_output` `{"n":7}`, `num_turns` `2`, `permission_denials` `[]`.
- Every JSON key path in this result is the same as in C-03 (compared with `jq paths` on the raw files; only values differ). The result is unchanged by the flag.

### C-08: `--permission-mode dontAsk` in place of `--permission-prompts none`

Requirement: SPIKE-07. Model call 6 of at most 10. Run with `--output-format stream-json --verbose` instead of `json` so the `system`/`init` line shows `permissionMode` (the `json` result has no such field); the `result` line carries the same keys as the `json` output (C-03 against C-04). Truncation noted: the call printed 5 lines; only the `system`/`init` line and the `result` line are kept.

Command: `cd /var/folders/_p/cht4w96x7kddklkb8f3v304w0000gn/T/tmp.psTpEcKBUU && printf 'Reply with the number 7.' | env -i HOME="$HOME" PATH=/usr/bin:/bin:/usr/sbin:/sbin USER="$USER" LOGNAME="$LOGNAME" TMPDIR="$TMPDIR" LANG=en_US.UTF-8 ~/.local/share/claude/versions/2.1.296 -p --model haiku --tools "" --output-format stream-json --verbose --json-schema '{"type":"object","properties":{"n":{"type":"integer"}},"required":["n"],"additionalProperties":false}' --no-session-persistence --strict-mcp-config --mcp-config '{"mcpServers":{}}' --safe-mode --permission-mode dontAsk --max-turns 1`

Exit code: 0

```text
{"type":"system","subtype":"init","cwd":"/private/var/folders/_p/cht4w96x7kddklkb8f3v304w0000gn/T/tmp.psTpEcKBUU","session_id":"62b11ea1-bd19-400e-bea2-af920a22c2b9","tools":["StructuredOutput"],"mcp_servers":[],"model":"claude-haiku-5-5","permissionMode":"dontAsk","slash_commands":["deep-research","design","design-sync","dataviz","update-config","verify","debug","code-review","simplify","batch","fewer-permission-prompts","doctor","loop","schedule","claude-api","workflow-authoring","run","run-skill-generator","plugin-authoring","advisor","agents","auto-mode-setup","autocompact","clear","color","compact","config","output-style","context","effort","fast","focus","heapdump","init","mcp","import","model","__remote-workflow","workflow-launch-exec","reload-plugins","reload-skills","rename","ultrareview","security-review","usage-credits","extra-usage","usage","insights","recap","skill-doctor","goal","design-consent","design-revoke","list-agents","team-onboarding"],"terminal_slash_commands":["doctor","color","focus","reload-plugins"],"apiKeySource":"none","claude_code_version":"2.1.296","output_style":"default","agents":["claude","Explore","general-purpose","Plan"],"skills":["deep-research","design","design-sync","dataviz","update-config","verify","debug","code-review","simplify","batch","fewer-permission-prompts","doctor","loop","schedule","claude-api","workflow-authoring","run","run-skill-generator","plugin-authoring"],"plugins":[{"name":"cc-plugin-agents-md","path":"builtin","source":"cc-plugin-agents-md@builtin"},{"name":"cc-plugin-telemetry","path":"builtin","source":"cc-plugin-telemetry@builtin"},{"name":"cc-plugin-plugin-authoring","path":"builtin","source":"cc-plugin-plugin-authoring@builtin"}],"capabilities":["interrupt_receipt_v1","interrupt_cancel_queued_v1","interrupt_send_now_v1","msg_lifecycle_v1","request_marker_lists_v1","sdk_mcp_tools_list_changed","sdk_mcp_manifests","mcp_read_resource_v1","mcp_tool_ui_meta_v1","ui_surface_v1"],"analytics_disabled":false,"product_feedback_disabled":false,"uuid": "<redacted>","messaging_socket_path":"/tmp/cc-socks/25969.sock","fast_mode_state":"off","fast_mode_disabled_reason":"sdk_opt_in_required","per_turn_effort_active":true,"view_mode":"default"}
{"duration_api_ms":885,"stop_reason":"tool_use","session_id":"62b11ea1-bd19-400e-bea2-af920a22c2b9","total_cost_usd":"<omitted: client-side estimate>","usage":{"input_tokens":2,"cache_creation_input_tokens":362,"cache_read_input_tokens":2919,"output_tokens":51,"output_tokens_details":{"thinking_tokens":0},"server_tool_use":{"web_search_requests":0,"web_fetch_requests":0},"service_tier":"standard","cache_creation":{"ephemeral_1h_input_tokens":362,"ephemeral_5m_input_tokens":0},"inference_geo":"global","iterations":[{"input_tokens":2,"output_tokens":51,"cache_read_input_tokens":2919,"cache_creation_input_tokens":362,"cache_creation":{"ephemeral_5m_input_tokens":0,"ephemeral_1h_input_tokens":362},"type":"message"}],"speed":"standard","fallback_credit":null},"modelUsage":{"claude-haiku-5-5":{"inputTokens":2,"outputTokens":51,"cacheReadInputTokens":2919,"cacheCreationInputTokens":362,"webSearchRequests":0,"costUSD":"<omitted: client-side estimate>","contextWindow":1000000,"maxOutputTokens":128000,"thinkingTokens":0,"canonicalModel":"claude-haiku-5-5","provider":"firstParty","costBasis":"list"}},"permission_denials":[],"terminal_reason":"completed","fast_mode_state":"off","fast_mode_disabled_reason":"sdk_opt_in_required","subagent_stats":{"spawned":0,"requested":{"background":0,"foreground":0,"unset":0},"started_in_background":0,"max_depth":0,"spawned_by_subagents":0,"completed":0,"failed":0,"killed":{"parent":0,"user":0,"system":0},"refused":{"depth_limit":0,"concurrency_limit":0,"budget":0},"by_type":{}},"safety_stops":0,"is_error":false,"num_turns":2,"subtype":"success","api_error_status":null,"result":"{\"n\":7}","structured_output":{"n":7},"ttft_ms":894,"type":"result","duration_ms":919,"uuid": "<redacted>","ttft_stream_ms":611,"time_to_request_ms":33,"first_content_frame_ms":612,"queued_turn_count":0,"result_index":0}
```

Stderr: empty (0 bytes).

- Accepted with `-p`: exit 0, no stderr. `permissionMode` is `"dontAsk"` here; under `--permission-prompts none` (C-04) it is `"default"`.
- Result unchanged: `subtype` `success`, `structured_output` `{"n":7}`, `num_turns` `2`, `permission_denials` `[]`.
- Both flags work with `-p`. With only `StructuredOutput` in `tools`, no call made a permission request, so neither call exercised the difference between them (C-13 records the help text for each).

### C-09: `--setting-sources ""`

Requirement: SPIKE-07. Model call 7 of at most 10.

Command: `cd /var/folders/_p/cht4w96x7kddklkb8f3v304w0000gn/T/tmp.jLvts71G3M && printf 'Reply with the number 7.' | env -i HOME="$HOME" PATH=/usr/bin:/bin:/usr/sbin:/sbin USER="$USER" LOGNAME="$LOGNAME" TMPDIR="$TMPDIR" LANG=en_US.UTF-8 ~/.local/share/claude/versions/2.1.296 -p --model haiku --tools "" --output-format json --json-schema '{"type":"object","properties":{"n":{"type":"integer"}},"required":["n"],"additionalProperties":false}' --no-session-persistence --strict-mcp-config --mcp-config '{"mcpServers":{}}' --safe-mode --permission-prompts none --max-turns 1 --setting-sources ""`

Exit code: 0

```text
{"duration_api_ms":608,"stop_reason":"tool_use","session_id":"8c2c17c3-8969-4004-8950-a92b3d93b364","total_cost_usd":"<omitted: client-side estimate>","usage":{"input_tokens":2,"cache_creation_input_tokens":361,"cache_read_input_tokens":2919,"output_tokens":51,"output_tokens_details":{"thinking_tokens":0},"server_tool_use":{"web_search_requests":0,"web_fetch_requests":0},"service_tier":"standard","cache_creation":{"ephemeral_1h_input_tokens":361,"ephemeral_5m_input_tokens":0},"inference_geo":"global","iterations":[{"input_tokens":2,"output_tokens":51,"cache_read_input_tokens":2919,"cache_creation_input_tokens":361,"cache_creation":{"ephemeral_5m_input_tokens":0,"ephemeral_1h_input_tokens":361},"type":"message"}],"speed":"standard","fallback_credit":null},"modelUsage":{"claude-haiku-5-5":{"inputTokens":2,"outputTokens":51,"cacheReadInputTokens":2919,"cacheCreationInputTokens":361,"webSearchRequests":0,"costUSD":"<omitted: client-side estimate>","contextWindow":1000000,"maxOutputTokens":128000,"thinkingTokens":0,"canonicalModel":"claude-haiku-5-5","provider":"firstParty","costBasis":"list"}},"permission_denials":[],"terminal_reason":"completed","fast_mode_state":"off","fast_mode_disabled_reason":"sdk_opt_in_required","subagent_stats":{"spawned":0,"requested":{"background":0,"foreground":0,"unset":0},"started_in_background":0,"max_depth":0,"spawned_by_subagents":0,"completed":0,"failed":0,"killed":{"parent":0,"user":0,"system":0},"refused":{"depth_limit":0,"concurrency_limit":0,"budget":0},"by_type":{}},"safety_stops":0,"is_error":false,"num_turns":2,"subtype":"success","api_error_status":null,"result":"{\"n\":7}","structured_output":{"n":7},"ttft_ms":612,"type":"result","duration_ms":645,"uuid": "<redacted>","ttft_stream_ms":469,"time_to_request_ms":34,"first_content_frame_ms":469,"queued_turn_count":0,"result_index":0}
```

Stderr: empty (0 bytes).

- Accepted: exit 0, no stderr. Auth still works (a model reply came back; `modelUsage.claude-haiku-5-5.provider` is `"firstParty"`).
- Result unchanged: `subtype` `success`, `structured_output` `{"n":7}`, `num_turns` `2`; every JSON key path is the same as in C-03.

### C-10: `--restricted`

Requirement: SPIKE-07. Model call 8 of at most 10.

Command: `cd /var/folders/_p/cht4w96x7kddklkb8f3v304w0000gn/T/tmp.Cw8LCtKjyE && printf 'Reply with the number 7.' | env -i HOME="$HOME" PATH=/usr/bin:/bin:/usr/sbin:/sbin USER="$USER" LOGNAME="$LOGNAME" TMPDIR="$TMPDIR" LANG=en_US.UTF-8 ~/.local/share/claude/versions/2.1.296 -p --model haiku --tools "" --output-format json --json-schema '{"type":"object","properties":{"n":{"type":"integer"}},"required":["n"],"additionalProperties":false}' --no-session-persistence --strict-mcp-config --mcp-config '{"mcpServers":{}}' --safe-mode --permission-prompts none --max-turns 1 --restricted`

Exit code: 0

```text
{"duration_api_ms":1042,"stop_reason":"tool_use","session_id":"bc97b170-9158-4205-b275-cda101dc3629","total_cost_usd":"<omitted: client-side estimate>","usage":{"input_tokens":2,"cache_creation_input_tokens":364,"cache_read_input_tokens":2919,"output_tokens":82,"output_tokens_details":{"thinking_tokens":30},"server_tool_use":{"web_search_requests":0,"web_fetch_requests":0},"service_tier":"standard","cache_creation":{"ephemeral_1h_input_tokens":364,"ephemeral_5m_input_tokens":0},"inference_geo":"global","iterations":[{"input_tokens":2,"output_tokens":82,"cache_read_input_tokens":2919,"cache_creation_input_tokens":364,"cache_creation":{"ephemeral_5m_input_tokens":0,"ephemeral_1h_input_tokens":364},"type":"message"}],"speed":"standard","fallback_credit":null},"modelUsage":{"claude-haiku-5-5":{"inputTokens":2,"outputTokens":82,"cacheReadInputTokens":2919,"cacheCreationInputTokens":364,"webSearchRequests":0,"costUSD":"<omitted: client-side estimate>","contextWindow":1000000,"maxOutputTokens":128000,"thinkingTokens":30,"canonicalModel":"claude-haiku-5-5","provider":"firstParty","costBasis":"list"}},"permission_denials":[],"terminal_reason":"completed","fast_mode_state":"off","fast_mode_disabled_reason":"sdk_opt_in_required","subagent_stats":{"spawned":0,"requested":{"background":0,"foreground":0,"unset":0},"started_in_background":0,"max_depth":0,"spawned_by_subagents":0,"completed":0,"failed":0,"killed":{"parent":0,"user":0,"system":0},"refused":{"depth_limit":0,"concurrency_limit":0,"budget":0},"by_type":{}},"safety_stops":0,"is_error":false,"num_turns":2,"subtype":"success","api_error_status":null,"result":"{\"n\":7}","structured_output":{"n":7},"ttft_ms":1042,"type":"result","duration_ms":1076,"uuid": "<redacted>","ttft_stream_ms":626,"time_to_request_ms":33,"first_content_frame_ms":626,"queued_turn_count":0,"result_index":0}
```

Stderr: empty (0 bytes).

- Accepted together with `--safe-mode` and `--strict-mcp-config`: exit 0, no stderr. Auth still works (`provider` `"firstParty"`).
- Result unchanged: `subtype` `success`, `structured_output` `{"n":7}`, `num_turns` `2`; every JSON key path is the same as in C-03.

### C-11: Unsatisfiable schema under `--max-turns 1`

Requirement: SPIKE-07. Model call 9 of at most 10. `{S2}` (`minimum` 10, `maximum` 5) in place of `{S1}`.

Command: `cd /var/folders/_p/cht4w96x7kddklkb8f3v304w0000gn/T/tmp.qII2dZKNNk && printf 'Reply with the number 7.' | env -i HOME="$HOME" PATH=/usr/bin:/bin:/usr/sbin:/sbin USER="$USER" LOGNAME="$LOGNAME" TMPDIR="$TMPDIR" LANG=en_US.UTF-8 ~/.local/share/claude/versions/2.1.296 -p --model haiku --tools "" --output-format json --json-schema '{"type":"object","properties":{"n":{"type":"integer","minimum":10,"maximum":5}},"required":["n"],"additionalProperties":false}' --no-session-persistence --strict-mcp-config --mcp-config '{"mcpServers":{}}' --safe-mode --permission-prompts none --max-turns 1`

Exit code: 1

```text
{"duration_api_ms":1712,"stop_reason":"tool_use","session_id":"9918cda8-7509-4817-b85d-1f3eddfca2f4","total_cost_usd":"<omitted: client-side estimate>","usage":{"input_tokens":2,"cache_creation_input_tokens":3297,"cache_read_input_tokens":0,"output_tokens":284,"output_tokens_details":{"thinking_tokens":232},"server_tool_use":{"web_search_requests":0,"web_fetch_requests":0},"service_tier":"standard","cache_creation":{"ephemeral_1h_input_tokens":3297,"ephemeral_5m_input_tokens":0},"inference_geo":"global","iterations":[{"input_tokens":2,"output_tokens":284,"cache_read_input_tokens":0,"cache_creation_input_tokens":3297,"cache_creation":{"ephemeral_5m_input_tokens":0,"ephemeral_1h_input_tokens":3297},"type":"message"}],"speed":"standard","fallback_credit":null},"modelUsage":{"claude-haiku-5-5":{"inputTokens":2,"outputTokens":284,"cacheReadInputTokens":0,"cacheCreationInputTokens":3297,"webSearchRequests":0,"costUSD":"<omitted: client-side estimate>","contextWindow":1000000,"maxOutputTokens":128000,"thinkingTokens":232,"canonicalModel":"claude-haiku-5-5","provider":"firstParty","costBasis":"list"}},"permission_denials":[],"terminal_reason":"max_turns","fast_mode_state":"off","fast_mode_disabled_reason":"sdk_opt_in_required","subagent_stats":{"spawned":0,"requested":{"background":0,"foreground":0,"unset":0},"started_in_background":0,"max_depth":0,"spawned_by_subagents":0,"completed":0,"failed":0,"killed":{"parent":0,"user":0,"system":0},"refused":{"depth_limit":0,"concurrency_limit":0,"budget":0},"by_type":{}},"safety_stops":0,"is_error":true,"num_turns":2,"subtype":"error_max_turns","errors":["Reached maximum number of turns (1)"],"type":"result","duration_ms":1751,"uuid": "<redacted>","queued_turn_count":0,"result_index":0}
```

Stderr: empty (0 bytes).

- Exit code `1`. `subtype`: `"error_max_turns"`; `is_error`: `true`; `num_turns`: `2`; `terminal_reason`: `"max_turns"`; `stop_reason`: `"tool_use"`; `errors`: `["Reached maximum number of turns (1)"]`.
- The result has no `structured_output` key and no `result` key. It has an `errors` array, which the success results do not.
- `usage.iterations` has 1 entry, equal to the top-level `usage` token counts (`input_tokens` 2, `output_tokens` 284).
- This does not show whether more turns would end in `error_max_structured_output_retries`, so C-12 runs once.

### C-12: Unsatisfiable schema under `--max-turns 3`

Requirement: SPIKE-07. Model call 10 of at most 10 (the last; no further model calls in this plan).

Command: `cd /var/folders/_p/cht4w96x7kddklkb8f3v304w0000gn/T/tmp.IcRbnaywkA && printf 'Reply with the number 7.' | env -i HOME="$HOME" PATH=/usr/bin:/bin:/usr/sbin:/sbin USER="$USER" LOGNAME="$LOGNAME" TMPDIR="$TMPDIR" LANG=en_US.UTF-8 ~/.local/share/claude/versions/2.1.296 -p --model haiku --tools "" --output-format json --json-schema '{"type":"object","properties":{"n":{"type":"integer","minimum":10,"maximum":5}},"required":["n"],"additionalProperties":false}' --no-session-persistence --strict-mcp-config --mcp-config '{"mcpServers":{}}' --safe-mode --permission-prompts none --max-turns 3`

Exit code: 1

```text
{"duration_api_ms":4082,"stop_reason":"tool_use","session_id":"8f33fed8-a870-442e-9821-04b293979233","total_cost_usd":"<omitted: client-side estimate>","usage":{"input_tokens":8,"cache_creation_input_tokens":839,"cache_read_input_tokens":9866,"output_tokens":499,"output_tokens_details":{"thinking_tokens":295},"server_tool_use":{"web_search_requests":0,"web_fetch_requests":0},"service_tier":"standard","cache_creation":{"ephemeral_1h_input_tokens":839,"ephemeral_5m_input_tokens":0},"inference_geo":"global","iterations":[{"input_tokens":4,"output_tokens":125,"cache_read_input_tokens":3634,"cache_creation_input_tokens":140,"cache_creation":{"ephemeral_5m_input_tokens":0,"ephemeral_1h_input_tokens":140},"type":"message"}],"speed":"standard","fallback_credit":null},"modelUsage":{"claude-haiku-5-5":{"inputTokens":8,"outputTokens":499,"cacheReadInputTokens":9866,"cacheCreationInputTokens":839,"webSearchRequests":0,"costUSD":"<omitted: client-side estimate>","contextWindow":1000000,"maxOutputTokens":128000,"thinkingTokens":295,"canonicalModel":"claude-haiku-5-5","provider":"firstParty","costBasis":"list"}},"permission_denials":[],"terminal_reason":"max_turns","fast_mode_state":"off","fast_mode_disabled_reason":"sdk_opt_in_required","subagent_stats":{"spawned":0,"requested":{"background":0,"foreground":0,"unset":0},"started_in_background":0,"max_depth":0,"spawned_by_subagents":0,"completed":0,"failed":0,"killed":{"parent":0,"user":0,"system":0},"refused":{"depth_limit":0,"concurrency_limit":0,"budget":0},"by_type":{}},"safety_stops":0,"is_error":true,"num_turns":4,"subtype":"error_max_turns","errors":["Reached maximum number of turns (3)"],"type":"result","duration_ms":4158,"uuid": "<redacted>","queued_turn_count":0,"result_index":0}
```

Stderr: empty (0 bytes).

- Exit code `1`. `subtype`: `"error_max_turns"`; `is_error`: `true`; `num_turns`: `4`; `terminal_reason`: `"max_turns"`; `errors`: `["Reached maximum number of turns (3)"]`. No `structured_output` and no `result` key.
- `error_max_structured_output_retries` did not appear under `--max-turns 1` or `--max-turns 3`; in both calls the turn cap ended the run first.
- `usage.iterations` has 1 entry (`input_tokens` 4, `output_tokens` 125) while the top-level `usage` totals are `input_tokens` 8 and `output_tokens` 499, the same as `modelUsage.claude-haiku-5-5.inputTokens`/`outputTokens`. `iterations` does not sum to the totals in a multi-turn call; the top-level `usage` and `modelUsage` counts do agree with each other.
- In C-11 and C-12 `num_turns` is one more than `--max-turns` (2 under 1, 4 under 3), the same offset as the successful schema calls (2 under 1).

### C-13: `--help` text for the tested flags

Requirement: SPIKE-07. No model call. Truncation noted: the help output was captured whole to a scratch file; only the option blocks for the flags this plan uses are kept, selected with `awk '/^  -/{p = ($0 ~ /--(disable-slash-commands|disallowedTools|json-schema|mcp-config|no-session-persistence|output-format|permission-mode|permission-prompts|restricted|safe-mode|setting-sources|strict-mcp-config|tools)[ ,]/)} p'`. The last line of the block is `grep -c -- '--max-turns'` run on the same captured output.

Command: `cd /var/folders/_p/cht4w96x7kddklkb8f3v304w0000gn/T/tmp.T1wLMQK7ZZ && env -i HOME="$HOME" PATH=/usr/bin:/bin:/usr/sbin:/sbin USER="$USER" LOGNAME="$LOGNAME" TMPDIR="$TMPDIR" LANG=en_US.UTF-8 ~/.local/share/claude/versions/2.1.296 --help`

Exit code: 0

```text
  --disable-slash-commands              Disable all skills
  --disallowedTools, --disallowed-tools <tools...>
      Comma or space-separated list of tool names to deny (e.g. "Bash(git *)
      Edit")
  --json-schema <schema>                JSON Schema for structured output
                                        validation. Example:
                                        {"type":"object","properties":{"name":{"type":"string"}},"required":["name"]}
  --mcp-config <configs...>             Load MCP servers from JSON files or
                                        strings (space-separated)
  --no-session-persistence              Disable session persistence - sessions
                                        will not be saved to disk and cannot be
                                        resumed (only works with --print)
  --output-format <format>              Output format (only works with --print):
                                        "text" (default), "json" (single
                                        result), or "stream-json" (realtime
                                        streaming) (choices: "text", "json",
                                        "stream-json")
  --permission-mode <mode>              Permission mode to use for the session
                                        (choices: "acceptEdits", "auto",
                                        "bypassPermissions", "manual",
                                        "dontAsk", "plan")
  --permission-prompts <target>         Who answers permission prompts with
                                        --print: "host" (the SDK host or
                                        --permission-prompt-tool) or "none"
                                        (nobody: anything that would prompt is
                                        denied automatically; the permission
                                        mode still decides everything else)
                                        (choices: "host", "none", default:
                                        "host")
  --restricted                          Restricted mode: removes the built-in
                                        tools that run commands or code (Bash,
                                        PowerShell, REPL and the other
                                        code-running tools) and WebFetch unless
                                        --tools names them, and ignores user,
                                        project and local settings files
                                        (managed settings and --settings still
                                        apply; add --strict-mcp-config to skip
                                        MCP servers too). Also confines the file
                                        tools to the working directories
                                        (--add-dir included), refuses
                                        bypassPermissions, and lets only a
                                        person or the configured permission
                                        handler approve writes to settings, git
                                        and tool-configuration files.
  --safe-mode                           Start with all customizations
                                        (CLAUDE.md, skills, installed plugins,
                                        hooks, MCP servers, custom commands and
                                        agents, output styles, workflows, custom
                                        themes, keybindings, and more) disabled
                                        — useful for troubleshooting a broken
                                        configuration. Admin-managed (policy)
                                        settings still apply. Auth, model
                                        selection, built-in tools and plugins,
                                        and permissions work normally. Sets
                                        CLAUDE_CODE_SAFE_MODE=1.
  --setting-sources <sources>           Comma-separated list of setting sources
                                        to load (user, project, local).
  --strict-mcp-config                   Only use MCP servers from --mcp-config,
                                        ignoring all other MCP configurations
  --tools <tools...>                    Specify the list of available tools from
                                        the built-in set. Use "" to disable all
                                        tools, "default" to use all tools, or
                                        specify tool names (e.g.
                                        "Bash,Edit,Read").
0
```

Stderr: empty (0 bytes).

- `--max-turns` does not appear in the help text, yet C-03 to C-12 show it is accepted and enforced (`errors: ["Reached maximum number of turns (1)"]` in C-11).
- `--permission-prompts` is documented as applying "with --print"; `--permission-mode` lists `dontAsk` among its choices.
- `--safe-mode` text: "Auth, model selection, built-in tools and plugins, and permissions work normally", matching the three builtin plugins still listed in C-04.

### C-14: No session directory for any spike work directory

Requirement: SPIKE-07 (`--no-session-persistence`). No model call. The pattern is the random suffix of each `mktemp -d` directory used for C-03 to C-12; `~/.claude/projects` is the `projectsDirectory` that C-02 printed.

Command: `cd /var/folders/_p/cht4w96x7kddklkb8f3v304w0000gn/T/tmp.OIhpsisTnq && ls ~/.claude/projects | grep -cE 'NMIH2pskvK|uSStOpkyJg|Xab8kwy82g|zJR1E9vJuj|3zZGOUmQKr|psTpEcKBUU|jLvts71G3M|Cw8LCtKjyE|qII2dZKNNk|IcRbnaywkA'`

Exit code: 1

```text
0
```

Stderr: empty (0 bytes).

- Zero entries under `~/.claude/projects` name any of the ten work directories (grep exit 1 means no match). Only directory names were listed; no file was opened.
- There was no control call without `--no-session-persistence` (the 10-call budget was spent), so this shows the flag was accepted and nothing landed in `projectsDirectory`; it does not show that the CLI would have written there without the flag.

### SPIKE-07 contract

| Item | Observed | Evidence |
|---|---|---|
| Canonical binary path and version | `realpath ~/.local/bin/claude` gives `~/.local/share/claude/versions/2.1.296`, which prints `2.1.296 (Claude Code)`. The first `claude` on the inherited PATH is a cmux bash shim under `$TMPDIR/cmux-cli-shims/`; it is never executed. | C-01 |
| Env allowlist works with subscription auth | Yes. Under `env -i` with only `HOME`, `PATH=/usr/bin:/bin:/usr/sbin:/sbin`, `USER`, `LOGNAME`, `TMPDIR`, `LANG=en_US.UTF-8`: `loggedIn` `true`, `authMethod` `"claude.ai"`, `apiProvider` `"firstParty"`, `subscriptionType` `"max"`; `apiKeySource` `"none"` in `system`/`init`; all ten model calls returned a `result` with non-zero `usage.output_tokens`. | C-02, C-04, C-03 to C-12 |
| Work directory | A fresh `mktemp -d` directory under `$TMPDIR`, outside any repository, one per call; `system`/`init` `cwd` reports it. | C-03 to C-12 |
| Prompt delivery | On stdin (`printf '...' \| ...`) with no positional prompt. | C-03 |
| Base argv | `-p --model haiku --tools "" --output-format json --json-schema '<schema>' --no-session-persistence --strict-mcp-config --mcp-config '{"mcpServers":{}}' --safe-mode --permission-prompts none --max-turns 1`: exit 0, one JSON object on stdout, empty stderr. | C-03 |
| `structured_output` shape | Top-level key `structured_output` holds the schema object as JSON (`{"n":7}`); `result` holds the same object as a JSON string (`"{\"n\":7}"`). Both keys are absent from error results, which carry `errors: [string]` instead. | C-03, C-11, C-12 |
| `subtype` values seen | `"success"` (C-03 to C-10) and `"error_max_turns"` (C-11, C-12). Not seen: `error_max_structured_output_retries`, `error_during_execution`, `error_max_budget_usd`. | C-03 to C-12 |
| `is_error` and exit code | `success`: `is_error` `false`, exit 0. `error_max_turns`: `is_error` `true`, exit 1. Stderr was empty in every call. | C-03, C-11, C-12 |
| `num_turns` under `--max-turns 1`, with a schema | `2`, `subtype` `success`, `stop_reason` `"tool_use"` (structured output goes through the `StructuredOutput` tool). | C-03, C-04, C-05, C-07 to C-10 |
| `num_turns` under `--max-turns 1`, without a schema | `1`, `subtype` `success`, `stop_reason` `"end_turn"`, `result` `"ok"`, no `structured_output` key. | C-06 |
| `usage` field names | `input_tokens`, `cache_creation_input_tokens`, `cache_read_input_tokens`, `output_tokens`, `output_tokens_details.thinking_tokens`, `server_tool_use.{web_search_requests, web_fetch_requests}`, `service_tier`, `cache_creation.{ephemeral_1h_input_tokens, ephemeral_5m_input_tokens}`, `inference_geo`, `iterations[].{input_tokens, output_tokens, cache_read_input_tokens, cache_creation_input_tokens, cache_creation.{ephemeral_5m_input_tokens, ephemeral_1h_input_tokens}, type}`, `speed`, `fallback_credit`. In a multi-turn call `iterations` held one entry that did not sum to the top-level totals; the top-level totals equal `modelUsage`. | C-03, C-12 |
| `modelUsage` field names | One key per resolved model id (`claude-haiku-5-5` for alias `haiku`), each with `inputTokens`, `outputTokens`, `cacheReadInputTokens`, `cacheCreationInputTokens`, `webSearchRequests`, `costUSD` (estimate, never recorded), `contextWindow`, `maxOutputTokens`, `thinkingTokens`, `canonicalModel`, `provider`, `costBasis`. | C-03 |
| Tools list under `--tools ""` | With `--json-schema`: `["StructuredOutput"]`. Without: `[]`. Agent, Skill, and every other built-in tool are absent in both. `mcp_servers` `[]`. | C-04, C-06 |
| Slash commands and skills under `--disable-slash-commands` | `slash_commands` `[]` and `skills` `[]` (55 and 19 without the flag); `terminal_slash_commands` absent. `agents` still lists 4 entries, but no Agent tool exists. Result unchanged. | C-04, C-05 |
| `--disallowedTools` | `--disallowedTools "mcp__*"` accepted alongside `--tools ""`; exit 0, result unchanged (same key paths as C-03). | C-07 |
| Permission flag | Both work with `-p`. `--permission-prompts none` leaves `permissionMode` `"default"`; `--permission-mode dontAsk` sets it to `"dontAsk"`. Results identical. No call made a permission request, so the behavioral difference between them is `open` (not exercised with tools off). | C-04, C-08, C-13 |
| `--setting-sources ""` | Accepted; auth works; result unchanged. | C-09 |
| `--restricted` | Accepted together with `--safe-mode` and `--strict-mcp-config`; auth works; result unchanged. | C-10 |
| `--safe-mode` | Accepted; three builtin plugins (`cc-plugin-agents-md`, `cc-plugin-telemetry`, `cc-plugin-plugin-authoring`) are still listed in `system`/`init`, as its help text says. | C-04, C-13 |
| `--strict-mcp-config` with an empty config | `mcp_servers` `[]`. | C-04 |
| `--max-turns` | Not in `--help`, but accepted and enforced (`errors: ["Reached maximum number of turns (1)"]`). At the cap `num_turns` reads one more than the flag value. | C-11, C-12, C-13 |
| `--no-session-persistence` | Accepted. No directory under `~/.claude/projects` names any of the ten work directories. Whether the CLI writes one without the flag is `open` (no control call; budget spent). | C-03, C-14 |
| Unsatisfiable-schema behavior | `--max-turns 1`: exit 1, `subtype` `error_max_turns`, `is_error` `true`, `num_turns` 2, `terminal_reason` `"max_turns"`, no `structured_output`. `--max-turns 3`: the same subtype and exit, `num_turns` 4. The turn cap ends the run before any structured-output retry limit shows. | C-11, C-12 |
| Model calls made by this plan | 10 (C-03 to C-12), all `haiku`. | C-03 to C-12 |

### Research claims checked

02-RESEARCH.md Pattern 7 (and Pitfall 9) claims, each against this plan's own calls. The research files stay as written (D-09); 02-10 carries any difference into CLAUDE.md.

| Claim (02-RESEARCH.md) | Status | Evidence |
|---|---|---|
| The base argv gives exit 0, `subtype` `success`, `is_error` `false`, `structured_output` `{"n":7}`, `result` `"{\"n\":7}"` | confirmed | C-03 |
| `num_turns` is 2 with `--max-turns 1` | confirmed | C-03 |
| `stop_reason` `tool_use`, `terminal_reason` `completed`, `permission_denials` `[]` | confirmed | C-03 |
| Usage field names `input_tokens`, `output_tokens`, `cache_creation_input_tokens`, `cache_read_input_tokens`, `output_tokens_details.thinking_tokens`, `server_tool_use`, `service_tier`, `cache_creation.{ephemeral_1h_input_tokens, ephemeral_5m_input_tokens}`, `iterations[]` | differs: all present, plus `inference_geo`, `speed`, and `fallback_credit`, which the research list omits | C-03 |
| `modelUsage.<model-id>` field names (12 keys, including `costUSD`) | confirmed | C-03 |
| The `haiku` alias resolves to `claude-haiku-5-5` | confirmed | C-03, C-04 |
| `system`/`init` `tools` is `["StructuredOutput"]`; `--tools ""` removes Agent, Skill, and every built-in tool | confirmed | C-04 |
| `mcp_servers` `[]`, `apiKeySource` `"none"`, `permissionMode` `"default"` | confirmed | C-04 |
| `skills`, `slash_commands`, and `agents` are still listed | confirmed | C-04 |
| `--disable-slash-commands` should empty them | differs: it empties `slash_commands` and `skills`, but `agents` is still listed | C-05 |
| Three builtin plugins are listed under `--safe-mode` | confirmed | C-04 |
| `claude auth status` under the same environment: `loggedIn` `true`, `authMethod` `claude.ai`, `apiProvider` `firstParty`, `subscriptionType` `max`, and it prints email and orgId | confirmed (it also prints `orgName`, redacted here) | C-02 |
| `--max-turns` is absent from `claude --help` for 2.1.296 but accepted | confirmed | C-13, C-03 |
| `--tools ""` without `--json-schema` gives `tools: []` (expectation) | confirmed | C-06 |
| A schema the model fails shows `error_max_structured_output_retries` (expectation) | differs: `error_max_turns` under `--max-turns 1` and `--max-turns 3`; the retries subtype did not appear | C-11, C-12 |
| Pitfall 9: `command -v claude` is a cmux shim; `~/.local/bin/claude` resolves to `~/.local/share/claude/versions/2.1.296` | confirmed | C-01 |

## Plan 02-03: minos-vmm launcher

Date: 2026-10-10

- `rustc --version`: rustc 1.97.1 (8bab26f4f 2026-07-14)
- `cargo --version`: cargo 1.97.1 (c980f4866 2026-06-30)
- `cargo clippy --version`: clippy 0.1.97 (8bab26f4f6 2026-07-14)
- `cargo deny --version`: cargo-deny 0.20.2
- macOS: 27.0, arm64
- HEAD at start: `80a220df03439c20a0f2fd6f73ecae72cd0d2790`

Redactions: the absolute home directory path is shown as `~`. Nothing else is altered. Cargo progress lines are removed.

In the commands below, `S` is the session scratch directory
`/private/tmp/claude-501/-Users-seanmcauliffe-Repos-minos/79087d67-27f5-41bb-a90a-460fa65ed311/scratchpad/vexit`. The
output shows it expanded, as printed.

### V-EXIT: pre-takeover exit codes

Requirement: SPIKE-04 (D-05, K7). No VM is started and no entitlement is needed: every case exits before
`launch::build`.

Setup: `cargo build -p minos-vmm` (exit 0). `$S/e1.json` holds E-1 exactly (566 bytes, no trailing newline,
SHA-256 `30d25bdbd586de10a5f9394306fdb9cb39132fb9d021c82e47a2cfcbcc83d895`). `$S/e1-extra-key.json` is E-1 with
`"v":1,` replaced by `"v":1,"x":1,`. `$S/does-not-exist.json` does not exist. `env | grep -cE '^(MSB_KRUN_|DYLD_)'`
printed `0`, and `/private/tmp/mns00000000` does not exist.

Command: `target/debug/minos-vmm; echo "exit: $?"`

Exit code: 64

```text
minos-vmm: usage: minos-vmm <config.json>
exit: 64
```

Command: `target/debug/minos-vmm "$S/does-not-exist.json"; echo "exit: $?"`

Exit code: 65

```text
minos-vmm: error: reading launch config /private/tmp/claude-501/-Users-seanmcauliffe-Repos-minos/79087d67-27f5-41bb-a90a-460fa65ed311/scratchpad/vexit/does-not-exist.json: No such file or directory (os error 2)
exit: 65
```

Command: `target/debug/minos-vmm "$S/e1-extra-key.json"; echo "exit: $?"`

Exit code: 65

```text
minos-vmm: error: launch config /private/tmp/claude-501/-Users-seanmcauliffe-Repos-minos/79087d67-27f5-41bb-a90a-460fa65ed311/scratchpad/vexit/e1-extra-key.json: config is not valid launch config JSON: unknown field `x`, expected one of `v`, `kernel`, `boot`, `disks`, `vsock`, `console`, `max_duration_ms`, `machine` at line 1 column 10
exit: 65
```

Command: `target/debug/minos-vmm "$S/e1.json"; echo "exit: $?"`

Exit code: 66 (libkrunfw at the E-1 path exists, so preflight reaches `disks.root`, which does not)

```text
minos-vmm: error: preflight: disks.root /private/tmp/mns00000000/root.img: No such file or directory (os error 2)
exit: 66
```

Extra case, not required by the plan: two arguments.

Command: `target/debug/minos-vmm a b; echo "exit: $?"`

Exit code: 64

```text
minos-vmm: usage: minos-vmm <config.json>
exit: 64
```

### C-03c: every minos-vmm disallowed path resolves

Requirement: SPIKE-02 precondition (KRUN-02 direction), T-02-14, T-02-15, T-02-16, T-02-17, T-02-18

Edit: `crates/minos-vmm/clippy.toml` written as E-5 (13 `disallowed-methods` entries, each with a `reason`). No
`clippy.toml` or `.clippy.toml` exists at the repository root. `crates/minos-vmm/src/main.rs` was touched (mtime only)
so clippy re-checks the crate.

Command: `cargo clippy -p minos-vmm --all-targets -- -D warnings > "$S/../c03c.txt" 2>&1; echo "exit: $?"; grep -ciE 'unknown path|does not refer|could not resolve|invalid path' "$S/../c03c.txt"; cat "$S/../c03c.txt"`

Exit code: 0

```text
exit: 0
0
    Checking minos-vmm v0.1.0 (~/Repos/minos/crates/minos-vmm)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.19s
```

### D-PROBE: each of the thirteen bans fires

Requirement: SPIKE-02 precondition (KRUN-02 direction), T-02-14, T-02-15, T-02-16, T-02-17, T-02-18

Edit: a throwaway `#[cfg(test)] mod disallowed_probe` with one `#[test] fn every_banned_method_fires()` appended to
`crates/minos-vmm/src/launch.rs` (a test function, so it is not dead code). It calls each banned method once on a fresh
builder and passes the result to `std::hint::black_box`: `VmBuilder::new().fs(|fs| fs)`,
`FsBuilder::new().root("/probe")`, `FsBuilder::new().path("/probe")`, `MachineBuilder::new().enable_inet_hijack(true)`,
`VsockBuilder::new().inet_hijack(true)`, `VsockBuilder::new().tcp_listen_remap(1, 2)`,
`DiskBuilder::new().format(DiskImageFormat::Raw)`, `DiskBuilder::new().layers(Vec::<DiskLayer>::new())`,
`ExecBuilder::new().env("A", "B")`, `ExecBuilder::new().envs([("A", "B")])`, `ConsoleBuilder::new().port("probe", 0, 1)`,
and `ConsoleBuilder::new().port_tty("probe", 0)`. `FsBuilder::custom` is referenced as a path expression bound to a
variable (`let custom = msb_krun::FsBuilder::custom;`), with no call; that form fired, so no call was built. Every
probe reaches the methods through the `msb_krun::` root re-exports, and every ban is written with the definition path
(`msb_krun::api::builder` or `msb_krun::api::builders`); no entry needed the re-export form. The probe uses no unwrap,
expect, indexing, unsafe, or allow.

Command: `cargo clippy -p minos-vmm --all-targets -- -D warnings > "$S/../dprobe.txt" 2>&1; echo "exit: $?"; grep '^error' "$S/../dprobe.txt" | sort | uniq -c`

Exit code: 101

```text
exit: 101
   1 error: could not compile `minos-vmm` (bin "minos-vmm" test) due to 13 previous errors
   1 error: use of a disallowed method `msb_krun::api::builder::VmBuilder::fs`
   1 error: use of a disallowed method `msb_krun::api::builders::ConsoleBuilder::port_tty`
   1 error: use of a disallowed method `msb_krun::api::builders::ConsoleBuilder::port`
   1 error: use of a disallowed method `msb_krun::api::builders::DiskBuilder::format`
   1 error: use of a disallowed method `msb_krun::api::builders::DiskBuilder::layers`
   1 error: use of a disallowed method `msb_krun::api::builders::ExecBuilder::env`
   1 error: use of a disallowed method `msb_krun::api::builders::ExecBuilder::envs`
   1 error: use of a disallowed method `msb_krun::api::builders::FsBuilder::custom`
   1 error: use of a disallowed method `msb_krun::api::builders::FsBuilder::path`
   1 error: use of a disallowed method `msb_krun::api::builders::FsBuilder::root`
   1 error: use of a disallowed method `msb_krun::api::builders::MachineBuilder::enable_inet_hijack`
   1 error: use of a disallowed method `msb_krun::api::builders::VsockBuilder::inet_hijack`
   1 error: use of a disallowed method `msb_krun::api::builders::VsockBuilder::tcp_listen_remap`
```

Thirteen distinct paths, one error each, and no other error. The `FsBuilder::custom` error in full shows the path
form firing and the reason text from `clippy.toml`:

Command: `grep -A9 'FsBuilder::custom`' "$S/../dprobe.txt"`

Exit code: 0

```text
error: use of a disallowed method `msb_krun::api::builders::FsBuilder::custom`
  --> crates/minos-vmm/src/launch.rs:68:22
   |
68 |         let custom = msb_krun::FsBuilder::custom;
   |                      ^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = note: no virtio-fs device of any kind (D-18)
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_methods
   = note: `-D clippy::disallowed-methods` implied by `-D warnings`
   = help: to override `-D warnings` add `#[allow(clippy::disallowed_methods)]`
```

Revert: `git checkout -- crates/minos-vmm/src/launch.rs`, then `git diff --quiet -- crates/minos-vmm/src` (exit 0). The
probe was never committed.

Command: `cargo clippy -p minos-vmm --all-targets -- -D warnings; echo "exit: $?"`

Exit code: 0

```text
    Checking minos-vmm v0.1.0 (~/Repos/minos/crates/minos-vmm)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.18s
exit: 0
```

## Plan 02-04: root image and initramfs writers, image production

Date: 2026-10-10

- `rustc --version`: rustc 1.97.1 (8bab26f4f 2026-07-14)
- `cargo --version`: cargo 1.97.1 (c980f4866 2026-06-30)
- `cargo clippy --version`: clippy 0.1.97 (8bab26f4f6 2026-07-14)
- `cargo deny --version`: cargo-deny 0.20.2
- macOS: 27.0, arm64
- HEAD at start: `88c196114c363390331b1ea2ce64b23094f1e997`

Redactions: the absolute home directory path is shown as `~`. Nothing else is altered. Cargo progress lines are removed.

In the commands below, `S` is the session scratch directory
`/private/tmp/claude-501/-Users-seanmcauliffe-Repos-minos/79087d67-27f5-41bb-a90a-460fa65ed311/scratchpad/0204`. The
output shows it expanded, as printed.

### U-PACK: root image packer unit tests

Requirement: SPIKE-06 (D-01, D-16, D-17), T-02-22, T-02-23, T-02-24, T-02-25, T-02-26

Edit: `crates/minos-backend-krun/src/rootfs.rs` at commit `cd743ed` (RED `f960095`, all 21 failing against a stub).
The golden test compares `minimal_root_image(b"minos-golden-init")` against the frozen literal
`8e4f12d45a06ef4bf20fe0f37e74ca06e2b7b108a8c4325ed2c7f6a6600e4e98`; it was run in two separate test processes after
freezing and passed both times.

Command: `cargo test -p minos-backend-krun rootfs`

Exit code: 0

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.24s
     Running unittests src/lib.rs (target/debug/deps/minos_backend_krun-6e3ba37808cb60b8)

running 21 tests
test rootfs::tests::rootfs_non_utf8_name_is_rejected ... ok
test rootfs::tests::rootfs_hardlink_to_missing_target_is_rejected ... ok
test rootfs::tests::rootfs_absolute_path_is_rejected ... ok
test rootfs::tests::rootfs_nul_in_name_is_rejected ... ok
test rootfs::tests::rootfs_character_device_is_rejected ... ok
test rootfs::tests::rootfs_fifo_is_rejected ... ok
test rootfs::tests::rootfs_parent_dir_component_is_rejected ... ok
test rootfs::tests::rootfs_regular_file_at_proc_is_rejected ... ok
test rootfs::tests::rootfs_oversized_tar_is_rejected ... ok
test rootfs::tests::rootfs_hardlink_copies_count_against_the_content_cap ... ok
test rootfs::tests::rootfs_oversized_entry_is_rejected_before_reading ... ok
test rootfs::tests::rootfs_too_many_entries_is_rejected ... ok
test rootfs::tests::rootfs_minimal_root_image_golden ... ok
test rootfs::tests::rootfs_setid_bits_are_counted_and_kept ... ok
test rootfs::tests::rootfs_tar_supplied_init_directory_is_replaced ... ok
test rootfs::tests::rootfs_tar_supplied_init_is_replaced ... ok
test rootfs::tests::rootfs_hardlink_becomes_a_copy ... ok
test rootfs::tests::rootfs_symlink_keeps_its_target ... ok
test rootfs::tests::rootfs_image_has_trusted_init_and_mount_points ... ok
test rootfs::tests::rootfs_same_tar_packs_to_identical_bytes ... ok
test rootfs::tests::rootfs_tar_mtimes_do_not_change_the_image ... ok

test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

```

### C-03d: every minos-backend-krun disallowed path resolves

Requirement: SPIKE-06 precondition, T-02-22, T-02-26

Edit: `crates/minos-backend-krun/clippy.toml` written as E-1 (4 `disallowed-methods` entries, each with a `reason`).
No `clippy.toml` or `.clippy.toml` exists at the repository root. `crates/minos-backend-krun/src/lib.rs` was touched
(mtime only) so clippy re-checks the crate.

Command: `cargo clippy -p minos-backend-krun --all-targets -- -D warnings > "$S/c03d.txt" 2>&1; echo "exit: $?"; grep -ciE 'unknown path|does not refer|could not resolve|invalid path' "$S/c03d.txt"; cat "$S/c03d.txt"`

Exit code: 0

```text
exit: 0
0
    Checking minos-backend-krun v0.1.0 (~/Repos/minos/crates/minos-backend-krun)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.15s
```

### D-PROBE-BK: each of the four bans fires

Requirement: SPIKE-06 precondition, T-02-22, T-02-26

Setup: run on the committed tree (`git status --porcelain -- crates/minos-backend-krun/src/rootfs.rs
crates/minos-backend-krun/clippy.toml` printed nothing; HEAD `cd743ed`).

Edit: a throwaway `#[cfg(test)] mod disallowed_probe` with one `#[test] fn every_banned_method_fires() ->
std::io::Result<()>` appended to `crates/minos-backend-krun/src/rootfs.rs`. It calls
`tar::Archive::new(std::io::empty()).unpack(dir)?`, then, inside a loop over a second archive's `entries()?`,
`entry.unpack(dir.join("x"))?` and `entry.unpack_in(dir)?` (each through `std::hint::black_box`), then
`backhand::FilesystemWriter::default().set_current_time()`. Every call goes through the public re-exports
(`tar::Archive`, `tar::Entry`, `backhand::FilesystemWriter`), and every ban is written with the definition path; all
four resolved, so no alternative path was needed. The probe uses no unwrap, expect, indexing, unsafe, or allow.

Command: `cargo clippy -p minos-backend-krun --all-targets -- -D warnings > "$S/dprobe.txt" 2>&1; echo "exit: $?"; grep '^error' "$S/dprobe.txt" | sort | uniq -c`

Exit code: 101

```text
exit: 101
   1 error: could not compile `minos-backend-krun` (lib test) due to 4 previous errors
   1 error: use of a disallowed method `backhand::v4::filesystem::writer::FilesystemWriter::set_current_time`
   1 error: use of a disallowed method `tar::Archive::unpack`
   1 error: use of a disallowed method `tar::Entry::unpack_in`
   1 error: use of a disallowed method `tar::Entry::unpack`
```

Four distinct paths, one error each, and no other error. The clock ban in full, with the reason text from
`clippy.toml`:

Command: `grep -A9 'set_current_time`' "$S/dprobe.txt"`

Exit code: 0

```text
error: use of a disallowed method `backhand::v4::filesystem::writer::FilesystemWriter::set_current_time`
    --> crates/minos-backend-krun/src/rootfs.rs:1205:12
     |
1205 |         fs.set_current_time();
     |            ^^^^^^^^^^^^^^^^
     |
     = note: reads the clock; images are deterministic with mtime 0
     = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#disallowed_methods

error: could not compile `minos-backend-krun` (lib test) due to 4 previous errors
```

Revert and re-check:

Command: `git checkout -- crates/minos-backend-krun/src/rootfs.rs && cargo clippy -p minos-backend-krun --all-targets -- -D warnings > "$S/dprobe-after.txt" 2>&1; echo "exit: $?"; cat "$S/dprobe-after.txt"; git diff -- crates/minos-backend-krun/src/rootfs.rs | wc -l`

Exit code: 0

```text
exit: 0
    Checking minos-backend-krun v0.1.0 (~/Repos/minos/crates/minos-backend-krun)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.12s
       0
```

### IR-01: the newc writer agrees with macOS cpio

Requirement: SPIKE-01 (D-17; D-18 negative-evidence input for 02-06)

Edit: `crates/minos-backend-krun/src/initramfs.rs` at commit `48774bb` (RED `61d6a54`). `cargo test -p
minos-backend-krun initramfs` passes 4 tests and ignores the writer below. The golden test compares
`initramfs_image(b"hello")` against the frozen literal
`6b2629df82fc94f9798c3853a508935c6a448a7e48fabd58e4e730a99148ad1a`; it was run in two separate test processes after
freezing and passed both times.

Setup: `cargo build -p minos-guest --release --target aarch64-unknown-linux-musl` (exit 0, nothing rebuilt).
`G` is `~/Repos/minos/target/aarch64-unknown-linux-musl/release/minos-guest`, 1,022,016 bytes. `$S/ir01` was created
empty.

Command: `MINOS_GUEST="$G" MINOS_SPIKE_OUT="$S/ir01/initramfs.cpio" cargo test -p minos-backend-krun --lib initramfs_write_for_cpio_cross_check -- --ignored`

Exit code: 0

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.04s
     Running unittests src/lib.rs (target/debug/deps/minos_backend_krun-6e3ba37808cb60b8)

running 1 test
test initramfs::tests::initramfs_write_for_cpio_cross_check ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 25 filtered out; finished in 0.00s

```

Command: `wc -c < "$S/ir01/initramfs.cpio"; shasum -a 256 "$S/ir01/initramfs.cpio"; cpio --version`

Exit code: 0

```text
 1022464
02ba5c8f3a26fec1cdee9599ea499d416fe7ad01dd9be3e61c11cb5e0e922aee  /private/tmp/claude-501/-Users-seanmcauliffe-Repos-minos/79087d67-27f5-41bb-a90a-460fa65ed311/scratchpad/0204/ir01/initramfs.cpio
bsdcpio 3.5.3 - libarchive 3.7.4 zlib/1.2.12 liblzma/5.4.3 bz2lib/1.0.8 
```

Command: `cpio -itv < "$S/ir01/initramfs.cpio" 2>&1`

Exit code: 0

```text
1997 blocks
-rwxr-xr-x   1 root     wheel     1022016 Dec 31  1969 init
```

One file entry, `init`, mode `-rwxr-xr-x`, owner `root`/`wheel` (uid 0, gid 0), mtime 0 (shown in local time as
Dec 31 1969). `1997 blocks` is cpio's stderr count of 512-byte blocks (1,022,464 / 512 = 1997).

Command: `mkdir "$S/ir01/extract2" && cd "$S/ir01/extract2" && cpio -id < "$S/ir01/initramfs.cpio" 2>&1; ls; shasum -a 256 init; shasum -a 256 "$G"`

Exit code: 0

```text
1997 blocks
init
10e59803c53ce60bb53445f95b86e07652a5029c5601170823f1fe156e4b3a89  init
10e59803c53ce60bb53445f95b86e07652a5029c5601170823f1fe156e4b3a89  ~/Repos/minos/target/aarch64-unknown-linux-musl/release/minos-guest
```

The extracted `init` and the guest binary have the same SHA-256
(`10e59803c53ce60bb53445f95b86e07652a5029c5601170823f1fe156e4b3a89`, the 02-01 G-GUEST value). The extract
directory holds nothing else. An earlier run of the same `cpio -itv` and `cpio -id` commands into `$S/ir01/extract`,
made while drafting this item, printed the same listing and the same two hashes.

### RI-01: base image pinned by index digest

Requirement: SPIKE-06 (D-14), T-02-27

Command: `docker buildx imagetools inspect docker.io/library/alpine:3`

Exit code: 0

```text
Name:      docker.io/library/alpine:3
MediaType: application/vnd.oci.image.index.v1+json
Digest:    sha256:294b683cb724975bec92580e1e685676bd4b50bda910ddb8c51d4cabeaec77e6
           
Manifests: 
  Name:        docker.io/library/alpine:3@sha256:d56c381f961d307a21b3ca004cf1e3910f106644aefb1f43e654c8a56c4fd395
  MediaType:   application/vnd.oci.image.manifest.v1+json
  Platform:    linux/amd64
  Annotations: 
    org.opencontainers.image.version:         3.24.2
    com.docker.official-images.bashbrew.arch: amd64
    org.opencontainers.image.base.name:       scratch
    org.opencontainers.image.created:         2026-09-17T20:36:57Z
    org.opencontainers.image.revision:        1c744e2d49059e51b063a11a6e3e18c9ccf04ab8
    org.opencontainers.image.source:          https://github.com/alpinelinux/docker-alpine.git#1c744e2d49059e51b063a11a6e3e18c9ccf04ab8:x86_64
    org.opencontainers.image.url:             https://hub.docker.com/_/alpine
               
  Name:        docker.io/library/alpine:3@sha256:350f747a86e8774d359a4f452dec12a2585e73bf072e71c5f628455843b88c71
  MediaType:   application/vnd.oci.image.manifest.v1+json
  Platform:    unknown/unknown
  Annotations: 
    com.docker.official-images.bashbrew.arch: amd64
    vnd.docker.reference.digest:              sha256:d56c381f961d307a21b3ca004cf1e3910f106644aefb1f43e654c8a56c4fd395
    vnd.docker.reference.type:                attestation-manifest
               
  Name:        docker.io/library/alpine:3@sha256:d3c3fda3e4d3f0fcfc86cf469c6dfbe65017143dde23feb4b11df37c373cdc76
  MediaType:   application/vnd.oci.image.manifest.v1+json
  Platform:    linux/arm/v6
  Annotations: 
    org.opencontainers.image.base.name:       scratch
    org.opencontainers.image.created:         2026-09-17T20:37:27Z
    org.opencontainers.image.revision:        1c744e2d49059e51b063a11a6e3e18c9ccf04ab8
    org.opencontainers.image.source:          https://github.com/alpinelinux/docker-alpine.git#1c744e2d49059e51b063a11a6e3e18c9ccf04ab8:armhf
    org.opencontainers.image.url:             https://hub.docker.com/_/alpine
    org.opencontainers.image.version:         3.24.2
    com.docker.official-images.bashbrew.arch: arm32v6
               
  Name:        docker.io/library/alpine:3@sha256:e1ca081cf7a9d7260dca29eedf497144adacc81cbdf0afd3f7402f7df5c5c12e
  MediaType:   application/vnd.oci.image.manifest.v1+json
  Platform:    unknown/unknown
  Annotations: 
    com.docker.official-images.bashbrew.arch: arm32v6
    vnd.docker.reference.digest:              sha256:d3c3fda3e4d3f0fcfc86cf469c6dfbe65017143dde23feb4b11df37c373cdc76
    vnd.docker.reference.type:                attestation-manifest
               
  Name:        docker.io/library/alpine:3@sha256:0a13654ecd9b19a728f0bee49d7245ffcd9b9f31ed8d69fe8753e39a8432bda2
  MediaType:   application/vnd.oci.image.manifest.v1+json
  Platform:    linux/arm/v7
  Annotations: 
    org.opencontainers.image.base.name:       scratch
    org.opencontainers.image.created:         2026-09-17T20:36:42Z
    org.opencontainers.image.revision:        1c744e2d49059e51b063a11a6e3e18c9ccf04ab8
    org.opencontainers.image.source:          https://github.com/alpinelinux/docker-alpine.git#1c744e2d49059e51b063a11a6e3e18c9ccf04ab8:armv7
    org.opencontainers.image.url:             https://hub.docker.com/_/alpine
    org.opencontainers.image.version:         3.24.2
    com.docker.official-images.bashbrew.arch: arm32v7
               
  Name:        docker.io/library/alpine:3@sha256:01d8e8fcca000e1455ca02632ace4f5c70c87f74fe28558328f9bd463a3bd2ac
  MediaType:   application/vnd.oci.image.manifest.v1+json
  Platform:    unknown/unknown
  Annotations: 
    com.docker.official-images.bashbrew.arch: arm32v7
    vnd.docker.reference.digest:              sha256:0a13654ecd9b19a728f0bee49d7245ffcd9b9f31ed8d69fe8753e39a8432bda2
    vnd.docker.reference.type:                attestation-manifest
               
  Name:        docker.io/library/alpine:3@sha256:260479a1cfaf304c4c20da7f8405d3ce313513dcd534bb743257bdd2fe0f3e2d
  MediaType:   application/vnd.oci.image.manifest.v1+json
  Platform:    linux/arm64/v8
  Annotations: 
    org.opencontainers.image.url:             https://hub.docker.com/_/alpine
    org.opencontainers.image.version:         3.24.2
    com.docker.official-images.bashbrew.arch: arm64v8
    org.opencontainers.image.base.name:       scratch
    org.opencontainers.image.created:         2026-09-17T20:36:42Z
    org.opencontainers.image.revision:        1c744e2d49059e51b063a11a6e3e18c9ccf04ab8
    org.opencontainers.image.source:          https://github.com/alpinelinux/docker-alpine.git#1c744e2d49059e51b063a11a6e3e18c9ccf04ab8:aarch64
               
  Name:        docker.io/library/alpine:3@sha256:ecc9a3031ac9bafed188097b6e65f328f5bf95f032d5e30030532cb259c6e006
  MediaType:   application/vnd.oci.image.manifest.v1+json
  Platform:    unknown/unknown
  Annotations: 
    com.docker.official-images.bashbrew.arch: arm64v8
    vnd.docker.reference.digest:              sha256:260479a1cfaf304c4c20da7f8405d3ce313513dcd534bb743257bdd2fe0f3e2d
    vnd.docker.reference.type:                attestation-manifest
               
  Name:        docker.io/library/alpine:3@sha256:c54a80678a9e7a744b39d478329d5eb8e568c2e0321defd14cab57047557e202
  MediaType:   application/vnd.oci.image.manifest.v1+json
  Platform:    linux/386
  Annotations: 
    org.opencontainers.image.source:          https://github.com/alpinelinux/docker-alpine.git#1c744e2d49059e51b063a11a6e3e18c9ccf04ab8:x86
    org.opencontainers.image.url:             https://hub.docker.com/_/alpine
    org.opencontainers.image.version:         3.24.2
    com.docker.official-images.bashbrew.arch: i386
    org.opencontainers.image.base.name:       scratch
    org.opencontainers.image.created:         2026-09-17T20:36:54Z
    org.opencontainers.image.revision:        1c744e2d49059e51b063a11a6e3e18c9ccf04ab8
               
  Name:        docker.io/library/alpine:3@sha256:5a8286dbe5f664111c93bc90b0105176872eede2a3af9512aeb4cc1c390af9d0
  MediaType:   application/vnd.oci.image.manifest.v1+json
  Platform:    unknown/unknown
  Annotations: 
    com.docker.official-images.bashbrew.arch: i386
    vnd.docker.reference.digest:              sha256:c54a80678a9e7a744b39d478329d5eb8e568c2e0321defd14cab57047557e202
    vnd.docker.reference.type:                attestation-manifest
               
  Name:        docker.io/library/alpine:3@sha256:f34b729ba6dc5bcd147ff99af45f81c1bc68ab619067c39cafd19327dd04b7db
  MediaType:   application/vnd.oci.image.manifest.v1+json
  Platform:    linux/ppc64le
  Annotations: 
    org.opencontainers.image.source:          https://github.com/alpinelinux/docker-alpine.git#1c744e2d49059e51b063a11a6e3e18c9ccf04ab8:ppc64le
    org.opencontainers.image.url:             https://hub.docker.com/_/alpine
    org.opencontainers.image.version:         3.24.2
    com.docker.official-images.bashbrew.arch: ppc64le
    org.opencontainers.image.base.name:       scratch
    org.opencontainers.image.created:         2026-09-17T20:36:20Z
    org.opencontainers.image.revision:        1c744e2d49059e51b063a11a6e3e18c9ccf04ab8
               
  Name:        docker.io/library/alpine:3@sha256:f627e01635bd94bbe4f6d438dd53476a3163e53730a83d1f0ea23ebd426a9acd
  MediaType:   application/vnd.oci.image.manifest.v1+json
  Platform:    unknown/unknown
  Annotations: 
    com.docker.official-images.bashbrew.arch: ppc64le
    vnd.docker.reference.digest:              sha256:f34b729ba6dc5bcd147ff99af45f81c1bc68ab619067c39cafd19327dd04b7db
    vnd.docker.reference.type:                attestation-manifest
               
  Name:        docker.io/library/alpine:3@sha256:1320e86f94641acc6c531e0519ca4285f1882612d52c654dfa4f1bb65b48a04f
  MediaType:   application/vnd.oci.image.manifest.v1+json
  Platform:    linux/riscv64
  Annotations: 
    org.opencontainers.image.base.name:       scratch
    org.opencontainers.image.created:         2026-09-18T16:48:56Z
    org.opencontainers.image.revision:        1c744e2d49059e51b063a11a6e3e18c9ccf04ab8
    org.opencontainers.image.source:          https://github.com/alpinelinux/docker-alpine.git#1c744e2d49059e51b063a11a6e3e18c9ccf04ab8:riscv64
    org.opencontainers.image.url:             https://hub.docker.com/_/alpine
    org.opencontainers.image.version:         3.24.2
    com.docker.official-images.bashbrew.arch: riscv64
               
  Name:        docker.io/library/alpine:3@sha256:ec6f2ae075c1023fbf18e7224a3c0984295f83c31e9311ddb3292c604699f9e5
  MediaType:   application/vnd.oci.image.manifest.v1+json
  Platform:    unknown/unknown
  Annotations: 
    com.docker.official-images.bashbrew.arch: riscv64
    vnd.docker.reference.digest:              sha256:1320e86f94641acc6c531e0519ca4285f1882612d52c654dfa4f1bb65b48a04f
    vnd.docker.reference.type:                attestation-manifest
               
  Name:        docker.io/library/alpine:3@sha256:ce2789264b55ef57693ad3aebc95d4ee9e46288f769f43ab5f37554853502289
  MediaType:   application/vnd.oci.image.manifest.v1+json
  Platform:    linux/s390x
  Annotations: 
    org.opencontainers.image.source:          https://github.com/alpinelinux/docker-alpine.git#1c744e2d49059e51b063a11a6e3e18c9ccf04ab8:s390x
    org.opencontainers.image.url:             https://hub.docker.com/_/alpine
    org.opencontainers.image.version:         3.24.2
    com.docker.official-images.bashbrew.arch: s390x
    org.opencontainers.image.base.name:       scratch
    org.opencontainers.image.created:         2026-09-17T21:38:11Z
    org.opencontainers.image.revision:        1c744e2d49059e51b063a11a6e3e18c9ccf04ab8
               
  Name:        docker.io/library/alpine:3@sha256:d72add410491397eebc3bc51264ba20500f329d198cdc729a16c693eb6643b68
  MediaType:   application/vnd.oci.image.manifest.v1+json
  Platform:    unknown/unknown
  Annotations: 
    com.docker.official-images.bashbrew.arch: s390x
    vnd.docker.reference.digest:              sha256:ce2789264b55ef57693ad3aebc95d4ee9e46288f769f43ab5f37554853502289
    vnd.docker.reference.type:                attestation-manifest
```

The index digest is `sha256:294b683cb724975bec92580e1e685676bd4b50bda910ddb8c51d4cabeaec77e6` (Alpine 3.24.2; the
linux/arm64/v8 manifest under it is `sha256:260479a1cfaf304c4c20da7f8405d3ce313513dcd534bb743257bdd2fe0f3e2d`).

Edit: `crates/minos-backend-krun/tests/fixtures/rootimg/Dockerfile` written as E-4 with tag `3` and that digest. The
fixture directory, which is the whole build context, holds only the Dockerfile (T-02-28).

Command: `cat crates/minos-backend-krun/tests/fixtures/rootimg/Dockerfile; ls -A crates/minos-backend-krun/tests/fixtures/rootimg`

Exit code: 0

```text
FROM docker.io/library/alpine:3@sha256:294b683cb724975bec92580e1e685676bd4b50bda910ddb8c51d4cabeaec77e6
RUN printf 'minos-spike-06\n' > /etc/minos-spike
Dockerfile
```

### RI-02: builder state, recorded read-only

Requirement: SPIKE-06 (D-10, D-14)

Nothing was started, installed, or reconfigured. The daemon was already running.

Command: `command -v docker; ls -l "$(command -v docker)"`

Exit code: 0

```text
/usr/local/bin/docker
lrwxr-xr-x@ 1 root  wheel  54 Jan 24  2026 /usr/local/bin/docker -> /Applications/Docker.app/Contents/Resources/bin/docker
```

Command: `docker version --format '{{.Server.Version}}'; docker buildx version; docker buildx ls`

Exit code: 0

```text
29.6.1
github.com/docker/buildx v0.35.0-desktop.2 b554ce1decd8b509893b1e7c6227eabfb923d094
NAME/NODE           DRIVER/ENDPOINT     STATUS    BUILDKIT   PLATFORMS
default             docker                                   
 \_ default          \_ default         running   v0.31.1    linux/amd64 (+2), linux/arm64, linux/ppc64le, linux/s390x, (2 more)
desktop-linux*      docker                                   
 \_ desktop-linux    \_ desktop-linux   running   v0.31.1    linux/amd64 (+2), linux/arm64, linux/ppc64le, linux/s390x, (2 more)
orbstack                                error                
podman                                  error                

Cannot load builder orbstack: failed to connect to the docker API at unix://~/.orbstack/run/docker.sock; check if the path is correct and if the daemon is running: dial unix ~/.orbstack/run/docker.sock: connect: no such file or directory
Cannot load builder podman: failed to connect to the docker API at unix:///var/folders/_p/cht4w96x7kddklkb8f3v304w0000gn/T/podman/podman-machine-default-api.sock; check if the path is correct and if the daemon is running: dial unix /var/folders/_p/cht4w96x7kddklkb8f3v304w0000gn/T/podman/podman-machine-default-api.sock: connect: no such file or directory
```

The build runs on the current builder, `desktop-linux` (docker driver, BuildKit v0.31.1). The `orbstack` and `podman`
entries are Docker contexts whose engines are not running; they were not started or used.

Engines checked for D-14: the Docker CLI with BuildKit (Docker Desktop), checked. lima, podman, and Apple's container
tool: not checked.

### RI-03: two builds through buildx and the Rust packer

Requirement: SPIKE-06 (D-14, D-16)

Setup: the guest from IR-01 (`cargo build -p minos-guest --release --target aarch64-unknown-linux-musl`, exit 0,
SHA-256 `10e59803c53ce60bb53445f95b86e07652a5029c5601170823f1fe156e4b3a89`).
`crates/minos-backend-krun/tests/spike_rootimg.rs` holds `produce` and the ignored `produce_twice` (E-5). `produce`
runs `docker buildx build --platform linux/arm64 --output type=tar,dest=<out_dir>/<name>.tar [--no-cache]
<CARGO_MANIFEST_DIR>/tests/fixtures/rootimg` with `env_clear()`, `HOME`, and `PATH` =
`/usr/local/bin:/Applications/Docker.app/Contents/Resources/bin:/usr/bin:/bin:/usr/sbin:/sbin` (the docker symlink's
directory, then its canonical directory, then the system directories; the trailing `/usr/local/bin` of E-5 is
already present, so it is not repeated). `out_dir` is `target/tmp/spike_rootimg` (`CARGO_TARGET_TMPDIR`), so the tars
stay for RI-04 and `cargo clean` removes them. `rootfs-1` allows the cache; `rootfs-2` passes `--no-cache`. No
credential-helper error occurred.

Command: `MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest MINOS_DOCKER=$(command -v docker) cargo test -p minos-backend-krun --test spike_rootimg -- --ignored --nocapture --test-threads=1`

Exit code: 0

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.07s
     Running tests/spike_rootimg.rs (target/debug/deps/spike_rootimg-df6a192048de69c2)

running 1 test
test produce_twice ... {"guest_sha256":"10e59803c53ce60bb53445f95b86e07652a5029c5601170823f1fe156e4b3a89","tar_1":{"bytes":8942592,"sha256":"f9832ee2a193875e09577a22e3eef01a4c2134fe5542ab034be6abf657a96872"},"tar_2":{"bytes":8942592,"sha256":"d1fb3975f55cb9e4b60183581d7181942908d4c8801b0c12fbf365a30f709078"},"image_1":{"bytes":4456448,"sha256":"d1e93ef0334243de484b036aec053eed241b77abc8176bd2e3e672aa2eb0a4ae"},"image_2":{"bytes":4456448,"sha256":"d1e93ef0334243de484b036aec053eed241b77abc8176bd2e3e672aa2eb0a4ae"},"image_1_repacked_sha256":"d1e93ef0334243de484b036aec053eed241b77abc8176bd2e3e672aa2eb0a4ae","tars_match":false,"images_match":true,"report_1":{"image_sha256":"d1e93ef0334243de484b036aec053eed241b77abc8176bd2e3e672aa2eb0a4ae","init_sha256":"10e59803c53ce60bb53445f95b86e07652a5029c5601170823f1fe156e4b3a89","files":85,"dirs":96,"symlinks":335,"hardlinks_copied":0,"setid_files":0,"skipped_global_headers":0,"replaced_duplicates":0,"init_overridden":false},"report_2":{"image_sha256":"d1e93ef0334243de484b036aec053eed241b77abc8176bd2e3e672aa2eb0a4ae","init_sha256":"10e59803c53ce60bb53445f95b86e07652a5029c5601170823f1fe156e4b3a89","files":85,"dirs":96,"symlinks":335,"hardlinks_copied":0,"setid_files":0,"skipped_global_headers":0,"replaced_duplicates":0,"init_overridden":false}}
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.53s

```

The three assertions passed: the first tar packed twice gives the same image
(`image_1_repacked_sha256` equals `image_1.sha256`), `/etc/minos-spike` in the first image holds `minos-spike-06\n`,
and `/init` is the guest. The image is 4,456,448 bytes from an 8,942,592-byte tar: 85 files (including `/init`), 96
directories, 335 symlinks (busybox applets), no hardlinks, no setuid or setgid files, no pax global headers, no
duplicates, and no tar-supplied `init`.

### RI-04: reproducibility of the two builds (D-16)

Requirement: SPIKE-06 (D-16)

Result: the two **tars differ** (`f9832ee2…6872` and `d1fb3975…9078`, same length); the two **images match**
(`d1e93ef0334243de484b036aec053eed241b77abc8176bd2e3e672aa2eb0a4ae` both times).

Command: `shasum -a 256 target/tmp/spike_rootimg/*.tar; tar -tvf target/tmp/spike_rootimg/rootfs-1.tar > "$S/l1.txt"; tar -tvf target/tmp/spike_rootimg/rootfs-2.tar > "$S/l2.txt"; wc -l < "$S/l1.txt"; wc -l < "$S/l2.txt"; diff "$S/l1.txt" "$S/l2.txt"; echo "diff exit: $?"`

Exit code: 0

```text
f9832ee2a193875e09577a22e3eef01a4c2134fe5542ab034be6abf657a96872  target/tmp/spike_rootimg/rootfs-1.tar
d1fb3975f55cb9e4b60183581d7181942908d4c8801b0c12fbf365a30f709078  target/tmp/spike_rootimg/rootfs-2.tar
     515
     515
diff exit: 0
```

The listings are identical: `tar -tv` shows mtimes to the minute, and both builds ran in the same minute. No
differing lines exist, so nothing is truncated. A byte comparison finds the difference:

Command: `cmp -l target/tmp/spike_rootimg/rootfs-1.tar target/tmp/spike_rootimg/rootfs-2.tar | awk '{print int(($1-1)/512), ($1-1)%512}'`

Exit code: 0 (the pipeline; `cmp` itself exits 1 because the files differ)

```text
1880 146
1880 152
1880 153
1926 146
1926 152
1926 153
```

Six bytes differ, all in two 512-byte header blocks (1880 and 1926), at header offsets 146 (the last digit of the
mtime field, bytes 136 to 147) and 152 to 153 (the checksum field, bytes 148 to 155). Decoding those two headers from
each tar, read-only:

Command: `for t in rootfs-1 rootfs-2; do for b in 1880 1926; do printf '%s block %s: ' $t $b; dd if=target/tmp/spike_rootimg/$t.tar bs=512 skip=$b count=1 2>/dev/null | python3 -I -c 'import sys,datetime; h=sys.stdin.buffer.read(); name=h[0:100].split(b"\0")[0].decode(); pre=h[345:500].split(b"\0")[0].decode(); m=int(h[136:148].split(b"\0")[0].strip() or b"0",8); print((pre+"/" if pre else "")+name, "type", h[156:157].decode(), "mtime", m, datetime.datetime.fromtimestamp(m, datetime.UTC).isoformat())'; done; done`

Exit code: 0

```text
rootfs-1 block 1880: etc/ type 5 mtime 1791639091 2026-10-10T13:31:31+00:00
rootfs-1 block 1926: etc/minos-spike type 0 mtime 1791639091 2026-10-10T13:31:31+00:00
rootfs-2 block 1880: etc/ type 5 mtime 1791639094 2026-10-10T13:31:34+00:00
rootfs-2 block 1926: etc/minos-spike type 0 mtime 1791639094 2026-10-10T13:31:34+00:00
```

Cause: the `RUN` step writes `/etc/minos-spike` and so updates `/etc`; both carry the wall-clock time of the step
(13:31:31 and 13:31:34 UTC). This plan made no earlier build of the fixture, so the cache-allowed build had no
cached result to reuse and ran the step too. Everything else in the two tars, including every base-layer file and the apk database,
is byte-identical. The packer sets every mtime to 0, so the images are identical. The packer was not changed to get
this result; byte-identical tars (for example through `SOURCE_DATE_EPOCH` and BuildKit's `rewrite-timestamp`) and
`minos.lock` remain Phase 5 work (D-16).

## Plan 02-06: signing, boot, root image boot, SPIKE-08

Date: 2026-10-10

- `rustc --version`: rustc 1.97.1 (8bab26f4f 2026-07-14)
- `cargo --version`: cargo 1.97.1 (c980f4866 2026-06-30)
- `cargo clippy --version`: clippy 0.1.97 (8bab26f4f6 2026-07-14)
- `cargo deny --version`: cargo-deny 0.20.2
- macOS: 27.0, arm64
- HEAD at start: `a47123c96a75eb579a9007af8c1857b9ade33516`

Redactions: the absolute home directory path is shown as `~`. Nothing else is altered. Cargo progress lines
(`Compiling`, `Finished`, `Running`) are removed.

In the commands below, `S` is the session scratch directory
`/private/tmp/claude-501/-Users-seanmcauliffe-Repos-minos/79087d67-27f5-41bb-a90a-460fa65ed311/scratchpad/0206`. The
output shows it expanded, as printed.

Environment for every VM test (E-1), set on the command line of each run:

- `MINOS_VMM=$(pwd)/target/debug/minos-vmm`
- `MINOS_VMM_UNSIGNED=$S/minos-vmm-unsigned` (S04-UNSIGNED only)
- `MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest`
- `MINOS_KRUNFW=$(realpath /opt/homebrew/lib/libkrunfw.5.dylib)` (= `/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib`, SHA-256 `59764391005357bbdd5218f3fd522dd8ab63722cbc96144752c7bddc2c8e5217`, H-01)
- `MINOS_DOCKER=$(command -v docker)` (S06-BOOT only)

### B-06: build and the unsigned copy

Requirement: SPIKE-04 (D-11, D-12)

Setup: Task 1's tests committed (`0c79791`).

Command: `cargo build -p minos-guest --release --target aarch64-unknown-linux-musl; cargo build -p minos-vmm; cp target/debug/minos-vmm $S/minos-vmm-unsigned; shasum -a 256 target/aarch64-unknown-linux-musl/release/minos-guest target/debug/minos-vmm $S/minos-vmm-unsigned`

Exit code: 0 (each)

```text
10e59803c53ce60bb53445f95b86e07652a5029c5601170823f1fe156e4b3a89  target/aarch64-unknown-linux-musl/release/minos-guest
67ff00c77a8a7d0921d13bfa13d33ee942432323ef30194bb49abc01d13eba92  target/debug/minos-vmm
67ff00c77a8a7d0921d13bfa13d33ee942432323ef30194bb49abc01d13eba92  /private/tmp/claude-501/-Users-seanmcauliffe-Repos-minos/79087d67-27f5-41bb-a90a-460fa65ed311/scratchpad/0206/minos-vmm-unsigned
```

The guest hash equals 02-01 G-GUEST. `cargo build -p minos-vmm` relinked the helper (`Compiling minos-vmm`), so the
copy is linker-signed with no entitlement.

### S04-UNSIGNED: a linker-signed helper with no entitlement is refused

Requirement: SPIKE-04 (D-12), T-02-38

Setup: B-06. The copy was never passed to `codesign`.

Command: `codesign -dvv $S/minos-vmm-unsigned`

Exit code: 0

```text
Executable=/private/tmp/claude-501/-Users-seanmcauliffe-Repos-minos/79087d67-27f5-41bb-a90a-460fa65ed311/scratchpad/0206/minos-vmm-unsigned
Identifier=minos_vmm-307854e340cc041e
Format=Mach-O thin (arm64)
CodeDirectory v=20400 size=138419 flags=0x20002(adhoc,linker-signed) hashes=4322+0 location=embedded
Signature=adhoc
Info.plist=not bound
TeamIdentifier=not set
Sealed Resources=none
Internal requirements=none
```

Command: `codesign -d --entitlements - $S/minos-vmm-unsigned`

Exit code: 0

```text
Executable=/private/tmp/claude-501/-Users-seanmcauliffe-Repos-minos/79087d67-27f5-41bb-a90a-460fa65ed311/scratchpad/0206/minos-vmm-unsigned
```

The flags are `0x20002(adhoc,linker-signed)` and the entitlement display is empty: no hypervisor key.

Command: `MINOS_VMM_UNSIGNED=$S/minos-vmm-unsigned MINOS_GUEST=... MINOS_KRUNFW=... cargo test -p minos-backend-krun --test spike_boot -- --ignored --nocapture --test-threads=1 unsigned_helper_is_refused`

Exit code: 0 (test passed; helper exit code 70)

```text

running 1 test
test unsigned_helper_is_refused ... == unsigned_helper_is_refused: exit_code Some(70), signal None, killed_at_deadline false, elapsed 683.600459ms, connections 0
-- console (0 bytes, overflowed false):

-- minos-vmm stderr (102 bytes):
minos-vmm: error: starting the VM: build error: start: build_microvm: Internal(Vm(VmSetup(VmCreate)))

-- minos-vmm stdout (0 bytes):

== end unsigned_helper_is_refused
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 1.15s

```

Result: the helper exits 70 before the VM starts; the console is empty. Its stderr line, verbatim:
`minos-vmm: error: starting the VM: build error: start: build_microvm: Internal(Vm(VmSetup(VmCreate)))`.

The line does not name `HV_DENIED`, `0xfae94007`, or `hv_vm_create`. Said plainly: the refusal text comes from
msb_krun, not from Hypervisor.framework. `VmCreate` is the error msb_krun_hvf 0.1.41 returns when `hv_vm_create`
returns anything other than `HV_SUCCESS` (`src/lib.rs` lines 463 to 466: `let ret = unsafe { hv_vm_create(config) };
if ret != HV_SUCCESS { Err(Error::VmCreate) }`), and it discards the return code. So the run shows that
`hv_vm_create` failed, not which `hv_return_t` it returned. The only difference between this binary and the one that
boots in S01-ROOTDISK run 2 is the signature (same bytes before signing, B-06), which ties the failure to the missing
entitlement.

### S04-SIGN: ad-hoc signing with the hardened runtime and the hypervisor entitlement (first plist)

Requirement: SPIKE-04 (D-11, D-13)

Setup: `crates/minos-vmm/minos-vmm.entitlements` as committed by 02-03 (hypervisor key only).

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
	[Key] com.apple.security.hypervisor
	[Value]
		[Bool] true
```

Command: `codesign -dvv target/debug/minos-vmm`

Exit code: 0

```text
Executable=~/Repos/minos/target/debug/minos-vmm
Identifier=minos-vmm-5555494420af20e944ef3a86b377db7adb7096ab
Format=Mach-O thin (arm64)
CodeDirectory v=20500 size=34963 flags=0x10002(adhoc,runtime) hashes=1081+7 location=embedded
Signature=adhoc
Info.plist=not bound
TeamIdentifier=not set
Runtime Version=26.5.0
Sealed Resources=none
Internal requirements count=0 size=12
Total signatures=1
Chosen signature=1
```

The flags are now `0x10002(adhoc,runtime)`: ad hoc, hardened runtime, `com.apple.security.hypervisor` true.

### S01-ROOTDISK: root-disk boot

Requirement: SPIKE-01 as amended by D-18, SPIKE-04, T-02-39, T-02-40

#### Run 1 (hypervisor entitlement alone)

Setup: S04-SIGN.

Command: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=... MINOS_KRUNFW=... cargo test -p minos-backend-krun --test spike_boot -- --ignored --nocapture --test-threads=1 boot_root_disk`

Exit code: 101 (helper exit code 70)

```text

running 1 test
test boot_root_disk ... == boot_root_disk: exit_code Some(70), signal None, killed_at_deadline false, elapsed 388.880458ms, connections 0
-- console (0 bytes, overflowed false):

-- minos-vmm stderr (885 bytes):
minos-vmm: error: starting the VM: build error: libkrunfw: load /opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib: dlopen(/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib, 0x0005): tried: '/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib' (code signature in <B3581C50-EC48-35A3-9445-F4CA9D645DA2> '/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib' not valid for use in process: mapping process and mapped file (non-platform) have different Team IDs), '/System/Volumes/Preboot/Cryptexes/OS/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib' (no such file), '/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib' (code signature in <B3581C50-EC48-35A3-9445-F4CA9D645DA2> '/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib' not valid for use in process: mapping process and mapped file (non-platform) have different Team IDs)

-- minos-vmm stdout (0 bytes):

== end boot_root_disk

thread 'boot_root_disk' (2519861) panicked at crates/minos-backend-krun/tests/spike_boot.rs:333:5:
assertion `left == right` failed
  left: Some(70)
 right: Some(0)
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
FAILED

failures:

failures:
    boot_root_disk

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.82s

error: test failed, to rerun pass `-p minos-backend-krun --test spike_boot`
```

The VM was never built: the hardened runtime's library validation refused the `dlopen` of Homebrew's libkrunfw
because the two signatures have different Team IDs. This is the K5 case; see K5 below. The plist was replaced with
E-5, the helper re-signed, and the test re-run.

#### Run 2 (E-5 plist: hypervisor plus disable-library-validation)

Setup: K5 (E-5 plist, re-signed; display shows both keys).

Command: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=... MINOS_KRUNFW=... cargo test -p minos-backend-krun --test spike_boot -- --ignored --nocapture --test-threads=1 boot_root_disk`

Exit code: 101 (helper exit code 0; the test failed only at the `net_ifaces` assertion, see S01-NETIFACES)

This is the first run that booted, so its console is recorded in full.

```text

running 1 test
test boot_root_disk ... == boot_root_disk: exit_code Some(0), signal None, killed_at_deadline false, elapsed 560.5795ms, connections 1
-- console (3622 bytes, overflowed false):
[    0.018686] random: crng init done
[    0.018893] loop: module loaded
[    0.018971] virtio_blk virtio2: 1/0/0 default/read/poll queues
[    0.019091] virtio_blk virtio2: [vda] 808 512-byte logical blocks (414 kB/404 KiB)
[    0.019596] virtio_blk virtio3: 1/0/0 default/read/poll queues
[    0.019718] virtio_blk virtio3: [vdb] 1 512-byte logical blocks (512 B/512 B)
[    0.019918] Dev vdb: unable to read RDB block 1
[    0.019973]  vdb: unable to read partition table
[    0.020035] vdb: partition table beyond EOD, truncated
[    0.020136] virtio_blk virtio4: 1/0/0 default/read/poll queues
[    0.020270] virtio_blk virtio4: [vdc] 131072 512-byte logical blocks (67.1 MB/64.0 MiB)
[    0.020546] zram: Added device: zram0
[    0.020613] Loading iSCSI transport class v2.0-870.
[    0.020738] tun: Universal TUN/TAP device driver, 1.6
[    0.020801] vcan: Virtual CAN interface driver
[    0.020850] CAN device driver interface
[    0.020955] rtc-pl031 a001000.rtc: registered as rtc0
[    0.021006] rtc-pl031 a001000.rtc: setting system clock to 2026-10-10T13:54:24 UTC (1791640464)
[    0.021084] i2c_dev: i2c /dev entries driver
[    0.021145] pps_ldisc: PPS line discipline registered
[    0.021264] NET: Registered PF_INET6 protocol family
[    0.021381] Segment Routing with IPv6
[    0.021429] In-situ OAM (IOAM) with IPv6
[    0.021486] NET: Registered PF_PACKET protocol family
[    0.021537] Bridge firewalling registered
[    0.021586] can: controller area network core
[    0.021638] NET: Registered PF_CAN protocol family
[    0.021688] can: raw protocol
[    0.021738] can: broadcast manager protocol
[    0.021786] can: netlink gateway - max_hops=1
[    0.021861] NET: Registered PF_KCM protocol family
[    0.021936] NET: Registered PF_VSOCK protocol family
[    0.022306] NET: Registered PF_TSI protocol family
[    0.022370] NET: Registered PF_TSI6 protocol family
[    0.022420] NET: Registered PF_TSIU protocol family
[    0.022711] registered taskstats version 1
[    0.023318] Btrfs loaded, zoned=yes, fsverity=no
[    0.023437] Key type encrypted registered
[    0.037255] alg: No test for fips(ansi_cprng) (fips_ansi_cprng)
[    0.037344] alg: No test for 842 (842-scomp)
[    0.037418] alg: No test for 842 (842-generic)
[    0.041565] "cryptomgr_test" (158) uses obsolete ecb(arc4) skcipher
[    0.048148] clk: Disabling unused clocks
[    0.048196] PM: genpd: Disabling unused power domains
[    0.048244] ALSA device list:
[    0.048289]   No soundcards found.
[    0.049013] VFS: Mounted root (squashfs filesystem) readonly on device 254:0.
[    0.049136] devtmpfs: mounted
[    0.049256] Freeing unused kernel memory: 2496K
[    0.049321] Run /init as init process
[    0.049363]   with arguments:
[    0.049407]     /init
[    0.049450]   with environment:
[    0.049495]     HOME=/
[    0.049536]     TERM=linux
[    0.049576]     KRUN_BOOT_START_NS=1791640464332629000
MINOS-GUEST-BOOT-OK
minos-guest: cmdline reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1791640464332629000  --
minos-guest: osrelease 6.12.109
minos-guest: args /init
minos-guest: env HOME=/ KRUN_BOOT_START_NS=1791640464332629000 TERM=linux
minos-guest: net_ifaces dummy0 lo
minos-guest: virtio_devices virtio0:0x0004 virtio1:0x0003 virtio2:0x0002 virtio3:0x0002 virtio4:0x0002 virtio5:0x0013 virtio6:0x4d47
MINOS-GUEST-FRAME-SENT 686 bytes
MINOS-GUEST-POWEROFF
[    0.090993] reboot: Power down

-- minos-vmm stderr (18 bytes):
minos-vmm: exit 0

-- minos-vmm stdout (0 bytes):

-- connection 0 (686 bytes), frame:
{
  "job_error": null,
  "steps": [
    {
      "report_boot": {
        "args": [
          "/init"
        ],
        "cmdline": "reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1791640464332629000  --",
        "env": [
          [
            "HOME",
            "/"
          ],
          [
            "KRUN_BOOT_START_NS",
            "1791640464332629000"
          ],
          [
            "TERM",
            "linux"
          ]
        ],
        "marker": "MINOS-GUEST-BOOT-OK",
        "net_ifaces": [
          "dummy0",
          "lo"
        ],
        "osrelease": "6.12.109",
        "virtio_devices": [
          "virtio0:0x0004",
          "virtio1:0x0003",
          "virtio2:0x0002",
          "virtio3:0x0002",
          "virtio4:0x0002",
          "virtio5:0x0013",
          "virtio6:0x4d47"
        ]
      }
    },
    {
      "scratch_roundtrip": {
        "bytes": 65536,
        "error": null,
        "ok": true
      }
    }
  ],
  "v": 1
}
== end boot_root_disk
normalized cmdline tokens: ["reboot=k", "panic=-1", "panic_print=0", "nomodule", "console=hvc0", "rootfstype=virtiofs", "rw", "quiet", "no-kvmapf", "rootfstype=squashfs", "ro", "ignore_loglevel", "root=/dev/root", "init=/init", "root=/dev/vda", "KRUN_BOOT_START_NS=<ns>", "--"]
osrelease: 6.12.109

thread 'boot_root_disk' (2520854) panicked at crates/minos-backend-krun/tests/spike_boot.rs:393:5:
network interfaces: ["dummy0", "lo"]
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
FAILED

failures:

failures:
    boot_root_disk

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 2 filtered out; finished in 1.03s

error: test failed, to rerun pass `-p minos-backend-krun --test spike_boot`
```

Result: the decided D-18 path boots. msb_krun, called only from `minos-vmm` (which is under `forbid(unsafe_code)`),
booted stock libkrunfw 5.6.2 from the raw read-only SquashFS image on `/dev/vda`
(`VFS: Mounted root (squashfs filesystem) readonly on device 254:0.`, then `Run /init as init process`). There was
no initramfs and no virtio-fs device. The guest printed `MINOS-GUEST-BOOT-OK`, `MINOS-GUEST-FRAME-SENT 686 bytes`,
and `MINOS-GUEST-POWEROFF`. It sent exactly one connection holding a valid frame with `"v": 1`. The helper exited 0,
was not killed at the deadline, and took 560 ms from spawn. These assertions passed before the run stopped at
`net_ifaces`:
- exit code 0, not killed;
- the three markers;
- one connection with a v1 frame;
- the exact command line (S01-CMDLINE);
- last `root=` is `/dev/vda`, last `rootfstype=` is `squashfs`, and `init=/init` is present;
- `osrelease` is non-empty.

The run never reached the `virtio_devices`, `env`, and `scratch_roundtrip` assertions. The frame above shows what they
would have checked: no device ends in `:0x001a`; env keys are `HOME`, `KRUN_BOOT_START_NS`, `TERM` (no `MSB_` or
`DYLD_` key, no value containing `/Users/`); and `scratch_roundtrip.ok` is `true`.

### K5: hardened runtime and the libkrunfw dlopen

Requirement: SPIKE-04 (D-13, K5), T-02-37

Answer: **the hypervisor entitlement alone is not enough under the hardened runtime.** The forcing error, from
S01-ROOTDISK run 1 (helper stderr, verbatim, home path not involved):

```text
minos-vmm: error: starting the VM: build error: libkrunfw: load /opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib: dlopen(/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib, 0x0005): tried: '/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib' (code signature in <B3581C50-EC48-35A3-9445-F4CA9D645DA2> '/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib' not valid for use in process: mapping process and mapped file (non-platform) have different Team IDs), '/System/Volumes/Preboot/Cryptexes/OS/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib' (no such file), '/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib' (code signature in <B3581C50-EC48-35A3-9445-F4CA9D645DA2> '/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib' not valid for use in process: mapping process and mapped file (non-platform) have different Team IDs)
```

Setup: `crates/minos-vmm/minos-vmm.entitlements` replaced with E-5 (adds `com.apple.security.cs.disable-library-validation`).

Command: `plutil -lint crates/minos-vmm/minos-vmm.entitlements`

Exit code: 0

```text
crates/minos-vmm/minos-vmm.entitlements: OK
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

With both keys, S01-ROOTDISK run 2 loaded libkrunfw and booted. Final entitlement set: `com.apple.security.hypervisor`
and `com.apple.security.cs.disable-library-validation` (`grep -c '<key>'` prints 2). The second key exists only
because Homebrew's libkrunfw dylib carries a signature from a different Team ID than the ad-hoc-signed helper, and the
hardened runtime's library validation refuses that. The libkrunfw SHA-256 is still checked by minos-vmm preflight
before the `dlopen` (02-03), which is the integrity check the dropped validation would otherwise have given. Not
tried, because the plan does not list it: signing without `--options runtime`.

### S01-CMDLINE: the exact kernel command line

Requirement: SPIKE-01 (D-18), T-02-39

Setup: S01-ROOTDISK run 2.

Prediction (E-3, from msb_krun 0.1.41 `src/api/vm.rs` `build_kernel_cmdline` and msb_krun_vmm 0.1.41
`src/vmm_config/kernel_cmdline.rs` aarch64 `DEFAULT_KERNEL_CMDLINE`):

```text
reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel root=/dev/root init=/init root=/dev/vda KRUN_BOOT_START_NS=<ns> --
```

Observation (guest `/proc/cmdline` from the frame, then the test's normalized tokens):

```text
minos-guest: cmdline reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1791640464332629000  --
normalized cmdline tokens: ["reboot=k", "panic=-1", "panic_print=0", "nomodule", "console=hvc0", "rootfstype=virtiofs", "rw", "quiet", "no-kvmapf", "rootfstype=squashfs", "ro", "ignore_loglevel", "root=/dev/root", "init=/init", "root=/dev/vda", "KRUN_BOOT_START_NS=<ns>", "--"]
osrelease: 6.12.109
```

Result: the normalized tokens equal E-3 exactly, so `EXPECTED_CMDLINE` in `spike_boot.rs` stays as predicted and
the exact assertion passed on the first boot. The runs of spaces before `KRUN_BOOT_START_NS` come from the four
empty `KRUN_INIT`, `KRUN_WORKDIR`, `KRUN_RLIMITS`, and `KRUN_ENV` slots in the `krun_env` format string
(`" {} {} {} {} KRUN_BOOT_START_NS={boot_start_ns}"`), and the trailing `--` from the epilog `" -- {}"` with no
arguments. Splitting on ASCII whitespace removes both. The last `root=` is `/dev/vda` and the last `rootfstype=`
is `squashfs`, as D-18 requires.

Kernel version: `osrelease` is **6.12.109**. This settles 02-RESEARCH.md Pitfall 7: the libkrunfw v5.6.2 Makefile
(`KERNEL_VERSION = linux-6.12.109`) is right, and the committed config header (`# Linux/arm64 6.12.91 Kernel
Configuration`, H-02) is stale. CLAUDE.md and `.claude/CLAUDE.md` still say 6.12.91 (correction is 02-10's, D-09).

### S01-FRAME: what the guest saw (02-RESEARCH.md open question 3)

Requirement: SPIKE-01, T-02-40

Setup: S01-ROOTDISK run 2, `report_boot` result in the frame.

- `args`: `["/init"]`
- `env`: `HOME=/`, `KRUN_BOOT_START_NS=1791640464332629000`, `TERM=linux`. The kernel passes the unrecognized
  `KRUN_BOOT_START_NS=` parameter to init as an environment variable.
- `net_ifaces`: `["dummy0", "lo"]` (see S01-NETIFACES)
- `virtio_devices`: `virtio0:0x0004` (entropy, the default rng), `virtio1:0x0003` (console), `virtio2:0x0002`,
  `virtio3:0x0002`, `virtio4:0x0002` (block: vda root, vdb input, vdc scratch), `virtio5:0x0013` (vsock),
  `virtio6:0x4d47` (msb_krun's vmgenid device, which 02-RESEARCH.md open question 3 notes is always attached). There is
  no `0x0001` (net) and no `0x001a` (fs).
- `scratch_roundtrip`: `{"bytes": 65536, "error": null, "ok": true}`

### S01-NETIFACES: the guest has a `dummy0` interface (open; Rule 4 question to the user)

Requirement: SPIKE-01 (E-2 `boot_root_disk` contract), SPIKE-02 context

Setup: S01-ROOTDISK run 2. The assertion that failed is E-2's "`net_ifaces` is empty or exactly `lo`":

```text
thread 'boot_root_disk' (2520854) panicked at crates/minos-backend-krun/tests/spike_boot.rs:393:5:
network interfaces: ["dummy0", "lo"]
```

Command: `grep -nE 'CONFIG_DUMMY|CONFIG_TUN=|CONFIG_VIRTIO_NET' <scratch>/config-libkrunfw_aarch64` (the v5.6.2
config fetched in 02-01 H-02)

Exit code: 0

```text
1501:# CONFIG_DUMMY_IRQ is not set
1585:CONFIG_DUMMY=y
1598:CONFIG_TUN=y
1601:CONFIG_VIRTIO_NET=y
```

Result: `dummy0` comes from the kernel itself. The dummy driver is built in (`CONFIG_DUMMY=y`), and its
`numdummies` parameter defaults to 1, so the kernel creates `dummy0` at boot. It is not a device the VMM attached:
the virtio list has no net device (`0x0001`). A dummy interface drops everything sent to it. This is a mismatch
between E-2's assertion and the stock kernel. It is not a boot failure.

The assertion was **not** relaxed and the launcher was **not** changed. Both possible fixes depart from a locked
text: E-2's assertion, or 02-03's fixed E-4 kernel arguments together with E-3's `EXPECTED_CMDLINE`. So the question
goes to the user (see the plan SUMMARY / checkpoint).

#### Decision (2026-10-10)

Date: 2026-10-10

Options presented:

- option-1: disable the interface in the launcher: append `dummy.numdummies=0` to minos-vmm's fixed root-disk kernel
  arguments (02-03 E-4) and to `EXPECTED_CMDLINE` (E-3), keeping E-2's `net_ifaces` assertion as written.
- option-2: option-1, plus a direct assertion in `boot_root_disk` that no virtio device with id `0x0001` (net) is in
  the guest's device list, beside the existing `0x001a` (fs) check.
- option-3: accept `dummy0` by name: relax E-2's assertion to allow `dummy0` and `lo`, with no launcher change.

User's choice, in their words: "option 2".

Reasoning given with the options: a dummy interface is a dead sink and harmless, but it is cheap to remove, and an
interface list that is exactly `lo` is easier to check than one with a named exception; the direct virtio check closes
the gap in using `net_ifaces` as a proxy for "no network device" (a VMM-attached NIC is visible in the virtio list
whatever the guest names it).

Applied:

- `crates/minos-vmm/src/launch.rs`: `ROOT_DISK_CMDLINE` is now `rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0`
  (commit 4863655). `INITRAMFS_CMDLINE` is a separate constant and is unchanged: that mode never reaches `/init`.
- `crates/minos-backend-krun/tests/spike_boot.rs`: `EXPECTED_CMDLINE` gains `dummy.numdummies=0` after `ignore_loglevel`;
  `boot_root_disk` asserts no device ends in `:0x0001`; the `net_ifaces` assertion is unchanged (commit 727e952).

This supersedes the S01-CMDLINE statement that `EXPECTED_CMDLINE` stays as E-3 predicted: the observed list is E-3
plus one Minos-originated token, `dummy.numdummies=0`, between `ignore_loglevel` and `root=/dev/root`. The 02-03 fixed
kernel argument list changes the same way.

#### Rebuild and re-sign

Command: `cargo build -p minos-guest --release --target aarch64-unknown-linux-musl && cargo build -p minos-vmm`

Exit code: 0 (progress lines only)

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

#### Rerun: `boot_root_disk`

Command: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=... MINOS_KRUNFW=... cargo test -p minos-backend-krun --test spike_boot -- --ignored --nocapture --test-threads=1 boot_root_disk`

Exit code: 0 (test passed; helper exit code 0)

```text

running 1 test
test boot_root_disk ... == boot_root_disk: exit_code Some(0), signal None, killed_at_deadline false, elapsed 514.715583ms, connections 1
-- console (3634 bytes, overflowed false):
[27 console lines omitted by the evidence protocol]
[    0.022456] can: raw protocol
[    0.022504] can: broadcast manager protocol
[    0.022548] can: netlink gateway - max_hops=1
[    0.022623] NET: Registered PF_KCM protocol family
[    0.022700] NET: Registered PF_VSOCK protocol family
[    0.023049] NET: Registered PF_TSI protocol family
[    0.023108] NET: Registered PF_TSI6 protocol family
[    0.023157] NET: Registered PF_TSIU protocol family
[    0.023456] registered taskstats version 1
[    0.024139] Btrfs loaded, zoned=yes, fsverity=no
[    0.024294] Key type encrypted registered
[    0.034613] alg: No test for fips(ansi_cprng) (fips_ansi_cprng)
[    0.034702] alg: No test for 842 (842-scomp)
[    0.034766] alg: No test for 842 (842-generic)
[    0.038989] "cryptomgr_test" (159) uses obsolete ecb(arc4) skcipher
[    0.048388] clk: Disabling unused clocks
[    0.048438] PM: genpd: Disabling unused power domains
[    0.048494] ALSA device list:
[    0.048538]   No soundcards found.
[    0.049460] VFS: Mounted root (squashfs filesystem) readonly on device 254:0.
[    0.049656] devtmpfs: mounted
[    0.049772] Freeing unused kernel memory: 2496K
[    0.049832] Run /init as init process
[    0.049875]   with arguments:
[    0.049922]     /init
[    0.049965]   with environment:
[    0.050008]     HOME=/
[    0.050051]     TERM=linux
[    0.050091]     KRUN_BOOT_START_NS=1791641332425136000
MINOS-GUEST-BOOT-OK
minos-guest: cmdline reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0 root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1791641332425136000  --
minos-guest: osrelease 6.12.109
minos-guest: args /init
minos-guest: env HOME=/ KRUN_BOOT_START_NS=1791641332425136000 TERM=linux
minos-guest: net_ifaces lo
minos-guest: virtio_devices virtio0:0x0004 virtio1:0x0003 virtio2:0x0002 virtio3:0x0002 virtio4:0x0002 virtio5:0x0013 virtio6:0x4d47
MINOS-GUEST-FRAME-SENT 696 bytes
MINOS-GUEST-POWEROFF
[    0.076960] reboot: Power down

-- minos-vmm stderr (18 bytes):
minos-vmm: exit 0

-- minos-vmm stdout (0 bytes):

-- connection 0 (696 bytes), frame:
{
  "job_error": null,
  "steps": [
    {
      "report_boot": {
        "args": [
          "/init"
        ],
        "cmdline": "reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0 root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1791641332425136000  --",
        "env": [
          [
            "HOME",
            "/"
          ],
          [
            "KRUN_BOOT_START_NS",
            "1791641332425136000"
          ],
          [
            "TERM",
            "linux"
          ]
        ],
        "marker": "MINOS-GUEST-BOOT-OK",
        "net_ifaces": [
          "lo"
        ],
        "osrelease": "6.12.109",
        "virtio_devices": [
          "virtio0:0x0004",
          "virtio1:0x0003",
          "virtio2:0x0002",
          "virtio3:0x0002",
          "virtio4:0x0002",
          "virtio5:0x0013",
          "virtio6:0x4d47"
        ]
      }
    },
    {
      "scratch_roundtrip": {
        "bytes": 65536,
        "error": null,
        "ok": true
      }
    }
  ],
  "v": 1
}
== end boot_root_disk
normalized cmdline tokens: ["reboot=k", "panic=-1", "panic_print=0", "nomodule", "console=hvc0", "rootfstype=virtiofs", "rw", "quiet", "no-kvmapf", "rootfstype=squashfs", "ro", "ignore_loglevel", "dummy.numdummies=0", "root=/dev/root", "init=/init", "root=/dev/vda", "KRUN_BOOT_START_NS=<ns>", "--"]
osrelease: 6.12.109
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.96s
```

Result: `boot_root_disk` passes every E-2 assertion as written. `net_ifaces` is exactly `["lo"]`; the virtio list has
no `0x0001` and no `0x001a`; the normalized command line equals the updated `EXPECTED_CMDLINE`.

#### Rerun: `boot_from_dockerfile`

Command: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=... MINOS_KRUNFW=... MINOS_DOCKER=$(command -v docker) cargo test -p minos-backend-krun --test spike_rootimg -- --ignored --nocapture --test-threads=1 boot_from_dockerfile`

Exit code: 0 (test passed; helper exit code 0; `docker version --format '{{.Server.Version}}'` printed `29.6.1`,
the daemon was already running)

```text

running 1 test
test boot_from_dockerfile ... image: 4456448 bytes, sha256 d1e93ef0334243de484b036aec053eed241b77abc8176bd2e3e672aa2eb0a4ae
== boot_from_dockerfile: exit_code Some(0), signal None, killed_at_deadline false, elapsed 322.848583ms, connections 1
-- console (3637 bytes, overflowed false):
[27 console lines omitted by the evidence protocol]
[    0.021873] can: raw protocol
[    0.021920] can: broadcast manager protocol
[    0.021964] can: netlink gateway - max_hops=1
[    0.022039] NET: Registered PF_KCM protocol family
[    0.022120] NET: Registered PF_VSOCK protocol family
[    0.022406] NET: Registered PF_TSI protocol family
[    0.022454] NET: Registered PF_TSI6 protocol family
[    0.022503] NET: Registered PF_TSIU protocol family
[    0.022789] registered taskstats version 1
[    0.023357] Btrfs loaded, zoned=yes, fsverity=no
[    0.023469] Key type encrypted registered
[    0.037460] alg: No test for fips(ansi_cprng) (fips_ansi_cprng)
[    0.037547] alg: No test for 842 (842-scomp)
[    0.037616] alg: No test for 842 (842-generic)
[    0.041700] "cryptomgr_test" (158) uses obsolete ecb(arc4) skcipher
[    0.048061] clk: Disabling unused clocks
[    0.048120] PM: genpd: Disabling unused power domains
[    0.048168] ALSA device list:
[    0.048222]   No soundcards found.
[    0.049054] VFS: Mounted root (squashfs filesystem) readonly on device 254:0.
[    0.049336] devtmpfs: mounted
[    0.049534] Freeing unused kernel memory: 2496K
[    0.049591] Run /init as init process
[    0.049634]   with arguments:
[    0.049680]     /init
[    0.049721]   with environment:
[    0.049764]     HOME=/
[    0.049803]     TERM=linux
[    0.049843]     KRUN_BOOT_START_NS=1791641345459408000
MINOS-GUEST-BOOT-OK
minos-guest: cmdline reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0 root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1791641345459408000  --
minos-guest: osrelease 6.12.109
minos-guest: args /init
minos-guest: env HOME=/ KRUN_BOOT_START_NS=1791641345459408000 TERM=linux
minos-guest: net_ifaces lo
minos-guest: virtio_devices virtio0:0x0004 virtio1:0x0003 virtio2:0x0002 virtio3:0x0002 virtio4:0x0002 virtio5:0x0013 virtio6:0x4d47
MINOS-GUEST-FRAME-SENT 812 bytes
MINOS-GUEST-POWEROFF
[    0.082037] reboot: Power down

-- minos-vmm stderr (18 bytes):
minos-vmm: exit 0

-- minos-vmm stdout (0 bytes):

-- connection 0 (812 bytes), frame:
{
  "job_error": null,
  "steps": [
    {
      "report_boot": {
        "args": [
          "/init"
        ],
        "cmdline": "reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0 root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1791641345459408000  --",
        "env": [
          [
            "HOME",
            "/"
          ],
          [
            "KRUN_BOOT_START_NS",
            "1791641345459408000"
          ],
          [
            "TERM",
            "linux"
          ]
        ],
        "marker": "MINOS-GUEST-BOOT-OK",
        "net_ifaces": [
          "lo"
        ],
        "osrelease": "6.12.109",
        "virtio_devices": [
          "virtio0:0x0004",
          "virtio1:0x0003",
          "virtio2:0x0002",
          "virtio3:0x0002",
          "virtio4:0x0002",
          "virtio5:0x0013",
          "virtio6:0x4d47"
        ]
      }
    },
    {
      "run": {
        "argv": [
          "/bin/cat",
          "/etc/minos-spike"
        ],
        "error": null,
        "signal": null,
        "status": 0,
        "stderr": "",
        "stderr_truncated": false,
        "stdout": "minos-spike-06\n",
        "stdout_truncated": false
      }
    }
  ],
  "v": 1
}
== end boot_from_dockerfile
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 4.42s
```

Result: SPIKE-06 still holds with the new token: same image hash, `run` status 0, stdout `minos-spike-06\n`, and
`net_ifaces` is `["lo"]`.

#### Rerun: the whole `spike_boot` suite (plan verification)

Setup: a fresh unsigned copy, so `unsigned_helper_is_refused` runs against the current launcher: `touch
crates/minos-vmm/src/main.rs && cargo build -p minos-vmm` (relinks), `cp target/debug/minos-vmm $S/minos-vmm-unsigned2`
(`codesign -dvv` shows `flags=0x20002(adhoc,linker-signed)`), then E-4's sign command on `target/debug/minos-vmm`.

Command: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_VMM_UNSIGNED=$S/minos-vmm-unsigned2 MINOS_GUEST=... MINOS_KRUNFW=... cargo test -p minos-backend-krun --test spike_boot -- --ignored --nocapture --test-threads=1`

Exit code: 0

```text

running 3 tests
test boot_initramfs_attempt ... == boot_initramfs_attempt: exit_code Some(0), signal None, killed_at_deadline false, elapsed 517.561958ms, connections 0
-- console (3902 bytes, overflowed false):
[37 console lines omitted by the evidence protocol]
[    0.039000] alg: No test for fips(ansi_cprng) (fips_ansi_cprng)
[    0.039104] alg: No test for 842 (842-scomp)
[    0.039170] alg: No test for 842 (842-generic)
[    0.043353] "cryptomgr_test" (158) uses obsolete ecb(arc4) skcipher
[    0.049290] clk: Disabling unused clocks
[    0.049347] PM: genpd: Disabling unused power domains
[    0.049494] ALSA device list:
[    0.049553]   No soundcards found.
[    0.049628] virtio-fs: tag </dev/root> not found
[    0.049680] virtio-fs: tag </dev/root> not found
[    0.049743] virtio-fs: tag </dev/root> not found
[    0.049801] List of all partitions:
[    0.049847] fe00             404 vda 
[    0.049848]  driver: virtio_blk
[    0.049942] fe10               0 vdb 
[    0.049943]  driver: virtio_blk
[    0.050033] fe20           65536 vdc 
[    0.050034]  driver: virtio_blk
[    0.050125] No filesystem could mount root, tried: 
[    0.050125]  virtiofs
[    0.050174] 
[    0.050265] Kernel panic - not syncing: VFS: Unable to mount root fs on "/dev/root" or unknown-block(0,0)
[    0.050327] CPU: 0 UID: 0 PID: 1 Comm: swapper/0 Not tainted 6.12.109 #1
[    0.050382] Hardware name: linux,dummy-virt (DT)
[    0.050428] Call trace:
[    0.050439]  dump_backtrace+0x94/0x114
[    0.050471]  show_stack+0x18/0x24
[    0.050503]  dump_stack_lvl+0x34/0x8c
[    0.050549]  dump_stack+0x18/0x24
[    0.050581]  panic+0x328/0x33c
[    0.050620]  mount_root_generic+0x35c/0x374
[    0.050652]  mount_root+0x198/0x26c
[    0.050687]  prepare_namespace+0x48/0x2b0
[    0.050719]  kernel_init_freeable+0x194/0x1c4
[    0.050754]  kernel_init+0x24/0x1d4
[    0.050789]  ret_from_fork+0x10/0x20
[    0.050821] Kernel Offset: disabled
[    0.050851] CPU features: 0x00,0000000c,00862928,6627f727
[    0.050878] Memory Limit: none

-- minos-vmm stderr (18 bytes):
minos-vmm: exit 0

-- minos-vmm stdout (0 bytes):

== end boot_initramfs_attempt
ok
test boot_root_disk ... == boot_root_disk: exit_code Some(0), signal None, killed_at_deadline false, elapsed 331.88525ms, connections 1
-- console (3634 bytes, overflowed false):
[27 console lines omitted by the evidence protocol]
[    0.021560] can: raw protocol
[    0.021603] can: broadcast manager protocol
[    0.021649] can: netlink gateway - max_hops=1
[    0.021721] NET: Registered PF_KCM protocol family
[    0.021798] NET: Registered PF_VSOCK protocol family
[    0.022091] NET: Registered PF_TSI protocol family
[    0.022165] NET: Registered PF_TSI6 protocol family
[    0.022214] NET: Registered PF_TSIU protocol family
[    0.022501] registered taskstats version 1
[    0.023071] Btrfs loaded, zoned=yes, fsverity=no
[    0.023187] Key type encrypted registered
[    0.037389] alg: No test for fips(ansi_cprng) (fips_ansi_cprng)
[    0.037504] alg: No test for 842 (842-scomp)
[    0.037572] alg: No test for 842 (842-generic)
[    0.041666] "cryptomgr_test" (158) uses obsolete ecb(arc4) skcipher
[    0.048107] clk: Disabling unused clocks
[    0.048260] PM: genpd: Disabling unused power domains
[    0.048326] ALSA device list:
[    0.048390]   No soundcards found.
[    0.049437] VFS: Mounted root (squashfs filesystem) readonly on device 254:0.
[    0.049701] devtmpfs: mounted
[    0.049836] Freeing unused kernel memory: 2496K
[    0.049921] Run /init as init process
[    0.049983]   with arguments:
[    0.050035]     /init
[    0.050084]   with environment:
[    0.050136]     HOME=/
[    0.050189]     TERM=linux
[    0.050236]     KRUN_BOOT_START_NS=1791641354066244000
MINOS-GUEST-BOOT-OK
minos-guest: cmdline reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0 root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1791641354066244000  --
minos-guest: osrelease 6.12.109
minos-guest: args /init
minos-guest: env HOME=/ KRUN_BOOT_START_NS=1791641354066244000 TERM=linux
minos-guest: net_ifaces lo
minos-guest: virtio_devices virtio0:0x0004 virtio1:0x0003 virtio2:0x0002 virtio3:0x0002 virtio4:0x0002 virtio5:0x0013 virtio6:0x4d47
MINOS-GUEST-FRAME-SENT 696 bytes
MINOS-GUEST-POWEROFF
[    0.082823] reboot: Power down

-- minos-vmm stderr (18 bytes):
minos-vmm: exit 0

-- minos-vmm stdout (0 bytes):

-- connection 0 (696 bytes), frame:
{
  "job_error": null,
  "steps": [
    {
      "report_boot": {
        "args": [
          "/init"
        ],
        "cmdline": "reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0 root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1791641354066244000  --",
        "env": [
          [
            "HOME",
            "/"
          ],
          [
            "KRUN_BOOT_START_NS",
            "1791641354066244000"
          ],
          [
            "TERM",
            "linux"
          ]
        ],
        "marker": "MINOS-GUEST-BOOT-OK",
        "net_ifaces": [
          "lo"
        ],
        "osrelease": "6.12.109",
        "virtio_devices": [
          "virtio0:0x0004",
          "virtio1:0x0003",
          "virtio2:0x0002",
          "virtio3:0x0002",
          "virtio4:0x0002",
          "virtio5:0x0013",
          "virtio6:0x4d47"
        ]
      }
    },
    {
      "scratch_roundtrip": {
        "bytes": 65536,
        "error": null,
        "ok": true
      }
    }
  ],
  "v": 1
}
== end boot_root_disk
normalized cmdline tokens: ["reboot=k", "panic=-1", "panic_print=0", "nomodule", "console=hvc0", "rootfstype=virtiofs", "rw", "quiet", "no-kvmapf", "rootfstype=squashfs", "ro", "ignore_loglevel", "dummy.numdummies=0", "root=/dev/root", "init=/init", "root=/dev/vda", "KRUN_BOOT_START_NS=<ns>", "--"]
osrelease: 6.12.109
ok
test unsigned_helper_is_refused ... == unsigned_helper_is_refused: exit_code Some(70), signal None, killed_at_deadline false, elapsed 468.982917ms, connections 0
-- console (0 bytes, overflowed false):

-- minos-vmm stderr (102 bytes):
minos-vmm: error: starting the VM: build error: start: build_microvm: Internal(Vm(VmSetup(VmCreate)))

-- minos-vmm stdout (0 bytes):

== end unsigned_helper_is_refused
ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.65s
```

### S01-INITRAMFS: the literal initramfs attempt (D-18 negative evidence)

Requirement: SPIKE-01 literal wording (D-18), 02-RESEARCH.md Pattern 2 and Pitfall 1

Setup: the E-5-signed helper; `initramfs_image(guest)` written to the job directory's `initramfs.cpio` and hashed;
`boot.initramfs` mode, so minos-vmm calls `initramfs_path` with no `init_path` and `cmdline("ignore_loglevel")`. The
minimal root disk is attached as `/dev/vda`. The kernel config line beside it (H-02):

```text
210:# CONFIG_BLK_DEV_INITRD is not set
```

Command: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=... MINOS_KRUNFW=... cargo test -p minos-backend-krun --test spike_boot -- --ignored --nocapture --test-threads=1 boot_initramfs_attempt`

Exit code: 0 (test passed; helper exit code 0)

```text

running 1 test
test boot_initramfs_attempt ... == boot_initramfs_attempt: exit_code Some(0), signal None, killed_at_deadline false, elapsed 340.313917ms, connections 0
-- console (3799 bytes, overflowed false):
[    0.022854] virtio_blk virtio2: [vda] 808 512-byte logical blocks (414 kB/404 KiB)
[    0.023593] virtio_blk virtio3: 1/0/0 default/read/poll queues
[    0.023878] virtio_blk virtio3: [vdb] 1 512-byte logical blocks (512 B/512 B)
[    0.024391] Dev vdb: unable to read RDB block 1
[    0.024458]  vdb: unable to read partition table
[    0.024562] vdb: partition table beyond EOD, truncated
[    0.024731] virtio_blk virtio4: 1/0/0 default/read/poll queues
[    0.024917] virtio_blk virtio4: [vdc] 131072 512-byte logical blocks (67.1 MB/64.0 MiB)
[    0.025293] zram: Added device: zram0
[    0.025405] Loading iSCSI transport class v2.0-870.
[    0.025579] tun: Universal TUN/TAP device driver, 1.6
[    0.025652] vcan: Virtual CAN interface driver
[    0.025727] CAN device driver interface
[    0.025863] rtc-pl031 a001000.rtc: registered as rtc0
[    0.025930] rtc-pl031 a001000.rtc: setting system clock to 2026-10-10T13:55:32 UTC (1791640532)
[    0.026070] i2c_dev: i2c /dev entries driver
[    0.026242] pps_ldisc: PPS line discipline registered
[    0.026674] NET: Registered PF_INET6 protocol family
[    0.027173] Segment Routing with IPv6
[    0.027231] In-situ OAM (IOAM) with IPv6
[    0.027294] NET: Registered PF_PACKET protocol family
[    0.027359] Bridge firewalling registered
[    0.027418] can: controller area network core
[    0.027491] NET: Registered PF_CAN protocol family
[    0.027664] can: raw protocol
[    0.027739] can: broadcast manager protocol
[    0.027826] can: netlink gateway - max_hops=1
[    0.028017] NET: Registered PF_KCM protocol family
[    0.028168] NET: Registered PF_VSOCK protocol family
[    0.028540] NET: Registered PF_TSI protocol family
[    0.028644] NET: Registered PF_TSI6 protocol family
[    0.028766] NET: Registered PF_TSIU protocol family
[    0.029525] registered taskstats version 1
[    0.030339] Btrfs loaded, zoned=yes, fsverity=no
[    0.030764] Key type encrypted registered
[    0.043255] alg: No test for fips(ansi_cprng) (fips_ansi_cprng)
[    0.043411] alg: No test for 842 (842-scomp)
[    0.043642] alg: No test for 842 (842-generic)
[    0.048302] "cryptomgr_test" (158) uses obsolete ecb(arc4) skcipher
[    0.049911] clk: Disabling unused clocks
[    0.050030] PM: genpd: Disabling unused power domains
[    0.050259] ALSA device list:
[    0.050369]   No soundcards found.
[    0.050485] virtio-fs: tag </dev/root> not found
[    0.050547] virtio-fs: tag </dev/root> not found
[    0.050621] virtio-fs: tag </dev/root> not found
[    0.050699] List of all partitions:
[    0.050793] fe00             404 vda 
[    0.050794]  driver: virtio_blk
[    0.050942] fe10               0 vdb 
[    0.050942]  driver: virtio_blk
[    0.051034] fe20           65536 vdc 
[    0.051035]  driver: virtio_blk
[    0.051135] No filesystem could mount root, tried: 
[    0.051135]  virtiofs
[    0.051191] 
[    0.051306] Kernel panic - not syncing: VFS: Unable to mount root fs on "/dev/root" or unknown-block(0,0)
[    0.051360] CPU: 0 UID: 0 PID: 1 Comm: swapper/0 Not tainted 6.12.109 #1
[    0.051418] Hardware name: linux,dummy-virt (DT)
[    0.051445] Call trace:
[    0.051486]  dump_backtrace+0x94/0x114
[    0.051608]  show_stack+0x18/0x24
[    0.051678]  dump_stack_lvl+0x34/0x8c
[    0.051925]  dump_stack+0x18/0x24
[    0.051967]  panic+0x328/0x33c
[    0.052229]  mount_root_generic+0x35c/0x374
[    0.052304]  mount_root+0x198/0x26c
[    0.052506]  prepare_namespace+0x48/0x2b0
[    0.052561]  kernel_init_freeable+0x194/0x1c4
[    0.053140]  kernel_init+0x24/0x1d4
[    0.053210]  ret_from_fork+0x10/0x20
[    0.053371] Kernel Offset: disabled
[    0.053743] CPU features: 0x00,0000000c,00862928,6627f727
[    0.054262] Memory Limit: none

-- minos-vmm stderr (18 bytes):
minos-vmm: exit 0

-- minos-vmm stdout (0 bytes):

== end boot_initramfs_attempt
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.81s

```

Result: as predicted. The kernel never ran `/init` from the initramfs: no `MINOS-GUEST-BOOT-OK`, and no connection.
It looked for the virtio-fs tag `/dev/root` (from msb_krun's default `rootfstype=virtiofs` and `root=/dev/root`) and
panicked with `Kernel panic - not syncing: VFS: Unable to mount root fs on "/dev/root" or unknown-block(0,0)`. Because
of `panic=-1`, the helper still exited 0, which is 02-RESEARCH.md Pitfall 1: exit 0 alone never means a boot.

### S06-BOOT: the Dockerfile image boots and runs a command from the image (D-15)

Requirement: SPIKE-06 (D-15), T-02-41

Setup: the E-5-signed helper; 02-04's digest-pinned fixture (`crates/minos-backend-krun/tests/fixtures/rootimg/Dockerfile`).

Command: `docker version --format '{{.Server.Version}}'`

Exit code: 0

```text
29.6.1
```

The daemon was already running; nothing was started or installed (D-10).

Command: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=... MINOS_KRUNFW=... MINOS_DOCKER=$(command -v docker) cargo test -p minos-backend-krun --test spike_rootimg -- --ignored --nocapture --test-threads=1 boot_from_dockerfile`

Exit code: 0 (test passed; helper exit code 0)

```text

running 1 test
test boot_from_dockerfile ... image: 4456448 bytes, sha256 d1e93ef0334243de484b036aec053eed241b77abc8176bd2e3e672aa2eb0a4ae
== boot_from_dockerfile: exit_code Some(0), signal None, killed_at_deadline false, elapsed 358.194416ms, connections 1
-- console (3625 bytes, overflowed false):
[27 console lines omitted by the evidence protocol]
[    0.025013] can: raw protocol
[    0.025040] can: broadcast manager protocol
[    0.025071] can: netlink gateway - max_hops=1
[    0.025135] NET: Registered PF_KCM protocol family
[    0.025196] NET: Registered PF_VSOCK protocol family
[    0.025491] NET: Registered PF_TSI protocol family
[    0.025526] NET: Registered PF_TSI6 protocol family
[    0.025565] NET: Registered PF_TSIU protocol family
[    0.025883] registered taskstats version 1
[    0.026513] Btrfs loaded, zoned=yes, fsverity=no
[    0.026644] Key type encrypted registered
[    0.037897] alg: No test for fips(ansi_cprng) (fips_ansi_cprng)
[    0.038123] alg: No test for 842 (842-generic)
[    0.038270] alg: No test for 842 (842-scomp)
[    0.040434] "cryptomgr_test" (158) uses obsolete ecb(arc4) skcipher
[    0.049709] clk: Disabling unused clocks
[    0.049809] PM: genpd: Disabling unused power domains
[    0.049895] ALSA device list:
[    0.049951]   No soundcards found.
[    0.050509] VFS: Mounted root (squashfs filesystem) readonly on device 254:0.
[    0.051228] devtmpfs: mounted
[    0.051548] Freeing unused kernel memory: 2496K
[    0.051632] Run /init as init process
[    0.051685]   with arguments:
[    0.051731]     /init
[    0.051774]   with environment:
[    0.051823]     HOME=/
[    0.051865]     TERM=linux
[    0.051908]     KRUN_BOOT_START_NS=1791640547021134000
MINOS-GUEST-BOOT-OK
minos-guest: cmdline reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1791640547021134000  --
minos-guest: osrelease 6.12.109
minos-guest: args /init
minos-guest: env HOME=/ KRUN_BOOT_START_NS=1791640547021134000 TERM=linux
minos-guest: net_ifaces dummy0 lo
minos-guest: virtio_devices virtio0:0x0004 virtio1:0x0003 virtio2:0x0002 virtio3:0x0002 virtio4:0x0002 virtio5:0x0013 virtio6:0x4d47
MINOS-GUEST-FRAME-SENT 802 bytes
MINOS-GUEST-POWEROFF
[    0.093873] reboot: Power down

-- minos-vmm stderr (18 bytes):
minos-vmm: exit 0

-- minos-vmm stdout (0 bytes):

-- connection 0 (802 bytes), frame:
{
  "job_error": null,
  "steps": [
    {
      "report_boot": {
        "args": [
          "/init"
        ],
        "cmdline": "reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1791640547021134000  --",
        "env": [
          [
            "HOME",
            "/"
          ],
          [
            "KRUN_BOOT_START_NS",
            "1791640547021134000"
          ],
          [
            "TERM",
            "linux"
          ]
        ],
        "marker": "MINOS-GUEST-BOOT-OK",
        "net_ifaces": [
          "dummy0",
          "lo"
        ],
        "osrelease": "6.12.109",
        "virtio_devices": [
          "virtio0:0x0004",
          "virtio1:0x0003",
          "virtio2:0x0002",
          "virtio3:0x0002",
          "virtio4:0x0002",
          "virtio5:0x0013",
          "virtio6:0x4d47"
        ]
      }
    },
    {
      "run": {
        "argv": [
          "/bin/cat",
          "/etc/minos-spike"
        ],
        "error": null,
        "signal": null,
        "status": 0,
        "stderr": "",
        "stderr_truncated": false,
        "stdout": "minos-spike-06\n",
        "stdout_truncated": false
      }
    }
  ],
  "v": 1
}
== end boot_from_dockerfile
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 5.06s

```

Result: SPIKE-06 is answered. The image built from the digest-pinned Dockerfile (4,456,448 bytes, SHA-256
`d1e93ef0334243de484b036aec053eed241b77abc8176bd2e3e672aa2eb0a4ae`, the same as 02-04 RI-03) booted as the root disk.
The guest's `/init` was the trusted guest (marker and frame), and `/bin/cat /etc/minos-spike`, which comes from the
image (busybox), returned status 0 with stdout `minos-spike-06\n` in the status frame.

### S08: SPIKE-08 not triggered

The decided D-18 path booted in S01-ROOTDISK run 2 (marker, a v1 frame, helper exit 0), and S01-CMDLINE matched E-3
exactly; the only open item, S01-NETIFACES, is a test-contract question about the kernel's `dummy0`, not a failure of
the boot mechanism.

## Plan 02-07: K7 exit map

Date: 2026-10-10

- `rustc --version`: rustc 1.97.1 (8bab26f4f 2026-07-14)
- `cargo --version`: cargo 1.97.1 (c980f4866 2026-06-30)
- `cargo clippy --version`: clippy 0.1.97 (8bab26f4f6 2026-07-14)
- `cargo deny --version`: cargo-deny 0.20.2
- macOS: 27.0, arm64
- HEAD at start: `5e13f6f696c8c20037851874603d6bebd4d154d7`

Redactions: the absolute home directory path is shown as `~`. Nothing else is altered. Cargo progress lines
(`Compiling`, `Checking`, `Finished`, `Running`) are removed.

Console logs: each test prints the console through `GuestBytes::excerpt` (supervised runs) or a capped `Read::take`
plus the same allowlist (directly spawned helpers), then reduces it to the last 40 lines plus every line containing
`MINOS-`, `Kernel panic`, `Attempted to kill init`, or `minos-vmm`. The first line of each console block gives the
total and omitted line counts. The test output below is therefore already in the evidence-protocol shape.

Environment for every VM test (02-06 E-1), set on the command line of each run:

- `MINOS_VMM=$(pwd)/target/debug/minos-vmm`
- `MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest`
- `MINOS_KRUNFW=$(realpath /opt/homebrew/lib/libkrunfw.5.dylib)` (= `/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib`, SHA-256 `59764391005357bbdd5218f3fd522dd8ab63722cbc96144752c7bddc2c8e5217`)

### K7-00: setup

Requirement: SPIKE-01 (K7, D-05)

Setup: 02-06 E-4, with the two-key entitlement set 02-06 K5 settled on (`com.apple.security.hypervisor` and
`com.apple.security.cs.disable-library-validation`). Both builds were already current, so neither relinked; the
re-sign was still run because the orchestrator's `cargo test --workspace` after 02-06 had relinked `minos-vmm`.

Command: the four commands below, each followed by `echo "exit $?"`

Exit code: 0 (all four)

```text
$ cargo build -p minos-guest --release --target aarch64-unknown-linux-musl
exit 0
$ cargo build -p minos-vmm
exit 0
$ codesign --force --sign - --options runtime --entitlements crates/minos-vmm/minos-vmm.entitlements target/debug/minos-vmm
target/debug/minos-vmm: replacing existing signature
exit 0
$ codesign --display --entitlements - target/debug/minos-vmm
Executable=~/Repos/minos/target/debug/minos-vmm
[Dict]
	[Key] com.apple.security.cs.disable-library-validation
	[Value]
		[Bool] true
	[Key] com.apple.security.hypervisor
	[Value]
		[Bool] true
exit 0
```

Re-sign after the DEFECT-01 fix (commit `8bd6881`), which relinked `minos-vmm`. The same four commands, each
followed by `echo "exit $?"`; exit code 0 (all four):

```text
$ cargo build -p minos-guest --release --target aarch64-unknown-linux-musl
exit 0
$ cargo build -p minos-vmm
exit 0
$ codesign --force --sign - --options runtime --entitlements crates/minos-vmm/minos-vmm.entitlements target/debug/minos-vmm
target/debug/minos-vmm: replacing existing signature
exit 0
$ codesign --display --entitlements - target/debug/minos-vmm
Executable=~/Repos/minos/target/debug/minos-vmm
[Dict]
	[Key] com.apple.security.cs.disable-library-validation
	[Value]
		[Bool] true
	[Key] com.apple.security.hypervisor
	[Value]
		[Bool] true
exit 0
```

`cargo test -p minos-backend-krun --test spike_exits --no-run` after the re-sign does not relink the helper:
`codesign -d --entitlements - target/debug/minos-vmm` still shows `com.apple.security.hypervisor`.

### K7-01: guest power-off

Requirement: SPIKE-01 (K7, D-05)

Setup: job `report_boot`, finish `poweroff`; launcher `max_duration_ms` 60000; host deadline 90 s; run through
`supervise::run`.

Command: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest MINOS_KRUNFW=$(realpath /opt/homebrew/lib/libkrunfw.5.dylib) cargo test -p minos-backend-krun --test spike_exits k7_01_poweroff -- --ignored --exact --nocapture --test-threads=1`

Exit code: 0

```text
running 1 test
test k7_01_poweroff ... == k7_01_poweroff: status code Some(0), signal None, killed_at_deadline false, elapsed 580.130958ms, connections 1, valid frames 1
-- console file on disk: 3634 bytes
-- console read back (3634 bytes, overflowed false):
[66 lines; 26 omitted: not in the last 40 and no marker]
[    0.021337] NET: Registered PF_CAN protocol family
[    0.021384] can: raw protocol
[    0.021432] can: broadcast manager protocol
[    0.021479] can: netlink gateway - max_hops=1
[    0.021554] NET: Registered PF_KCM protocol family
[    0.021629] NET: Registered PF_VSOCK protocol family
[    0.021969] NET: Registered PF_TSI protocol family
[    0.022035] NET: Registered PF_TSI6 protocol family
[    0.022083] NET: Registered PF_TSIU protocol family
[    0.022376] registered taskstats version 1
[    0.022964] Btrfs loaded, zoned=yes, fsverity=no
[    0.023075] Key type encrypted registered
[    0.036769] alg: No test for fips(ansi_cprng) (fips_ansi_cprng)
[    0.036847] alg: No test for 842 (842-scomp)
[    0.036909] alg: No test for 842 (842-generic)
[    0.040959] "cryptomgr_test" (158) uses obsolete ecb(arc4) skcipher
[    0.047976] clk: Disabling unused clocks
[    0.048027] PM: genpd: Disabling unused power domains
[    0.048073] ALSA device list:
[    0.048128]   No soundcards found.
[    0.049761] VFS: Mounted root (squashfs filesystem) readonly on device 254:0.
[    0.049883] devtmpfs: mounted
[    0.049997] Freeing unused kernel memory: 2496K
[    0.050055] Run /init as init process
[    0.050101]   with arguments:
[    0.050143]     /init
[    0.050179]   with environment:
[    0.050225]     HOME=/
[    0.050268]     TERM=linux
[    0.050308]     KRUN_BOOT_START_NS=1791641905802615000
MINOS-GUEST-BOOT-OK
minos-guest: cmdline reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0 root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1791641905802615000  --
minos-guest: osrelease 6.12.109
minos-guest: args /init
minos-guest: env HOME=/ KRUN_BOOT_START_NS=1791641905802615000 TERM=linux
minos-guest: net_ifaces lo
minos-guest: virtio_devices virtio0:0x0004 virtio1:0x0003 virtio2:0x0002 virtio3:0x0002 virtio4:0x0002 virtio5:0x0013 virtio6:0x4d47
MINOS-GUEST-FRAME-SENT 635 bytes
MINOS-GUEST-POWEROFF
[    0.073482] reboot: Power down

-- minos-vmm stderr (18 bytes):
minos-vmm: exit 0

-- minos-vmm stdout (0 bytes):

-- connection 0 (635 bytes): v1 frame, steps ["report_boot"]
== end k7_01_poweroff
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 1.04s
```

Observed: exit 0, stderr `minos-vmm: exit 0` (the `on_exit` line), one v1 frame. Matches E-2.

### K7-02: guest init returns

Requirement: SPIKE-01 (K7, D-05)

Setup: job `report_boot`, finish `return` (PID 1 returns from `main`); `max_duration_ms` 60000; host deadline 2 min.

First run, with E-2's expectation that the console contains `Attempted to kill init`:

Command: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest MINOS_KRUNFW=$(realpath /opt/homebrew/lib/libkrunfw.5.dylib) cargo test -p minos-backend-krun --test spike_exits k7_02_init_returns -- --ignored --exact --nocapture --test-threads=1`

Exit code: 101

```text
running 1 test
test k7_02_init_returns ... == k7_02_init_returns: status code Some(0), signal None, killed_at_deadline false, elapsed 313.094375ms, connections 1, valid frames 1
-- console file on disk: 3651 bytes
-- console read back (3651 bytes, overflowed false):
[65 lines; 25 omitted: not in the last 40 and no marker]
[    0.021141] can: controller area network core
[    0.021193] NET: Registered PF_CAN protocol family
[    0.021241] can: raw protocol
[    0.021289] can: broadcast manager protocol
[    0.021336] can: netlink gateway - max_hops=1
[    0.021424] NET: Registered PF_KCM protocol family
[    0.021498] NET: Registered PF_VSOCK protocol family
[    0.021801] NET: Registered PF_TSI protocol family
[    0.021856] NET: Registered PF_TSI6 protocol family
[    0.021903] NET: Registered PF_TSIU protocol family
[    0.022187] registered taskstats version 1
[    0.022752] Btrfs loaded, zoned=yes, fsverity=no
[    0.022868] Key type encrypted registered
[    0.036737] alg: No test for fips(ansi_cprng) (fips_ansi_cprng)
[    0.036860] alg: No test for 842 (842-scomp)
[    0.036924] alg: No test for 842 (842-generic)
[    0.041016] "cryptomgr_test" (158) uses obsolete ecb(arc4) skcipher
[    0.048004] clk: Disabling unused clocks
[    0.048062] PM: genpd: Disabling unused power domains
[    0.048125] ALSA device list:
[    0.048172]   No soundcards found.
[    0.049320] VFS: Mounted root (squashfs filesystem) readonly on device 254:0.
[    0.049536] devtmpfs: mounted
[    0.049651] Freeing unused kernel memory: 2496K
[    0.049715] Run /init as init process
[    0.049760]   with arguments:
[    0.049803]     /init
[    0.049844]   with environment:
[    0.049890]     HOME=/
[    0.049929]     TERM=linux
[    0.049971]     KRUN_BOOT_START_NS=1791641906612855000
MINOS-GUEST-BOOT-OK
minos-guest: cmdline reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0 root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1791641906612855000  --
minos-guest: osrelease 6.12.109
minos-guest: args /init
minos-guest: env HOME=/ KRUN_BOOT_START_NS=1791641906612855000 TERM=linux
minos-guest: net_ifaces lo
minos-guest: virtio_devices virtio0:0x0004 virtio1:0x0003 virtio2:0x0002 virtio3:0x0002 virtio4:0x0002 virtio5:0x0013 virtio6:0x4d47
MINOS-GUEST-FRAME-SENT 635 bytes
[    0.069781] reboot: Failed to start orderly reboot: forcing the issue

-- minos-vmm stderr (18 bytes):
minos-vmm: exit 0

-- minos-vmm stdout (0 bytes):

-- connection 0 (635 bytes): v1 frame, steps ["report_boot"]
== end k7_02_init_returns

thread 'k7_02_init_returns' (2602771) panicked at crates/minos-backend-krun/tests/spike_exits.rs:532:5:
the console lacks the init-exit panic
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
FAILED

failures:

failures:
    k7_02_init_returns

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 10 filtered out; finished in 0.74s

error: test failed, to rerun pass `-p minos-backend-krun --test spike_exits`
```

The helper exited 0 with one frame, as predicted, but the kernel did not panic. The console ends with
`reboot: Failed to start orderly reboot: forcing the issue`. `strings` on the dylib finds that line and no
`Attempted to kill init` at all:

Command: `strings /opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib | grep -iE 'kill init|init exit|exitcode=|orderly reboot|forcing the issue'`

Exit code: 0

```text
4reboot: Failed to start orderly reboot: forcing the issue
4reboot: Failed to start orderly shutdown: forcing the issue
```

Explanation (source): libkrunfw v5.6.2 carries two kernel patches that replace the init-exit panic.
`gh api 'repos/libkrun/libkrunfw/contents/patches/0001-krunfw-Don-t-panic-when-init-dies.patch?ref=v5.6.2'` shows, in
`kernel/exit.c` `do_exit`:

```text
 		if (unlikely(is_global_init(tsk)))
+#ifdef VERBOSE_LIBKRUN
 			panic("Attempted to kill init! exitcode=0x%08x\n",
 				tsk->signal->group_exit_code ?: (int)code);
+#else
+            orderly_reboot();
+#endif
```

and `0002-krunfw-Ignore-run_cmd-on-orderly-reboot.patch` makes `__orderly_reboot` skip `run_cmd(reboot_cmd)`
(`ret = 1`), so it prints `Failed to start orderly reboot: forcing the issue` and falls back to an emergency sync
and `kernel_restart`. The restart is PSCI `SYSTEM_RESET`, which msb_krun treats like `SYSTEM_OFF`
(msb_krun_hvf 0.1.41 `src/lib.rs` lines 1105-1111: `0x8400_0008` and `0x8400_0009` both return
`VcpuExit::Shutdown`), so the helper still exits 0. 02-RESEARCH.md Pattern 3 has the right status (0, indistinguishable from power-off) but the wrong mechanism:
there is no panic and `panic=-1` is not involved. The kernel code is still never visible to the host.

Change: the K7-02 assertion now requires the orderly-reboot line and the absence of `Attempted to kill init`. Rerun:

Command: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest MINOS_KRUNFW=$(realpath /opt/homebrew/lib/libkrunfw.5.dylib) cargo test -p minos-backend-krun --test spike_exits k7_02_init_returns -- --ignored --exact --nocapture --test-threads=1`

Exit code: 0

```text
running 1 test
test k7_02_init_returns ... == k7_02_init_returns: status code Some(0), signal None, killed_at_deadline false, elapsed 312.324875ms, connections 1, valid frames 1
-- console file on disk: 3651 bytes
-- console read back (3651 bytes, overflowed false):
[65 lines; 25 omitted: not in the last 40 and no marker]
[    0.021275] can: controller area network core
[    0.021327] NET: Registered PF_CAN protocol family
[    0.021374] can: raw protocol
[    0.021415] can: broadcast manager protocol
[    0.021460] can: netlink gateway - max_hops=1
[    0.021527] NET: Registered PF_KCM protocol family
[    0.021604] NET: Registered PF_VSOCK protocol family
[    0.021888] NET: Registered PF_TSI protocol family
[    0.021930] NET: Registered PF_TSI6 protocol family
[    0.021978] NET: Registered PF_TSIU protocol family
[    0.022265] registered taskstats version 1
[    0.022854] Btrfs loaded, zoned=yes, fsverity=no
[    0.022964] Key type encrypted registered
[    0.036766] alg: No test for fips(ansi_cprng) (fips_ansi_cprng)
[    0.036850] alg: No test for 842 (842-scomp)
[    0.036915] alg: No test for 842 (842-generic)
[    0.040955] "cryptomgr_test" (158) uses obsolete ecb(arc4) skcipher
[    0.048014] clk: Disabling unused clocks
[    0.048064] PM: genpd: Disabling unused power domains
[    0.048113] ALSA device list:
[    0.048162]   No soundcards found.
[    0.049875] VFS: Mounted root (squashfs filesystem) readonly on device 254:0.
[    0.050002] devtmpfs: mounted
[    0.050114] Freeing unused kernel memory: 2496K
[    0.050183] Run /init as init process
[    0.050224]   with arguments:
[    0.050277]     /init
[    0.050318]   with environment:
[    0.050366]     HOME=/
[    0.050408]     TERM=linux
[    0.050449]     KRUN_BOOT_START_NS=1791641958823323000
MINOS-GUEST-BOOT-OK
minos-guest: cmdline reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0 root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1791641958823323000  --
minos-guest: osrelease 6.12.109
minos-guest: args /init
minos-guest: env HOME=/ KRUN_BOOT_START_NS=1791641958823323000 TERM=linux
minos-guest: net_ifaces lo
minos-guest: virtio_devices virtio0:0x0004 virtio1:0x0003 virtio2:0x0002 virtio3:0x0002 virtio4:0x0002 virtio5:0x0013 virtio6:0x4d47
MINOS-GUEST-FRAME-SENT 635 bytes
[    0.073582] reboot: Failed to start orderly reboot: forcing the issue

-- minos-vmm stderr (18 bytes):
minos-vmm: exit 0

-- minos-vmm stdout (0 bytes):

-- connection 0 (635 bytes): v1 frame, steps ["report_boot"]
== end k7_02_init_returns
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 0.78s
```

### K7-03: launcher deadline

Requirement: SPIKE-01 (K7, D-05)

Setup: job `report_boot`, `hang`; `max_duration_ms` 5000; host deadline 30 s.

Command: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest MINOS_KRUNFW=$(realpath /opt/homebrew/lib/libkrunfw.5.dylib) cargo test -p minos-backend-krun --test spike_exits k7_03_deadline -- --ignored --exact --nocapture --test-threads=1`

Exit code: 0

```text
running 1 test
test k7_03_deadline ... == k7_03_deadline: status code Some(124), signal None, killed_at_deadline false, elapsed 5.217782208s, connections 0, valid frames 0
-- console file on disk: 3543 bytes
-- console read back (3543 bytes, overflowed false):
[63 lines; 23 omitted: not in the last 40 and no marker]
[    0.021211] NET: Registered PF_PACKET protocol family
[    0.021263] Bridge firewalling registered
[    0.021311] can: controller area network core
[    0.021368] NET: Registered PF_CAN protocol family
[    0.021415] can: raw protocol
[    0.021463] can: broadcast manager protocol
[    0.021510] can: netlink gateway - max_hops=1
[    0.021586] NET: Registered PF_KCM protocol family
[    0.021661] NET: Registered PF_VSOCK protocol family
[    0.021950] NET: Registered PF_TSI protocol family
[    0.021997] NET: Registered PF_TSI6 protocol family
[    0.022045] NET: Registered PF_TSIU protocol family
[    0.022331] registered taskstats version 1
[    0.022922] Btrfs loaded, zoned=yes, fsverity=no
[    0.023033] Key type encrypted registered
[    0.036759] alg: No test for fips(ansi_cprng) (fips_ansi_cprng)
[    0.036850] alg: No test for 842 (842-scomp)
[    0.036912] alg: No test for 842 (842-generic)
[    0.040972] "cryptomgr_test" (158) uses obsolete ecb(arc4) skcipher
[    0.048215] clk: Disabling unused clocks
[    0.048265] PM: genpd: Disabling unused power domains
[    0.048313] ALSA device list:
[    0.048358]   No soundcards found.
[    0.048685] VFS: Mounted root (squashfs filesystem) readonly on device 254:0.
[    0.048834] devtmpfs: mounted
[    0.048951] Freeing unused kernel memory: 2496K
[    0.049015] Run /init as init process
[    0.049061]   with arguments:
[    0.049108]     /init
[    0.049149]   with environment:
[    0.049196]     HOME=/
[    0.049237]     TERM=linux
[    0.049276]     KRUN_BOOT_START_NS=1791641907419489000
MINOS-GUEST-BOOT-OK
minos-guest: cmdline reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0 root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1791641907419489000  --
minos-guest: osrelease 6.12.109
minos-guest: args /init
minos-guest: env HOME=/ KRUN_BOOT_START_NS=1791641907419489000 TERM=linux
minos-guest: net_ifaces lo
minos-guest: virtio_devices virtio0:0x0004 virtio1:0x0003 virtio2:0x0002 virtio3:0x0002 virtio4:0x0002 virtio5:0x0013 virtio6:0x4d47

-- minos-vmm stderr (49 bytes):
minos-vmm: watchdog deadline
minos-vmm: exit 124

-- minos-vmm stdout (0 bytes):

== end k7_03_deadline
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 5.66s
```

Observed: exit 124 after 5.22 s (the supervisor did not kill it), stderr `minos-vmm: watchdog deadline` and
`minos-vmm: exit 124`, no connection. Matches E-2.

### K7-04: console cap

Requirement: SPIKE-01 (K7, D-05), T-02-45

Setup: job `report_boot`, `console_flood` 8388608 bytes, finish `poweroff`; launcher console cap 262144 bytes;
`max_duration_ms` 60000; host deadline 2 min; supervisor console read-back cap 16 MiB (so the overshoot is read in
full).

Command: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest MINOS_KRUNFW=$(realpath /opt/homebrew/lib/libkrunfw.5.dylib) cargo test -p minos-backend-krun --test spike_exits k7_04_console_cap -- --ignored --exact --nocapture --test-threads=1`

Exit code: 0

```text
running 1 test
test k7_04_console_cap ... == k7_04_console_cap: status code Some(125), signal None, killed_at_deadline false, elapsed 543.797833ms, connections 0, valid frames 0
-- console file on disk: 311098 bytes
-- console read back (311098 bytes, overflowed false):
[3860 lines; 3819 omitted: not in the last 40 and no marker]
MINOS-GUEST-BOOT-OK
jklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghij
lmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqabcdefghijklmnopqrstu
wxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvw
yzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxy
abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqabcdefghij
lmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijkl
nopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmn
pqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnop
abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyza
cdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabc
efghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcde
ghijklmnopqabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnop
rstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqr
tuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrst
vwxyzabcdefghijklmnopqabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcde
ghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefg
ijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghi
klmnopqrstuvwxyzabcdefghijklmnopqabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrst
vwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuv
xyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwx
zabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqabcdefghijklmnopqrstuvwxyzabcdefghi
klmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijk
mnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklm
opqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqabcdefghijklmnopqrstuvwx
zabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz
bcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzab
defghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqabcdefghijklm
opqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmno
qrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopq
stuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqab
defghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcd
fghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdef
hijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefgh
jklmnopqabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrs
uvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstu
wxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvw
yzabcdefghijklmnopqabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefgh
jklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghij
lmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijkl
nopqrstuvwxyzabcdefghijklmnopqabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvw

-- minos-vmm stderr (52 bytes):
minos-vmm: watchdog console-cap
minos-vmm: exit 125

-- minos-vmm stdout (0 bytes):

== end k7_04_console_cap
console file size 311098 bytes against the 262144-byte cap: overshoot 48954 bytes
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 1.00s
```

Observed: exit 125 after 0.54 s, stderr `minos-vmm: watchdog console-cap` and `minos-vmm: exit 125`, no frame (the
flood step never finished). The console file is 311098 bytes against the 262144-byte cap: an overshoot of 48954
bytes (18.7 % of the cap) at the watchdog's 50 ms poll (Pitfall 8). Phase 5 must size any host disk budget for the
cap plus at least one poll interval of console throughput, or replace the file console with a capped writer.

### K7-05: SIGTERM to the helper

Requirement: SPIKE-01 (K7, D-05)

Setup: job `report_boot`, `hang`; `max_duration_ms` 60000. The helper is spawned directly (same argv, empty
environment, `/dev/null` stdin, files for stdout and stderr, own process group). After the console shows
`MINOS-GUEST-BOOT-OK`, the test runs `/bin/kill -TERM <pid>` as an argv array.

Command: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest MINOS_KRUNFW=$(realpath /opt/homebrew/lib/libkrunfw.5.dylib) cargo test -p minos-backend-krun --test spike_exits k7_05_sigterm -- --ignored --exact --nocapture --test-threads=1`

Exit code: 0

```text
running 1 test
test k7_05_sigterm ... k7_05_sigterm: helper pid 62325 showed the boot marker after 321.404334ms
/bin/kill -TERM 62325: code Some(0), signal None
== k7_05_sigterm: status code Some(0), signal None, exited 55.042625ms after SIGTERM, pending connections 0
-- console (3543 bytes read, cap 1048576):
[63 lines; 23 omitted: not in the last 40 and no marker]
[    0.024122] NET: Registered PF_PACKET protocol family
[    0.024235] Bridge firewalling registered
[    0.024291] can: controller area network core
[    0.024348] NET: Registered PF_CAN protocol family
[    0.024402] can: raw protocol
[    0.024447] can: broadcast manager protocol
[    0.024495] can: netlink gateway - max_hops=1
[    0.024583] NET: Registered PF_KCM protocol family
[    0.024670] NET: Registered PF_VSOCK protocol family
[    0.025109] NET: Registered PF_TSI protocol family
[    0.025183] NET: Registered PF_TSI6 protocol family
[    0.025292] NET: Registered PF_TSIU protocol family
[    0.025594] registered taskstats version 1
[    0.026200] Btrfs loaded, zoned=yes, fsverity=no
[    0.026352] Key type encrypted registered
[    0.036612] alg: No test for fips(ansi_cprng) (fips_ansi_cprng)
[    0.036775] alg: No test for 842 (842-generic)
[    0.036825] alg: No test for 842 (842-scomp)
[    0.039888] "cryptomgr_test" (159) uses obsolete ecb(arc4) skcipher
[    0.040471] clk: Disabling unused clocks
[    0.040520] PM: genpd: Disabling unused power domains
[    0.040576] ALSA device list:
[    0.040623]   No soundcards found.
[    0.050643] VFS: Mounted root (squashfs filesystem) readonly on device 254:0.
[    0.051231] devtmpfs: mounted
[    0.051602] Freeing unused kernel memory: 2496K
[    0.051672] Run /init as init process
[    0.051715]   with arguments:
[    0.051766]     /init
[    0.051811]   with environment:
[    0.051857]     HOME=/
[    0.051899]     TERM=linux
[    0.051942]     KRUN_BOOT_START_NS=1791641965424195000
MINOS-GUEST-BOOT-OK
minos-guest: cmdline reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0 root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1791641965424195000  --
minos-guest: osrelease 6.12.109
minos-guest: args /init
minos-guest: env HOME=/ KRUN_BOOT_START_NS=1791641965424195000 TERM=linux
minos-guest: net_ifaces lo
minos-guest: virtio_devices virtio0:0x0004 virtio1:0x0003 virtio2:0x0002 virtio3:0x0002 virtio4:0x0002 virtio5:0x0013 virtio6:0x4d47

-- minos-vmm stderr (18 bytes):
minos-vmm: exit 0

-- minos-vmm stdout (0 bytes):

== end k7_05_sigterm
ps after exit: exit code Some(1), signal None; 
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 0.82s
```

Observed: exit 0 within 55 ms of SIGTERM, stderr `minos-vmm: exit 0`, no frame. As research predicted, a SIGTERM
cannot be told apart from a guest power-off by the status alone.

### K7-06: SIGKILL to the helper

Requirement: SPIKE-01 (K7, D-05), T-02-44

Setup: as K7-05, but `Child::kill` (SIGKILL) after the boot marker.

Command: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest MINOS_KRUNFW=$(realpath /opt/homebrew/lib/libkrunfw.5.dylib) cargo test -p minos-backend-krun --test spike_exits k7_06_sigkill -- --ignored --exact --nocapture --test-threads=1`

Exit code: 0

```text
running 1 test
test k7_06_sigkill ... k7_06_sigkill: helper pid 62332 showed the boot marker after 330.304458ms
== k7_06_sigkill: status code None, signal Some(9), reaped 55.025083ms after Child::kill, pending connections 0
-- console (3543 bytes read, cap 1048576):
[63 lines; 23 omitted: not in the last 40 and no marker]
[    0.021273] NET: Registered PF_PACKET protocol family
[    0.021323] Bridge firewalling registered
[    0.021367] can: controller area network core
[    0.021419] NET: Registered PF_CAN protocol family
[    0.021467] can: raw protocol
[    0.021510] can: broadcast manager protocol
[    0.021554] can: netlink gateway - max_hops=1
[    0.021634] NET: Registered PF_KCM protocol family
[    0.021716] NET: Registered PF_VSOCK protocol family
[    0.022005] NET: Registered PF_TSI protocol family
[    0.022053] NET: Registered PF_TSI6 protocol family
[    0.022099] NET: Registered PF_TSIU protocol family
[    0.022393] registered taskstats version 1
[    0.022975] Btrfs loaded, zoned=yes, fsverity=no
[    0.023110] Key type encrypted registered
[    0.036960] alg: No test for fips(ansi_cprng) (fips_ansi_cprng)
[    0.037099] alg: No test for 842 (842-scomp)
[    0.037169] alg: No test for 842 (842-generic)
[    0.041301] "cryptomgr_test" (159) uses obsolete ecb(arc4) skcipher
[    0.048027] clk: Disabling unused clocks
[    0.048080] PM: genpd: Disabling unused power domains
[    0.048126] ALSA device list:
[    0.048173]   No soundcards found.
[    0.048927] VFS: Mounted root (squashfs filesystem) readonly on device 254:0.
[    0.049102] devtmpfs: mounted
[    0.049305] Freeing unused kernel memory: 2496K
[    0.049368] Run /init as init process
[    0.049416]   with arguments:
[    0.049457]     /init
[    0.049498]   with environment:
[    0.049542]     HOME=/
[    0.049583]     TERM=linux
[    0.049623]     KRUN_BOOT_START_NS=1791641966308425000
MINOS-GUEST-BOOT-OK
minos-guest: cmdline reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0 root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1791641966308425000  --
minos-guest: osrelease 6.12.109
minos-guest: args /init
minos-guest: env HOME=/ KRUN_BOOT_START_NS=1791641966308425000 TERM=linux
minos-guest: net_ifaces lo
minos-guest: virtio_devices virtio0:0x0004 virtio1:0x0003 virtio2:0x0002 virtio3:0x0002 virtio4:0x0002 virtio5:0x0013 virtio6:0x4d47

-- minos-vmm stderr (0 bytes):

-- minos-vmm stdout (0 bytes):

== end k7_06_sigkill
ps after reaping: exit code Some(1), signal None; 
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 0.83s
```

Observed: terminated by signal 9, no stderr line (`on_exit` never runs), and `/bin/ps -p <pid>` exits 1 once it is
reaped. Matches E-2.

### K7-07: helper orphaned (parent gone)

Requirement: SPIKE-01 (K7, D-05), T-02-44, T-02-48

Setup: job `report_boot`, `hang`; `max_duration_ms` 60000. The test runs its own binary (`current_exe`) with argv
`orphan_launcher --exact --ignored --nocapture --test-threads=1`, an empty environment, and `MINOS_VMM`,
`MINOS_SPIKE_ORPHAN_CONFIG`, `MINOS_SPIKE_ORPHAN_DIR` (E-1). The intermediate spawns the helper, writes its PID, and
returns, so the helper is reparented. The test then polls `/bin/ps -p <pid>` for 15 s. If the helper is still there,
the test's drop guard sends it `/bin/kill -KILL`.

First run:

Command: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest MINOS_KRUNFW=$(realpath /opt/homebrew/lib/libkrunfw.5.dylib) cargo test -p minos-backend-krun --test spike_exits k7_07_parent_gone -- --ignored --exact --nocapture --test-threads=1`

Exit code: 101

```text
running 1 test
test k7_07_parent_gone ... == intermediate (this test binary, orphan_launcher): status code Some(0), signal None
-- stdout:

running 1 test
test orphan_launcher ... orphan_launcher: pid 62338 spawned minos-vmm pid 62339; returning without waiting
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 0.00s


-- stderr:

ps right after the intermediate exited: exit code Some(0), signal None; 62339     1 R    ~/Repos/minos/target/debug/minos-vmm
== k7_07_parent_gone: helper pid 62339 gone after None (limit 15s), pending connections 0
-- console (3543 bytes read, cap 1048576):
[63 lines; 23 omitted: not in the last 40 and no marker]
[    0.021334] NET: Registered PF_PACKET protocol family
[    0.021387] Bridge firewalling registered
[    0.021434] can: controller area network core
[    0.021482] NET: Registered PF_CAN protocol family
[    0.021528] can: raw protocol
[    0.021572] can: broadcast manager protocol
[    0.021622] can: netlink gateway - max_hops=1
[    0.021713] NET: Registered PF_KCM protocol family
[    0.021800] NET: Registered PF_VSOCK protocol family
[    0.022111] NET: Registered PF_TSI protocol family
[    0.022158] NET: Registered PF_TSI6 protocol family
[    0.022207] NET: Registered PF_TSIU protocol family
[    0.022491] registered taskstats version 1
[    0.023080] Btrfs loaded, zoned=yes, fsverity=no
[    0.023193] Key type encrypted registered
[    0.037073] alg: No test for fips(ansi_cprng) (fips_ansi_cprng)
[    0.037159] alg: No test for 842 (842-scomp)
[    0.037221] alg: No test for 842 (842-generic)
[    0.041294] "cryptomgr_test" (159) uses obsolete ecb(arc4) skcipher
[    0.048148] clk: Disabling unused clocks
[    0.048207] PM: genpd: Disabling unused power domains
[    0.048254] ALSA device list:
[    0.048301]   No soundcards found.
[    0.049108] VFS: Mounted root (squashfs filesystem) readonly on device 254:0.
[    0.049259] devtmpfs: mounted
[    0.049372] Freeing unused kernel memory: 2496K
[    0.049429] Run /init as init process
[    0.049472]   with arguments:
[    0.049518]     /init
[    0.049559]   with environment:
[    0.049605]     HOME=/
[    0.049645]     TERM=linux
[    0.049686]     KRUN_BOOT_START_NS=1791641967206832000
MINOS-GUEST-BOOT-OK
minos-guest: cmdline reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0 root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1791641967206832000  --
minos-guest: osrelease 6.12.109
minos-guest: args /init
minos-guest: env HOME=/ KRUN_BOOT_START_NS=1791641967206832000 TERM=linux
minos-guest: net_ifaces lo
minos-guest: virtio_devices virtio0:0x0004 virtio1:0x0003 virtio2:0x0002 virtio3:0x0002 virtio4:0x0002 virtio5:0x0013 virtio6:0x4d47

-- minos-vmm stderr (0 bytes):

-- minos-vmm stdout (0 bytes):

== end k7_07_parent_gone

thread 'k7_07_parent_gone' (2605375) panicked at crates/minos-backend-krun/tests/spike_exits.rs:707:5:
the orphaned helper outlived the 15 s limit
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
FAILED

failures:

failures:
    k7_07_parent_gone

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 10 filtered out; finished in 15.46s

error: test failed, to rerun pass `-p minos-backend-krun --test spike_exits`
```

Second run, after `orphan_scenario` was factored out and `ps at the limit` was added (the same code path):

Command: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest MINOS_KRUNFW=$(realpath /opt/homebrew/lib/libkrunfw.5.dylib) cargo test -p minos-backend-krun --test spike_exits k7_07_parent_gone -- --ignored --exact --nocapture --test-threads=1`

Exit code: 101

```text
running 1 test
test k7_07_parent_gone ... == intermediate (this test binary, orphan_launcher): status code Some(0), signal None
-- stdout:

running 1 test
test orphan_launcher ... orphan_launcher: pid 62887 spawned minos-vmm pid 62888; returning without waiting for it to exit
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 11 filtered out; finished in 0.00s


-- stderr:

ps right after the intermediate exited: exit code Some(0), signal None; 62888     1 R    ~/Repos/minos/target/debug/minos-vmm
== k7_07_parent_gone: helper pid 62888 gone after None (limit 15s), pending connections 0
ps at the limit: exit code Some(0), signal None; 62888     1 S    ~/Repos/minos/target/debug/minos-vmm
-- console (3543 bytes read, cap 1048576):
[63 lines; 23 omitted: not in the last 40 and no marker]
[    0.021364] NET: Registered PF_PACKET protocol family
[    0.021417] Bridge firewalling registered
[    0.021469] can: controller area network core
[    0.021534] NET: Registered PF_CAN protocol family
[    0.021580] can: raw protocol
[    0.021626] can: broadcast manager protocol
[    0.021674] can: netlink gateway - max_hops=1
[    0.021750] NET: Registered PF_KCM protocol family
[    0.021829] NET: Registered PF_VSOCK protocol family
[    0.022131] NET: Registered PF_TSI protocol family
[    0.022179] NET: Registered PF_TSI6 protocol family
[    0.022228] NET: Registered PF_TSIU protocol family
[    0.022515] registered taskstats version 1
[    0.023107] Btrfs loaded, zoned=yes, fsverity=no
[    0.023224] Key type encrypted registered
[    0.037383] alg: No test for fips(ansi_cprng) (fips_ansi_cprng)
[    0.037518] alg: No test for 842 (842-scomp)
[    0.037586] alg: No test for 842 (842-generic)
[    0.041648] "cryptomgr_test" (159) uses obsolete ecb(arc4) skcipher
[    0.048754] clk: Disabling unused clocks
[    0.048812] PM: genpd: Disabling unused power domains
[    0.048861] ALSA device list:
[    0.048911]   No soundcards found.
[    0.049432] VFS: Mounted root (squashfs filesystem) readonly on device 254:0.
[    0.049607] devtmpfs: mounted
[    0.049732] Freeing unused kernel memory: 2496K
[    0.049796] Run /init as init process
[    0.049839]   with arguments:
[    0.049886]     /init
[    0.049929]   with environment:
[    0.050019]     HOME=/
[    0.050062]     TERM=linux
[    0.050104]     KRUN_BOOT_START_NS=1791642043468699000
MINOS-GUEST-BOOT-OK
minos-guest: cmdline reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0 root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1791642043468699000  --
minos-guest: osrelease 6.12.109
minos-guest: args /init
minos-guest: env HOME=/ KRUN_BOOT_START_NS=1791642043468699000 TERM=linux
minos-guest: net_ifaces lo
minos-guest: virtio_devices virtio0:0x0004 virtio1:0x0003 virtio2:0x0002 virtio3:0x0002 virtio4:0x0002 virtio5:0x0013 virtio6:0x4d47

-- minos-vmm stderr (0 bytes):

-- minos-vmm stdout (0 bytes):

== end k7_07_parent_gone

thread 'k7_07_parent_gone' (2608307) panicked at crates/minos-backend-krun/tests/spike_exits.rs:717:5:
the orphaned helper outlived the 15 s limit
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
FAILED

failures:

failures:
    k7_07_parent_gone

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 11 filtered out; finished in 15.51s

error: test failed, to rerun pass `-p minos-backend-krun --test spike_exits`
```

Observed, twice: the helper's parent is already PID 1 right after the intermediate exits, the guest boots, and the
helper is still running at 15 s with an empty stderr. The parent-gone watchdog never trips. The drop guard killed it
with SIGKILL (K7-11 finds nothing left). Without the guard it would have run to its 60 s deadline and exited 124.
This differs from E-2 and is a defect in shared code: see DEFECT-01. The test is left asserting E-2's expectation.

Third run, after the DEFECT-01 fix (commit `8bd6881`) and the re-sign recorded under K7-00. The test is unchanged:
it still asserts E-2's expectation.

Command: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest MINOS_KRUNFW=$(realpath /opt/homebrew/lib/libkrunfw.5.dylib) cargo test -p minos-backend-krun --test spike_exits k7_07_parent_gone -- --ignored --exact --nocapture --test-threads=1`

Exit code: 0

```text
running 1 test
test k7_07_parent_gone ... == intermediate (this test binary, orphan_launcher): status code Some(0), signal None
-- stdout:

running 1 test
test orphan_launcher ... orphan_launcher: pid 64544 spawned minos-vmm pid 64545; returning without waiting for it to exit
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 11 filtered out; finished in 0.00s


-- stderr:

ps right after the intermediate exited: exit code Some(0), signal None; 64545     1 S    ~/Repos/minos/target/debug/minos-vmm
== k7_07_parent_gone: helper pid 64545 gone after Some(505.159292ms) (limit 15s), pending connections 0
-- console (0 bytes read, cap 1048576):
[0 lines; 0 omitted: not in the last 40 and no marker]

-- minos-vmm stderr (52 bytes):
minos-vmm: watchdog parent-gone
minos-vmm: exit 126

-- minos-vmm stdout (0 bytes):

== end k7_07_parent_gone
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 11 filtered out; finished in 0.96s
```

Observed: the helper was reparented to PID 1 before its first `ps` sample, as in the failing runs, and was gone
505 ms after the intermediate exited, before the guest wrote any console output, with stderr
`minos-vmm: watchdog parent-gone` and `minos-vmm: exit 126`. Matches E-2. The current parent is PID 1, so the check
trips whatever PID `main` recorded; if `main` ran before the intermediate exited, the recorded parent (the
intermediate) also differs from the current one. The full-suite rerun under K7-11 repeats it (gone after 285 ms). T-02-44 is closed by the fix.

### K7-07b: helper orphaned after the guest booted (diagnostic for DEFECT-01)

Requirement: SPIKE-01 (K7, D-05), T-02-44

Setup: as K7-07, plus `MINOS_SPIKE_ORPHAN_WAIT_BOOT=1`, which makes `orphan_launcher` return only after the console
shows `MINOS-GUEST-BOOT-OK`. By then the helper's watchdog has already recorded the intermediate as its parent.

Command: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest MINOS_KRUNFW=$(realpath /opt/homebrew/lib/libkrunfw.5.dylib) cargo test -p minos-backend-krun --test spike_exits k7_07b_parent_gone_after_boot -- --ignored --exact --nocapture --test-threads=1`

Exit code: 0

```text
running 1 test
test k7_07b_parent_gone_after_boot ... == intermediate (this test binary, orphan_launcher): status code Some(0), signal None
-- stdout:

running 1 test
test orphan_launcher ... orphan_launcher: boot marker after 324.821958ms
orphan_launcher: pid 62879 spawned minos-vmm pid 62880; returning without waiting for it to exit
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 11 filtered out; finished in 0.33s


-- stderr:

ps right after the intermediate exited: exit code Some(0), signal None; 62880     1 S    ~/Repos/minos/target/debug/minos-vmm
== k7_07b_parent_gone_after_boot: helper pid 62880 gone after Some(59.47275ms) (limit 15s), pending connections 0
-- console (3543 bytes read, cap 1048576):
[63 lines; 23 omitted: not in the last 40 and no marker]
[    0.021987] NET: Registered PF_PACKET protocol family
[    0.022039] Bridge firewalling registered
[    0.022084] can: controller area network core
[    0.022136] NET: Registered PF_CAN protocol family
[    0.022186] can: raw protocol
[    0.022234] can: broadcast manager protocol
[    0.022292] can: netlink gateway - max_hops=1
[    0.022357] NET: Registered PF_KCM protocol family
[    0.022431] NET: Registered PF_VSOCK protocol family
[    0.022763] NET: Registered PF_TSI protocol family
[    0.022826] NET: Registered PF_TSI6 protocol family
[    0.022871] NET: Registered PF_TSIU protocol family
[    0.023277] registered taskstats version 1
[    0.023899] Btrfs loaded, zoned=yes, fsverity=no
[    0.024049] Key type encrypted registered
[    0.034377] alg: No test for fips(ansi_cprng) (fips_ansi_cprng)
[    0.034481] alg: No test for 842 (842-scomp)
[    0.034550] alg: No test for 842 (842-generic)
[    0.038595] "cryptomgr_test" (159) uses obsolete ecb(arc4) skcipher
[    0.048468] clk: Disabling unused clocks
[    0.048536] PM: genpd: Disabling unused power domains
[    0.048611] ALSA device list:
[    0.048657]   No soundcards found.
[    0.049554] VFS: Mounted root (squashfs filesystem) readonly on device 254:0.
[    0.049710] devtmpfs: mounted
[    0.049833] Freeing unused kernel memory: 2496K
[    0.049890] Run /init as init process
[    0.049935]   with arguments:
[    0.049979]     /init
[    0.050022]   with environment:
[    0.050069]     HOME=/
[    0.050115]     TERM=linux
[    0.050160]     KRUN_BOOT_START_NS=1791642042589643000
MINOS-GUEST-BOOT-OK
minos-guest: cmdline reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0 root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1791642042589643000  --
minos-guest: osrelease 6.12.109
minos-guest: args /init
minos-guest: env HOME=/ KRUN_BOOT_START_NS=1791642042589643000 TERM=linux
minos-guest: net_ifaces lo
minos-guest: virtio_devices virtio0:0x0004 virtio1:0x0003 virtio2:0x0002 virtio3:0x0002 virtio4:0x0002 virtio5:0x0013 virtio6:0x4d47

-- minos-vmm stderr (52 bytes):
minos-vmm: watchdog parent-gone
minos-vmm: exit 126

-- minos-vmm stdout (0 bytes):

== end k7_07b_parent_gone_after_boot
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 11 filtered out; finished in 0.86s
```

Observed: the helper was gone 59 ms after the intermediate exited, with stderr `minos-vmm: watchdog parent-gone` and
`minos-vmm: exit 126`. Once the watchdog runs, the parent-gone trip and code 126 work as designed. K7-07 fails only
because the watchdog reads the parent too late.

### K7-08: kernel hash mismatch

Requirement: SPIKE-01 (K7, D-05), T-02-47

Setup: job `report_boot`, finish `poweroff`; config `kernel.sha256` = 64 zeros; `kernel.krunfw` is the real file.

Command: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest MINOS_KRUNFW=$(realpath /opt/homebrew/lib/libkrunfw.5.dylib) cargo test -p minos-backend-krun --test spike_exits k7_08_kernel_hash_mismatch -- --ignored --exact --nocapture --test-threads=1`

Exit code: 0

```text
running 1 test
test k7_08_kernel_hash_mismatch ... == k7_08_kernel_hash_mismatch: status code Some(67), signal None, killed_at_deadline false, elapsed 199.024833ms, connections 0, valid frames 0
-- console file on disk: absent
-- console read back (0 bytes, overflowed false):
[0 lines; 0 omitted: not in the last 40 and no marker]

-- minos-vmm stderr (264 bytes):
minos-vmm: error: preflight: kernel.krunfw /opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib: SHA-256 is 59764391005357bbdd5218f3fd522dd8ab63722cbc96144752c7bddc2c8e5217, the config expects 0000000000000000000000000000000000000000000000000000000000000000

-- minos-vmm stdout (0 bytes):

== end k7_08_kernel_hash_mismatch
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 0.63s
```

Observed: exit 67, a `minos-vmm: error: preflight:` line naming both digests, no console file. Matches E-2.

### K7-09: kernel missing

Requirement: SPIKE-01 (K7, D-05)

Setup: job `report_boot`, finish `poweroff`; config `kernel.krunfw` = `/private/tmp/minos-no-such-libkrunfw.dylib`
(checked absent by the test); `kernel.sha256` is the real digest.

Command: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest MINOS_KRUNFW=$(realpath /opt/homebrew/lib/libkrunfw.5.dylib) cargo test -p minos-backend-krun --test spike_exits k7_09_kernel_missing -- --ignored --exact --nocapture --test-threads=1`

Exit code: 0

```text
running 1 test
test k7_09_kernel_missing ... == k7_09_kernel_missing: status code Some(66), signal None, killed_at_deadline false, elapsed 12.838875ms, connections 0, valid frames 0
-- console file on disk: absent
-- console read back (0 bytes, overflowed false):
[0 lines; 0 omitted: not in the last 40 and no marker]

-- minos-vmm stderr (126 bytes):
minos-vmm: error: preflight: kernel.krunfw /private/tmp/minos-no-such-libkrunfw.dylib: No such file or directory (os error 2)

-- minos-vmm stdout (0 bytes):

== end k7_09_kernel_missing
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 0.45s
```

Observed: exit 66 in 13 ms, a `minos-vmm: error: preflight: kernel.krunfw` line, no console file. Matches E-2.

### K7-10: injected `MSB_KRUN_KERNEL_CMDLINE`

Requirement: SPIKE-01 (K7, D-05), T-02-46

Setup: job `report_boot`, finish `poweroff`. The helper is spawned directly with `env_clear()` and then
`MSB_KRUN_KERNEL_CMDLINE=init=/bin/sh` as its only variable.

Command: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest MINOS_KRUNFW=$(realpath /opt/homebrew/lib/libkrunfw.5.dylib) cargo test -p minos-backend-krun --test spike_exits k7_10_injected_cmdline_env -- --ignored --exact --nocapture --test-threads=1`

Exit code: 0

```text
running 1 test
test k7_10_injected_cmdline_env ... == k7_10_injected_cmdline_env: status code Some(66), signal None, exited after 55.026ms, pending connections 0
-- console: absent
-- minos-vmm stderr (126 bytes):
minos-vmm: error: preflight: environment variable MSB_KRUN_KERNEL_CMDLINE is set; MSB_KRUN_* and DYLD_* variables are refused

-- minos-vmm stdout (0 bytes):

== end k7_10_injected_cmdline_env
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 0.49s
```

Observed: exit 66, stderr names `MSB_KRUN_KERNEL_CMDLINE`, no console file. Preflight refuses the variable before
msb_krun reads it. Matches E-2.

### K7-11: no helper left running

Requirement: SPIKE-01 (K7, D-05), T-02-44

Command: `/usr/bin/pgrep -x minos-vmm; echo "exit: $?"` (after every run above)

Exit code: 1

```text
exit: 1
```

Full-suite rerun after the DEFECT-01 fix (plan verification block), helper signed as K7-00's re-sign shows:

Command: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest MINOS_KRUNFW=$(realpath /opt/homebrew/lib/libkrunfw.5.dylib) cargo test -p minos-backend-krun --test spike_exits -- --ignored --nocapture --test-threads=1`, then
`/usr/bin/pgrep -x minos-vmm; echo "pgrep exit $?"`

Exit code: 0 (the suite), 1 (`pgrep`)

```text
running 12 tests
test k7_01_poweroff ... == k7_01_poweroff: status code Some(0), signal None, killed_at_deadline false, elapsed 309.03125ms, connections 1, valid frames 1
-- console file on disk: 3595 bytes
-- console read back (3595 bytes, overflowed false):
[65 lines; 25 omitted: not in the last 40 and no marker]
[    0.021432] NET: Registered PF_CAN protocol family
[    0.021479] can: raw protocol
[    0.021527] can: broadcast manager protocol
[    0.021574] can: netlink gateway - max_hops=1
[    0.021648] NET: Registered PF_KCM protocol family
[    0.021722] NET: Registered PF_VSOCK protocol family
[    0.022018] NET: Registered PF_TSI protocol family
[    0.022077] NET: Registered PF_TSI6 protocol family
[    0.022122] NET: Registered PF_TSIU protocol family
[    0.022417] registered taskstats version 1
[    0.022991] Btrfs loaded, zoned=yes, fsverity=no
[    0.023100] Key type encrypted registered
[    0.037176] alg: No test for fips(ansi_cprng) (fips_ansi_cprng)
[    0.037270] alg: No test for 842 (842-scomp)
[    0.037335] alg: No test for 842 (842-generic)
[    0.041430] "cryptomgr_test" (158) uses obsolete ecb(arc4) skcipher
[    0.048055] clk: Disabling unused clocks
[    0.048105] PM: genpd: Disabling unused power domains
[    0.048154] ALSA device list:
[    0.048205]   No soundcards found.
[    0.048588] VFS: Mounted root (squashfs filesystem) readonly on device 254:0.
[    0.048708] devtmpfs: mounted
[    0.048819] Freeing unused kernel memory: 2496K
[    0.048876] Run /init as init process
[    0.048920]   with arguments:
[    0.048962]     /init
[    0.049003]   with environment:
[    0.049045]     HOME=/
[    0.049086]     TERM=linux
[    0.049126]     KRUN_BOOT_START_NS=1791642346511032000
MINOS-GUEST-BOOT-OK
minos-guest: cmdline reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0 root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1791642346511032000  --
minos-guest: osrelease 6.12.109
minos-guest: args /init
minos-guest: env HOME=/ KRUN_BOOT_START_NS=1791642346511032000 TERM=linux
minos-guest: net_ifaces lo
minos-guest: virtio_devices virtio0:0x0004 virtio1:0x0003 virtio2:0x0002 virtio3:0x0002 virtio4:0x0002 virtio5:0x0013 virtio6:0x4d47
MINOS-GUEST-FRAME-SENT 635 bytes
MINOS-GUEST-POWEROFF
[    0.069921] reboot: Power down

-- minos-vmm stderr (18 bytes):
minos-vmm: exit 0

-- minos-vmm stdout (0 bytes):

-- connection 0 (635 bytes): v1 frame, steps ["report_boot"]
== end k7_01_poweroff
ok
test k7_02_init_returns ... == k7_02_init_returns: status code Some(0), signal None, killed_at_deadline false, elapsed 312.968917ms, connections 1, valid frames 1
-- console file on disk: 3651 bytes
-- console read back (3651 bytes, overflowed false):
[65 lines; 25 omitted: not in the last 40 and no marker]
[    0.021559] can: controller area network core
[    0.021612] NET: Registered PF_CAN protocol family
[    0.021660] can: raw protocol
[    0.021703] can: broadcast manager protocol
[    0.021747] can: netlink gateway - max_hops=1
[    0.021814] NET: Registered PF_KCM protocol family
[    0.021889] NET: Registered PF_VSOCK protocol family
[    0.022177] NET: Registered PF_TSI protocol family
[    0.022224] NET: Registered PF_TSI6 protocol family
[    0.022273] NET: Registered PF_TSIU protocol family
[    0.022560] registered taskstats version 1
[    0.023131] Btrfs loaded, zoned=yes, fsverity=no
[    0.023243] Key type encrypted registered
[    0.037008] alg: No test for fips(ansi_cprng) (fips_ansi_cprng)
[    0.037123] alg: No test for 842 (842-scomp)
[    0.037194] alg: No test for 842 (842-generic)
[    0.041261] "cryptomgr_test" (158) uses obsolete ecb(arc4) skcipher
[    0.048171] clk: Disabling unused clocks
[    0.048223] PM: genpd: Disabling unused power domains
[    0.048281] ALSA device list:
[    0.048325]   No soundcards found.
[    0.049122] VFS: Mounted root (squashfs filesystem) readonly on device 254:0.
[    0.049292] devtmpfs: mounted
[    0.049404] Freeing unused kernel memory: 2496K
[    0.049462] Run /init as init process
[    0.049505]   with arguments:
[    0.049551]     /init
[    0.049592]   with environment:
[    0.049639]     HOME=/
[    0.049680]     TERM=linux
[    0.049719]     KRUN_BOOT_START_NS=1791642347260917000
MINOS-GUEST-BOOT-OK
minos-guest: cmdline reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0 root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1791642347260917000  --
minos-guest: osrelease 6.12.109
minos-guest: args /init
minos-guest: env HOME=/ KRUN_BOOT_START_NS=1791642347260917000 TERM=linux
minos-guest: net_ifaces lo
minos-guest: virtio_devices virtio0:0x0004 virtio1:0x0003 virtio2:0x0002 virtio3:0x0002 virtio4:0x0002 virtio5:0x0013 virtio6:0x4d47
MINOS-GUEST-FRAME-SENT 635 bytes
[    0.069437] reboot: Failed to start orderly reboot: forcing the issue

-- minos-vmm stderr (18 bytes):
minos-vmm: exit 0

-- minos-vmm stdout (0 bytes):

-- connection 0 (635 bytes): v1 frame, steps ["report_boot"]
== end k7_02_init_returns
ok
test k7_03_deadline ... == k7_03_deadline: status code Some(124), signal None, killed_at_deadline false, elapsed 5.233535208s, connections 0, valid frames 0
-- console file on disk: 3543 bytes
-- console read back (3543 bytes, overflowed false):
[63 lines; 23 omitted: not in the last 40 and no marker]
[    0.021351] NET: Registered PF_PACKET protocol family
[    0.021402] Bridge firewalling registered
[    0.021449] can: controller area network core
[    0.021516] NET: Registered PF_CAN protocol family
[    0.021563] can: raw protocol
[    0.021610] can: broadcast manager protocol
[    0.021657] can: netlink gateway - max_hops=1
[    0.021740] NET: Registered PF_KCM protocol family
[    0.021822] NET: Registered PF_VSOCK protocol family
[    0.022110] NET: Registered PF_TSI protocol family
[    0.022160] NET: Registered PF_TSI6 protocol family
[    0.022207] NET: Registered PF_TSIU protocol family
[    0.022490] registered taskstats version 1
[    0.023073] Btrfs loaded, zoned=yes, fsverity=no
[    0.023185] Key type encrypted registered
[    0.037429] alg: No test for fips(ansi_cprng) (fips_ansi_cprng)
[    0.037512] alg: No test for 842 (842-scomp)
[    0.037579] alg: No test for 842 (842-generic)
[    0.041676] "cryptomgr_test" (159) uses obsolete ecb(arc4) skcipher
[    0.048028] clk: Disabling unused clocks
[    0.048079] PM: genpd: Disabling unused power domains
[    0.048127] ALSA device list:
[    0.048174]   No soundcards found.
[    0.048988] VFS: Mounted root (squashfs filesystem) readonly on device 254:0.
[    0.049163] devtmpfs: mounted
[    0.049279] Freeing unused kernel memory: 2496K
[    0.049336] Run /init as init process
[    0.049378]   with arguments:
[    0.049425]     /init
[    0.049466]   with environment:
[    0.049513]     HOME=/
[    0.049553]     TERM=linux
[    0.049592]     KRUN_BOOT_START_NS=1791642348006130000
MINOS-GUEST-BOOT-OK
minos-guest: cmdline reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0 root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1791642348006130000  --
minos-guest: osrelease 6.12.109
minos-guest: args /init
minos-guest: env HOME=/ KRUN_BOOT_START_NS=1791642348006130000 TERM=linux
minos-guest: net_ifaces lo
minos-guest: virtio_devices virtio0:0x0004 virtio1:0x0003 virtio2:0x0002 virtio3:0x0002 virtio4:0x0002 virtio5:0x0013 virtio6:0x4d47

-- minos-vmm stderr (49 bytes):
minos-vmm: watchdog deadline
minos-vmm: exit 124

-- minos-vmm stdout (0 bytes):

== end k7_03_deadline
ok
test k7_04_console_cap ... == k7_04_console_cap: status code Some(125), signal None, killed_at_deadline false, elapsed 543.115625ms, connections 0, valid frames 0
-- console file on disk: 292144 bytes
-- console read back (292144 bytes, overflowed false):
[3626 lines; 3585 omitted: not in the last 40 and no marker]
MINOS-GUEST-BOOT-OK
klmnopqrstuvwxyzabcdefghijklmnopqabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrst
vwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuv
xyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwx
zabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqabcdefghijklmnopqrstuvwxyzabcdefghi
klmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijk
mnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklm
opqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqabcdefghijklmnopqrstuvwx
zabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz
bcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzab
defghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqabcdefghijklm
opqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmno
qrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopq
stuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqab
defghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcd
fghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdef
hijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefgh
jklmnopqabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrs
uvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstu
wxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvw
yzabcdefghijklmnopqabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefgh
jklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghij
lmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijkl
nopqrstuvwxyzabcdefghijklmnopqabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvw
yzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxy
abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyza
cdefghijklmnopqrstuvwxyzabcdefghijklmnopqabcdefghijklmnopqrstuvwxyzabcdefghijkl
nopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmn
pqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnop
rstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqabcdefghijklmnopqrstuvwxyza
cdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabc
efghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcde
ghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqabcdefghijklmnop
rstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqr
tuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrst
vwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqabcde
ghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefg
ijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghi
klmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijk
mnopqabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuv
xyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwx

-- minos-vmm stderr (52 bytes):
minos-vmm: watchdog console-cap
minos-vmm: exit 125

-- minos-vmm stdout (0 bytes):

== end k7_04_console_cap
console file size 292144 bytes against the 262144-byte cap: overshoot 30000 bytes
ok
test k7_05_sigterm ... k7_05_sigterm: helper pid 64641 showed the boot marker after 322.236875ms
/bin/kill -TERM 64641: code Some(0), signal None
== k7_05_sigterm: status code Some(0), signal None, exited 53.576584ms after SIGTERM, pending connections 0
-- console (3543 bytes read, cap 1048576):
[63 lines; 23 omitted: not in the last 40 and no marker]
[    0.022337] NET: Registered PF_PACKET protocol family
[    0.022387] Bridge firewalling registered
[    0.022430] can: controller area network core
[    0.022482] NET: Registered PF_CAN protocol family
[    0.022538] can: raw protocol
[    0.022587] can: broadcast manager protocol
[    0.022634] can: netlink gateway - max_hops=1
[    0.022724] NET: Registered PF_KCM protocol family
[    0.022799] NET: Registered PF_VSOCK protocol family
[    0.023109] NET: Registered PF_TSI protocol family
[    0.023173] NET: Registered PF_TSI6 protocol family
[    0.023230] NET: Registered PF_TSIU protocol family
[    0.023514] registered taskstats version 1
[    0.024093] Btrfs loaded, zoned=yes, fsverity=no
[    0.024212] Key type encrypted registered
[    0.034376] alg: No test for fips(ansi_cprng) (fips_ansi_cprng)
[    0.034469] alg: No test for 842 (842-scomp)
[    0.034536] alg: No test for 842 (842-generic)
[    0.038979] "cryptomgr_test" (159) uses obsolete ecb(arc4) skcipher
[    0.048516] clk: Disabling unused clocks
[    0.048577] PM: genpd: Disabling unused power domains
[    0.048643] ALSA device list:
[    0.048685]   No soundcards found.
[    0.049482] VFS: Mounted root (squashfs filesystem) readonly on device 254:0.
[    0.049715] devtmpfs: mounted
[    0.049856] Freeing unused kernel memory: 2496K
[    0.049933] Run /init as init process
[    0.049980]   with arguments:
[    0.050037]     /init
[    0.050078]   with environment:
[    0.050129]     HOME=/
[    0.050172]     TERM=linux
[    0.050216]     KRUN_BOOT_START_NS=1791642354717836000
MINOS-GUEST-BOOT-OK
minos-guest: cmdline reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0 root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1791642354717836000  --
minos-guest: osrelease 6.12.109
minos-guest: args /init
minos-guest: env HOME=/ KRUN_BOOT_START_NS=1791642354717836000 TERM=linux
minos-guest: net_ifaces lo
minos-guest: virtio_devices virtio0:0x0004 virtio1:0x0003 virtio2:0x0002 virtio3:0x0002 virtio4:0x0002 virtio5:0x0013 virtio6:0x4d47

-- minos-vmm stderr (18 bytes):
minos-vmm: exit 0

-- minos-vmm stdout (0 bytes):

== end k7_05_sigterm
ps after exit: exit code Some(1), signal None; 
ok
test k7_06_sigkill ... k7_06_sigkill: helper pid 64646 showed the boot marker after 328.38725ms
== k7_06_sigkill: status code None, signal Some(9), reaped 55.028292ms after Child::kill, pending connections 0
-- console (3543 bytes read, cap 1048576):
[63 lines; 23 omitted: not in the last 40 and no marker]
[    0.021729] NET: Registered PF_PACKET protocol family
[    0.021779] Bridge firewalling registered
[    0.021826] can: controller area network core
[    0.021881] NET: Registered PF_CAN protocol family
[    0.021932] can: raw protocol
[    0.021984] can: broadcast manager protocol
[    0.022035] can: netlink gateway - max_hops=1
[    0.022127] NET: Registered PF_KCM protocol family
[    0.022209] NET: Registered PF_VSOCK protocol family
[    0.022520] NET: Registered PF_TSI protocol family
[    0.022577] NET: Registered PF_TSI6 protocol family
[    0.022627] NET: Registered PF_TSIU protocol family
[    0.022912] registered taskstats version 1
[    0.023500] Btrfs loaded, zoned=yes, fsverity=no
[    0.023625] Key type encrypted registered
[    0.037665] alg: No test for fips(ansi_cprng) (fips_ansi_cprng)
[    0.037751] alg: No test for 842 (842-scomp)
[    0.037816] alg: No test for 842 (842-generic)
[    0.041285] "cryptomgr_test" (159) uses obsolete ecb(arc4) skcipher
[    0.048186] clk: Disabling unused clocks
[    0.048237] PM: genpd: Disabling unused power domains
[    0.048296] ALSA device list:
[    0.048344]   No soundcards found.
[    0.049176] VFS: Mounted root (squashfs filesystem) readonly on device 254:0.
[    0.049338] devtmpfs: mounted
[    0.049459] Freeing unused kernel memory: 2496K
[    0.049514] Run /init as init process
[    0.049560]   with arguments:
[    0.049605]     /init
[    0.049648]   with environment:
[    0.049695]     HOME=/
[    0.049739]     TERM=linux
[    0.049782]     KRUN_BOOT_START_NS=1791642355538961000
MINOS-GUEST-BOOT-OK
minos-guest: cmdline reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0 root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1791642355538961000  --
minos-guest: osrelease 6.12.109
minos-guest: args /init
minos-guest: env HOME=/ KRUN_BOOT_START_NS=1791642355538961000 TERM=linux
minos-guest: net_ifaces lo
minos-guest: virtio_devices virtio0:0x0004 virtio1:0x0003 virtio2:0x0002 virtio3:0x0002 virtio4:0x0002 virtio5:0x0013 virtio6:0x4d47

-- minos-vmm stderr (0 bytes):

-- minos-vmm stdout (0 bytes):

== end k7_06_sigkill
ps after reaping: exit code Some(1), signal None; 
ok
test k7_07_parent_gone ... == intermediate (this test binary, orphan_launcher): status code Some(0), signal None
-- stdout:

running 1 test
test orphan_launcher ... orphan_launcher: pid 64654 spawned minos-vmm pid 64655; returning without waiting for it to exit
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 11 filtered out; finished in 0.00s


-- stderr:

ps right after the intermediate exited: exit code Some(0), signal None; 64655     1 R    ~/Repos/minos/target/debug/minos-vmm
== k7_07_parent_gone: helper pid 64655 gone after Some(285.214375ms) (limit 15s), pending connections 0
-- console (0 bytes read, cap 1048576):
[0 lines; 0 omitted: not in the last 40 and no marker]

-- minos-vmm stderr (52 bytes):
minos-vmm: watchdog parent-gone
minos-vmm: exit 126

-- minos-vmm stdout (0 bytes):

== end k7_07_parent_gone
ok
test k7_07b_parent_gone_after_boot ... == intermediate (this test binary, orphan_launcher): status code Some(0), signal None
-- stdout:

running 1 test
test orphan_launcher ... orphan_launcher: boot marker after 321.677ms
orphan_launcher: pid 64664 spawned minos-vmm pid 64665; returning without waiting for it to exit
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 11 filtered out; finished in 0.32s


-- stderr:

ps right after the intermediate exited: exit code Some(0), signal None; 64665     1 R    ~/Repos/minos/target/debug/minos-vmm
== k7_07b_parent_gone_after_boot: helper pid 64665 gone after Some(59.386ms) (limit 15s), pending connections 0
-- console (3504 bytes read, cap 1048576):
[62 lines; 22 omitted: not in the last 40 and no marker]
[    0.024109] NET: Registered PF_PACKET protocol family
[    0.024160] Bridge firewalling registered
[    0.024215] can: controller area network core
[    0.024265] NET: Registered PF_CAN protocol family
[    0.024314] can: raw protocol
[    0.024367] can: broadcast manager protocol
[    0.024415] can: netlink gateway - max_hops=1
[    0.024489] NET: Registered PF_KCM protocol family
[    0.024567] NET: Registered PF_VSOCK protocol family
[    0.024890] NET: Registered PF_TSI protocol family
[    0.024946] NET: Registered PF_TSI6 protocol family
[    0.024995] NET: Registered PF_TSIU protocol family
[    0.025308] registered taskstats version 1
[    0.025966] Btrfs loaded, zoned=yes, fsverity=no
[    0.026090] Key type encrypted registered
[    0.036744] alg: No test for fips(ansi_cprng) (fips_ansi_cprng)
[    0.036839] alg: No test for 842 (842-scomp)
[    0.036903] alg: No test for 842 (842-generic)
[    0.040199] "cryptomgr_test" (158) uses obsolete ecb(arc4) skcipher
[    0.040770] clk: Disabling unused clocks
[    0.040823] PM: genpd: Disabling unused power domains
[    0.040874] ALSA device list:
[    0.040924]   No soundcards found.
[    0.050736] VFS: Mounted root (squashfs filesystem) readonly on device 254:0.
[    0.051317] devtmpfs: mounted
[    0.051437] Freeing unused kernel memory: 2496K
[    0.051498] Run /init as init process
[    0.051546]   with arguments:
[    0.051594]     /init
[    0.051635]   with environment:
[    0.051679]     HOME=/
[    0.051719]     TERM=linux
[    0.051759]     KRUN_BOOT_START_NS=1791642357136048000
MINOS-GUEST-BOOT-OK
minos-guest: cmdline reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0 root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1791642357136048000  --
minos-guest: osrelease 6.12.109
minos-guest: args /init
minos-guest: env HOME=/ KRUN_BOOT_START_NS=1791642357136048000 TERM=linux
minos-guest: net_ifaces lo
minos-guest: virtio_devices virtio0:0x0004 virtio1:0x0003 virtio2:0x0002 virtio3:0x0002 virtio4:0x0002 virtio5:0x0013 virtio6:0x4d47

-- minos-vmm stderr (52 bytes):
minos-vmm: watchdog parent-gone
minos-vmm: exit 126

-- minos-vmm stdout (0 bytes):

== end k7_07b_parent_gone_after_boot
ok
test k7_08_kernel_hash_mismatch ... == k7_08_kernel_hash_mismatch: status code Some(67), signal None, killed_at_deadline false, elapsed 208.993834ms, connections 0, valid frames 0
-- console file on disk: absent
-- console read back (0 bytes, overflowed false):
[0 lines; 0 omitted: not in the last 40 and no marker]

-- minos-vmm stderr (264 bytes):
minos-vmm: error: preflight: kernel.krunfw /opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib: SHA-256 is 59764391005357bbdd5218f3fd522dd8ab63722cbc96144752c7bddc2c8e5217, the config expects 0000000000000000000000000000000000000000000000000000000000000000

-- minos-vmm stdout (0 bytes):

== end k7_08_kernel_hash_mismatch
ok
test k7_09_kernel_missing ... == k7_09_kernel_missing: status code Some(66), signal None, killed_at_deadline false, elapsed 12.826084ms, connections 0, valid frames 0
-- console file on disk: absent
-- console read back (0 bytes, overflowed false):
[0 lines; 0 omitted: not in the last 40 and no marker]

-- minos-vmm stderr (126 bytes):
minos-vmm: error: preflight: kernel.krunfw /private/tmp/minos-no-such-libkrunfw.dylib: No such file or directory (os error 2)

-- minos-vmm stdout (0 bytes):

== end k7_09_kernel_missing
ok
test k7_10_injected_cmdline_env ... == k7_10_injected_cmdline_env: status code Some(66), signal None, exited after 55.018958ms, pending connections 0
-- console: absent
-- minos-vmm stderr (126 bytes):
minos-vmm: error: preflight: environment variable MSB_KRUN_KERNEL_CMDLINE is set; MSB_KRUN_* and DYLD_* variables are refused

-- minos-vmm stdout (0 bytes):

== end k7_10_injected_cmdline_env
ok
test orphan_launcher ... orphan_launcher: MINOS_SPIKE_ORPHAN_CONFIG is not set; nothing to do
ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 13.03s
```

```text
pgrep exit 1
```

All 12 ignored tests pass (`k7_01` to `k7_10`, `k7_07b`, and `orphan_launcher`, which does nothing without
`MINOS_SPIKE_ORPHAN_CONFIG`), and no `minos-vmm` is left running. K7-04's overshoot on this run is 30000 bytes
(292144 bytes for the 262144-byte cap), against 48954 on its own run: the overshoot varies with how much the guest
writes between two 50 ms watchdog polls.

### DEFECT-01: crates/minos-vmm/src/watchdog.rs: a helper orphaned before its watchdog starts is never stopped by parent-gone

Requirement: SPIKE-01 (K7, D-05), T-02-44

Failing runs: K7-07, both runs above (exit 101, `the orphaned helper outlived the 15 s limit`, empty helper stderr,
`ps` showing parent PID 1 right after the intermediate exited and again at 15 s).

Cause (source): `watchdog::spawn` reads `std::os::unix::process::parent_id()` when it is called, and `main::run`
calls it only after `read_config`, `preflight::check` (which hashes libkrunfw), and `launch::build`. If the parent
exits during that window, the helper has already been reparented to launchd, the watchdog records PID 1 as the
parent, and `parent_id() != parent` is never true. The helper then runs until `max_duration_ms` (up to one hour
under the config limit). K7-07b shows the trip works once the parent is recorded correctly.

Proposed fix (shared code, not applied here, Rule 4): read `parent_id()` as the first statement of `main` (before
the config is read) and pass it into `watchdog::spawn` instead of reading it there; also treat a parent of PID 1 as
parent-gone, since launchd never legitimately starts `minos-vmm`, which closes the remaining window between the
parent's `fork` and the helper's first instruction. A unit test in `watchdog.rs` can cover both
(`tripped` with a recorded parent that differs from the current one, and with a current parent of 1). After the fix,
rebuild and re-sign `minos-vmm` (02-06 E-4) and rerun `k7_07_parent_gone`; E-2's expectation (gone within 15 s,
`minos-vmm: watchdog parent-gone`, `minos-vmm: exit 126`) should then hold unchanged.

Fix applied (orchestrator decision, Rule 4, approved as a deviation outside this plan's `files_modified`), commit
`8bd6881` `fix(02-07): record the launcher's parent PID at main entry and treat a PID 1 parent as gone`:

- `crates/minos-vmm/src/main.rs`: `main` reads `std::os::unix::process::parent_id()` as its first statement, before
  argument parsing, the config read, preflight, and the VM build, and passes it through `run` to `watchdog::spawn`.
- `crates/minos-vmm/src/watchdog.rs`: `spawn` takes the expected parent PID instead of reading it; the parent-gone
  check is `parent_gone(expected, current)`, true when the current parent differs from the recorded one or is PID 1
  (launchd never legitimately starts `minos-vmm`), which also closes the window between the parent's `fork` and the
  helper's first instruction. Unit tests `parent_gone_when_the_recorded_parent_differs_from_the_current_one` and
  `parent_gone_when_the_current_parent_is_launchd`.
- Unchanged: exit codes 64, 65, 66, 67, 70, 124, 125, 126; the `minos-vmm: watchdog parent-gone` line; the
  launch config wire format; `clippy.toml`; the entitlements.

Rerun outcome: `k7_07_parent_gone` passes unchanged (K7-07 third run: gone after 505 ms, exit 126; K7-11 full suite:
gone after 285 ms, exit 126). T-02-44 is closed by the fix.

Carry-forward for 02-10: CLAUDE.md's launcher description must say that `minos-vmm` captures its parent PID at
`main` entry and treats a PID 1 parent as gone.

### K7 table

| Scenario | Mechanism (source) | Observed status | stderr markers | Frame | Evidence |
|---|---|---|---|---|---|
| Guest power-off | `reboot(POWER_OFF)` -> PSCI `SYSTEM_OFF` -> `VcpuExit::Shutdown` -> exit `FC_EXIT_CODE_OK` (msb_krun_hvf `lib.rs` 1105) | exit 0 | `minos-vmm: exit 0` | present (1, v1) | K7-01 |
| Guest init returns | libkrunfw patch 0001: `do_exit` of PID 1 calls `orderly_reboot()` instead of panicking; patch 0002 skips `run_cmd`, prints `Failed to start orderly reboot: forcing the issue`, `kernel_restart` -> PSCI `SYSTEM_RESET` -> `Shutdown` | exit 0 | `minos-vmm: exit 0` | present (sent before the return) | K7-02 |
| Launcher deadline | watchdog stores 124 in `Vm::exit_code`, triggers `ExitHandle`; the VMM prefers the stored code | exit 124 | `minos-vmm: watchdog deadline`, `minos-vmm: exit 124` | absent | K7-03 |
| Console cap | watchdog sees the console file over `cap_bytes`, stores 125 | exit 125; file 311098 bytes for a 262144-byte cap (overshoot 48954; 30000 in the K7-11 suite run) | `minos-vmm: watchdog console-cap`, `minos-vmm: exit 125` | absent | K7-04 |
| SIGTERM to the helper | msb_krun's handler writes `exit_evt`, graceful stop, no stored code | exit 0 | `minos-vmm: exit 0` | absent | K7-05 |
| SIGKILL to the helper | process dies with the in-process HVF VM | signal 9; `ps -p` exits 1 | none | absent | K7-06 |
| Helper orphaned before the watchdog starts | before the fix: the watchdog read the parent late and recorded PID 1, so parent-gone never tripped (DEFECT-01). After `8bd6881`: `main` records the parent PID first; the watchdog sees it differ from the current parent, and the current parent is PID 1; stores 126 | before the fix: still running at 15 s (killed by the test guard). After: exit 126 within 505 ms (285 ms in the suite run) | `minos-vmm: watchdog parent-gone`, `minos-vmm: exit 126` | absent; no console output | K7-07 (third run), K7-11, DEFECT-01 |
| Helper orphaned after the guest booted | watchdog sees `parent_id()` change, stores 126 | exit 126 within 59 ms | `minos-vmm: watchdog parent-gone`, `minos-vmm: exit 126` | absent | K7-07b |
| Kernel hash mismatch | preflight `hash_matches` -> `PreflightError::Hash` | exit 67 | `minos-vmm: error: preflight: kernel.krunfw ...: SHA-256 is ..., the config expects ...` | absent; no console file | K7-08 |
| Kernel missing | preflight `regular_file` -> `canonicalize` fails | exit 66 | `minos-vmm: error: preflight: kernel.krunfw ...: No such file or directory (os error 2)` | absent; no console file | K7-09 |
| Injected `MSB_KRUN_KERNEL_CMDLINE` | preflight `check_environment` refuses `MSB_KRUN_*` before msb_krun reads it | exit 66 | `minos-vmm: error: preflight: environment variable MSB_KRUN_KERNEL_CMDLINE is set; ...` | absent; no console file | K7-10 |
| Wrong argument count | `config_path` | exit 64 | `minos-vmm: usage: minos-vmm <config.json>` | n/a | 02-03 V-EXIT |
| Config unreadable or invalid | `read_config` / `config::parse` | exit 65 | `minos-vmm: error: reading launch config ...` or `... launch config ...: ...` | n/a | 02-03 V-EXIT |
| Unentitled helper | `hv_vm_create` refused -> `msb_krun` build error before takeover | exit 70 | `minos-vmm: error: starting the VM: build error: ...` | n/a | 02-06 S04-UNSIGNED |

### Classification rule for Phase 5

The helper's exit status never carries the guest's result: with no virtio-fs device the guest cannot set one, and
a power-off (K7-01), an init exit (K7-02), and a host SIGTERM (K7-05) all exit 0. Phase 5 classifies a run in this
order, using what the host itself observed:

1. **Infrastructure failure (retried):** exit 64, 65, 66, 67, or 70 with a `minos-vmm: error:` or `minos-vmm: usage:`
   line; a spawn failure; a signal the host did not send; or any exit code not in this table. None of these reached
   the guest (no console file in K7-08 to K7-10).
2. **`TimedOut`:** the host deadline fired (`VmRun::killed_at_deadline`), or exit 124 with
   `minos-vmm: watchdog deadline`.
3. **Infrastructure failure (parent gone, not the candidate's fault):** exit 126 with `minos-vmm: watchdog
   parent-gone`. Only an orphaned helper reports it, so normally no supervisor is left to read it.
4. **`Crashed` (guest-caused, console cap):** exit 125 with `minos-vmm: watchdog console-cap`. The guest chose to
   flood and sent no frame (K7-04); classify it as `Crashed` with the console-cap reason recorded, never as an
   infrastructure failure, and never retry it.
5. **Guest decided (frame present):** exit 0 and exactly one connection that decodes as a valid v1 status frame
   through `GuestBytes::status_frame`. The frame, not the exit code, says what happened.
6. **`Crashed`:** exit 0 (or a status the host caused on purpose, such as its own SIGTERM or SIGKILL during
   cleanup) with no valid frame: the guest panicked, returned from init or powered off before sending, or sent
   bytes that do not decode as a frame. A hang never reaches this row: the deadlines in row 2 catch it first.

Rows 1 to 4 are recognised by their exit code and stderr marker together; the marker is written by `minos-vmm`
itself, never by the guest, which only reaches the console file.

This overturns `.planning/research/ARCHITECTURE.md` Pattern 5, "Exit code" bullet ("libkrun exit()s the helper with
the workload's exit code"): that holds only for libkrun's `init.krun` plus virtio-fs path. D-09: the research file
stays as written; 02-10 records the correction.

## Plan 02-08: egress and vsock

Date: 2026-10-10

- `rustc --version`: rustc 1.97.1 (8bab26f4f 2026-07-14)
- `cargo --version`: cargo 1.97.1 (c980f4866 2026-06-30)
- `cargo clippy --version`: clippy 0.1.97 (8bab26f4f6 2026-07-14)
- `cargo deny --version`: cargo-deny 0.20.2
- macOS: 27.0, arm64
- HEAD at start: `455edef1` (plan base); the runs below are from `f964a219755487551f28cd07df8dd791fc57229a`
  (Task 1, `tests/spike_net.rs`)

Redactions: the absolute home directory path is shown as `~`, and the value of `MINOS_SPIKE_HOST_IP` (the host's
LAN address from `ipconfig getifaddr en0`; `route -n get default` names `en0`) is shown as `<host-lan-ip>`
everywhere. Each run's output was piped through `sed -e "s|<the address>|<host-lan-ip>|g" -e "s|$HOME|~|g"`.
Nothing else is altered. Cargo progress lines (`Compiling`, `Checking`, `Finished`, `Running`) are removed.

Console logs: each test prints the console through `GuestBytes::excerpt`, reduced to the last 40 lines plus every
line containing `MINOS-`, `Kernel panic`, or `minos-vmm`. The first line of each console block gives the total and
omitted line counts.

Environment for every VM test (02-06 E-1), set on the command line of each run:

- `MINOS_VMM=$(pwd)/target/debug/minos-vmm`
- `MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest` (SHA-256
  `10e59803c53ce60bb53445f95b86e07652a5029c5601170823f1fe156e4b3a89`)
- `MINOS_KRUNFW=$(realpath /opt/homebrew/lib/libkrunfw.5.dylib)` (= `/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib`)
- `MINOS_SPIKE_HOST_IP=$(ipconfig getifaddr en0)` (`egress_fails_closed` only)

### 08-00: build and sign

Requirement: SPIKE-02, SPIKE-03 (setup)

Setup: 02-06 E-4 with the two-key entitlement set 02-06 K5 settled on. Nothing in this plan changes the launcher:
`msb_krun`'s inet hijack is left at its default (off), no `tcp_listen_remap` is configured, there is no network
device, and the only vsock route is 02-05's `unix_connect` on port 1024.

Command: `cargo build -p minos-guest --release --target aarch64-unknown-linux-musl`

Exit code: 0

```
(no output once the cargo progress line is removed)
```

Command: `cargo build -p minos-vmm`

Exit code: 0

```
(no output once the cargo progress line is removed)
```

Command: `codesign --force --sign - --options runtime --entitlements crates/minos-vmm/minos-vmm.entitlements target/debug/minos-vmm`

Exit code: 0

```
target/debug/minos-vmm: replacing existing signature
```

Command: `codesign --display --entitlements - target/debug/minos-vmm`

Exit code: 0

```
Executable=~/Repos/minos/target/debug/minos-vmm
[Dict]
	[Key] com.apple.security.cs.disable-library-validation
	[Value]
		[Bool] true
	[Key] com.apple.security.hypervisor
	[Value]
		[Bool] true
```

### S02-EGRESS: no guest egress, canaries see nothing

Requirement: SPIKE-02 (K3, D-02)

Setup: 08-00. Before the launch the test binds two non-blocking canary listeners, `0.0.0.0:0` and `[::]:0`. The
job runs `report_boot`, then `connect` to six targets with `timeout_ms` 3000 (`1.1.1.1:443`, `8.8.8.8:53`,
`127.0.0.1:<v4 canary>`, `<host-lan-ip>:<v4 canary>`, `[::1]:<v6 canary>`, `[2606:4700:4700::1111]:443`), then
`vsock_connect` to port 1025, which has no route. Launch limits: 60 s launcher deadline, 90 s host deadline. After
`run` returns, each canary is drained with non-blocking `accept` (a connection aborted before the accept would
also count).

Command: `MINOS_SPIKE_HOST_IP=<host-lan-ip> cargo test -p minos-backend-krun --test spike_net egress_fails_closed -- --ignored --exact --nocapture --test-threads=1`

Exit code: 0

```

running 1 test
test egress_fails_closed ... canaries: IPv4 0.0.0.0:59879, IPv6 [::]:59880
== egress_fails_closed: exit_code Some(0), signal None, killed_at_deadline false, elapsed 499.943083ms, connections 1
-- console (3635 bytes, overflowed false):
[66 lines; 26 omitted: not in the last 40 and no marker]
[    0.021427] NET: Registered PF_CAN protocol family
[    0.021475] can: raw protocol
[    0.021524] can: broadcast manager protocol
[    0.021572] can: netlink gateway - max_hops=1
[    0.021648] NET: Registered PF_KCM protocol family
[    0.021725] NET: Registered PF_VSOCK protocol family
[    0.022026] NET: Registered PF_TSI protocol family
[    0.022097] NET: Registered PF_TSI6 protocol family
[    0.022146] NET: Registered PF_TSIU protocol family
[    0.022431] registered taskstats version 1
[    0.023005] Btrfs loaded, zoned=yes, fsverity=no
[    0.023118] Key type encrypted registered
[    0.036840] alg: No test for fips(ansi_cprng) (fips_ansi_cprng)
[    0.036940] alg: No test for 842 (842-scomp)
[    0.037006] alg: No test for 842 (842-generic)
[    0.041018] "cryptomgr_test" (158) uses obsolete ecb(arc4) skcipher
[    0.048011] clk: Disabling unused clocks
[    0.048058] PM: genpd: Disabling unused power domains
[    0.048111] ALSA device list:
[    0.048155]   No soundcards found.
[    0.049238] VFS: Mounted root (squashfs filesystem) readonly on device 254:0.
[    0.049417] devtmpfs: mounted
[    0.049529] Freeing unused kernel memory: 2496K
[    0.049587] Run /init as init process
[    0.049635]   with arguments:
[    0.049679]     /init
[    0.049721]   with environment:
[    0.049764]     HOME=/
[    0.049804]     TERM=linux
[    0.049846]     KRUN_BOOT_START_NS=1791642795597388000
MINOS-GUEST-BOOT-OK
minos-guest: cmdline reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0 root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1791642795597388000  --
minos-guest: osrelease 6.12.109
minos-guest: args /init
minos-guest: env HOME=/ KRUN_BOOT_START_NS=1791642795597388000 TERM=linux
minos-guest: net_ifaces lo
minos-guest: virtio_devices virtio0:0x0004 virtio1:0x0003 virtio2:0x0002 virtio3:0x0002 virtio4:0x0002 virtio5:0x0013 virtio6:0x4d47
MINOS-GUEST-FRAME-SENT 1399 bytes
MINOS-GUEST-POWEROFF
[    0.069880] reboot: Power down

-- minos-vmm stderr (18 bytes):
minos-vmm: exit 0

-- minos-vmm stdout (0 bytes):

-- connection 0 (1399 bytes), frame:
{
  "job_error": null,
  "steps": [
    {
      "report_boot": {
        "args": [
          "/init"
        ],
        "cmdline": "reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0 root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1791642795597388000  --",
        "env": [
          [
            "HOME",
            "/"
          ],
          [
            "KRUN_BOOT_START_NS",
            "1791642795597388000"
          ],
          [
            "TERM",
            "linux"
          ]
        ],
        "marker": "MINOS-GUEST-BOOT-OK",
        "net_ifaces": [
          "lo"
        ],
        "osrelease": "6.12.109",
        "virtio_devices": [
          "virtio0:0x0004",
          "virtio1:0x0003",
          "virtio2:0x0002",
          "virtio3:0x0002",
          "virtio4:0x0002",
          "virtio5:0x0013",
          "virtio6:0x4d47"
        ]
      }
    },
    {
      "connect": {
        "results": [
          {
            "error": "Network unreachable (os error 101)",
            "ok": false,
            "raw_os_error": 101,
            "target": "1.1.1.1:443"
          },
          {
            "error": "Network unreachable (os error 101)",
            "ok": false,
            "raw_os_error": 101,
            "target": "8.8.8.8:53"
          },
          {
            "error": "Network unreachable (os error 101)",
            "ok": false,
            "raw_os_error": 101,
            "target": "127.0.0.1:59879"
          },
          {
            "error": "Network unreachable (os error 101)",
            "ok": false,
            "raw_os_error": 101,
            "target": "<host-lan-ip>:59879"
          },
          {
            "error": "Address not available (os error 99)",
            "ok": false,
            "raw_os_error": 99,
            "target": "[::1]:59880"
          },
          {
            "error": "Address not available (os error 99)",
            "ok": false,
            "raw_os_error": 99,
            "target": "[2606:4700:4700::1111]:443"
          }
        ]
      }
    },
    {
      "vsock_connect": {
        "error": "Connection reset by peer (os error 104)",
        "ok": false,
        "port": 1025,
        "raw_os_error": 104
      }
    }
  ],
  "v": 1
}
== end egress_fails_closed
connect "1.1.1.1:443": ok false, error "Network unreachable (os error 101)", raw_os_error 101
connect "8.8.8.8:53": ok false, error "Network unreachable (os error 101)", raw_os_error 101
connect "127.0.0.1:59879": ok false, error "Network unreachable (os error 101)", raw_os_error 101
connect "<host-lan-ip>:59879": ok false, error "Network unreachable (os error 101)", raw_os_error 101
connect "[::1]:59880": ok false, error "Address not available (os error 99)", raw_os_error 99
connect "[2606:4700:4700::1111]:443": ok false, error "Address not available (os error 99)", raw_os_error 99
vsock_connect port 1025: ok false, error "Connection reset by peer (os error 104)", raw_os_error 104
canary connections accepted: IPv4 0, IPv6 0
net_ifaces ["lo"]; virtio_devices ["virtio0:0x0004", "virtio1:0x0003", "virtio2:0x0002", "virtio3:0x0002", "virtio4:0x0002", "virtio5:0x0013", "virtio6:0x4d47"]
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.97s
```

Per target (Linux errno values):

| Target | `ok` | `raw_os_error` | errno |
|---|---|---|---|
| `1.1.1.1:443` | false | 101 | `ENETUNREACH` |
| `8.8.8.8:53` | false | 101 | `ENETUNREACH` |
| `127.0.0.1:<v4 canary>` | false | 101 | `ENETUNREACH` |
| `<host-lan-ip>:<v4 canary>` | false | 101 | `ENETUNREACH` |
| `[::1]:<v6 canary>` | false | 99 | `EADDRNOTAVAIL` |
| `[2606:4700:4700::1111]:443` | false | 99 | `EADDRNOTAVAIL` |
| vsock CID 2 port 1025 (no route) | false | 104 | `ECONNRESET` |

Canary connections accepted: IPv4 0, IPv6 0. `net_ifaces` is `["lo"]`, and `lo` has no address: even the
guest's own loopback is unreachable (101 for `127.0.0.1`, 99 for `::1`). `virtio_devices` has no `0x0001`
(virtio-net); the list matches 02-06 S01-FRAME.

TSI is compiled into this kernel: the console shows `NET: Registered PF_TSI protocol family`, `PF_TSI6`, and
`PF_TSIU` (libkrunfw 5.6.2, kernel 6.12.109, `CONFIG_TSI=y`). The guest's ordinary `AF_INET` and `AF_INET6`
sockets still failed, so the VMM did not enable inet hijack. The isolation result therefore rests on `msb_krun`
0.1.41 keeping `inet_hijack` off by default and on minos-vmm never calling the hijack methods (the clippy ban from
02-03). This test is the live check of both, and Phase 5 promotes it to KRUN-03.

### S03-WITHIN: a stream of exactly the cap

Requirement: SPIKE-03 (K4)

Setup: 08-00. Job: `vsock_stream` of 1048576 bytes on port 1024, then the frame, then power off. `conn_caps`
[1048576, 1048576]; 60 s launcher deadline, 90 s host deadline.

Command: `cargo test -p minos-backend-krun --test spike_net vsock_stream_within_cap -- --ignored --exact --nocapture --test-threads=1`

Exit code: 101 (FAILED: see DEFECT-01)

```

running 1 test
test vsock_stream_within_cap ... == vsock_stream_within_cap: exit_code Some(124), signal None, killed_at_deadline false, elapsed 60.239922834s, connections 1
-- console (2987 bytes, overflowed false):
[57 lines; 17 omitted: not in the last 40 and no marker]
[    0.020746] rtc-pl031 a001000.rtc: setting system clock to 2026-10-10T14:33:27 UTC (1791642807)
[    0.020821] i2c_dev: i2c /dev entries driver
[    0.020883] pps_ldisc: PPS line discipline registered
[    0.021005] NET: Registered PF_INET6 protocol family
[    0.021125] Segment Routing with IPv6
[    0.021174] In-situ OAM (IOAM) with IPv6
[    0.021230] NET: Registered PF_PACKET protocol family
[    0.021282] Bridge firewalling registered
[    0.021331] can: controller area network core
[    0.021386] NET: Registered PF_CAN protocol family
[    0.021432] can: raw protocol
[    0.021478] can: broadcast manager protocol
[    0.021522] can: netlink gateway - max_hops=1
[    0.021597] NET: Registered PF_KCM protocol family
[    0.021674] NET: Registered PF_VSOCK protocol family
[    0.021997] NET: Registered PF_TSI protocol family
[    0.022064] NET: Registered PF_TSI6 protocol family
[    0.022110] NET: Registered PF_TSIU protocol family
[    0.022400] registered taskstats version 1
[    0.022979] Btrfs loaded, zoned=yes, fsverity=no
[    0.023091] Key type encrypted registered
[    0.036984] alg: No test for fips(ansi_cprng) (fips_ansi_cprng)
[    0.037067] alg: No test for 842 (842-scomp)
[    0.037135] alg: No test for 842 (842-generic)
[    0.041151] "cryptomgr_test" (158) uses obsolete ecb(arc4) skcipher
[    0.049071] clk: Disabling unused clocks
[    0.049121] PM: genpd: Disabling unused power domains
[    0.049168] ALSA device list:
[    0.049215]   No soundcards found.
[    0.049787] VFS: Mounted root (squashfs filesystem) readonly on device 254:0.
[    0.049944] devtmpfs: mounted
[    0.050132] Freeing unused kernel memory: 2496K
[    0.050190] Run /init as init process
[    0.050235]   with arguments:
[    0.050280]     /init
[    0.050326]   with environment:
[    0.050369]     HOME=/
[    0.050410]     TERM=linux
[    0.050451]     KRUN_BOOT_START_NS=1791642807305612000
MINOS-GUEST-BOOT-OK

-- minos-vmm stderr (49 bytes):
minos-vmm: watchdog deadline
minos-vmm: exit 124

-- minos-vmm stdout (0 bytes):

-- connection 0 (0 bytes, overflowed false, matches pattern true, sha256 e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855): not a frame: status frame is shorter than its 20-byte header
== end vsock_stream_within_cap
Error: "no connection 1"
FAILED

failures:

failures:
    vsock_stream_within_cap

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3 filtered out; finished in 60.68s

error: test failed, to rerun pass `-p minos-backend-krun --test spike_net`
```

The host accepted the stream connection and read 0 bytes from it until the launcher's watchdog killed the VM at
60 s (exit 124). The guest never sent its frame (`MINOS-GUEST-FRAME-SENT` is absent), so it was blocked inside the
stream step.

Status (added 2026-10-10, after DIAG-01): the cause is proven, and it is in the pinned `msb_krun_devices` 0.1.41, not in
Minos. The guest's 64 KiB writes are dropped by H2, and the stream hangs at 256 KiB by H3 (DIAG-01, "Mechanism of each
cliff"). The test was not re-run: it fails for those reasons, and the user's decision keeps the pinned VMM
(S03-ANSWER).

### S03-OVER: a stream of four times the cap

Requirement: SPIKE-03 (T-02-51)

Setup: 08-00. Job: `vsock_stream` of 4194304 bytes. `conn_caps` [1048576, 1048576]; 60 s launcher deadline, 90 s
host deadline.

Command: `cargo test -p minos-backend-krun --test spike_net vsock_stream_over_cap -- --ignored --exact --nocapture --test-threads=1`

Exit code: 101 (FAILED: see DEFECT-01)

```

running 1 test
test vsock_stream_over_cap ... == vsock_stream_over_cap: exit_code Some(124), signal None, killed_at_deadline false, elapsed 60.201874291s, connections 1
-- console (2987 bytes, overflowed false):
[57 lines; 17 omitted: not in the last 40 and no marker]
[    0.022397] rtc-pl031 a001000.rtc: setting system clock to 2026-10-10T14:34:28 UTC (1791642868)
[    0.022511] i2c_dev: i2c /dev entries driver
[    0.022586] pps_ldisc: PPS line discipline registered
[    0.022727] NET: Registered PF_INET6 protocol family
[    0.022846] Segment Routing with IPv6
[    0.022894] In-situ OAM (IOAM) with IPv6
[    0.022951] NET: Registered PF_PACKET protocol family
[    0.023014] Bridge firewalling registered
[    0.023064] can: controller area network core
[    0.023128] NET: Registered PF_CAN protocol family
[    0.023179] can: raw protocol
[    0.023227] can: broadcast manager protocol
[    0.023285] can: netlink gateway - max_hops=1
[    0.023369] NET: Registered PF_KCM protocol family
[    0.023448] NET: Registered PF_VSOCK protocol family
[    0.023779] NET: Registered PF_TSI protocol family
[    0.023843] NET: Registered PF_TSI6 protocol family
[    0.023893] NET: Registered PF_TSIU protocol family
[    0.024194] registered taskstats version 1
[    0.024784] Btrfs loaded, zoned=yes, fsverity=no
[    0.024936] Key type encrypted registered
[    0.035412] alg: No test for fips(ansi_cprng) (fips_ansi_cprng)
[    0.035595] alg: No test for 842 (842-generic)
[    0.035692] alg: No test for 842 (842-scomp)
[    0.038332] "cryptomgr_test" (159) uses obsolete ecb(arc4) skcipher
[    0.038902] clk: Disabling unused clocks
[    0.038956] PM: genpd: Disabling unused power domains
[    0.039013] ALSA device list:
[    0.039064]   No soundcards found.
[    0.050110] VFS: Mounted root (squashfs filesystem) readonly on device 254:0.
[    0.050620] devtmpfs: mounted
[    0.050758] Freeing unused kernel memory: 2496K
[    0.050829] Run /init as init process
[    0.050893]   with arguments:
[    0.050934]     /init
[    0.050980]   with environment:
[    0.051030]     HOME=/
[    0.051073]     TERM=linux
[    0.051120]     KRUN_BOOT_START_NS=1791642868073508000
MINOS-GUEST-BOOT-OK

-- minos-vmm stderr (49 bytes):
minos-vmm: watchdog deadline
minos-vmm: exit 124

-- minos-vmm stdout (0 bytes):

-- connection 0 (0 bytes, overflowed false, matches pattern true, sha256 e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855): not a frame: status frame is shorter than its 20-byte header
== end vsock_stream_over_cap
Error: "no connection 1"
FAILED

failures:

failures:
    vsock_stream_over_cap

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3 filtered out; finished in 60.63s

error: test failed, to rerun pass `-p minos-backend-krun --test spike_net`
```

Same failure as S03-WITHIN: 0 bytes reached the host, the guest blocked, and the watchdog ended the run at 60 s.
The guest's `written`, `error`, and `raw_os_error` were never reported, because the frame was never sent. The host
cap itself was not exercised.

Status (added 2026-10-10, after DIAG-01): same cause as S03-WITHIN (H2 and H3 in `msb_krun_devices` 0.1.41). The test
was not re-run (S03-ANSWER). The cap behavior it was meant to show (the host keeps exactly the cap, flags the overflow,
and drops the stream) is still unobserved in a VM run.

### K4-THROUGHPUT: 64 MiB

Requirement: SPIKE-03 (K4)

Setup: 08-00. Job: `vsock_stream` of 67108864 bytes. `conn_caps` [134217728, 1048576]; 120 s launcher deadline,
150 s host deadline.

Command: `cargo test -p minos-backend-krun --test spike_net vsock_throughput_64mib -- --ignored --exact --nocapture --test-threads=1`

Exit code: 101 (FAILED: see DEFECT-01)

```

running 1 test
test vsock_throughput_64mib ... == vsock_throughput_64mib: exit_code Some(124), signal None, killed_at_deadline false, elapsed 120.208325958s, connections 1
-- console (2948 bytes, overflowed false):
[56 lines; 16 omitted: not in the last 40 and no marker]
[    0.021228] rtc-pl031 a001000.rtc: setting system clock to 2026-10-10T14:35:28 UTC (1791642928)
[    0.021305] i2c_dev: i2c /dev entries driver
[    0.021366] pps_ldisc: PPS line discipline registered
[    0.021496] NET: Registered PF_INET6 protocol family
[    0.021610] Segment Routing with IPv6
[    0.021658] In-situ OAM (IOAM) with IPv6
[    0.021715] NET: Registered PF_PACKET protocol family
[    0.021768] Bridge firewalling registered
[    0.021816] can: controller area network core
[    0.021869] NET: Registered PF_CAN protocol family
[    0.021928] can: raw protocol
[    0.021975] can: broadcast manager protocol
[    0.022022] can: netlink gateway - max_hops=1
[    0.022097] NET: Registered PF_KCM protocol family
[    0.022173] NET: Registered PF_VSOCK protocol family
[    0.022492] NET: Registered PF_TSI protocol family
[    0.022542] NET: Registered PF_TSI6 protocol family
[    0.022588] NET: Registered PF_TSIU protocol family
[    0.022918] registered taskstats version 1
[    0.023514] Btrfs loaded, zoned=yes, fsverity=no
[    0.023629] Key type encrypted registered
[    0.033727] alg: No test for fips(ansi_cprng) (fips_ansi_cprng)
[    0.033828] alg: No test for 842 (842-scomp)
[    0.033911] alg: No test for 842 (842-generic)
[    0.038074] "cryptomgr_test" (158) uses obsolete ecb(arc4) skcipher
[    0.048366] clk: Disabling unused clocks
[    0.048417] PM: genpd: Disabling unused power domains
[    0.048464] ALSA device list:
[    0.048515]   No soundcards found.
[    0.049334] VFS: Mounted root (squashfs filesystem) readonly on device 254:0.
[    0.049472] devtmpfs: mounted
[    0.049672] Freeing unused kernel memory: 2496K
[    0.049733] Run /init as init process
[    0.049779]   with arguments:
[    0.049820]     /init
[    0.049861]   with environment:
[    0.049904]     HOME=/
[    0.049943]     TERM=linux
[    0.049983]     KRUN_BOOT_START_NS=1791642928808867000
MINOS-GUEST-BOOT-OK

-- minos-vmm stderr (49 bytes):
minos-vmm: watchdog deadline
minos-vmm: exit 124

-- minos-vmm stdout (0 bytes):

-- connection 0 (0 bytes, overflowed false, matches pattern true, sha256 e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855): not a frame: status frame is shorter than its 20-byte header
== end vsock_throughput_64mib
Error: "no connection 1"
FAILED

failures:

failures:
    vsock_throughput_64mib

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3 filtered out; finished in 120.64s

error: test failed, to rerun pass `-p minos-backend-krun --test spike_net`
```

No throughput figure exists: 0 of 67108864 bytes arrived before the watchdog ended the run at 120 s.

Status (added 2026-10-10, after DIAG-01): same cause as S03-WITHIN. The test was not re-run (S03-ANSWER). The only
throughput figure is DIAG-01's, from instrumented and counterfactual VMM builds rather than this test: about 6 MiB/s on
one connection. DIAG-01 "Latent observations" records one 8 KiB socket-buffer fill per `OUT` event about 1.2 ms apart
(about 6.5 MiB/s), 262144 bytes drained in about 51 ms (about 4.9 MiB/s), and 4 MiB in 1.02 s including boot. The rate
is set by the 8 KiB send buffer and the supervisor's 1 ms read poll.

### DEFECT-01: msb_krun_devices 0.1.41 src/virtio/vsock (packet.rs, unix.rs): guest-to-host vsock data past 8 KiB is silently lost

Requirement: SPIKE-03

This is a defect in the pinned external VMM crate, not in a Minos file. Nothing in `crates/` outside
`tests/spike_net.rs` was edited. It is recorded here and the plan stops (Rule 4).

Setup: 08-00, with this diagnostic test appended to `tests/spike_net.rs` for the sweep only and removed afterwards
(the committed file has the four E-1 tests and nothing else):

```rust
#[test]
#[ignore = "diagnostic"]
fn vsock_stream_probe() -> TestResult {
    let vmm = signed_vmm()?;
    let bytes: u64 = std::env::var("MINOS_SPIKE_STREAM_BYTES")?.parse()?;
    let run = launch(
        &vmm,
        &stream_job(bytes),
        Limits {
            max_duration_ms: 15_000,
            host_deadline: Duration::from_secs(30),
            conn_caps: vec![THROUGHPUT_CAP, FRAME_CAP],
        },
    )?;
    print_run("vsock_stream_probe", &run)?;
    stream_result(&run, 1)?;
    Ok(())
}
```

Command, once per size:
`MINOS_SPIKE_STREAM_BYTES=<n> cargo test -p minos-backend-krun --test spike_net vsock_stream_probe -- --ignored --exact --nocapture --test-threads=1`

Output, filtered to the run line, the guest's frame and power-off markers, the minos-vmm lines, the connection
lines, the guest's `vsock_stream` result, and the test error (the frame body and the boot console are left out):

```
## MINOS_SPIKE_STREAM_BYTES=1399
== vsock_stream_probe: exit_code Some(0), signal None, killed_at_deadline false, elapsed 310.72775ms, connections 2
MINOS-GUEST-FRAME-SENT 138 bytes
MINOS-GUEST-POWEROFF
minos-vmm: exit 0
-- connection 0 (1399 bytes, overflowed false, matches pattern true, sha256 2269ee2e44aa4d9af7952464a48cf92e1f27f4d1911e0b425e7f8598d29ebfda): not a frame: status frame magic is [00, 01, 02, 03, 04, 05, 06, 07], expected MINOSST1
-- connection 1 (138 bytes), frame:
guest vsock_stream: requested 1399, written 1399, error null, raw_os_error null
## MINOS_SPIKE_STREAM_BYTES=4096
== vsock_stream_probe: exit_code Some(0), signal None, killed_at_deadline false, elapsed 305.662916ms, connections 2
MINOS-GUEST-FRAME-SENT 138 bytes
MINOS-GUEST-POWEROFF
minos-vmm: exit 0
-- connection 0 (4096 bytes, overflowed false, matches pattern true, sha256 d67c656e01756650d77717b0839985a056ec28ffe174601d690fc407a2ceffca): not a frame: status frame magic is [00, 01, 02, 03, 04, 05, 06, 07], expected MINOSST1
-- connection 1 (138 bytes), frame:
guest vsock_stream: requested 4096, written 4096, error null, raw_os_error null
## MINOS_SPIKE_STREAM_BYTES=8192
== vsock_stream_probe: exit_code Some(0), signal None, killed_at_deadline false, elapsed 312.358ms, connections 2
MINOS-GUEST-FRAME-SENT 138 bytes
MINOS-GUEST-POWEROFF
minos-vmm: exit 0
-- connection 0 (8192 bytes, overflowed false, matches pattern true, sha256 25df2449b2e5a35fea14e02a7158e283801a1069c9f84631b9a9dacb2f809a7f): not a frame: status frame magic is [00, 01, 02, 03, 04, 05, 06, 07], expected MINOSST1
-- connection 1 (138 bytes), frame:
guest vsock_stream: requested 8192, written 8192, error null, raw_os_error null
## MINOS_SPIKE_STREAM_BYTES=8193
== vsock_stream_probe: exit_code Some(0), signal None, killed_at_deadline false, elapsed 310.517875ms, connections 2
MINOS-GUEST-FRAME-SENT 138 bytes
MINOS-GUEST-POWEROFF
minos-vmm: exit 0
-- connection 0 (8192 bytes, overflowed false, matches pattern true, sha256 25df2449b2e5a35fea14e02a7158e283801a1069c9f84631b9a9dacb2f809a7f): not a frame: status frame magic is [00, 01, 02, 03, 04, 05, 06, 07], expected MINOSST1
-- connection 1 (138 bytes), frame:
guest vsock_stream: requested 8193, written 8193, error null, raw_os_error null
## MINOS_SPIKE_STREAM_BYTES=16384
== vsock_stream_probe: exit_code Some(0), signal None, killed_at_deadline false, elapsed 311.561042ms, connections 2
MINOS-GUEST-FRAME-SENT 140 bytes
MINOS-GUEST-POWEROFF
minos-vmm: exit 0
-- connection 0 (8192 bytes, overflowed false, matches pattern true, sha256 25df2449b2e5a35fea14e02a7158e283801a1069c9f84631b9a9dacb2f809a7f): not a frame: status frame magic is [00, 01, 02, 03, 04, 05, 06, 07], expected MINOSST1
-- connection 1 (140 bytes), frame:
guest vsock_stream: requested 16384, written 16384, error null, raw_os_error null
## MINOS_SPIKE_STREAM_BYTES=24576
== vsock_stream_probe: exit_code Some(0), signal None, killed_at_deadline false, elapsed 310.205833ms, connections 2
MINOS-GUEST-FRAME-SENT 140 bytes
MINOS-GUEST-POWEROFF
minos-vmm: exit 0
-- connection 0 (8192 bytes, overflowed false, matches pattern true, sha256 25df2449b2e5a35fea14e02a7158e283801a1069c9f84631b9a9dacb2f809a7f): not a frame: status frame magic is [00, 01, 02, 03, 04, 05, 06, 07], expected MINOSST1
-- connection 1 (140 bytes), frame:
guest vsock_stream: requested 24576, written 24576, error null, raw_os_error null
## MINOS_SPIKE_STREAM_BYTES=32768
== vsock_stream_probe: exit_code Some(0), signal None, killed_at_deadline false, elapsed 313.25825ms, connections 2
MINOS-GUEST-FRAME-SENT 140 bytes
MINOS-GUEST-POWEROFF
minos-vmm: exit 0
-- connection 0 (8192 bytes, overflowed false, matches pattern true, sha256 25df2449b2e5a35fea14e02a7158e283801a1069c9f84631b9a9dacb2f809a7f): not a frame: status frame magic is [00, 01, 02, 03, 04, 05, 06, 07], expected MINOSST1
-- connection 1 (140 bytes), frame:
guest vsock_stream: requested 32768, written 32768, error null, raw_os_error null
## MINOS_SPIKE_STREAM_BYTES=40960
== vsock_stream_probe: exit_code Some(0), signal None, killed_at_deadline false, elapsed 313.198333ms, connections 2
MINOS-GUEST-FRAME-SENT 140 bytes
MINOS-GUEST-POWEROFF
minos-vmm: exit 0
-- connection 0 (0 bytes, overflowed false, matches pattern true, sha256 e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855): not a frame: status frame is shorter than its 20-byte header
-- connection 1 (140 bytes), frame:
guest vsock_stream: requested 40960, written 40960, error null, raw_os_error null
## MINOS_SPIKE_STREAM_BYTES=65536
== vsock_stream_probe: exit_code Some(0), signal None, killed_at_deadline false, elapsed 312.221916ms, connections 2
MINOS-GUEST-FRAME-SENT 140 bytes
MINOS-GUEST-POWEROFF
minos-vmm: exit 0
-- connection 0 (0 bytes, overflowed false, matches pattern true, sha256 e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855): not a frame: status frame is shorter than its 20-byte header
-- connection 1 (140 bytes), frame:
guest vsock_stream: requested 65536, written 65536, error null, raw_os_error null
## MINOS_SPIKE_STREAM_BYTES=262144
== vsock_stream_probe: exit_code Some(0), signal None, killed_at_deadline false, elapsed 313.160542ms, connections 2
MINOS-GUEST-FRAME-SENT 142 bytes
MINOS-GUEST-POWEROFF
minos-vmm: exit 0
-- connection 0 (0 bytes, overflowed false, matches pattern true, sha256 e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855): not a frame: status frame is shorter than its 20-byte header
-- connection 1 (142 bytes), frame:
guest vsock_stream: requested 262144, written 262144, error null, raw_os_error null
## MINOS_SPIKE_STREAM_BYTES=1048576
== vsock_stream_probe: exit_code Some(124), signal None, killed_at_deadline false, elapsed 15.215074917s, connections 1
minos-vmm: watchdog deadline
minos-vmm: exit 124
-- connection 0 (0 bytes, overflowed false, matches pattern true, sha256 e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855): not a frame: status frame is shorter than its 20-byte header
Error: "no connection 1"
```

| Guest stream bytes | Host received | Guest `written`, `error` | Run |
|---|---|---|---|
| 1399, 4096, 8192 | all of it, pattern matches | all, null | exit 0, frame on connection 1 |
| 8193, 16384, 24576, 32768 | exactly 8192, pattern matches, `overflowed` false (clean EOF) | all, null | exit 0, frame on connection 1 |
| 40960, 65536, 262144 | 0 (clean EOF) | all, null | exit 0, frame on connection 1 |
| 1048576 (and S03's 4194304, K4's 67108864) | 0 | never reported | guest blocks; watchdog exit 124 |

In every row that completes, the loss is silent on both sides: the guest's writes and its `shutdown(Write)` all succeed, and the host sees an
ordinary end of stream, not an error and not an overflow.

The 8192 boundary is the macOS Unix-socket send buffer that the VMM writes into:

Command: `sysctl net.local.stream.sendspace net.local.stream.recvspace`

Exit code: 0

```
net.local.stream.sendspace: 8192
net.local.stream.recvspace: 8192
```

Source read (`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/msb_krun_devices-0.1.41/src/virtio/vsock/`):

1. `unix.rs` `UnixProxy::sendmsg` appends each guest packet to `pending_write`, returns its credit to the guest
   at once, and calls `flush_pending_write`, which stops at `WouldBlock` and leaves the rest for a later `OUT`
   event. `UnixProxy::shutdown` (on the guest's `VSOCK_OP_SHUTDOWN`) calls `self.endpoint.shutdown(how)` straight
   away, without draining `pending_write`. Anything still queued when the guest shuts down is never written: with an
   8192-byte socket buffer, that is everything past the first 8192 bytes of a fast writer. This matches the
   8193 to 32768 rows exactly.
2. `packet.rs` `VsockPacket::from_tx_virtq_head` accepts only a header descriptor followed by one data
   descriptor at least as long as the header's `len` (`BufDescTooSmall` otherwise). `device.rs` `process_stream_tx`
   logs a malformed TX packet at error level, marks the descriptor used with length 0, and moves on; the guest is
   never told. A guest packet whose payload the kernel spreads over several descriptors is therefore dropped whole.
   The guest writes 64 KiB at a time (`minos-guest` `CHUNK`), and the drop starts between 32768 and 40960 bytes,
   which fits a guest kernel that builds non-linear (multi-fragment) TX buffers above about 32 KiB. That kernel
   path is inferred from the threshold, not read in the 6.12.109 source; the msb_krun side is read.
3. The 1 MiB and larger rows block the guest. The likely cause is credit: dropped packets never advance the
   proxy's `tx_cnt`, so the guest's view of the peer's free buffer never recovers. Unverified.

Impact: the vsock route carries at most 8192 bytes per connection reliably. The status frame (1399 bytes here)
fits, which is why 02-06 and 02-07 passed. A frame or output stream over 8 KiB would arrive truncated: the frame
decoder would reject it (`LengthMismatch`) and the run would be classified Crashed; an unframed stream would look
complete. The host-side cap (`take(cap + 1)`) was never reached and is not implicated. No isolation property is
affected: data is lost in the guest-to-host direction only, and nothing extra reaches the host.

Proposed fix (needs a decision; both parts are in the external crate):

- A. Patch `msb_krun_devices` through a fork pinned by `rev` under `[patch.crates-io]`: make
  `UnixProxy::shutdown` defer `endpoint.shutdown(Write)` until `pending_write` is empty (finish it from the `OUT`
  path), and make `from_tx_virtq_head` gather a payload chained over several read-only descriptors (bounded by
  `MAX_PKT_BUF_SIZE`) instead of dropping it. Report both upstream (superradcompany/msb_krun) at the same time.
  crates.io shows 0.1.41 (2026-10-08) is still the newest `msb_krun_devices`.
- B. Without touching the VMM, a partial workaround in `minos-guest` (shared code): write in chunks well under 32 KiB,
  for example 16 KiB (avoids part 2) and add a host acknowledgement before `shutdown(Write)` (avoids part 1). This needs a
  host-to-guest message on the route that the supervisor does not send today, and it leaves the VMM defect in place
  for any guest that writes differently, so a hostile guest could still lose its own output. That harms only the
  guest, but Phase 5's output reader would have to treat every short stream as suspect.

Decision (added 2026-10-10): neither A nor B. DIAG-01 below found the root causes, and the user chose to work within the
limits. See S03-ANSWER at the end of this fragment.

### SPIKE-02 and SPIKE-03 answers

SPIKE-02 is answered: the guest has no network path. With inet hijack at its default (off), no
`tcp_listen_remap`, and only the `unix_connect` route, every TCP connect failed: public IPv4, the host LAN address,
and the guest's own `127.0.0.1` with `ENETUNREACH` (101), and public IPv6 and `::1` with `EADDRNOTAVAIL` (99).
Both host canaries accepted 0 connections, the guest sees only `lo` and no virtio-net device, and a vsock connect to
the unmapped port 1025 was reset (`ECONNRESET`, 104) (S02-EGRESS). 02-RESEARCH.md Pattern 5 expected `ENETUNREACH`
and no TSI, and the unmapped vsock port refused or reset; both held. The IPv6 targets fail with
`EADDRNOTAVAIL` rather than `ENETUNREACH`, which the research did not predict (D-09: the research stays as written;
02-10 records the difference). TSI is compiled into the guest kernel, so this result holds only while the VMM
keeps inet hijack off; the test is the guard.

SPIKE-03 is answered, with a limit. This paragraph was revised on 2026-10-10 after DIAG-01 and the user's decision.
Its first version said "not answered" while DEFECT-01 was open.

A guest vsock port maps to the host Unix socket through `unix_connect`, and the host reads each connection through
its cap. One connection delivers every guest-to-host stream of 8192 bytes or less intact: the DEFECT-01 sweep, DIAG-01
E1, and every status frame in 02-06, 02-07, and this plan. Above 8192 bytes per connection the pinned
`msb_krun_devices` 0.1.41 loses data. DIAG-01 proves three causes, none of them in Minos:

- H1: a shutdown race discards bytes queued above the 8 KiB macOS send buffer.
- H2: payloads over 32 KiB, which the guest kernel spreads over several descriptors, are dropped silently.
- H3: a credit update sent only every 4 MiB hangs any stream over 256 KiB.

S03-WITHIN, S03-OVER, and K4-THROUGHPUT therefore fail as recorded. The plan's acceptance (1 MiB at the cap, 4 MiB over
it, and 64 MiB intact on one connection) is not met on this VMM. The research expectation that the host cuts the
stream at `take(cap + 1)` and the guest then sees EPIPE or ECONNRESET (Pattern 5) is untested, because no stream ever
reached the cap.

K4: the 64 MiB transfer never completed on the stock VMM. DIAG-01's instrumented runs put one connection's rate at
about 6 MiB/s (K4-THROUGHPUT status note).

The user chose to work within the limit. Phase 5's guest-to-host output is a sequence of short connections, each
carrying at most one 4 KiB chunk (S03-ANSWER). The research did not predict any of the three limits (D-09: the research
stays as written, and 02-10 records the differences).

### DIAG-01: vsock cliff root-cause diagnosis

Requirement: SPIKE-03 (DEFECT-01 follow-up: explain each cliff end to end and rule Minos's own code and configuration in
or out before any dependency change)

Date: 2026-10-10. HEAD `3b2358b`. Toolchain as in 08-00. libkrunfw 5.6.2 (`MINOS_KRUNFW` as in 02-06 E-1).

**Verdict.** DEFECT-01 is three independent defects in the pinned `msb_krun_devices` 0.1.41. Each is proven with
device-side instrumentation and a counterfactual run. None of the five Minos-side suspects is a cause: the supervisor,
the host socket buffers Minos controls, the guest's write pattern, the launch config, and the probe's byte counting are
all ruled out (S1 to S5). Two of the three defects are fixed in upstream `containers/libkrun` but not in the `msb_krun`
fork. The third exists only in the fork. Details: "Mechanism of each cliff" and "Upstream status" below.

**Scope.** Diagnosis only. Nothing in the repository was changed except this section: no file under `crates/`, and no
`Cargo.toml`, `Cargo.lock`, `deny.toml`, `.cargo/config.toml`, or `[patch]` entry. Every experiment ran from a scratch
copy of the workspace (`git archive HEAD`) in the session scratchpad, shown below as `$SCR`, with its own target
directory `$SCR/target`. "Stock" means the repository's `target/debug/minos-vmm` (built from this HEAD and signed with
the 02-06 E-4 argv) and `target/aarch64-unknown-linux-musl/release/minos-guest` (SHA-256 `10e59803…3a89`, as in 08-00).
The scratch-only additions:

1. A harness, `crates/minos-backend-krun/tests/diag_vsock.rs` (source at the end of this section). For `SO_RCVBUF` and
   `SO_SNDBUF`, `nix = { version = "0.31.3", features = ["socket"] }` was added to the scratch crate's manifest
   (`nix 0.31.3` is already in `Cargo.lock`).
2. Guest variants selected by a compile-time `DIAG_GUEST` (diff at the end). Each was built with
   `DIAG_GUEST=<v> cargo build -p minos-guest --release --target aarch64-unknown-linux-musl` and copied to
   `$SCR/bin/minos-guest-<v>`.
3. An instrumented copy of the pinned crate at `$SCR/msb_krun_devices-0.1.41` (diff at the end), wired in only by
   `[patch.crates-io] msb_krun_devices = { path = "../msb_krun_devices-0.1.41" }` in the scratch `Cargo.toml`.
   - Cargo builds the registry original with `--cap-lints allow`, but gives a path patch no cap, so the backstop's
     `-Funsafe_code` would refuse the third-party crate's own `unsafe`. The scratch build therefore used
     `RUSTC_WRAPPER=$SCR/tools/rustc-cap-devices.sh`, which appends `--cap-lints allow` for
     `--crate-name msb_krun_devices` only. The `.cargo/config.toml` backstop stayed in force for every Minos crate.
   - The instrumentation appends lines to `$SCR/trace/vsock.log`.
   - Two counterfactual toggles, both off unless the crate is built with `DIAG_CF`: `drain` (in
     `UnixProxy::shutdown`, retry the flush every 1 ms until `pending_write` is empty, then shut down) and `credit64k`
     (send `CREDIT_UPDATE` every 64 KiB instead of every 4 MiB).
   - Four VMM binaries were built from it, each copied to `$SCR/bin` and signed with the 02-06 E-4 argv:
     `minos-vmm-trace` (trace only), `minos-vmm-cf-drain`, `minos-vmm-cf-credit64k`, and
     `minos-vmm-cf-drain+credit64k`.

The registry copy was never modified. A fresh extract of
`~/.cargo/registry/cache/index.crates.io-1949cf8c6b5b557f/msb_krun_devices-0.1.41.crate`, whose SHA-256 `b039a4c3…0fe7`
equals the `Cargo.lock` checksum, diffs clean against `~/.cargo/registry/src/.../msb_krun_devices-0.1.41` (only
`.cargo-ok` differs).

Redactions: `~` for the home directory and `$SCR` for the scratch directory. The harness's `running 1 test`, blank,
`test diag_run ...`, and `test result:` lines are removed, along with the `MINOS-GUEST-BOOT-OK` and
`MINOS-GUEST-POWEROFF` console lines. Of the console, only lines containing `MINOS-` or `minos-guest:` are kept (not the
cmdline or env lines), as printable ASCII. No stream byte is printed: a stream connection is shown as its length,
pattern match, and SHA-256, and a status frame is reduced to the guest's `vsock_stream` result. Trace lines are
VMM-side metadata (descriptor lengths and header fields), never payload.

Command for every VM run (`<label>`, `<vmm>`, `<guest>`, and the `DIAG_*` values are in each `##` line):

```
MINOS_VMM=<vmm> MINOS_GUEST=<guest> MINOS_KRUNFW=$(realpath /opt/homebrew/lib/libkrunfw.5.dylib) \
  DIAG_MODE=<supervisor|blocking> DIAG_BYTES=<n> [DIAG_RCVBUF=<n>] [DIAG_ACK_MS=<n>] DIAG_RUN_DIR=$SCR/runs/f-<label> \
  $SCR/target/debug/deps/diag_vsock-a3bba50bc60bd2ff --ignored --exact diag_run --nocapture
```

Every run uses the 02-08 stream job (`vsock_stream` on port 1024, then the frame, then power off), launcher
`max_duration_ms` 15000, a 30 s host deadline, 1 vCPU, and 256 MiB. `harness exit` is the harness process's exit code.
The VM's outcome is in the `DIAG` line, where `exit=` is minos-vmm's exit code. All 45 runs in E1 to E6 were made in
one pass after the exploratory runs and repeat them.

#### Minos-side suspects

##### S5: the probe's byte counting

Test: compute the SHA-256 of the pattern prefix `i % 251` for `[0, n)` independently, and compare it with every digest the
host recorded.

Command: `for n in 1399 4096 8192 16384 32768 40960 262144 1048576 4194304; do python3 -I -c "import hashlib,sys; n=int(sys.argv[1]); print(n, hashlib.sha256(bytes(i%251 for i in range(n))).hexdigest())" $n; done; python3 -I -c "import hashlib; print(0, hashlib.sha256(b'').hexdigest())"`

Exit code: 0

```
1399 2269ee2e44aa4d9af7952464a48cf92e1f27f4d1911e0b425e7f8598d29ebfda
4096 d67c656e01756650d77717b0839985a056ec28ffe174601d690fc407a2ceffca
8192 25df2449b2e5a35fea14e02a7158e283801a1069c9f84631b9a9dacb2f809a7f
16384 4348e3b98e8a327b34ced39c1da9e67cdb4cd5e48e4d7960607a3ae403d35f0c
32768 09fed9cbfb98b6ab0f3e8ff63b7b1f9b0e07d58b225295c78fdc023cc4985a72
40960 dfb4847de067bacf1057c453e3860ac05782032ac193b011b1c2bfee36a8636b
262144 31a1f9dea0169551092d05e8bf4a446228c8c3eb4c9b713c66adcb7fd53c89be
1048576 631b84027d6b9e52b539c4e8373622d23032dfadc64d60af87339c9037e4f769
4194304 a117210941a0b00dcb2d8577e680d84b6fa0eaf760d2afc654c953b9859d54fa
0 e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
```

Observation: every digest in DEFECT-01 and below equals the digest of the prefix of its length. The 8192-byte results
(`25df2449…`) are the first 8192 bytes of the stream, not a later slice. The guest's `written` is the sum of `write`
return values (02-01's `write_counted`).

Verdict: **ruled out.** Both ends count correctly.

##### S1: the host supervisor's read side

Test: run the same size sweep twice.
- `DIAG_MODE=supervisor` goes through `supervise::run`, the same path as 02-08's `vsock_stream_probe`.
- `DIAG_MODE=blocking` uses an independent listener. The harness binds its own `UnixListener` and spawns minos-vmm
  directly with `Command`, without `supervise::run`. It switches each accepted connection to blocking and reads it to
  EOF with plain `read` calls, in its own thread, from the moment of accept.

Output (E1):

```
## E1-supervisor-8192: vmm=minos-vmm guest=minos-guest DIAG_MODE=supervisor DIAG_BYTES=8192 
DIAG mode=supervisor bytes=8192 exit=Some(0) killed=false elapsed=298.22725ms connections=2
  conn0 len 8192 overflowed false pattern true sha 25df2449b2e5a35fea14e02a7158e283801a1069c9f84631b9a9dacb2f809a7f
  conn1 frame (138 bytes): guest requested 8192 written 8192 error null raw_os_error null
  console: MINOS-GUEST-FRAME-SENT 138 bytes
  minos-vmm: exit 0
harness exit 0
## E1-supervisor-8193: vmm=minos-vmm guest=minos-guest DIAG_MODE=supervisor DIAG_BYTES=8193 
DIAG mode=supervisor bytes=8193 exit=Some(0) killed=false elapsed=298.512916ms connections=2
  conn0 len 8192 overflowed false pattern true sha 25df2449b2e5a35fea14e02a7158e283801a1069c9f84631b9a9dacb2f809a7f
  conn1 frame (138 bytes): guest requested 8193 written 8193 error null raw_os_error null
  console: MINOS-GUEST-FRAME-SENT 138 bytes
  minos-vmm: exit 0
harness exit 0
## E1-supervisor-16384: vmm=minos-vmm guest=minos-guest DIAG_MODE=supervisor DIAG_BYTES=16384 
DIAG mode=supervisor bytes=16384 exit=Some(0) killed=false elapsed=300.654917ms connections=2
  conn0 len 8192 overflowed false pattern true sha 25df2449b2e5a35fea14e02a7158e283801a1069c9f84631b9a9dacb2f809a7f
  conn1 frame (140 bytes): guest requested 16384 written 16384 error null raw_os_error null
  console: MINOS-GUEST-FRAME-SENT 140 bytes
  minos-vmm: exit 0
harness exit 0
## E1-supervisor-32768: vmm=minos-vmm guest=minos-guest DIAG_MODE=supervisor DIAG_BYTES=32768 
DIAG mode=supervisor bytes=32768 exit=Some(0) killed=false elapsed=299.297375ms connections=2
  conn0 len 8192 overflowed false pattern true sha 25df2449b2e5a35fea14e02a7158e283801a1069c9f84631b9a9dacb2f809a7f
  conn1 frame (140 bytes): guest requested 32768 written 32768 error null raw_os_error null
  console: MINOS-GUEST-FRAME-SENT 140 bytes
  minos-vmm: exit 0
harness exit 0
## E1-supervisor-32769: vmm=minos-vmm guest=minos-guest DIAG_MODE=supervisor DIAG_BYTES=32769 
DIAG mode=supervisor bytes=32769 exit=Some(0) killed=false elapsed=297.681791ms connections=2
  conn0 len 0 overflowed false pattern true sha e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
  conn1 frame (140 bytes): guest requested 32769 written 32769 error null raw_os_error null
  console: MINOS-GUEST-FRAME-SENT 140 bytes
  minos-vmm: exit 0
harness exit 0
## E1-supervisor-40960: vmm=minos-vmm guest=minos-guest DIAG_MODE=supervisor DIAG_BYTES=40960 
DIAG mode=supervisor bytes=40960 exit=Some(0) killed=false elapsed=300.62025ms connections=2
  conn0 len 0 overflowed false pattern true sha e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
  conn1 frame (140 bytes): guest requested 40960 written 40960 error null raw_os_error null
  console: MINOS-GUEST-FRAME-SENT 140 bytes
  minos-vmm: exit 0
harness exit 0
## E1-supervisor-262144: vmm=minos-vmm guest=minos-guest DIAG_MODE=supervisor DIAG_BYTES=262144 
DIAG mode=supervisor bytes=262144 exit=Some(0) killed=false elapsed=311.449708ms connections=2
  conn0 len 0 overflowed false pattern true sha e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
  conn1 frame (142 bytes): guest requested 262144 written 262144 error null raw_os_error null
  console: MINOS-GUEST-FRAME-SENT 142 bytes
  minos-vmm: exit 0
harness exit 0
## E1-supervisor-1048576: vmm=minos-vmm guest=minos-guest DIAG_MODE=supervisor DIAG_BYTES=1048576 
DIAG mode=supervisor bytes=1048576 exit=Some(124) killed=false elapsed=15.26046125s connections=1
  conn0 len 0 overflowed false pattern true sha e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
  minos-vmm: watchdog deadline
  minos-vmm: exit 124
harness exit 0
## E1-blocking-8192: vmm=minos-vmm guest=minos-guest DIAG_MODE=blocking DIAG_BYTES=8192 
DIAG mode=blocking rcvbuf_req=0 bytes=8192 exit=Some(0) killed=false elapsed=302.400375ms connections=2
  conn0 len 8192 pattern true sha 25df2449b2e5a35fea14e02a7158e283801a1069c9f84631b9a9dacb2f809a7f; first_byte Some(296.607917ms) eof 296.610458ms error None SO_RCVBUF Some(8192) SO_SNDBUF Some(8192)
  conn1 frame (138 bytes): guest requested 8192 written 8192 error null raw_os_error null; first_byte Some(296.607917ms) eof 296.610458ms error None SO_RCVBUF Some(8192) SO_SNDBUF Some(8192)
  console: MINOS-GUEST-FRAME-SENT 138 bytes
  minos-vmm: exit 0
harness exit 0
## E1-blocking-8193: vmm=minos-vmm guest=minos-guest DIAG_MODE=blocking DIAG_BYTES=8193 
DIAG mode=blocking rcvbuf_req=0 bytes=8193 exit=Some(0) killed=false elapsed=305.257125ms connections=2
  conn0 len 8192 pattern true sha 25df2449b2e5a35fea14e02a7158e283801a1069c9f84631b9a9dacb2f809a7f; first_byte Some(300.372334ms) eof 300.375625ms error None SO_RCVBUF Some(8192) SO_SNDBUF Some(8192)
  conn1 frame (138 bytes): guest requested 8193 written 8193 error null raw_os_error null; first_byte Some(300.374292ms) eof 300.375584ms error None SO_RCVBUF Some(8192) SO_SNDBUF Some(8192)
  console: MINOS-GUEST-FRAME-SENT 138 bytes
  minos-vmm: exit 0
harness exit 0
## E1-blocking-16384: vmm=minos-vmm guest=minos-guest DIAG_MODE=blocking DIAG_BYTES=16384 
DIAG mode=blocking rcvbuf_req=0 bytes=16384 exit=Some(0) killed=false elapsed=304.031959ms connections=2
  conn0 len 8192 pattern true sha 25df2449b2e5a35fea14e02a7158e283801a1069c9f84631b9a9dacb2f809a7f; first_byte Some(299.137625ms) eof 299.140667ms error None SO_RCVBUF Some(8192) SO_SNDBUF Some(8192)
  conn1 frame (140 bytes): guest requested 16384 written 16384 error null raw_os_error null; first_byte Some(299.137417ms) eof 299.140209ms error None SO_RCVBUF Some(8192) SO_SNDBUF Some(8192)
  console: MINOS-GUEST-FRAME-SENT 140 bytes
  minos-vmm: exit 0
harness exit 0
## E1-blocking-32768: vmm=minos-vmm guest=minos-guest DIAG_MODE=blocking DIAG_BYTES=32768 
DIAG mode=blocking rcvbuf_req=0 bytes=32768 exit=Some(0) killed=false elapsed=306.234917ms connections=2
  conn0 len 8192 pattern true sha 25df2449b2e5a35fea14e02a7158e283801a1069c9f84631b9a9dacb2f809a7f; first_byte Some(300.03925ms) eof 300.042334ms error None SO_RCVBUF Some(8192) SO_SNDBUF Some(8192)
  conn1 frame (140 bytes): guest requested 32768 written 32768 error null raw_os_error null; first_byte Some(300.03925ms) eof 300.042125ms error None SO_RCVBUF Some(8192) SO_SNDBUF Some(8192)
  console: MINOS-GUEST-FRAME-SENT 140 bytes
  minos-vmm: exit 0
harness exit 0
## E1-blocking-32769: vmm=minos-vmm guest=minos-guest DIAG_MODE=blocking DIAG_BYTES=32769 
DIAG mode=blocking rcvbuf_req=0 bytes=32769 exit=Some(0) killed=false elapsed=306.466958ms connections=2
  conn0 len 0 pattern true sha e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855; first_byte None eof 301.580208ms error None SO_RCVBUF Some(8192) SO_SNDBUF Some(8192)
  conn1 frame (140 bytes): guest requested 32769 written 32769 error null raw_os_error null; first_byte Some(301.58025ms) eof 301.582958ms error None SO_RCVBUF Some(8192) SO_SNDBUF Some(8192)
  console: MINOS-GUEST-FRAME-SENT 140 bytes
  minos-vmm: exit 0
harness exit 0
## E1-blocking-40960: vmm=minos-vmm guest=minos-guest DIAG_MODE=blocking DIAG_BYTES=40960 
DIAG mode=blocking rcvbuf_req=0 bytes=40960 exit=Some(0) killed=false elapsed=305.147625ms connections=2
  conn0 len 0 pattern true sha e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855; first_byte None eof 301.617875ms error None SO_RCVBUF Some(8192) SO_SNDBUF Some(8192)
  conn1 frame (140 bytes): guest requested 40960 written 40960 error null raw_os_error null; first_byte Some(301.618083ms) eof 301.620458ms error None SO_RCVBUF Some(8192) SO_SNDBUF Some(8192)
  console: MINOS-GUEST-FRAME-SENT 140 bytes
  minos-vmm: exit 0
harness exit 0
## E1-blocking-262144: vmm=minos-vmm guest=minos-guest DIAG_MODE=blocking DIAG_BYTES=262144 
DIAG mode=blocking rcvbuf_req=0 bytes=262144 exit=Some(0) killed=false elapsed=306.27475ms connections=2
  conn0 len 0 pattern true sha e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855; first_byte None eof 301.505542ms error None SO_RCVBUF Some(8192) SO_SNDBUF Some(8192)
  conn1 frame (142 bytes): guest requested 262144 written 262144 error null raw_os_error null; first_byte Some(302.739167ms) eof 302.741875ms error None SO_RCVBUF Some(8192) SO_SNDBUF Some(8192)
  console: MINOS-GUEST-FRAME-SENT 142 bytes
  minos-vmm: exit 0
harness exit 0
## E1-blocking-1048576: vmm=minos-vmm guest=minos-guest DIAG_MODE=blocking DIAG_BYTES=1048576 
DIAG mode=blocking rcvbuf_req=0 bytes=1048576 exit=Some(124) killed=false elapsed=15.212690625s connections=1
  conn0 len 0 pattern true sha e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855; first_byte None eof 15.20508525s error None SO_RCVBUF Some(8192) SO_SNDBUF Some(8192)
  minos-vmm: watchdog deadline
  minos-vmm: exit 124
harness exit 0
```

Observation:
- Both modes give the same length and digest at every size:

  | Bytes sent | Bytes received |
  |---|---|
  | 8192 | 8192 |
  | 8193, 16384, 32768 | 8192 |
  | 32769, 40960, 262144 | 0 |
  | 1048576 | 0, then watchdog exit 124 |

- The boundary between "8192 arrive" and "0 arrive" is exactly 32768|32769.
- In blocking mode the first byte and the EOF arrive about 3 µs apart. For example, in `E1-blocking-16384` the first
  byte came at 299.137625 ms and the EOF at 299.140667 ms. The stream had already ended by the time the reader woke.

Verdict: **ruled out.** The supervisor's 1 ms non-blocking poll, its deadline window, and its sequential accept change
nothing.

##### S2: host socket buffers

Command: `sysctl net.local.stream.sendspace net.local.stream.recvspace kern.ipc.maxsockbuf` (read only, nothing changed)

Exit code: 0

```
net.local.stream.sendspace: 8192
net.local.stream.recvspace: 8192
kern.ipc.maxsockbuf: 8388608
```

Test (host only, no VM): a non-blocking writer fills a Unix stream whose peer never reads. It runs three times: with
default buffers, with the reader's `SO_RCVBUF` at 1 MiB, and with the writer's `SO_SNDBUF` at 1 MiB.

Command: `$SCR/target/debug/deps/diag_vsock-a3bba50bc60bd2ff --ignored --exact host_unix_capacity --nocapture`

Exit code: 0

```
running 1 test
HOST default buffers: writer SO_SNDBUF 8192, reader SO_RCVBUF 8192: 8192 bytes accepted before EWOULDBLOCK
HOST reader SO_RCVBUF 1 MiB: writer SO_SNDBUF 8192, reader SO_RCVBUF 1048576: 8192 bytes accepted before EWOULDBLOCK
HOST writer SO_SNDBUF 1 MiB: writer SO_SNDBUF 1048576, reader SO_RCVBUF 8192: 1048576 bytes accepted before EWOULDBLOCK
test host_unix_capacity ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s
```

VM test: blocking listener with `SO_RCVBUF` 1048576 set on each accepted connection (E2):

```
## E2-rcvbuf-16384: vmm=minos-vmm guest=minos-guest DIAG_MODE=blocking DIAG_BYTES=16384 DIAG_RCVBUF=1048576
DIAG mode=blocking rcvbuf_req=1048576 bytes=16384 exit=Some(0) killed=false elapsed=308.895666ms connections=2
  conn0 len 8192 pattern true sha 25df2449b2e5a35fea14e02a7158e283801a1069c9f84631b9a9dacb2f809a7f; first_byte Some(303.736041ms) eof 303.739375ms error None SO_RCVBUF Some(1048576) SO_SNDBUF Some(8192)
  conn1 frame (140 bytes): guest requested 16384 written 16384 error null raw_os_error null; first_byte Some(303.738875ms) eof 303.74025ms error None SO_RCVBUF Some(1048576) SO_SNDBUF Some(8192)
  console: MINOS-GUEST-FRAME-SENT 140 bytes
  minos-vmm: exit 0
harness exit 0
## E2-rcvbuf-32768: vmm=minos-vmm guest=minos-guest DIAG_MODE=blocking DIAG_BYTES=32768 DIAG_RCVBUF=1048576
DIAG mode=blocking rcvbuf_req=1048576 bytes=32768 exit=Some(0) killed=false elapsed=298.026125ms connections=2
  conn0 len 8192 pattern true sha 25df2449b2e5a35fea14e02a7158e283801a1069c9f84631b9a9dacb2f809a7f; first_byte Some(291.552125ms) eof 291.554417ms error None SO_RCVBUF Some(1048576) SO_SNDBUF Some(8192)
  conn1 frame (140 bytes): guest requested 32768 written 32768 error null raw_os_error null; first_byte Some(291.551958ms) eof 291.554417ms error None SO_RCVBUF Some(1048576) SO_SNDBUF Some(8192)
  console: MINOS-GUEST-FRAME-SENT 140 bytes
  minos-vmm: exit 0
harness exit 0
```

Observation:
- On macOS the bytes a non-blocking writer can queue are bounded by the writer's own `SO_SNDBUF` (8192, which is
  `net.local.stream.sendspace`), not by the reader's `SO_RCVBUF`.
- In the VM path the writer is `msb_krun`'s proxy socket. `proxy_fd_create` (unix.rs:94-132) sets `O_NONBLOCK` and
  `SO_NOSIGPIPE`, and never sets `SO_SNDBUF`. The trace below shows `endpoint SO_SNDBUF=Ok(8192)`.
- Raising the receiving side to 1 MiB leaves both 16384 and 32768 at 8192 (E2).

Verdict: **ruled out.** The 8192 figure is the send buffer of the VMM's socket at the host default, which the receiving
socket Minos owns cannot move. Raising the global `sendspace` sysctl needs sudo and was not done. By H1 below, it would
only move the cliff, not remove it.

##### S3: the guest's write side

Code (`crates/minos-guest/src/linux.rs`, `vsock_stream`):
- The socket is a blocking `vsock 0.5.4` `VsockStream`. No `set_nonblocking`, `SO_SNDBUF`, or
  `SO_VM_SOCKETS_BUFFER_SIZE` is set.
- It writes 64 KiB `CHUNK`s through `write_counted`, then calls `shutdown(Write)` and `drop` at once, without waiting
  for anything.

Test: guest variants (scratch only; diff at the end):
- `delay`: sleep 2 s between the last write and `shutdown(Write)`.
- `chunk16k`: 16 KiB writes.
- `sndtimeo`: a 3 s `SO_SNDTIMEO`, so a blocked write returns.
- Combinations of the three. `none` is the variant build with no flag, the same behavior as stock.

Output (E3; stock minos-vmm):

```
## E3-none-16384: vmm=minos-vmm guest=minos-guest-none DIAG_MODE=supervisor DIAG_BYTES=16384 
DIAG mode=supervisor bytes=16384 exit=Some(0) killed=false elapsed=310.987875ms connections=2
  conn0 len 8192 overflowed false pattern true sha 25df2449b2e5a35fea14e02a7158e283801a1069c9f84631b9a9dacb2f809a7f
  conn1 frame (140 bytes): guest requested 16384 written 16384 error null raw_os_error null
  console: minos-guest: DIAG_GUEST [none] chunk 65536
  console: minos-guest: DIAG vsock_stream wrote 16384 before shutdown
  console: MINOS-GUEST-FRAME-SENT 140 bytes
  minos-vmm: exit 0
harness exit 0
## E3-delay-16384: vmm=minos-vmm guest=minos-guest-delay DIAG_MODE=supervisor DIAG_BYTES=16384 
DIAG mode=supervisor bytes=16384 exit=Some(0) killed=false elapsed=2.318879334s connections=2
  conn0 len 16384 overflowed false pattern true sha 4348e3b98e8a327b34ced39c1da9e67cdb4cd5e48e4d7960607a3ae403d35f0c
  conn1 frame (140 bytes): guest requested 16384 written 16384 error null raw_os_error null
  console: minos-guest: DIAG_GUEST [delay] chunk 65536
  console: minos-guest: DIAG vsock_stream wrote 16384 before shutdown
  console: MINOS-GUEST-FRAME-SENT 140 bytes
  minos-vmm: exit 0
harness exit 0
## E3-delay-32768: vmm=minos-vmm guest=minos-guest-delay DIAG_MODE=supervisor DIAG_BYTES=32768 
DIAG mode=supervisor bytes=32768 exit=Some(0) killed=false elapsed=2.31663575s connections=2
  conn0 len 32768 overflowed false pattern true sha 09fed9cbfb98b6ab0f3e8ff63b7b1f9b0e07d58b225295c78fdc023cc4985a72
  conn1 frame (140 bytes): guest requested 32768 written 32768 error null raw_os_error null
  console: minos-guest: DIAG_GUEST [delay] chunk 65536
  console: minos-guest: DIAG vsock_stream wrote 32768 before shutdown
  console: MINOS-GUEST-FRAME-SENT 140 bytes
  minos-vmm: exit 0
harness exit 0
## E3-delay-32769: vmm=minos-vmm guest=minos-guest-delay DIAG_MODE=supervisor DIAG_BYTES=32769 
DIAG mode=supervisor bytes=32769 exit=Some(0) killed=false elapsed=2.301948667s connections=2
  conn0 len 0 overflowed false pattern true sha e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
  conn1 frame (140 bytes): guest requested 32769 written 32769 error null raw_os_error null
  console: minos-guest: DIAG_GUEST [delay] chunk 65536
  console: minos-guest: DIAG vsock_stream wrote 32769 before shutdown
  console: MINOS-GUEST-FRAME-SENT 140 bytes
  minos-vmm: exit 0
harness exit 0
## E3-delay-262144: vmm=minos-vmm guest=minos-guest-delay DIAG_MODE=supervisor DIAG_BYTES=262144 
DIAG mode=supervisor bytes=262144 exit=Some(0) killed=false elapsed=2.313448417s connections=2
  conn0 len 0 overflowed false pattern true sha e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
  conn1 frame (142 bytes): guest requested 262144 written 262144 error null raw_os_error null
  console: minos-guest: DIAG_GUEST [delay] chunk 65536
  console: minos-guest: DIAG vsock_stream wrote 262144 before shutdown
  console: MINOS-GUEST-FRAME-SENT 142 bytes
  minos-vmm: exit 0
harness exit 0
## E3-chunk16k-16384: vmm=minos-vmm guest=minos-guest-chunk16k DIAG_MODE=supervisor DIAG_BYTES=16384 
DIAG mode=supervisor bytes=16384 exit=Some(0) killed=false elapsed=313.198ms connections=2
  conn0 len 8192 overflowed false pattern true sha 25df2449b2e5a35fea14e02a7158e283801a1069c9f84631b9a9dacb2f809a7f
  conn1 frame (140 bytes): guest requested 16384 written 16384 error null raw_os_error null
  console: minos-guest: DIAG_GUEST [chunk16k] chunk 16384
  console: minos-guest: DIAG vsock_stream wrote 16384 before shutdown
  console: MINOS-GUEST-FRAME-SENT 140 bytes
  minos-vmm: exit 0
harness exit 0
## E3-chunk16k-40960: vmm=minos-vmm guest=minos-guest-chunk16k DIAG_MODE=supervisor DIAG_BYTES=40960 
DIAG mode=supervisor bytes=40960 exit=Some(0) killed=false elapsed=312.871959ms connections=2
  conn0 len 8192 overflowed false pattern true sha 25df2449b2e5a35fea14e02a7158e283801a1069c9f84631b9a9dacb2f809a7f
  conn1 frame (140 bytes): guest requested 40960 written 40960 error null raw_os_error null
  console: minos-guest: DIAG_GUEST [chunk16k] chunk 16384
  console: minos-guest: DIAG vsock_stream wrote 40960 before shutdown
  console: MINOS-GUEST-FRAME-SENT 140 bytes
  minos-vmm: exit 0
harness exit 0
## E3-chunk16k-262144: vmm=minos-vmm guest=minos-guest-chunk16k DIAG_MODE=supervisor DIAG_BYTES=262144 
DIAG mode=supervisor bytes=262144 exit=Some(0) killed=false elapsed=313.164833ms connections=2
  conn0 len 8192 overflowed false pattern true sha 25df2449b2e5a35fea14e02a7158e283801a1069c9f84631b9a9dacb2f809a7f
  conn1 frame (142 bytes): guest requested 262144 written 262144 error null raw_os_error null
  console: minos-guest: DIAG_GUEST [chunk16k] chunk 16384
  console: minos-guest: DIAG vsock_stream wrote 262144 before shutdown
  console: MINOS-GUEST-FRAME-SENT 142 bytes
  minos-vmm: exit 0
harness exit 0
## E3-chunk16k+delay-40960: vmm=minos-vmm guest=minos-guest-chunk16k+delay DIAG_MODE=supervisor DIAG_BYTES=40960 
DIAG mode=supervisor bytes=40960 exit=Some(0) killed=false elapsed=2.318098334s connections=2
  conn0 len 40960 overflowed false pattern true sha dfb4847de067bacf1057c453e3860ac05782032ac193b011b1c2bfee36a8636b
  conn1 frame (140 bytes): guest requested 40960 written 40960 error null raw_os_error null
  console: minos-guest: DIAG_GUEST [chunk16k,delay] chunk 16384
  console: minos-guest: DIAG vsock_stream wrote 40960 before shutdown
  console: MINOS-GUEST-FRAME-SENT 140 bytes
  minos-vmm: exit 0
harness exit 0
## E3-chunk16k+delay-262144: vmm=minos-vmm guest=minos-guest-chunk16k+delay DIAG_MODE=supervisor DIAG_BYTES=262144 
DIAG mode=supervisor bytes=262144 exit=Some(0) killed=false elapsed=2.31813625s connections=2
  conn0 len 262144 overflowed false pattern true sha 31a1f9dea0169551092d05e8bf4a446228c8c3eb4c9b713c66adcb7fd53c89be
  conn1 frame (142 bytes): guest requested 262144 written 262144 error null raw_os_error null
  console: minos-guest: DIAG_GUEST [chunk16k,delay] chunk 16384
  console: minos-guest: DIAG vsock_stream wrote 262144 before shutdown
  console: MINOS-GUEST-FRAME-SENT 142 bytes
  minos-vmm: exit 0
harness exit 0
## E3-chunk16k+delay-1048576: vmm=minos-vmm guest=minos-guest-chunk16k+delay DIAG_MODE=supervisor DIAG_BYTES=1048576 
DIAG mode=supervisor bytes=1048576 exit=Some(124) killed=false elapsed=15.219756583s connections=1
  conn0 len 262144 overflowed false pattern true sha 31a1f9dea0169551092d05e8bf4a446228c8c3eb4c9b713c66adcb7fd53c89be
  console: minos-guest: DIAG_GUEST [chunk16k,delay] chunk 16384
  minos-vmm: watchdog deadline
  minos-vmm: exit 124
harness exit 0
## E3-chunk16k+sndtimeo-1048576: vmm=minos-vmm guest=minos-guest-chunk16k+sndtimeo DIAG_MODE=supervisor DIAG_BYTES=1048576 
DIAG mode=supervisor bytes=1048576 exit=Some(0) killed=false elapsed=3.37283775s connections=2
  conn0 len 262144 overflowed false pattern true sha 31a1f9dea0169551092d05e8bf4a446228c8c3eb4c9b713c66adcb7fd53c89be
  conn1 frame (185 bytes): guest requested 1048576 written 262144 error "Resource temporarily unavailable (os error 11)" raw_os_error 11
  console: minos-guest: DIAG_GUEST [chunk16k,sndtimeo] chunk 16384
  console: minos-guest: DIAG vsock_stream wrote 262144 before shutdown
  console: MINOS-GUEST-FRAME-SENT 185 bytes
  minos-vmm: exit 0
harness exit 0
## E3-sndtimeo-1048576: vmm=minos-vmm guest=minos-guest-sndtimeo DIAG_MODE=supervisor DIAG_BYTES=1048576 
DIAG mode=supervisor bytes=1048576 exit=Some(0) killed=false elapsed=3.373784917s connections=2
  conn0 len 0 overflowed false pattern true sha e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
  conn1 frame (185 bytes): guest requested 1048576 written 262144 error "Resource temporarily unavailable (os error 11)" raw_os_error 11
  console: minos-guest: DIAG_GUEST [sndtimeo] chunk 65536
  console: minos-guest: DIAG vsock_stream wrote 262144 before shutdown
  console: MINOS-GUEST-FRAME-SENT 185 bytes
  minos-vmm: exit 0
harness exit 0
```

Observation:
- **delay:** 16384 and 32768 arrive whole, but 32769 and 262144 still deliver 0. Losing everything after the first
  8 KiB depends on how soon `shutdown` follows the last write. Losing everything for writes over 32768 bytes does not.
- **chunk16k:** 40960 and 262144 deliver 8192 instead of 0. The all-or-nothing loss depends on a write size above 32768
  bytes.
- **chunk16k+delay:** 40960 and 262144 arrive whole. At 1048576, exactly 262144 bytes arrive and the guest then blocks
  until the watchdog, although not one packet was lost. The hang is therefore not explained by lost packets alone (H3).
- **sndtimeo** (16 KiB or 64 KiB writes) at 1048576: the guest's write fails with `EAGAIN` (11) after exactly 262144
  bytes.

Verdict: **ruled out as a defect.**
- A blocking write followed by `shutdown(SHUT_WR)` is ordinary stream use, and a 64 KiB write is within the virtio-vsock
  maximum packet size (64 KiB on both sides: `MAX_PKT_BUF_SIZE` in the device, `VIRTIO_VSOCK_MAX_PKT_BUF_SIZE` in the
  guest).
- The guest's timing and write size only decide which VMM defect fires.
- A guest-side workaround exists (small writes, and wait before `shutdown`), but it covers H1 and H2 only, not H3. It
  was not applied.

##### S4: the launch config and vsock builder options

Facts:
- minos-vmm's only vsock call is `.vsock(|vsock| vsock.unix_connect(config.vsock_port(), config.socket()))`
  (`crates/minos-vmm/src/launch.rs`, `build`). `msb_krun` 0.1.41 documents the same call shape:
  `.unix_connect(5000, "/run/supervisor.sock")` (src/api/builders.rs:110).
- `VsockBuilder` has `unix_connect`, `unix_listen`, `custom`, `custom_dgram`, `tcp_listen_remap`, and `inet_hijack`, and
  nothing else: no buffer size, credit, or socket option.
- The guest CID is fixed at 3 by `msb_krun` (src/api/vm.rs:820). The device offers `VERSION_1`, `IN_ORDER`, and
  `VSOCK_F_DGRAM`, but not `INDIRECT_DESC` (device.rs:35-39).
- The trace shows the device advertising `buf_alloc` 8388608 to the guest and the guest advertising 262144.

The other stream route kind, `custom` (`CustomStreamProxy`), would not avoid H2 or H3:
- It shares the TX parser (`from_tx_virtq_head`), which is H2.
- It uses the same credit threshold `CONN_TX_BUF_SIZE / 2` (custom_stream.rs:324-325), which is H3.
- Its `shutdown` (custom_stream.rs:482) also forwards at once.

Verdict: **ruled out.** No option Minos owns reaches the data path.

##### Race count for S1 and H1

Command: 20 runs each of `DIAG_BYTES=16384` with the stock VMM through the supervisor, the stock VMM through the
blocking listener, and the trace VMM through the supervisor, counting the length of connection 0. Every run exited 0.

```
stock-vmm supervisor DIAG_BYTES=16384, 20 runs: 20 x conn0 len 8192
stock-vmm blocking DIAG_BYTES=16384, 20 runs: 20 x conn0 len 8192
trace-vmm supervisor DIAG_BYTES=16384, 20 runs: 20 x conn0 len 8192
```

#### Dependency hypotheses

Trace summary, computed from each run's `$SCR/trace/vsock.log`:
- "RW packets" counts every guest data packet, including the status-frame packet of 140 to 144 bytes.
- "pending_write at each SHUTDOWN(SEND)" lists the stream connection first, then the frame connection.

```
E4-c16d1m: RW packets 16 (chains [44, 16384]), dropped 0, pending_write at each SHUTDOWN(SEND) [], max tx_cnt 262144, threshold ['4194304'], CREDIT_UPDATE pushed 0, unhandled ops 0
E4-d16384: RW packets 2 (chains [44, 140], [44, 16384]), dropped 0, pending_write at each SHUTDOWN(SEND) ['0', '0'], max tx_cnt 16384, threshold ['4194304'], CREDIT_UPDATE pushed 0, unhandled ops 0
E4-s16384: RW packets 2 (chains [44, 140], [44, 16384]), dropped 0, pending_write at each SHUTDOWN(SEND) ['0', '0'], max tx_cnt 16384, threshold ['4194304'], CREDIT_UPDATE pushed 0, unhandled ops 0
E4-s1m: RW packets 4 (chains [44, 32768, 32768]), dropped 4, pending_write at each SHUTDOWN(SEND) [], max tx_cnt 0, threshold [], CREDIT_UPDATE pushed 0, unhandled ops 0
E4-s32768: RW packets 2 (chains [44, 140], [44, 32768]), dropped 0, pending_write at each SHUTDOWN(SEND) ['24576', '0'], max tx_cnt 32768, threshold ['4194304'], CREDIT_UPDATE pushed 0, unhandled ops 0
E4-s32769: RW packets 2 (chains [44, 140], [44, 32768, 1]), dropped 1, pending_write at each SHUTDOWN(SEND) ['0', '0'], max tx_cnt 140, threshold ['4194304'], CREDIT_UPDATE pushed 0, unhandled ops 0
E5-credit64k-c16d-1m: RW packets 65 (chains [44, 144], [44, 16384]), dropped 0, pending_write at each SHUTDOWN(SEND) ['0', '0'], max tx_cnt 1048576, threshold ['65536'], CREDIT_UPDATE pushed 16, unhandled ops 0
E5-credit64k-stock-1m: RW packets 4 (chains [44, 32768, 32768]), dropped 4, pending_write at each SHUTDOWN(SEND) [], max tx_cnt 0, threshold [], CREDIT_UPDATE pushed 0, unhandled ops 0
E5-drain-16384: RW packets 2 (chains [44, 140], [44, 16384]), dropped 0, pending_write at each SHUTDOWN(SEND) ['8192', '0'], max tx_cnt 16384, threshold ['4194304'], CREDIT_UPDATE pushed 0, unhandled ops 0, CF: UNIX id=0x95e0a88000000400 CF drain before shutdown: pending now 0 after 4 tries
E5-drain-32768: RW packets 2 (chains [44, 140], [44, 32768]), dropped 0, pending_write at each SHUTDOWN(SEND) ['24576', '0'], max tx_cnt 32768, threshold ['4194304'], CREDIT_UPDATE pushed 0, unhandled ops 0, CF: UNIX id=0x36fec66500000400 CF drain before shutdown: pending now 0 after 4 tries
E5-drain-32769: RW packets 2 (chains [44, 140], [44, 32768, 1]), dropped 1, pending_write at each SHUTDOWN(SEND) ['0', '0'], max tx_cnt 140, threshold ['4194304'], CREDIT_UPDATE pushed 0, unhandled ops 0, CF: UNIX id=0x25cd02d200000400 CF drain before shutdown: pending now 0 after 0 tries
E5-drain+credit64k-c16-1m: RW packets 65 (chains [44, 144], [44, 16384]), dropped 0, pending_write at each SHUTDOWN(SEND) ['1024000', '0'], max tx_cnt 1048576, threshold ['65536'], CREDIT_UPDATE pushed 16, unhandled ops 0, CF: UNIX id=0x36d6a6fb00000400 CF drain before shutdown: pending now 0 after 126 tries
E5-drain+credit64k-c16-4m: RW packets 257 (chains [44, 144], [44, 16384]), dropped 0, pending_write at each SHUTDOWN(SEND) ['4063232', '0'], max tx_cnt 4194304, threshold ['65536'], CREDIT_UPDATE pushed 64, unhandled ops 0, CF: UNIX id=0xa869d81d00000400 CF drain before shutdown: pending now 0 after 499 tries
```

##### H1: `UnixProxy::shutdown` applies the guest's `SHUT_WR` while `pending_write` still holds bytes

Instrumentation: logs the length of `pending_write` at every `SHUTDOWN`, every flush (before and after, with its
result), every muxer event on the proxy, and every reset pushed to the guest.

Trace `E4-s32768` (the first 14 lines; the rest are the frame connection):

```
     0.072ms [main] TX op=1 len=0 flags=0 type=1 src=3:3942592853 dst=2:1024 buf_alloc=262144 fwd_cnt=0 chain_desc_lens=[44]
     0.141ms [main] UNIX id=0xeaff315500000400 connect_vsock ok: endpoint SO_SNDBUF=Ok(8192) SO_RCVBUF=Ok(8192)
     0.177ms [main] RX to guest: RESPONSE port 1024->3942592853 buf_alloc=8388608 fwd_cnt(as left in guest RX buffer)=0 flags=0
     0.306ms [main] TX op=5 len=32768 flags=0 type=1 src=3:3942592853 dst=2:1024 buf_alloc=262144 fwd_cnt=0 chain_desc_lens=[44, 32768]
     0.342ms [main] UNIX id=0xeaff315500000400 sendmsg: hdr len 32768 buf() len 32768 pending before 0 tx_cnt before 0
     0.390ms [main] UNIX id=0xeaff315500000400 flush: pending 32768 -> 24576 result Ok(())
     0.417ms [main] UNIX id=0xeaff315500000400 sendmsg done: pending 24576 tx_cnt 32768 last_tx_cnt_sent 0 unacked 32768 credit_update_threshold 4194304
     0.472ms [main] TX op=4 len=0 flags=2 type=1 src=3:3942592853 dst=2:1024 buf_alloc=262144 fwd_cnt=0 chain_desc_lens=[44]
     0.502ms [main] UNIX id=0xeaff315500000400 SHUTDOWN flags=2 how=Write: pending_write 24576 bytes at shutdown, tx_cnt 32768
     0.559ms [vsock muxer] UNIX id=0xeaff315500000400 event OUT | HANG_UP status Connected pending 24576
     0.561ms [main] TX op=4 len=0 flags=3 type=1 src=3:3942592853 dst=2:1024 buf_alloc=262144 fwd_cnt=0 chain_desc_lens=[44]
     0.578ms [vsock muxer] UNIX id=0xeaff315500000400 push_reset to guest
     0.647ms [vsock muxer] RX to guest: RST port 1024->3942592853
     0.705ms [main] UNIX id=0xeaff315500000400 SHUTDOWN flags=3 how=Both: pending_write 24576 bytes at shutdown, tx_cnt 32768
```

Trace of a 16384-byte loss: an exploratory run with the same trace binary, whose harness reported
`conn0 len 8192 overflowed false pattern true sha 25df2449…`. The first 12 lines:

```
     0.079ms [main] TX op=1 len=0 flags=0 type=1 src=3:2431014356 dst=2:1024 buf_alloc=262144 fwd_cnt=0 chain_desc_lens=[44]
     0.220ms [main] UNIX id=0x90e655d400000400 connect_vsock ok: endpoint SO_SNDBUF=Ok(8192) SO_RCVBUF=Ok(8192)
     0.452ms [main] RX to guest: RESPONSE port 1024->2431014356 buf_alloc=8388608 fwd_cnt(as left in guest RX buffer)=0 flags=0
     0.565ms [main] TX op=5 len=16384 flags=0 type=1 src=3:2431014356 dst=2:1024 buf_alloc=262144 fwd_cnt=0 chain_desc_lens=[44, 16384]
     0.589ms [main] UNIX id=0x90e655d400000400 sendmsg: hdr len 16384 buf() len 16384 pending before 0 tx_cnt before 0
     0.655ms [main] UNIX id=0x90e655d400000400 flush: pending 16384 -> 8192 result Ok(())
     0.703ms [main] UNIX id=0x90e655d400000400 sendmsg done: pending 8192 tx_cnt 16384 last_tx_cnt_sent 0 unacked 16384 credit_update_threshold 4194304
     0.762ms [main] TX op=4 len=0 flags=2 type=1 src=3:2431014356 dst=2:1024 buf_alloc=262144 fwd_cnt=0 chain_desc_lens=[44]
     0.797ms [main] UNIX id=0x90e655d400000400 SHUTDOWN flags=2 how=Write: pending_write 8192 bytes at shutdown, tx_cnt 16384
     0.848ms [main] TX op=4 len=0 flags=3 type=1 src=3:2431014356 dst=2:1024 buf_alloc=262144 fwd_cnt=0 chain_desc_lens=[44]
     0.862ms [vsock muxer] UNIX id=0x90e655d400000400 event OUT | HANG_UP status Connected pending 8192
     0.893ms [vsock muxer] UNIX id=0x90e655d400000400 push_reset to guest
```

Trace `E4-s16384`: the one traced run that won the race and delivered all 16384 bytes. The first 12 lines:

```
     0.079ms [main] TX op=1 len=0 flags=0 type=1 src=3:1580963776 dst=2:1024 buf_alloc=262144 fwd_cnt=0 chain_desc_lens=[44]
     0.201ms [main] UNIX id=0x5e3b97c000000400 connect_vsock ok: endpoint SO_SNDBUF=Ok(8192) SO_RCVBUF=Ok(8192)
     0.475ms [main] RX to guest: RESPONSE port 1024->1580963776 buf_alloc=8388608 fwd_cnt(as left in guest RX buffer)=0 flags=0
     0.580ms [main] TX op=5 len=16384 flags=0 type=1 src=3:1580963776 dst=2:1024 buf_alloc=262144 fwd_cnt=0 chain_desc_lens=[44, 16384]
     0.610ms [main] UNIX id=0x5e3b97c000000400 sendmsg: hdr len 16384 buf() len 16384 pending before 0 tx_cnt before 0
     0.642ms [main] UNIX id=0x5e3b97c000000400 flush: pending 16384 -> 8192 result Ok(())
     0.669ms [main] UNIX id=0x5e3b97c000000400 sendmsg done: pending 8192 tx_cnt 16384 last_tx_cnt_sent 0 unacked 16384 credit_update_threshold 4194304
     0.698ms [main] TX op=4 len=0 flags=2 type=1 src=3:1580963776 dst=2:1024 buf_alloc=262144 fwd_cnt=0 chain_desc_lens=[44]
     0.706ms [vsock muxer] UNIX id=0x5e3b97c000000400 event OUT status Connected pending 8192
     0.733ms [vsock muxer] UNIX id=0x5e3b97c000000400 flush: pending 8192 -> 0 result Ok(())
     0.759ms [main] UNIX id=0x5e3b97c000000400 SHUTDOWN flags=2 how=Write: pending_write 0 bytes at shutdown, tx_cnt 16384
     0.785ms [main] TX op=4 len=0 flags=3 type=1 src=3:1580963776 dst=2:1024 buf_alloc=262144 fwd_cnt=0 chain_desc_lens=[44]
```

Observation:
- The proxy socket takes 8192 bytes. `flush` stops at `WouldBlock` and leaves the rest in `pending_write`: 8192 bytes for
  a 16384-byte packet, 24576 for a 32768-byte one.
- The guest's `SHUTDOWN` (flags 2, SEND) arrives about 0.1 ms later on the VMM's main thread, and `shutdown`
  (unix.rs:656-671) calls `shutdown(SHUT_WR)` with those bytes still queued.
- The muxer thread's next event on the socket is `OUT | HANG_UP`, the result of that `SHUT_WR`. `process_event` checks
  `HANG_UP` first (unix.rs:689-700): `push_reset`, `Closed`, deferred removal, return. No flush is tried, nothing is
  logged, and the queued bytes are discarded. A flush after `SHUT_WR` would have failed with `EPIPE` anyway.
- The host sees 8192 bytes and then a clean EOF.

It is a race between two threads. The muxer thread flushes after an `OUT` event, once per 8 KiB the host drains. The main
thread handles the guest's `SHUTDOWN`.
- In `E4-s16384` the muxer flushed the last 8192 bytes at 0.733 ms, 26 µs before the main thread handled `SHUTDOWN` at
  0.759 ms, so all 16384 arrived.
- The stock VMM lost the race in 40 of 40 runs at 16384 bytes, with both readers. The trace VMM won it once in 23
  traced runs.
- Every further 8 KiB needs another drain-and-flush cycle, so a larger overflow practically always loses.

Counterfactual (`minos-vmm-cf-drain`, E5): the stock guest's 16384 and 32768 arrive whole, after a drain of 4 tries
each, with `pending_write` at 8192 and 24576 when `SHUTDOWN` arrived. 32769 still delivers 0: H2 is independent.

Verdict: **proven.** This is the cause of the "8193 to 32768 bytes deliver exactly 8192" cliff. 8192 is the proxy
socket's `SO_SNDBUF`.

##### H2: `from_tx_virtq_head` drops a packet whose payload spans more than one data descriptor

Instrumentation: logs each TX descriptor chain's lengths and header fields, the rejection, and the drop.

Trace `E4-s32769` (the first 10 lines):

```
     0.081ms [main] TX op=1 len=0 flags=0 type=1 src=3:2774011407 dst=2:1024 buf_alloc=262144 fwd_cnt=0 chain_desc_lens=[44]
     0.197ms [main] UNIX id=0xa5580e0f00000400 connect_vsock ok: endpoint SO_SNDBUF=Ok(8192) SO_RCVBUF=Ok(8192)
     0.486ms [main] RX to guest: RESPONSE port 1024->2774011407 buf_alloc=8388608 fwd_cnt(as left in guest RX buffer)=0 flags=0
     0.617ms [main] TX op=5 len=32769 flags=0 type=1 src=3:2774011407 dst=2:1024 buf_alloc=262144 fwd_cnt=0 chain_desc_lens=[44, 32768, 1]
     0.650ms [main] TX REJECT BufDescTooSmall: first data descriptor 32768 bytes < header len 32769
     0.700ms [main] TX DROPPED BufDescTooSmall: descriptor marked used (len 0), guest not told
     0.755ms [main] TX op=4 len=0 flags=2 type=1 src=3:2774011407 dst=2:1024 buf_alloc=262144 fwd_cnt=0 chain_desc_lens=[44]
     0.802ms [main] UNIX id=0xa5580e0f00000400 SHUTDOWN flags=2 how=Write: pending_write 0 bytes at shutdown, tx_cnt 0
     0.851ms [main] TX op=4 len=0 flags=3 type=1 src=3:2774011407 dst=2:1024 buf_alloc=262144 fwd_cnt=0 chain_desc_lens=[44]
     0.895ms [main] UNIX id=0xa5580e0f00000400 SHUTDOWN flags=3 how=Both: pending_write 0 bytes at shutdown, tx_cnt 0
```

Trace `E4-s1m` (complete; the guest sends nothing more):

```
     0.069ms [main] TX op=1 len=0 flags=0 type=1 src=3:287152366 dst=2:1024 buf_alloc=262144 fwd_cnt=0 chain_desc_lens=[44]
     0.184ms [main] UNIX id=0x111d98ee00000400 connect_vsock ok: endpoint SO_SNDBUF=Ok(8192) SO_RCVBUF=Ok(8192)
     0.484ms [main] RX to guest: RESPONSE port 1024->287152366 buf_alloc=8388608 fwd_cnt(as left in guest RX buffer)=0 flags=0
     0.631ms [main] TX op=5 len=65536 flags=0 type=1 src=3:287152366 dst=2:1024 buf_alloc=262144 fwd_cnt=0 chain_desc_lens=[44, 32768, 32768]
     0.661ms [main] TX REJECT BufDescTooSmall: first data descriptor 32768 bytes < header len 65536
     0.714ms [main] TX DROPPED BufDescTooSmall: descriptor marked used (len 0), guest not told
     0.760ms [main] TX op=5 len=65536 flags=0 type=1 src=3:287152366 dst=2:1024 buf_alloc=262144 fwd_cnt=0 chain_desc_lens=[44, 32768, 32768]
     0.810ms [main] TX REJECT BufDescTooSmall: first data descriptor 32768 bytes < header len 65536
     0.853ms [main] TX DROPPED BufDescTooSmall: descriptor marked used (len 0), guest not told
     0.906ms [main] TX op=5 len=65536 flags=0 type=1 src=3:287152366 dst=2:1024 buf_alloc=262144 fwd_cnt=0 chain_desc_lens=[44, 32768, 32768]
     0.950ms [main] TX REJECT BufDescTooSmall: first data descriptor 32768 bytes < header len 65536
     0.998ms [main] TX DROPPED BufDescTooSmall: descriptor marked used (len 0), guest not told
     1.049ms [main] TX op=5 len=65536 flags=0 type=1 src=3:287152366 dst=2:1024 buf_alloc=262144 fwd_cnt=0 chain_desc_lens=[44, 32768, 32768]
     1.092ms [main] TX REJECT BufDescTooSmall: first data descriptor 32768 bytes < header len 65536
     1.139ms [main] TX DROPPED BufDescTooSmall: descriptor marked used (len 0), guest not told
```

Observation:
- A 32768-byte packet arrives as chain `[44, 32768]` and is accepted (`E4-s32768`).
- A 32769-byte packet arrives as `[44, 32768, 1]`. It fails with `BufDescTooSmall`, because the first data descriptor
  (32768) is shorter than the header's `len` (32769), and `process_stream_tx` marks it used with length 0 and moves on
  (device.rs:172-182).
- 64 KiB packets arrive as `[44, 32768, 32768]` and are all dropped.
- The guest is never told. Its write returned success.

Guest side (Linux 6.12.109, libkrunfw 5.6.2's `KERNEL_VERSION`):
- Sources fetched from
  `https://git.kernel.org/pub/scm/linux/kernel/git/stable/linux.git/plain/<file>?h=v6.12.109`, read as text:
  `include/linux/virtio_vsock.h` (SHA-256 `ef4e16ab…1ca0`), `net/vmw_vsock/virtio_transport.c` (`c24b2e9c…4a59`),
  `net/vmw_vsock/virtio_transport_common.c` (`2943259a…9ddc`), and `net/vmw_vsock/af_vsock.c` (`875793ab…4eec`).
- libkrunfw 5.6.2's vsock patches (0003 to 0012) touch datagram support, transport lookup, and TSI. None touches the
  functions below.
- `include/linux/virtio_vsock.h:84-92`:

  ```
      84	static inline struct sk_buff *virtio_vsock_alloc_skb(unsigned int size, gfp_t mask)
      85	{
      86		if (size <= SKB_WITH_OVERHEAD(PAGE_SIZE << PAGE_ALLOC_COSTLY_ORDER))
      87			return virtio_vsock_alloc_linear_skb(size, mask);
      88	
      89		size -= VIRTIO_VSOCK_SKB_HEADROOM;
      90		return __virtio_vsock_alloc_skb_with_frags(VIRTIO_VSOCK_SKB_HEADROOM,
      91							   size, mask);
      92	}
  ```

- `net/vmw_vsock/virtio_transport.c:109-135` (`virtio_transport_send_skb`) adds one scatterlist entry for the linear
  payload, or one per page frag:

  ```
     109		if (!skb_is_nonlinear(skb)) {
     110			if (skb->len > 0) {
     111				sg_init_one(sgs[out_sg], skb->data, skb->len);
     112				out_sg++;
     113			}
     114		} else {
     ...
     126			for (i = 0; i < si->nr_frags; i++) {
     ...
     135				va = page_to_virt(skb_frag_page(skb_frag));
  ```

- So the payload goes in one data descriptor up to 32768 bytes, and in two or more above that. Above
  `SKB_WITH_OVERHEAD(32 KiB)` the skb is nonlinear, with frags of at most order 3 (32 KiB with 4 KiB pages) from
  `alloc_skb_with_frags`. The observed boundary, 32768|32769 (E1), matches exactly.
- The 6.12.y log names the change: `69c5bf306115` "vsock/virtio: Allocate nonlinear SKBs for handling large transmit
  buffers" (2026-01-30, Will Deacon).
- A packet is as large as the `write` (up to 64 KiB) when credit allows, so `minos-guest`'s 64 KiB writes produce
  64 KiB packets.

Verdict: **proven.** This is the cause of the "over 32768 bytes per write deliver 0" cliff.

##### H3: the hang at 1 MiB and above

DEFECT-01's guess ("credit never returned for dropped packets") is right but not sufficient.

Instrumentation: logs the guest's `buf_alloc` and `fwd_cnt` in each TX header. After every `sendmsg` it logs the
device's `tx_cnt`, `last_tx_cnt_sent`, and credit-update threshold. It also logs every `CREDIT_UPDATE` pushed and every
unhandled op (`CREDIT_REQUEST` is unhandled; muxer.rs:1250).

Trace `E4-c16d1m` (16 KiB writes and the 2 s delay, so no packet is dropped): the first 8 lines, the last two `sendmsg`
lines, and the last 3 lines:

```
     0.080ms [main] TX op=1 len=0 flags=0 type=1 src=3:3926211362 dst=2:1024 buf_alloc=262144 fwd_cnt=0 chain_desc_lens=[44]
     0.161ms [main] UNIX id=0xea053b2200000400 connect_vsock ok: endpoint SO_SNDBUF=Ok(8192) SO_RCVBUF=Ok(8192)
     0.191ms [main] RX to guest: RESPONSE port 1024->3926211362 buf_alloc=8388608 fwd_cnt(as left in guest RX buffer)=0 flags=0
     0.367ms [main] TX op=5 len=16384 flags=0 type=1 src=3:3926211362 dst=2:1024 buf_alloc=262144 fwd_cnt=0 chain_desc_lens=[44, 16384]
     0.402ms [main] UNIX id=0xea053b2200000400 sendmsg: hdr len 16384 buf() len 16384 pending before 0 tx_cnt before 0
     0.436ms [main] UNIX id=0xea053b2200000400 flush: pending 16384 -> 8192 result Ok(())
     0.458ms [main] UNIX id=0xea053b2200000400 sendmsg done: pending 8192 tx_cnt 16384 last_tx_cnt_sent 0 unacked 16384 credit_update_threshold 4194304
     0.486ms [main] TX op=5 len=16384 flags=0 type=1 src=3:3926211362 dst=2:1024 buf_alloc=262144 fwd_cnt=0 chain_desc_lens=[44, 16384]
...
     1.671ms [main] UNIX id=0xea053b2200000400 sendmsg done: pending 237568 tx_cnt 245760 last_tx_cnt_sent 0 unacked 245760 credit_update_threshold 4194304
     1.764ms [main] UNIX id=0xea053b2200000400 sendmsg done: pending 253952 tx_cnt 262144 last_tx_cnt_sent 0 unacked 262144 credit_update_threshold 4194304
...
    49.697ms [vsock muxer] UNIX id=0xea053b2200000400 flush: pending 16384 -> 8192 result Ok(())
    50.926ms [vsock muxer] UNIX id=0xea053b2200000400 event OUT status Connected pending 8192
    50.966ms [vsock muxer] UNIX id=0xea053b2200000400 flush: pending 8192 -> 0 result Ok(())
```

Observation, without drops:
- 16 packets × 16384 = 262144 bytes, all of them delivered to the host.
- `tx_cnt` reaches 262144 against a threshold of 4194304. Zero `CREDIT_UPDATE`s are sent, zero `CREDIT_REQUEST`s
  arrive, and the guest sends nothing more.
- With `sndtimeo`, the guest's write returns `EAGAIN` at exactly 262144 bytes (E3).

Guest source:
- `virtio_transport_tx_buf_size` returns `min(peer_buf_alloc, buf_alloc)` (virtio_transport_common.c:960-967), and
  `virtio_transport_has_space` (1024-1037) uses it:

  ```
     960	static u32 virtio_transport_tx_buf_size(struct virtio_vsock_sock *vvs)
     961	{
     962		/* The peer advertises its receive buffer via peer_buf_alloc, but we
     963		 * cap it to our local buf_alloc so a remote peer cannot force us to
     964		 * queue more data than our own buffer configuration allows.
     965		 */
     966		return min(vvs->peer_buf_alloc, vvs->buf_alloc);
     967	}
    1032		bytes = (s64)virtio_transport_tx_buf_size(vvs) -
    1033			(vvs->tx_cnt - vvs->peer_fwd_cnt);
  ```

- The local `buf_alloc` defaults to `VSOCK_DEFAULT_BUFFER_SIZE` = `(1024 * 256)` (af_vsock.c:138).
- The 6.12.y log: `c0e42fb0e054` "vsock/virtio: cap TX credit to local buffer size" (2026-01-30, Melbin K Mathew).
- The guest only answers `CREDIT_REQUEST` (virtio_transport_common.c:1602). It never sends one.

So the guest keeps at most 262144 bytes unacknowledged, while `msb_krun`'s `UnixProxy` sends `CREDIT_UPDATE` only after
4194304 bytes, `CONN_TX_BUF_SIZE / 2` (unix.rs:552). The window never reopens, so a stream over 256 KiB deadlocks with
or without drops.

With drops (`E4-s1m`, 64 KiB writes): 4 packets are dropped. Dropped bytes never reach `sendmsg`, so `tx_cnt` stays 0
and no threshold can fire. With `minos-vmm-cf-credit64k` and the stock guest (`E5-credit64k-stock-1m`), the guest still
hangs, with 0 `CREDIT_UPDATE`s. This is DEFECT-01's mechanism, and it applies to writes over 32 KiB.

Counterfactuals (E5, E6):

```
## E5-drain-16384: vmm=minos-vmm-cf-drain guest=minos-guest DIAG_MODE=supervisor DIAG_BYTES=16384 
DIAG mode=supervisor bytes=16384 exit=Some(0) killed=false elapsed=315.647167ms connections=2
  conn0 len 16384 overflowed false pattern true sha 4348e3b98e8a327b34ced39c1da9e67cdb4cd5e48e4d7960607a3ae403d35f0c
  conn1 frame (140 bytes): guest requested 16384 written 16384 error null raw_os_error null
  console: MINOS-GUEST-FRAME-SENT 140 bytes
  minos-vmm: exit 0
harness exit 0
## E5-drain-32768: vmm=minos-vmm-cf-drain guest=minos-guest DIAG_MODE=supervisor DIAG_BYTES=32768 
DIAG mode=supervisor bytes=32768 exit=Some(0) killed=false elapsed=316.93075ms connections=2
  conn0 len 32768 overflowed false pattern true sha 09fed9cbfb98b6ab0f3e8ff63b7b1f9b0e07d58b225295c78fdc023cc4985a72
  conn1 frame (140 bytes): guest requested 32768 written 32768 error null raw_os_error null
  console: MINOS-GUEST-FRAME-SENT 140 bytes
  minos-vmm: exit 0
harness exit 0
## E5-drain-32769: vmm=minos-vmm-cf-drain guest=minos-guest DIAG_MODE=supervisor DIAG_BYTES=32769 
DIAG mode=supervisor bytes=32769 exit=Some(0) killed=false elapsed=311.043875ms connections=2
  conn0 len 0 overflowed false pattern true sha e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
  conn1 frame (140 bytes): guest requested 32769 written 32769 error null raw_os_error null
  console: MINOS-GUEST-FRAME-SENT 140 bytes
  minos-vmm: exit 0
harness exit 0
## E5-credit64k-c16d-1m: vmm=minos-vmm-cf-credit64k guest=minos-guest-chunk16k+delay DIAG_MODE=supervisor DIAG_BYTES=1048576 
DIAG mode=supervisor bytes=1048576 exit=Some(0) killed=false elapsed=2.325851542s connections=2
  conn0 len 1048576 overflowed false pattern true sha 631b84027d6b9e52b539c4e8373622d23032dfadc64d60af87339c9037e4f769
  conn1 frame (144 bytes): guest requested 1048576 written 1048576 error null raw_os_error null
  console: minos-guest: DIAG_GUEST [chunk16k,delay] chunk 16384
  console: minos-guest: DIAG vsock_stream wrote 1048576 before shutdown
  console: MINOS-GUEST-FRAME-SENT 144 bytes
  minos-vmm: exit 0
harness exit 0
## E5-credit64k-stock-1m: vmm=minos-vmm-cf-credit64k guest=minos-guest DIAG_MODE=supervisor DIAG_BYTES=1048576 
DIAG mode=supervisor bytes=1048576 exit=Some(124) killed=false elapsed=15.222175584s connections=1
  conn0 len 0 overflowed false pattern true sha e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
  minos-vmm: watchdog deadline
  minos-vmm: exit 124
harness exit 0
## E5-drain+credit64k-c16-1m: vmm=minos-vmm-cf-drain+credit64k guest=minos-guest-chunk16k DIAG_MODE=supervisor DIAG_BYTES=1048576 
DIAG mode=supervisor bytes=1048576 exit=Some(0) killed=false elapsed=489.150417ms connections=2
  conn0 len 1048576 overflowed false pattern true sha 631b84027d6b9e52b539c4e8373622d23032dfadc64d60af87339c9037e4f769
  conn1 frame (144 bytes): guest requested 1048576 written 1048576 error null raw_os_error null
  console: minos-guest: DIAG_GUEST [chunk16k] chunk 16384
  console: minos-guest: DIAG vsock_stream wrote 1048576 before shutdown
  console: MINOS-GUEST-FRAME-SENT 144 bytes
  minos-vmm: exit 0
harness exit 0
## E5-drain+credit64k-c16-4m: vmm=minos-vmm-cf-drain+credit64k guest=minos-guest-chunk16k DIAG_MODE=supervisor DIAG_BYTES=4194304 
DIAG mode=supervisor bytes=4194304 exit=Some(0) killed=false elapsed=1.018254166s connections=2
  conn0 len 4194304 overflowed false pattern true sha a117210941a0b00dcb2d8577e680d84b6fa0eaf760d2afc654c953b9859d54fa
  conn1 frame (144 bytes): guest requested 4194304 written 4194304 error null raw_os_error null
  console: minos-guest: DIAG_GUEST [chunk16k] chunk 16384
  console: minos-guest: DIAG vsock_stream wrote 4194304 before shutdown
  console: MINOS-GUEST-FRAME-SENT 144 bytes
  minos-vmm: exit 0
harness exit 0
## E6-ack20-c16d-1m: vmm=minos-vmm guest=minos-guest-chunk16k+delay DIAG_MODE=blocking DIAG_BYTES=1048576 DIAG_ACK_MS=20
DIAG mode=blocking rcvbuf_req=0 bytes=1048576 exit=Some(0) killed=false elapsed=2.399976292s connections=2
  conn0 len 1048576 pattern true sha 631b84027d6b9e52b539c4e8373622d23032dfadc64d60af87339c9037e4f769; first_byte Some(307.870375ms) eof 2.389655125s error Some(" (acks sent 88)") SO_RCVBUF Some(8192) SO_SNDBUF Some(8192)
  conn1 frame (144 bytes): guest requested 1048576 written 1048576 error null raw_os_error null; first_byte Some(2.381937375s) eof 2.381947167s error None SO_RCVBUF Some(8192) SO_SNDBUF Some(8192)
  console: minos-guest: DIAG_GUEST [chunk16k,delay] chunk 16384
  console: minos-guest: DIAG vsock_stream wrote 1048576 before shutdown
  console: MINOS-GUEST-FRAME-SENT 144 bytes
  minos-vmm: exit 0
harness exit 0
```

- `minos-vmm-cf-credit64k` with 16 KiB writes and the delay: 1048576 arrive whole, with 16 `CREDIT_UPDATE`s.
- `minos-vmm-cf-drain+credit64k` with 16 KiB writes and no delay: 1048576 and 4194304 arrive whole.
- The stock VMM, with the host writing one byte back every 20 ms (`E6`): 1048576 arrive whole. Every host-to-guest data
  packet carries `fwd_cnt = tx_cnt` (unix.rs:437), and the guest refreshes its credit from any packet it receives.

Verdict: **proven.** Two mechanisms cause the hang:
- (a) The device's credit-update threshold (4 MiB) is above the 256 KiB in-flight cap of the 6.12.109 guest. This hangs
  every stream over 256 KiB.
- (b) Dropped packets are never credited. This hangs writes over 32 KiB even if (a) were fixed.

#### Mechanism of each cliff

| Guest stream | Host receives | Mechanism (all in `msb_krun_devices` 0.1.41) |
|---|---|---|
| ≤ 8192 bytes | all | Fits the proxy socket's 8 KiB `SO_SNDBUF` in one `send`. Nothing is queued when `SHUTDOWN` arrives. |
| 8193 to 32768 bytes (one packet per write) | exactly 8192, then a clean EOF | H1. The rest waits in `pending_write`. The guest's `SHUTDOWN` is applied at once (`SHUT_WR`), and the muxer's `HANG_UP` branch discards the queue without a flush. Timing-dependent in principle; the stock VMM lost 40 of 40 runs. |
| over 32768 bytes per write | 0, then a clean EOF | H2. The 6.12.109 guest spreads such payloads over several 32 KiB-frag descriptors, and `from_tx_virtq_head` drops every such packet silently with used length 0. The guest's writes succeed. |
| over 262144 bytes total | as above, then the guest blocks until the watchdog (124) | H3. The guest caps unacknowledged bytes at min(8 MiB, 256 KiB) = 256 KiB, the device returns credit only every 4 MiB, the guest never asks, and dropped bytes are never credited. |

An undersized status frame is not exposed: the frame (138 to 185 bytes here) always fits in 8192 bytes, which is why
02-06, 02-07, and every frame above arrived intact. A frame over 8 KiB would be cut by H1, and a frame over 32 KiB written
in one call would be dropped by H2.

#### Upstream status (read 2026-10-10, as text; no dependency added, nothing built from it)

- `superradcompany/libkrun` (the `msb_krun` fork): on the default branch `krun`, `unix.rs`, `packet.rs`, `mod.rs`,
  `muxer.rs`, and `device.rs` under `src/devices/src/virtio/vsock/` are byte-identical to the pinned 0.1.41
  (`diff -q`). Its latest vsock commit, `2fe32cd6d3` "fix(vsock): drain host data before handling hangup (#152)"
  (2026-10-02), covers the host-to-guest direction and is already in 0.1.41.
- Open PR `superradcompany/libkrun#138`, "fix(vsock): preserve fragmented stream uploads" (opened 2026-09-15, unmerged
  at its last update on 2026-10-06), reports the same symptoms through microsandbox: 32 KiB works, 65,535 bytes gives
  `BufDescTooSmall`, larger uploads stall. It gathers multi-descriptor TX payloads (H2) and reworks the credit windows
  and updates (H3). None of its `unix.rs` hunks touches `shutdown` or the `HANG_UP` branch, so H1 is not addressed. This
  is from reading the diff; it was not built.
- `containers/libkrun` main:
  - H2 is fixed by `0ecf4d5f79` "devices/vsock: support multi-descriptor TX chains" (2026-05-18).
  - H3(a) is fixed by `4b5b451e6b` "virtio/vsock: use device's peer_buf_alloc" (2026-05-05), whose message cites
    kernel commit `c0e42fb0e0`. The threshold becomes `peer_buf_alloc / 2`.
  - H1 does not exist there: `switch_to_connected` puts the socket in blocking mode, and `sendmsg` calls `send` with no
    pending queue (`unix_proxy/unix.rs`).

H1 is specific to the fork's non-blocking `pending_write` design.

#### Latent observations (not causal in these runs)

- minos-vmm installs no `log` backend, so every `msb_krun` `warn!` and `error!` is discarded, including
  "error reading TX packet". The drops are silent on both sides by construction.
- `UnixProxy::sendmsg` forwards `pkt.buf()` (the whole data descriptor) rather than `pkt.payload()` (the header's `len`).
  `custom_stream.rs` uses `payload()`. The two were equal in every traced packet.
- `sendmsg` returns credit when bytes enter `pending_write`, not when they reach the socket, and the queue has no bound
  of its own. With `credit64k`, 1024000 bytes and 4063232 bytes sat in `pending_write` at `SHUTDOWN` (trace summary).
  Only the guest's credit limits it. A guest that ignores credit was not tested.
- The `RESPONSE` packet leaves `fwd_cnt` and `flags` as they were in the guest's RX buffer (muxer_rxq.rs, `OpResponse`
  arm). They were 0 in every trace.
- Throughput, for K4: each 8 KiB took one `OUT` event about 1.2 ms apart (`E4-c16d1m`: 262144 bytes drained by about
  51 ms), and `E5-drain+credit64k-c16-4m` moved 4 MiB in 1.02 s including boot. That is the 8 KiB socket buffer paced by
  the supervisor's 1 ms read poll.

#### Not verified

- Under guest memory pressure, an order-3 allocation can fail and `alloc_skb_with_frags` falls back to smaller frags.
  A 32405 to 32768-byte payload could then also span two descriptors and be dropped. Inferred from the source, not
  exercised.
- The H1 race window is characterized only by the counts above.
- The upstream fixes and PR #138 were read, not built or run.
- Whether a guest that raises `SO_VM_SOCKETS_BUFFER_SIZE` above 4 MiB avoids H3(a) was not tested: the guest's
  dependencies have no safe `setsockopt` for `AF_VSOCK`.
- The `sendspace` sysctl was not changed (it needs sudo). Only the host-only test shows the writer-side buffer's
  effect.

#### Scratch sources

<details><summary>Instrumentation diff of msb_krun_devices 0.1.41 (scratch only; trace sink path shown as $SCR)</summary>

```diff
diff -ru a/src/virtio/vsock/device.rs b/src/virtio/vsock/device.rs
--- a/src/virtio/vsock/device.rs
+++ b/src/virtio/vsock/device.rs
@@ -173,6 +173,7 @@
             let pkt = match VsockPacket::from_tx_virtq_head(&head) {
                 Ok(pkt) => pkt,
                 Err(e) => {
+                    super::diag(format_args!("TX DROPPED {e:?}: descriptor marked used (len 0), guest not told"));
                     error!("error reading TX packet: {e:?}");
                     have_used = true;
                     if let Err(e) = queue_tx.add_used(mem, head.index, 0) {
diff -ru a/src/virtio/vsock/mod.rs b/src/virtio/vsock/mod.rs
--- a/src/virtio/vsock/mod.rs
+++ b/src/virtio/vsock/mod.rs
@@ -211,3 +211,29 @@
 }
 
 type Result<T> = std::result::Result<T, VsockError>;
+
+/// DIAG-01 scratch instrumentation: append one timestamped line to a fixed trace file.
+pub(crate) fn diag(args: std::fmt::Arguments<'_>) {
+    use std::io::Write;
+    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
+    let start = START.get_or_init(std::time::Instant::now);
+    if let Ok(mut f) = std::fs::OpenOptions::new()
+        .create(true)
+        .append(true)
+        .open(DIAG_PATH)
+    {
+        let thread = std::thread::current();
+        let line = format!(
+            "{:>10.3}ms [{}] {}\n",
+            start.elapsed().as_secs_f64() * 1000.0,
+            thread.name().unwrap_or("?"),
+            args
+        );
+        let _ = f.write_all(line.as_bytes());
+    }
+}
+pub(crate) const DIAG_PATH: &str = "$SCR/trace/vsock.log";
+/// DIAG-01 counterfactual toggles, off unless the crate is built with DIAG_CF set.
+pub(crate) fn diag_cf(flag: &str) -> bool {
+    option_env!("DIAG_CF").is_some_and(|v| v.split(',').any(|f| f == flag))
+}
diff -ru a/src/virtio/vsock/muxer.rs b/src/virtio/vsock/muxer.rs
--- a/src/virtio/vsock/muxer.rs
+++ b/src/virtio/vsock/muxer.rs
@@ -1247,7 +1247,10 @@
             uapi::VSOCK_OP_CREDIT_UPDATE => self.process_op_credit_update(pkt),
             uapi::VSOCK_OP_RW => self.process_stream_rw(pkt),
             uapi::VSOCK_OP_RST => self.process_stream_rst(pkt),
-            _ => warn!("stream: unhandled op={}", pkt.op()),
+            _ => {
+                super::diag(format_args!("MUXER stream: unhandled op={} (ignored)", pkt.op()));
+                warn!("stream: unhandled op={}", pkt.op())
+            }
         }
         Ok(())
     }
diff -ru a/src/virtio/vsock/muxer_rxq.rs b/src/virtio/vsock/muxer_rxq.rs
--- a/src/virtio/vsock/muxer_rxq.rs
+++ b/src/virtio/vsock/muxer_rxq.rs
@@ -104,6 +104,7 @@
             local_port,
             peer_port,
         } => {
+            super::diag(format_args!("RX to guest: RST port {}->{}", local_port, peer_port));
             pkt.set_op(uapi::VSOCK_OP_RST)
                 .set_src_cid(uapi::VSOCK_HOST_CID)
                 .set_dst_cid(cid)
@@ -158,6 +159,10 @@
                 .set_buf_alloc(defs::CONN_TX_BUF_SIZE as u32);
 
             pkt.set_len(0);
+            super::diag(format_args!(
+                "RX to guest: RESPONSE port {}->{} buf_alloc={} fwd_cnt(as left in guest RX buffer)={} flags={}",
+                local_port, peer_port, pkt.buf_alloc(), pkt.fwd_cnt(), pkt.flags()
+            ));
         }
         #[cfg(unix)]
         MuxerRx::GetnameResponse {
@@ -194,6 +199,7 @@
             peer_port,
             fwd_cnt,
         } => {
+            super::diag(format_args!("RX to guest: CREDIT_UPDATE port {}->{} fwd_cnt={}", local_port, peer_port, fwd_cnt));
             pkt.set_op(uapi::VSOCK_OP_CREDIT_UPDATE)
                 .set_src_cid(uapi::VSOCK_HOST_CID)
                 .set_dst_cid(cid)
diff -ru a/src/virtio/vsock/packet.rs b/src/virtio/vsock/packet.rs
--- a/src/virtio/vsock/packet.rs
+++ b/src/virtio/vsock/packet.rs
@@ -240,6 +240,19 @@
             buf_size: 0,
         };
 
+        {
+            let mut chain = vec![head.len];
+            let mut next = head.next_descriptor();
+            while let Some(d) = next {
+                chain.push(d.len);
+                next = d.next_descriptor();
+            }
+            super::diag(format_args!(
+                "TX op={} len={} flags={} type={} src={}:{} dst={}:{} buf_alloc={} fwd_cnt={} chain_desc_lens={:?}",
+                pkt.op(), pkt.len(), pkt.flags(), pkt.type_(), pkt.src_cid(), pkt.src_port(),
+                pkt.dst_cid(), pkt.dst_port(), pkt.buf_alloc(), pkt.fwd_cnt(), chain
+            ));
+        }
         // No point looking for a data/buffer descriptor, if the packet is zero-lengthed.
         if pkt.len() == 0 {
             return Ok(pkt);
@@ -248,6 +261,7 @@
         // Reject weirdly-sized packets.
         //
         if pkt.len() > defs::MAX_PKT_BUF_SIZE as u32 {
+            super::diag(format_args!("TX REJECT InvalidPktLen({})", pkt.len()));
             return Err(VsockError::InvalidPktLen(pkt.len()));
         }
 
@@ -262,6 +276,10 @@
         // The data buffer should be large enough to fit the size of the data, as described by
         // the header descriptor.
         if buf_desc.len < pkt.len() {
+            super::diag(format_args!(
+                "TX REJECT BufDescTooSmall: first data descriptor {} bytes < header len {}",
+                buf_desc.len, pkt.len()
+            ));
             return Err(VsockError::BufDescTooSmall);
         }
 
diff -ru a/src/virtio/vsock/unix.rs b/src/virtio/vsock/unix.rs
--- a/src/virtio/vsock/unix.rs
+++ b/src/virtio/vsock/unix.rs
@@ -220,7 +220,16 @@
         let mut update = ProxyUpdate::default();
 
         match connect(self.endpoint.as_raw_fd(), &addr) {
-            Ok(()) => self.switch_to_connected(),
+            Ok(()) => {
+                let StreamEndpoint::Unix(fd) = &self.endpoint;
+                super::diag(format_args!(
+                    "UNIX id={:#x} connect_vsock ok: endpoint SO_SNDBUF={:?} SO_RCVBUF={:?}",
+                    self.id,
+                    getsockopt(fd, sockopt::SndBuf),
+                    getsockopt(fd, sockopt::RcvBuf)
+                ));
+                self.switch_to_connected()
+            }
             Err(Errno::EINPROGRESS) => self.status = ProxyStatus::Connecting,
             Err(err) => {
                 #[cfg(target_os = "macos")]
@@ -280,6 +289,7 @@
     }
 
     fn push_reset(&self) {
+        super::diag(format_args!("UNIX id={:#x} push_reset to guest", self.id));
         debug!(
             "push_reset: id: {}, peer_port: {}, local_port: {}",
             self.id, self.peer_port, self.local_port
@@ -392,6 +402,18 @@
     }
 
     fn flush_pending_write(&mut self) -> io::Result<()> {
+        let before = self.pending_write.len();
+        let r = self.flush_pending_write_inner();
+        if before != 0 || r.is_err() {
+            super::diag(format_args!(
+                "UNIX id={:#x} flush: pending {} -> {} result {:?}",
+                self.id, before, self.pending_write.len(), r
+            ));
+        }
+        r
+    }
+
+    fn flush_pending_write_inner(&mut self) -> io::Result<()> {
         while !self.pending_write.is_empty() {
             let (front, back) = self.pending_write.as_slices();
             let buf = if front.is_empty() { back } else { front };
@@ -527,6 +549,10 @@
         let mut update = ProxyUpdate::default();
 
         let ret = if let Some(buf) = pkt.buf() {
+            super::diag(format_args!(
+                "UNIX id={:#x} sendmsg: hdr len {} buf() len {} pending before {} tx_cnt before {}",
+                self.id, pkt.len(), buf.len(), self.pending_write.len(), self.tx_cnt
+            ));
             // Credit is returned when bytes enter this bounded proxy queue;
             // readiness drives later partial flushes without blocking a VMM thread.
             self.pending_write.extend(buf);
@@ -548,9 +574,16 @@
             -libc::EINVAL
         };
 
+        let threshold = if super::diag_cf("credit64k") { 64 * 1024 } else { defs::CONN_TX_BUF_SIZE / 2 };
+        super::diag(format_args!(
+            "UNIX id={:#x} sendmsg done: pending {} tx_cnt {} last_tx_cnt_sent {} unacked {} credit_update_threshold {}",
+            self.id, self.pending_write.len(), self.tx_cnt, self.last_tx_cnt_sent,
+            (self.tx_cnt - self.last_tx_cnt_sent).0, threshold
+        ));
         if ret > 0
-            && (self.tx_cnt - self.last_tx_cnt_sent).0 as usize >= (defs::CONN_TX_BUF_SIZE / 2)
+            && (self.tx_cnt - self.last_tx_cnt_sent).0 as usize >= threshold
         {
+            super::diag(format_args!("UNIX id={:#x} pushing CREDIT_UPDATE fwd_cnt={}", self.id, self.tx_cnt));
             debug!(
                 "sending credit update: id={}, tx_cnt={}, last_tx_cnt={}",
                 self.id, self.tx_cnt, self.last_tx_cnt_sent
@@ -665,12 +698,36 @@
             Shutdown::Write
         };
 
+        super::diag(format_args!(
+            "UNIX id={:#x} SHUTDOWN flags={} how={:?}: pending_write {} bytes at shutdown, tx_cnt {}",
+            self.id, pkt.flags(), how, self.pending_write.len(), self.tx_cnt
+        ));
+        if super::diag_cf("drain") && how != Shutdown::Read {
+            let mut tries = 0_u32;
+            while !self.pending_write.is_empty() && tries < 5000 {
+                if self.flush_pending_write().is_err() {
+                    break;
+                }
+                if !self.pending_write.is_empty() {
+                    std::thread::sleep(std::time::Duration::from_millis(1));
+                }
+                tries += 1;
+            }
+            super::diag(format_args!(
+                "UNIX id={:#x} CF drain before shutdown: pending now {} after {} tries",
+                self.id, self.pending_write.len(), tries
+            ));
+        }
         if let Err(e) = self.endpoint.shutdown(how) {
             warn!("error sending shutdown to socket: {e}");
         }
     }
 
     fn release(&mut self) -> ProxyUpdate {
+        super::diag(format_args!(
+            "UNIX id={:#x} RELEASE (guest RST): pending_write {} tx_cnt {}",
+            self.id, self.pending_write.len(), self.tx_cnt
+        ));
         debug!(
             "release: id={}, tx_cnt={}, last_tx_cnt={}",
             self.id, self.tx_cnt, self.last_tx_cnt_sent
@@ -685,6 +742,10 @@
 
     fn process_event(&mut self, evset: EventSet) -> ProxyUpdate {
         let mut update = ProxyUpdate::default();
+        super::diag(format_args!(
+            "UNIX id={:#x} event {:?} status {:?} pending {}",
+            self.id, evset, self.status, self.pending_write.len()
+        ));
 
         if evset.contains(EventSet::HANG_UP) {
             debug!("process_event: HANG_UP");
```

</details>

<details><summary>Guest variants diff of crates/minos-guest/src/linux.rs (scratch only)</summary>

```diff
--- a/crates/minos-guest/src/linux.rs
+++ b/crates/minos-guest/src/linux.rs
@@ -274,11 +274,20 @@
             };
         }
     };
-    let mut buf = vec![0_u8; CHUNK];
+    // DIAG-01 scratch variants (compile-time DIAG_GUEST, comma list): chunk16k, sndtimeo, delay.
+    let diag = option_env!("DIAG_GUEST").unwrap_or("");
+    let has = |flag: &str| diag.split(',').any(|f| f == flag);
+    let diag_chunk: usize = if has("chunk16k") { 16 * 1024 } else { CHUNK };
+    if has("sndtimeo") {
+        drop(stream.set_write_timeout(Some(Duration::from_secs(3))));
+    }
+    console_line(&format!("minos-guest: DIAG_GUEST [{diag}] chunk {diag_chunk}"));
+    let mut buf = vec![0_u8; diag_chunk];
     let mut written = 0_u64;
     let mut failure = None;
     while written < requested {
-        let Some(chunk) = buf.get_mut(..chunk_len(requested.saturating_sub(written))) else {
+        let want = usize::try_from(requested.saturating_sub(written)).map_or(diag_chunk, |r| r.min(diag_chunk));
+        let Some(chunk) = buf.get_mut(..want) else {
             break;
         };
         fill_pattern(written, chunk);
@@ -289,6 +298,10 @@
             break;
         }
     }
+    if has("delay") {
+        std::thread::sleep(Duration::from_secs(2));
+    }
+    console_line(&format!("minos-guest: DIAG vsock_stream wrote {written} before shutdown"));
     // The host may already have closed its end after its cap; a shutdown error adds nothing to the report.
     drop(stream.shutdown(Shutdown::Write));
     drop(stream);
```

</details>

<details><summary>Harness crates/minos-backend-krun/tests/diag_vsock.rs (scratch only)</summary>

```rust
//! DIAG-01 scratch harness (scratch workspace only; never committed).
//!
//! `diag_run` launches one VM with the 02-08 stream job and collects the guest's connections in one of two ways:
//! - `DIAG_MODE=supervisor`: through `supervise::run`, exactly like 02-08's `vsock_stream_probe`.
//! - `DIAG_MODE=blocking`: an independent listener. Each accepted connection is switched to blocking I/O and
//!   read with plain `read` calls to EOF in its own thread from the moment it is accepted; minos-vmm is spawned
//!   directly with `Command`, without `supervise::run`. `DIAG_RCVBUF=<n>` sets `SO_RCVBUF` on every accepted
//!   connection first.
//!
//! `host_unix_capacity` needs no VM: it measures how many bytes a non-blocking writer can put into a macOS Unix
//! stream socket whose peer never reads, with default buffers and with each side's buffer raised.

use std::error::Error;
use std::fs::{self, File};
use std::io::{ErrorKind, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use minos_backend_krun::input_disk::input_disk_image;
use minos_backend_krun::launch_config::{
    Boot, Console, Disks, Kernel, LaunchConfig, Machine, Vsock,
};
use minos_backend_krun::rootfs::minimal_root_image;
use minos_backend_krun::supervise::{self, JobFiles, Launch, MAX_FRAME_BYTES};
use nix::sys::socket::{getsockopt, setsockopt, sockopt};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const VSOCK_PORT: u32 = 1024;
const SCRATCH_CAP_BYTES: u64 = 64 * 1024 * 1024;
const CONSOLE_CAP_BYTES: u64 = 1024 * 1024;
const MACHINE: Machine = Machine::new(1, 256);
const STREAM_CAP: u64 = 134_217_728;
const FRAME_CAP: u64 = 1_048_576;

fn env_path(name: &str) -> TestResult<PathBuf> {
    Ok(PathBuf::from(
        std::env::var_os(name).ok_or_else(|| format!("{name} is not set"))?,
    ))
}

fn env_u64(name: &str, default: u64) -> TestResult<u64> {
    match std::env::var(name) {
        Ok(text) => Ok(text.parse()?),
        Err(_) => Ok(default),
    }
}

fn stream_job(bytes: u64) -> String {
    json!({
        "v": 1,
        "vsock_port": VSOCK_PORT,
        "steps": [{"vsock_stream": {"bytes": bytes}}],
        "finish": "poweroff",
    })
    .to_string()
}

fn pattern_ok(bytes: &[u8]) -> bool {
    (0_u64..)
        .zip(bytes)
        .all(|(index, byte)| u64::from(*byte) == index % 251)
}

fn sha_hex(bytes: &[u8]) -> String {
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    base16ct::lower::encode_string(&digest)
}

/// Keeps printable ASCII, newline, and tab only.
fn printable(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_ascii_graphic() || *c == ' ' || *c == '\n' || *c == '\t')
        .collect()
}

/// The MINOSST1 frame payload as JSON, or `None`.
fn decode_frame(bytes: &[u8]) -> Option<Value> {
    let (magic, rest) = bytes.split_first_chunk::<8>()?;
    let (_version, rest) = rest.split_first_chunk::<4>()?;
    let (length, payload) = rest.split_first_chunk::<8>()?;
    if magic != b"MINOSST1" || u64::from_be_bytes(*length) != u64::try_from(payload.len()).ok()? {
        return None;
    }
    serde_json::from_slice(payload).ok()
}

fn stream_line(frame: &Value) -> String {
    frame
        .get("steps")
        .and_then(Value::as_array)
        .and_then(|steps| steps.iter().find_map(|step| step.get("vsock_stream")))
        .map_or_else(
            || "no vsock_stream result".to_owned(),
            |result| {
                printable(&format!(
                    "guest requested {} written {} error {} raw_os_error {}",
                    result.get("requested").unwrap_or(&Value::Null),
                    result.get("written").unwrap_or(&Value::Null),
                    result.get("error").unwrap_or(&Value::Null),
                    result.get("raw_os_error").unwrap_or(&Value::Null)
                ))
            },
        )
}

fn marker_lines(console: &[u8]) -> String {
    String::from_utf8_lossy(console)
        .lines()
        .filter(|line| line.contains("MINOS-") || line.contains("minos-guest:"))
        .filter(|line| !line.contains("cmdline") && !line.contains(" env "))
        .map(|line| format!("  console: {}\n", printable(line)))
        .collect()
}

struct Prepared {
    _dir: tempfile::TempDir,
    files: JobFiles,
}

fn prepare(bytes: u64) -> TestResult<Prepared> {
    let guest = fs::read(env_path("MINOS_GUEST")?)?;
    let image = minimal_root_image(&guest)?;
    let dir = supervise::short_job_dir()?;
    let files = JobFiles::in_dir(dir.path())?;
    let input = input_disk_image(stream_job(bytes).as_bytes())?;
    supervise::write_disks(&files, image.bytes(), &input, SCRATCH_CAP_BYTES)?;
    let krunfw = fs::canonicalize(env_path("MINOS_KRUNFW")?)?;
    let krunfw_sha = supervise::sha256_file(&krunfw)?;
    let config = LaunchConfig::new(
        Kernel::new(krunfw, krunfw_sha),
        Boot::RootDisk,
        Disks::new(
            files.root(),
            files.input(),
            files.scratch(),
            SCRATCH_CAP_BYTES,
        ),
        Vsock::new(VSOCK_PORT, files.socket()),
        Console::new(files.console(), CONSOLE_CAP_BYTES),
        env_u64("DIAG_MAX_MS", 15_000)?,
        MACHINE,
    );
    supervise::write_config(&files, &config)?;
    Ok(Prepared { _dir: dir, files })
}

struct Conn {
    index: usize,
    bytes: Vec<u8>,
    first_byte: Option<Duration>,
    eof: Duration,
    error: Option<String>,
    rcvbuf: Option<usize>,
    sndbuf: Option<usize>,
}

fn read_blocking(index: usize, mut stream: UnixStream, start: Instant, rcvbuf: u64) -> Conn {
    let mut error = None;
    if let Err(e) = stream.set_nonblocking(false) {
        error = Some(format!("set_nonblocking: {e}"));
    }
    if rcvbuf > 0
        && let Ok(size) = usize::try_from(rcvbuf)
        && let Err(e) = setsockopt(&stream, sockopt::RcvBuf, &size)
    {
        error = Some(format!("setsockopt SO_RCVBUF: {e}"));
    }
    // DIAG_ACK_MS: write one byte back on the first connection every N ms (host-to-guest packets carry the
    // device's fwd_cnt), to test whether host traffic reopens the guest's credit window.
    let ack_ms = env_u64("DIAG_ACK_MS", 0).unwrap_or(0);
    let mut acks_sent = 0_u64;
    let acker = if index == 0 && ack_ms > 0 {
        stream.try_clone().ok().map(|mut writer| {
            thread::spawn(move || -> u64 {
                let mut sent = 0_u64;
                loop {
                    thread::sleep(Duration::from_millis(ack_ms));
                    if writer.write_all(b"a").is_err() {
                        return sent;
                    }
                    sent = sent.saturating_add(1);
                }
            })
        })
    } else {
        None
    };
    let rcv = getsockopt(&stream, sockopt::RcvBuf).ok();
    let snd = getsockopt(&stream, sockopt::SndBuf).ok();
    let mut bytes = Vec::new();
    let mut first_byte = None;
    let mut chunk = vec![0_u8; 65_536];
    let cap = usize::try_from(STREAM_CAP).unwrap_or(usize::MAX);
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => {
                if first_byte.is_none() {
                    first_byte = Some(start.elapsed());
                }
                bytes.extend_from_slice(chunk.get(..read).unwrap_or_default());
                if bytes.len() > cap {
                    break;
                }
            }
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) => {
                error = Some(format!("read: {e}"));
                break;
            }
        }
    }
    drop(stream.shutdown(std::net::Shutdown::Both));
    if let Some(handle) = acker {
        acks_sent = handle.join().unwrap_or(0);
    }
    if ack_ms > 0 && index == 0 {
        error = Some(format!("{} (acks sent {acks_sent})", error.unwrap_or_default()));
    }
    Conn {
        index,
        bytes,
        first_byte,
        eof: start.elapsed(),
        error,
        rcvbuf: rcv,
        sndbuf: snd,
    }
}

fn run_blocking(vmm: &PathBuf, bytes: u64) -> TestResult<String> {
    let prepared = prepare(bytes)?;
    let files = &prepared.files;
    let rcvbuf = env_u64("DIAG_RCVBUF", 0)?;
    let listener = UnixListener::bind(files.socket())?;
    listener.set_nonblocking(true)?;
    let start = Instant::now();
    let stop = Arc::new(AtomicBool::new(false));
    let (sender, receiver) = mpsc::channel::<Conn>();
    let acceptor = {
        let stop = Arc::clone(&stop);
        thread::spawn(move || -> usize {
            let mut accepted = 0_usize;
            while !stop.load(Ordering::SeqCst) && accepted < 4 {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let sender = sender.clone();
                        let index = accepted;
                        thread::spawn(move || {
                            drop(sender.send(read_blocking(index, stream, start, rcvbuf)));
                        });
                        accepted = accepted.saturating_add(1);
                    }
                    Err(e) if e.kind() == ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(1));
                    }
                    Err(_) => thread::sleep(Duration::from_millis(1)),
                }
            }
            accepted
        })
    };
    let run_dir = env_path("DIAG_RUN_DIR")?;
    fs::create_dir_all(&run_dir)?;
    let stderr = File::create(run_dir.join("vmm.stderr"))?;
    let stdout = File::create(run_dir.join("vmm.stdout"))?;
    let mut child = Command::new(vmm)
        .arg(files.config())
        .env_clear()
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr)
        .process_group(0)
        .spawn()?;
    let deadline = Duration::from_secs(env_u64("DIAG_HOST_DEADLINE_S", 30)?);
    let mut killed = false;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if start.elapsed() >= deadline {
            child.kill()?;
            killed = true;
            break child.wait()?;
        }
        thread::sleep(Duration::from_millis(10));
    };
    let elapsed = start.elapsed();
    thread::sleep(Duration::from_millis(500));
    stop.store(true, Ordering::SeqCst);
    let accepted = acceptor.join().map_err(|_| "acceptor panicked")?;
    let mut conns = Vec::new();
    for _ in 0..accepted {
        conns.push(receiver.recv_timeout(Duration::from_secs(5))?);
    }
    conns.sort_by_key(|conn| conn.index);
    let mut out = format!(
        "DIAG mode=blocking rcvbuf_req={rcvbuf} bytes={bytes} exit={:?} killed={killed} elapsed={elapsed:?} connections={accepted}\n",
        status.code()
    );
    for conn in &conns {
        let frame = decode_frame(&conn.bytes);
        let what = frame.as_ref().map_or_else(
            || {
                format!(
                    "len {} pattern {} sha {}",
                    conn.bytes.len(),
                    pattern_ok(&conn.bytes),
                    sha_hex(&conn.bytes)
                )
            },
            |frame| format!("frame ({} bytes): {}", conn.bytes.len(), stream_line(frame)),
        );
        out.push_str(&format!(
            "  conn{} {what}; first_byte {:?} eof {:?} error {:?} SO_RCVBUF {:?} SO_SNDBUF {:?}\n",
            conn.index, conn.first_byte, conn.eof, conn.error, conn.rcvbuf, conn.sndbuf
        ));
    }
    let console = fs::read(files.console()).unwrap_or_default();
    out.push_str(&marker_lines(&console));
    let vmm_err = fs::read(run_dir.join("vmm.stderr")).unwrap_or_default();
    for line in String::from_utf8_lossy(&vmm_err).lines() {
        if line.starts_with("minos-vmm") {
            out.push_str(&format!("  {}\n", printable(line)));
        }
    }
    Ok(out)
}

fn run_supervisor(vmm: &PathBuf, bytes: u64) -> TestResult<String> {
    let prepared = prepare(bytes)?;
    let files = &prepared.files;
    let listener = supervise::bind_output(files)?;
    let deadline = Duration::from_secs(env_u64("DIAG_HOST_DEADLINE_S", 30)?);
    let run = supervise::run(
        Launch::new([vmm.as_path(), files.config()], files, listener)
            .host_deadline(deadline)
            .conn_caps(vec![STREAM_CAP, FRAME_CAP]),
    )?;
    let mut out = format!(
        "DIAG mode=supervisor bytes={bytes} exit={:?} killed={} elapsed={:?} connections={}\n",
        run.exit_code(),
        run.killed_at_deadline(),
        run.elapsed(),
        run.connections().len()
    );
    for (index, conn) in run.connections().iter().enumerate() {
        let what = match conn.status_frame(MAX_FRAME_BYTES) {
            Ok(frame) => format!("frame ({} bytes): {}", conn.len(), stream_line(&frame)),
            Err(_) => format!(
                "len {} overflowed {} pattern {} sha {}",
                conn.len(),
                conn.overflowed(),
                conn.matches_pattern(|i| u8::try_from(i % 251).unwrap_or(0)),
                conn.sha256_hex()
            ),
        };
        out.push_str(&format!("  conn{index} {what}\n"));
    }
    for line in run.console().excerpt(usize::MAX).lines() {
        if (line.contains("MINOS-") || line.contains("minos-guest:"))
            && !line.contains("cmdline")
            && !line.contains(" env ")
        {
            out.push_str(&format!("  console: {line}\n"));
        }
    }
    for line in run.vmm_stderr().excerpt(usize::MAX).lines() {
        if line.starts_with("minos-vmm") {
            out.push_str(&format!("  {line}\n"));
        }
    }
    Ok(out)
}

#[test]
#[ignore = "DIAG-01 scratch harness"]
fn diag_run() -> TestResult {
    let vmm = env_path("MINOS_VMM")?;
    let bytes = env_u64("DIAG_BYTES", 16_384)?;
    let mode = std::env::var("DIAG_MODE").unwrap_or_else(|_| "supervisor".to_owned());
    let out = match mode.as_str() {
        "blocking" => run_blocking(&vmm, bytes)?,
        _ => run_supervisor(&vmm, bytes)?,
    };
    print!("{out}");
    Ok(())
}

/// Bytes a non-blocking writer gets into `writer` before `EWOULDBLOCK` when the peer never reads.
fn fill_until_blocked(writer: &mut UnixStream) -> TestResult<usize> {
    writer.set_nonblocking(true)?;
    let chunk = vec![0x5a_u8; 4096];
    let mut total = 0_usize;
    loop {
        match writer.write(&chunk) {
            Ok(written) => total = total.saturating_add(written),
            Err(e) if e.kind() == ErrorKind::WouldBlock => return Ok(total),
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) => return Err(e.into()),
        }
    }
}

#[test]
#[ignore = "DIAG-01 scratch harness"]
fn host_unix_capacity() -> TestResult {
    for (label, sndbuf, rcvbuf) in [
        ("default buffers", 0_usize, 0_usize),
        ("reader SO_RCVBUF 1 MiB", 0, 1_048_576),
        ("writer SO_SNDBUF 1 MiB", 1_048_576, 0),
    ] {
        let dir = tempfile::tempdir_in("/private/tmp")?;
        let path = dir.path().join("c.sock");
        let listener = UnixListener::bind(&path)?;
        let mut writer = UnixStream::connect(&path)?;
        let (reader, _) = listener.accept()?;
        if sndbuf > 0 {
            setsockopt(&writer, sockopt::SndBuf, &sndbuf)?;
        }
        if rcvbuf > 0 {
            setsockopt(&reader, sockopt::RcvBuf, &rcvbuf)?;
        }
        let w_snd: usize = getsockopt(&writer, sockopt::SndBuf)?;
        let r_rcv: usize = getsockopt(&reader, sockopt::RcvBuf)?;
        let total = fill_until_blocked(&mut writer)?;
        println!(
            "HOST {label}: writer SO_SNDBUF {w_snd}, reader SO_RCVBUF {r_rcv}: {total} bytes accepted before EWOULDBLOCK"
        );
        drop(reader);
    }
    Ok(())
}
```

</details>

#### Cleanup

Command: `rm -rf $SCR/target $SCR/bin $SCR/msb_krun_devices-0.1.41 $SCR/ws $SCR/trace $SCR/crate-check $SCR/tools`

Exit code: 0. This removes every instrumented build, the patched crate copy, the scratch workspace, and the rustc
wrapper. Only text outputs remain in the scratchpad.

Command: `cargo clean -p minos-vmm && cargo build -p minos-vmm` (repository workspace, unchanged sources)

Exit code: 0

```
     Removed 2648 files, 260.0MiB total
```

Command: `codesign --force --sign - --options runtime --entitlements crates/minos-vmm/minos-vmm.entitlements target/debug/minos-vmm`,
then `codesign --display --entitlements - target/debug/minos-vmm`

Exit code: 0 and 0

```
target/debug/minos-vmm: replacing existing signature
Executable=~/Repos/minos/target/debug/minos-vmm
[Dict]
	[Key] com.apple.security.cs.disable-library-validation
	[Value]
		[Bool] true
	[Key] com.apple.security.hypervisor
	[Value]
		[Bool] true
```

`strings target/debug/minos-vmm | grep -c -E "trace/vsock.log|TX REJECT|CF drain"` printed `0`: no instrumentation
string is in the rebuilt binary.

A freshness check run after signing, `cargo build -p minos-vmm -v`, reported `Fresh minos-vmm` but still copied the
unsigned binary from `target/debug/deps/` over the signed one. The next egress run failed before launch with the
harness's "lacks com.apple.security.hypervisor" setup error (exit 101). The binary was re-signed with the same argv, and
`codesign --display` again showed both keys. Then:

Command: `MINOS_SPIKE_HOST_IP=<host-lan-ip> cargo test -p minos-backend-krun --test spike_net egress_fails_closed -- --ignored`
(with 02-06 E-1's variables)

Exit code: 0

```

running 1 test
test egress_fails_closed ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.96s

```

Command: `git status --short` (before this commit)

Exit code: 0

```
 M .planning/config.json
 M .planning/phases/02-platform-spikes/evidence/02-08.md
```

### S03-ANSWER: SPIKE-03 answer and the user's decision

Requirement: SPIKE-03 (K4)

Date: 2026-10-10

Setup: none. This section records a decision taken on S03-WITHIN, S03-OVER, K4-THROUGHPUT, DEFECT-01, and DIAG-01.
Nothing was run for it. No file under `crates/` changed. `Cargo.toml`, `Cargo.lock`, `deny.toml`, and
`.cargo/config.toml` are unchanged, and there is no `[patch]` entry.

Command: none

Exit code: not applicable

**Answer.** The vsock route works, with a limit: up to 8192 bytes per connection, guest to host. A guest vsock port
maps to a host Unix socket (`unix_connect`, port 1024), and the host reads each accepted connection through
`supervise::run`'s per-connection cap. Any stream of 8192 bytes or less arrives whole and in order, and its pattern
matches. Above 8192 bytes, the pinned `msb_krun_devices` 0.1.41 loses data silently or stalls the guest, by DIAG-01's
three proven defects:

- H1: a shutdown race that discards bytes queued above the 8 KiB macOS send buffer.
- H2: a parser that takes one descriptor per packet and drops payloads over 32 KiB.
- H3: a credit update sent only every 4 MiB, so any stream over 256 KiB deadlocks.

Minos's supervisor, buffers, guest writer, launch config, and probe were each ruled out by test (DIAG-01 S1 to S5).

**Options considered.**

| Option | What it is | Outcome |
|---|---|---|
| A. Fork and pin | Fork `msb_krun_devices` (or `msb_krun`) and pin the fork by `rev` under `[patch.crates-io]`. Fix H1 by draining `pending_write` before `shutdown(Write)`. Fix H2 by gathering multi-descriptor TX chains, as upstream `0ecf4d5f79` does. Fix H3 by sending credit at `peer_buf_alloc / 2`, as upstream `4b5b451e6b` does. | Rejected for now |
| B. Guest-side workaround on one connection | Write in chunks under 32 KiB (avoids H2). Wait for a host acknowledgement before `shutdown(Write)` (avoids H1). Have the host send a packet at least every 256 KiB to refresh the guest's credit (avoids H3; DIAG-01 E6 showed one byte every 20 ms works). | Not chosen |
| C. Wait for upstream | Fork PR `superradcompany/libkrun#138` covers H2 and H3 but not H1. `containers/libkrun` has fixed H2 and H3 and has no H1. Nobody has reported H1. | Not chosen as the plan: there is no release date, and #138 leaves H1 in place |
| D. Work within the limits | Keep the pinned VMM unchanged. Carry guest-to-host output as a sequence of short connections, each below the 8 KiB limit. | **Chosen** |

User's decision (2026-10-10), in their words: "work within the limits for now, if we need to reassess later fine."

**Decision for Phase 5 (the guest's output writer and the GUEST-04 output reader).**

- Guest-to-host output is a sequence of short vsock connections on the existing `unix_connect` route. Each connection
  carries at most one chunk.
- A chunk's payload is at most 4 KiB. That is half of 8192, because 8192 is the macOS `net.local.stream.sendspace`
  tunable (DIAG-01 S2) and not a constant. The margin also covers the chunk header.
- Each chunk is length-prefixed and sequence-numbered.
- The host reassembles chunks in sequence order under a per-chunk cap and a total cap. It treats a gap, a duplicate, a
  length mismatch, or an over-cap chunk as a candidate fault, never as an infrastructure retry.
- The protocol is one-directional, guest to host. There are no host-to-guest messages, no change to `minos-vmm`, and no
  dependency change.
- Why each defect stays dormant:
  - H1: each connection's total is under the socket's 8 KiB send buffer, so `pending_write` is empty when the guest's
    `SHUTDOWN` arrives.
  - H2: each payload is under 32 KiB, so it fits one descriptor.
  - H3: each connection's total is under 256 KiB, and credit is per connection.
- Unverified until Phase 5 builds it: the cost of one connection per 4 KiB. 64 MiB would take 16384 connections. The
  supervisor's sequential accept and per-connection caps have only ever handled two connections per run here. Phase 5
  measures throughput again for the chunked protocol, and DIAG-01's single-connection figure of about 6 MiB/s does not
  carry over.

**Reasoning.**

- A fork replaces the one dependency trusted to be dangerous, the VMM, with a copy that only Minos reviews. Every
  later upstream fix would then need a manual merge into it. The isolation boundary should stay on code with outside
  reviewers.
- Chunking is better hostile-input hygiene anyway. Every unit the host accepts is small, bounded, and validated before
  the next one is read. A truncated or reordered stream is detected by sequence and length, not inferred from EOF.
  DEFECT-01 showed that EOF can be clean while data is missing.
- Option B needs a host-to-guest channel the supervisor does not have, and it only works for a guest that writes the
  intended way. Option D needs neither.
- No isolation property is affected by any of the three defects. Data is lost guest to host only, and nothing extra
  reaches the host.

**Upstream reporting.** Left to the user. Nothing was filed from this plan.

**Reassessment.** Only if chunking proves to be a real bottleneck in Phase 5. If an `msb_krun` release fixes H1, H2,
and H3, the three vsock tests in `tests/spike_net.rs` can be re-run as written to confirm the fix.

**This plan's tests.** `tests/spike_net.rs` is unchanged from `f964a21`:

- `egress_fails_closed` passes, and Phase 5 promotes it to KRUN-03.
- `vsock_stream_within_cap`, `vsock_stream_over_cap`, and `vsock_throughput_64mib` encode the original single-connection
  contract at 1 MiB, 4 MiB, and 64 MiB. They fail against the pinned VMM for the reasons above.
- The three failing tests stay committed and ignored as the record of the limit. Phase 5 replaces them with tests of
  the chunked protocol.

## Plan 02-09: Seatbelt

Date: 2026-10-10

- `rustc --version`: rustc 1.97.1 (8bab26f4f 2026-07-14)
- `cargo --version`: cargo 1.97.1 (c980f4866 2026-06-30)
- `cargo clippy --version`: clippy 0.1.97 (8bab26f4f6 2026-07-14)
- `cargo deny --version`: cargo-deny 0.20.2
- macOS: 27.0 (build 26A428), arm64
- HEAD at start: `a63968f` (plan base); the runs below are from `3c72528f33dc0328cab3f1c6212296d84655a254` (Task 1:
  the E-1 profile and `tests/spike_seatbelt.rs`) with `crates/minos-vmm/minos-vmm.sb` as printed in each iteration.
  The final profile is the ITER-4 text, committed with this fragment.

Redactions: the absolute home directory path is shown as `~` (every run's output was piped through
`sed "s|$HOME|~|g"`). Nothing else is altered. Cargo progress lines (`Compiling`, `Checking`, `Finished`, `Running`)
are removed.

Console logs: each run of `boot_under_seatbelt` prints the console through `GuestBytes::excerpt`. Every run of
consecutive kernel lines (`[    0.xxxxxx] ...`) is replaced by one line giving their count; every `MINOS-` and
`minos-guest:` line is kept.

Denial logs: Seatbelt reports denials to the unified log as kernel `(Sandbox)` messages. The `/usr/bin/log` binary is
named in full because zsh has a `log` builtin (`(eval):log:1: too many arguments` on the first try). The log shows
local time (UTC-4); `date -u` gave 15:43:12 just before ITER-1. Iterations 2 onward and the controls read the log from
a `--start` time captured with `date '+%Y-%m-%d %H:%M:%S'` immediately before the run, so each log holds only that
run's denials.

Environment for every VM test (02-06 E-1), set on the command line of each run:

- `MINOS_VMM=$(pwd)/target/debug/minos-vmm`
- `MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest` (SHA-256
  `10e59803c53ce60bb53445f95b86e07652a5029c5601170823f1fe156e4b3a89`)
- `MINOS_KRUNFW=$(realpath /opt/homebrew/lib/libkrunfw.5.dylib)` (= `/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib`)

The two profile tests need only `MINOS_KRUNFW` (they fill the `KRUNFW` parameter) and run no VM.

### 09-00: build, sign, and the sandbox-exec binary

Requirement: SPIKE-05 (setup)

Setup: 02-06 E-4 with the two-key entitlement set 02-06 K5 settled on. Nothing in this plan changes the launcher, the
guest, or the supervisor; the profile wraps the helper from outside through `Launch::new`'s argv.

Command: `cargo build -p minos-guest --release --target aarch64-unknown-linux-musl`

Exit code: 0

```
(no output once the cargo progress line is removed)
```

Command: `cargo build -p minos-vmm`

Exit code: 0

```
(no output once the cargo progress line is removed)
```

Command: `codesign --display --entitlements - target/debug/minos-vmm` and `codesign -dvv target/debug/minos-vmm`

Exit code: 0

```
Executable=~/Repos/minos/target/debug/minos-vmm
Identifier=minos_vmm-307854e340cc041e
CodeDirectory v=20400 size=138419 flags=0x20002(adhoc,linker-signed) hashes=4322+0 location=embedded
TeamIdentifier=not set
```

The helper had been relinked since 02-08 signed it (linker-signed, no entitlements; 02-RESEARCH.md Pitfall 2), so it
was signed again:

Command: `codesign --force --sign - --options runtime --entitlements crates/minos-vmm/minos-vmm.entitlements target/debug/minos-vmm`

Exit code: 0

```
target/debug/minos-vmm: replacing existing signature
```

Command: `codesign --display --entitlements - target/debug/minos-vmm` and `codesign -dvv target/debug/minos-vmm | grep flags`

Exit code: 0

```
Executable=~/Repos/minos/target/debug/minos-vmm
[Dict]
	[Key] com.apple.security.cs.disable-library-validation
	[Value]
		[Bool] true
	[Key] com.apple.security.hypervisor
	[Value]
		[Bool] true
CodeDirectory v=20500 size=34963 flags=0x10002(adhoc,runtime) hashes=1081+7 location=embedded
```

`sandbox-exec` has no version flag. Its usage text, its `what` strings (none), its hash, and the deprecation notice
from its man page:

Command: `/usr/bin/sandbox-exec`

Exit code: 64

```
Usage: sandbox-exec [options] command [args]
Options:
  -f profile-file     Read profile from file.
  -n profile-name     Use pre-defined profile.
  -p profile-string   Specify profile on the command line.
  -D key=value        Define a profile parameter.
Exactly one of -f, -n, -p must be specified.
```

Command: `what /usr/bin/sandbox-exec; shasum -a 256 /usr/bin/sandbox-exec; ls -la /usr/bin/sandbox-exec`

Exit code: 0

```
/usr/bin/sandbox-exec:
58839ef01b4eef8aac0d2aa8f9d1c074ae45aafe3533965b030672450064acc8  /usr/bin/sandbox-exec
-rwxr-xr-x  1 root  wheel  135136 Sep  3 06:34 /usr/bin/sandbox-exec
```

Command: `man sandbox-exec | col -b | head -40` (excerpt)

Exit code: 0

```
NAME
     sandbox-exec – execute within a sandbox (DEPRECATED)
...
DESCRIPTION
     The sandbox-exec command is DEPRECATED.  Developers who wish to sandbox
     an app should instead adopt the App Sandbox feature described in the App
     Sandbox Design Guide.  The sandbox-exec command enters a sandbox using a
     profile specified by the -f, -n, or -p option and executes command with
     arguments.
...
Mac OS X			 March 9, 2017		       SANDBOX-EXEC(1)
```

At run time `sandbox-exec` printed no deprecation warning: its stderr is the helper's stderr file, and in every run
below that file holds only `minos-vmm`'s own lines (or nothing, for `nc` and `cat`).

### S05-ITER-1: the E-1 starting profile

Requirement: SPIKE-05

Setup: the profile is 02-09 E-1 verbatim. Parameters are canonical: the job directory comes from `short_job_dir` under `/private/tmp`, `KRUNFW` is the Cellar path, `KRUNFW_LINK` is the Homebrew symlink with only its parent canonicalized.

Profile text run (`crates/minos-vmm/minos-vmm.sb`, whole file):

```scheme
(version 1)
(deny default)
; Seatbelt profile for one minos-vmm helper (SPIKE-05). Every path arrives as a canonical -D parameter.
; Allowances below the starting set are added only from logged denials, each with a comment.
(allow process-exec (literal (param "VMM")))
(allow file-read* (literal "/") (subpath "/usr/lib") (subpath "/System/Library")
                  (subpath "/private/var/db/dyld") (literal "/dev/null") (literal "/dev/urandom"))
(allow file-read* (literal (param "VMM")) (literal (param "CONFIG"))
                  (literal (param "KRUNFW")) (literal (param "KRUNFW_LINK"))
                  (literal (param "ROOT_IMG")) (literal (param "INPUT_IMG")))
(allow file-read* file-write* (literal (param "SCRATCH_IMG")) (literal (param "CONSOLE_LOG")))
; AF_UNIX connect to the vsock route socket is a network-outbound operation (02-RESEARCH.md Pitfall 4).
(allow network-outbound (literal (param "SOCK")))
(allow sysctl-read (sysctl-name "hw.pagesize") (sysctl-name "hw.ncpu") (sysctl-name "hw.memsize")
                   (sysctl-name-prefix "hw.optional.arm.") (sysctl-name-prefix "kern.proc.pid."))
```

Command: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest MINOS_KRUNFW=$(realpath /opt/homebrew/lib/libkrunfw.5.dylib) cargo test -p minos-backend-krun --test spike_seatbelt boot_under_seatbelt -- --ignored --exact --nocapture --test-threads=1`

Exit code: 101

```text

running 1 test
test boot_under_seatbelt ... == boot_under_seatbelt argv:
  /usr/bin/sandbox-exec
  -f
  ~/Repos/minos/crates/minos-vmm/minos-vmm.sb
  -D
  CONFIG=/private/tmp/mnsGlk4Zuzg/vmm.json
  -D
  CONSOLE_LOG=/private/tmp/mnsGlk4Zuzg/console.log
  -D
  INPUT_IMG=/private/tmp/mnsGlk4Zuzg/input.img
  -D
  KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib
  -D
  KRUNFW_LINK=/opt/homebrew/lib/libkrunfw.5.dylib
  -D
  ROOT_IMG=/private/tmp/mnsGlk4Zuzg/root.img
  -D
  SCRATCH_IMG=/private/tmp/mnsGlk4Zuzg/scratch.img
  -D
  SOCK=/private/tmp/mnsGlk4Zuzg/o.sock
  -D
  VMM=~/Repos/minos/target/debug/minos-vmm
  ~/Repos/minos/target/debug/minos-vmm
  /private/tmp/mnsGlk4Zuzg/vmm.json
== boot_under_seatbelt: exit_code None, signal Some(6), killed_at_deadline false, elapsed 187.99625ms, connections 0
-- console (0 bytes, overflowed false):

-- minos-vmm stderr (343 bytes):

thread 'main' (2744065) panicked at /rustc/8bab26f4f68e0e26f0bb7960be334d5b520ea452/library/std/src/sys/pal/unix/stack_overflow.rs:539:13:
failed to allocate a guard page: Invalid argument (os error 22)
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
fatal runtime error: initialization or cleanup bug, aborting

-- minos-vmm stdout (0 bytes):

== end boot_under_seatbelt

thread 'boot_under_seatbelt' (2744060) panicked at crates/minos-backend-krun/tests/spike_seatbelt.rs:314:5:
assertion `left == right` failed
  left: None
 right: Some(0)
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
FAILED

failures:

failures:
    boot_under_seatbelt

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.63s

error: test failed, to rerun pass `-p minos-backend-krun --test spike_seatbelt`
```

Command: `/usr/bin/log show --last 3m --style compact --predicate 'eventMessage CONTAINS "minos-vmm" AND eventMessage CONTAINS "deny"'`

Exit code: 0

```text
Timestamp               Ty Process[PID:TID]
2026-10-10 11:43:13.577 E  kernel[0:29df01] (Sandbox) 1 duplicate report for Sandbox: minos-vmm(84336) deny(1) sysctl-read kern.bootargs
2026-10-10 11:43:13.577 E  kernel[0:29df01] (Sandbox) 1 duplicate report for Sandbox: minos-vmm(84336) deny(1) sysctl-read kern.bootargs
2026-10-10 11:43:13.577 E  kernel[0:29df01] (Sandbox) Sandbox: minos-vmm(84336) deny(1) file-read-metadata /System/Cryptexes/Rosetta
2026-10-10 11:43:13.579 E  kernel[0:29df01] (Sandbox) Sandbox: minos-vmm(84336) deny(1) file-read-data /dev/dtracehelper
2026-10-10 11:43:13.579 E  kernel[0:29df01] (Sandbox) Sandbox: minos-vmm(84336) deny(1) sysctl-read kern.osproductversion
2026-10-10 11:43:13.580 E  kernel[0:29df01] (Sandbox) Sandbox: minos-vmm(84336) deny(1) sysctl-read kern.iossupportversion
2026-10-10 11:43:13.580 E  kernel[0:29df01] (Sandbox) Sandbox: minos-vmm(84336) deny(1) sysctl-read kern.osproductversion
2026-10-10 11:43:13.580 E  kernel[0:29df01] (Sandbox) Sandbox: minos-vmm(84336) deny(1) file-read-metadata /var
2026-10-10 11:43:13.580 E  kernel[0:29df01] (Sandbox) Sandbox: minos-vmm(84336) deny(1) sysctl-read kern.osvariant_status
2026-10-10 11:43:13.580 E  kernel[0:29df01] (Sandbox) Sandbox: minos-vmm(84336) deny(1) sysctl-read hw.ephemeral_storage
2026-10-10 11:43:13.580 E  kernel[0:29df01] (Sandbox) Sandbox: minos-vmm(84336) deny(1) file-read-metadata /var
2026-10-10 11:43:13.580 E  kernel[0:29ddf5] (Sandbox) 2 duplicate reports for Sandbox: minos-vmm(84336) deny(1) file-read-metadata /var
2026-10-10 11:43:13.580 E  kernel[0:29ddf5] (Sandbox) Sandbox: minos-vmm(84336) deny(1) mach-lookup com.apple.system.notification_center
2026-10-10 11:43:13.580 E  kernel[0:29ddf5] (Sandbox) Sandbox: minos-vmm(84336) deny(1) mach-lookup com.apple.system.opendirectoryd.libinfo
2026-10-10 11:43:13.580 E  kernel[0:29df01] (Sandbox) Sandbox: minos-vmm(84336) deny(1) file-read-metadata /etc
2026-10-10 11:43:13.580 E  kernel[0:29ddf5] (Sandbox) 1 duplicate report for Sandbox: minos-vmm(84336) deny(1) file-read-metadata /etc
2026-10-10 11:43:13.580 E  kernel[0:29ddf5] (Sandbox) Sandbox: minos-vmm(84336) deny(1) mach-lookup com.apple.system.opendirectoryd.libinfo
2026-10-10 11:43:13.580 E  kernel[0:29df01] (Sandbox) Sandbox: minos-vmm(84336) deny(1) file-read-metadata /etc
2026-10-10 11:43:13.581 E  kernel[0:29df01] (Sandbox) 1 duplicate report for Sandbox: minos-vmm(84336) deny(1) file-read-metadata /etc
2026-10-10 11:43:13.581 E  kernel[0:29df01] (Sandbox) Sandbox: minos-vmm(84336) deny(1) sysctl-read hw.pagesize_compat
```

Reading: the helper aborted inside Rust std startup, before `main` (signal 6). The only denial that bears on a page
size is `sysctl-read hw.pagesize_compat` (E-1 allows `hw.pagesize` but not this name); an unreadable page size makes
std's guard-page `mprotect` fail with `EINVAL`. The other denials (`kern.bootargs`, the Cryptex and Rosetta probes,
`/dev/dtracehelper`, the OS version sysctls, `/var` and `/etc` metadata, the notification-center and opendirectoryd
lookups) come from dyld and libSystem initialization and did not stop the process, so they stay denied. ITER-2 adds
one rule: `(allow sysctl-read (sysctl-name "hw.pagesize_compat"))`.

### S05-ITER-2: hw.pagesize_compat allowed

Requirement: SPIKE-05

Setup: ITER-1's profile plus the `hw.pagesize_compat` rule.

Profile text run (`crates/minos-vmm/minos-vmm.sb`, whole file):

```scheme
(version 1)
(deny default)
; Seatbelt profile for one minos-vmm helper (SPIKE-05). Every path arrives as a canonical -D parameter.
; Allowances below the starting set are added only from logged denials, each with a comment.
(allow process-exec (literal (param "VMM")))
(allow file-read* (literal "/") (subpath "/usr/lib") (subpath "/System/Library")
                  (subpath "/private/var/db/dyld") (literal "/dev/null") (literal "/dev/urandom"))
(allow file-read* (literal (param "VMM")) (literal (param "CONFIG"))
                  (literal (param "KRUNFW")) (literal (param "KRUNFW_LINK"))
                  (literal (param "ROOT_IMG")) (literal (param "INPUT_IMG")))
(allow file-read* file-write* (literal (param "SCRATCH_IMG")) (literal (param "CONSOLE_LOG")))
; AF_UNIX connect to the vsock route socket is a network-outbound operation (02-RESEARCH.md Pitfall 4).
(allow network-outbound (literal (param "SOCK")))
(allow sysctl-read (sysctl-name "hw.pagesize") (sysctl-name "hw.ncpu") (sysctl-name "hw.memsize")
                   (sysctl-name-prefix "hw.optional.arm.") (sysctl-name-prefix "kern.proc.pid."))
; S05-ITER-1: "deny(1) sysctl-read hw.pagesize_compat"; without it Rust std's startup got an invalid page size and
; aborted with "failed to allocate a guard page: Invalid argument (os error 22)" before main.
(allow sysctl-read (sysctl-name "hw.pagesize_compat"))
```

Command: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest MINOS_KRUNFW=$(realpath /opt/homebrew/lib/libkrunfw.5.dylib) cargo test -p minos-backend-krun --test spike_seatbelt boot_under_seatbelt -- --ignored --exact --nocapture --test-threads=1`

Exit code: 101

```text

running 1 test
test boot_under_seatbelt ... == boot_under_seatbelt argv:
  /usr/bin/sandbox-exec
  -f
  ~/Repos/minos/crates/minos-vmm/minos-vmm.sb
  -D
  CONFIG=/private/tmp/mnsKADfyAzg/vmm.json
  -D
  CONSOLE_LOG=/private/tmp/mnsKADfyAzg/console.log
  -D
  INPUT_IMG=/private/tmp/mnsKADfyAzg/input.img
  -D
  KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib
  -D
  KRUNFW_LINK=/opt/homebrew/lib/libkrunfw.5.dylib
  -D
  ROOT_IMG=/private/tmp/mnsKADfyAzg/root.img
  -D
  SCRATCH_IMG=/private/tmp/mnsKADfyAzg/scratch.img
  -D
  SOCK=/private/tmp/mnsKADfyAzg/o.sock
  -D
  VMM=~/Repos/minos/target/debug/minos-vmm
  ~/Repos/minos/target/debug/minos-vmm
  /private/tmp/mnsKADfyAzg/vmm.json
== boot_under_seatbelt: exit_code Some(66), signal None, killed_at_deadline false, elapsed 12.750375ms, connections 0
-- console (0 bytes, overflowed false):

-- minos-vmm stderr (140 bytes):
minos-vmm: error: preflight: kernel.krunfw /opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib: Operation not permitted (os error 1)

-- minos-vmm stdout (0 bytes):

== end boot_under_seatbelt

thread 'boot_under_seatbelt' (2744722) panicked at crates/minos-backend-krun/tests/spike_seatbelt.rs:314:5:
assertion `left == right` failed
  left: Some(66)
 right: Some(0)
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
FAILED

failures:

failures:
    boot_under_seatbelt

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.42s

error: test failed, to rerun pass `-p minos-backend-krun --test spike_seatbelt`
```

Command: `/usr/bin/log show --start "<the local time just before the run>" --style compact --predicate 'eventMessage CONTAINS "minos-vmm" AND eventMessage CONTAINS "deny"'`

Exit code: 0

```text
Timestamp               Ty Process[PID:TID]
2026-10-10 11:43:43.259 E  kernel[0:29e197] (Sandbox) Sandbox: minos-vmm(84559) deny(1) sysctl-read security.mac.lockdown_mode_state
2026-10-10 11:43:43.259 E  kernel[0:29e197] (Sandbox) Sandbox: minos-vmm(84559) deny(1) sysctl-read kern.bootargs
2026-10-10 11:43:43.259 E  kernel[0:29e197] (Sandbox) Sandbox: minos-vmm(84559) deny(1) file-read-metadata /System/Cryptexes/OS
2026-10-10 11:43:43.259 E  kernel[0:29e197] (Sandbox) Sandbox: minos-vmm(84559) deny(1) sysctl-read kern.bootargs
2026-10-10 11:43:43.259 E  kernel[0:29e197] (Sandbox) 1 duplicate report for Sandbox: minos-vmm(84559) deny(1) sysctl-read kern.bootargs
2026-10-10 11:43:43.259 E  kernel[0:29e197] (Sandbox) Sandbox: minos-vmm(84559) deny(1) file-read-data /System/Volumes/Preboot/Cryptexes/OS/System/Library/dyld
2026-10-10 11:43:43.259 E  kernel[0:29e197] (Sandbox) Sandbox: minos-vmm(84559) deny(1) sysctl-read kern.bootargs
2026-10-10 11:43:43.259 E  kernel[0:29e197] (Sandbox) 1 duplicate report for Sandbox: minos-vmm(84559) deny(1) sysctl-read kern.bootargs
2026-10-10 11:43:43.259 E  kernel[0:29e197] (Sandbox) Sandbox: minos-vmm(84559) deny(1) file-read-metadata /System/Cryptexes/OS
2026-10-10 11:43:43.259 E  kernel[0:29e197] (Sandbox) Sandbox: minos-vmm(84559) deny(1) sysctl-read kern.bootargs
2026-10-10 11:43:43.259 E  kernel[0:29e197] (Sandbox) Sandbox: minos-vmm(84559) deny(1) file-read-metadata /System/Cryptexes/Rosetta
2026-10-10 11:43:43.259 E  kernel[0:29e197] (Sandbox) Sandbox: minos-vmm(84559) deny(1) file-read-data /dev/dtracehelper
2026-10-10 11:43:43.260 E  kernel[0:29e197] (Sandbox) Sandbox: minos-vmm(84559) deny(1) sysctl-read kern.osproductversion
2026-10-10 11:43:43.260 E  kernel[0:29e197] (Sandbox) Sandbox: minos-vmm(84559) deny(1) sysctl-read kern.iossupportversion
2026-10-10 11:43:43.260 E  kernel[0:29e197] (Sandbox) Sandbox: minos-vmm(84559) deny(1) sysctl-read kern.osproductversion
2026-10-10 11:43:43.260 E  kernel[0:29e197] (Sandbox) Sandbox: minos-vmm(84559) deny(1) file-read-metadata /var
2026-10-10 11:43:43.260 E  kernel[0:29e197] (Sandbox) Sandbox: minos-vmm(84559) deny(1) sysctl-read kern.osvariant_status
2026-10-10 11:43:43.260 E  kernel[0:29e197] (Sandbox) Sandbox: minos-vmm(84559) deny(1) sysctl-read hw.ephemeral_storage
2026-10-10 11:43:43.260 E  kernel[0:29e197] (Sandbox) Sandbox: minos-vmm(84559) deny(1) file-read-metadata /var
2026-10-10 11:43:43.260 E  kernel[0:29e143] (Sandbox) 2 duplicate reports for Sandbox: minos-vmm(84559) deny(1) file-read-metadata /var
2026-10-10 11:43:43.260 E  kernel[0:29e143] (Sandbox) Sandbox: minos-vmm(84559) deny(1) mach-lookup com.apple.system.notification_center
2026-10-10 11:43:43.260 E  kernel[0:29df0f] (Sandbox) Sandbox: minos-vmm(84559) deny(1) mach-lookup com.apple.system.opendirectoryd.libinfo
2026-10-10 11:43:43.260 E  kernel[0:29e197] (Sandbox) Sandbox: minos-vmm(84559) deny(1) file-read-metadata /etc
2026-10-10 11:43:43.260 E  kernel[0:29df0f] (Sandbox) 1 duplicate report for Sandbox: minos-vmm(84559) deny(1) file-read-metadata /etc
2026-10-10 11:43:43.260 E  kernel[0:29df0f] (Sandbox) Sandbox: minos-vmm(84559) deny(1) mach-lookup com.apple.system.opendirectoryd.libinfo
2026-10-10 11:43:43.260 E  kernel[0:29e197] (Sandbox) Sandbox: minos-vmm(84559) deny(1) file-read-metadata /etc
2026-10-10 11:43:43.261 E  kernel[0:29e197] (Sandbox) 1 duplicate report for Sandbox: minos-vmm(84559) deny(1) file-read-metadata /etc
2026-10-10 11:43:43.261 E  kernel[0:29e197] (Sandbox) Sandbox: minos-vmm(84559) deny(1) file-read-metadata /opt
```

Reading: std now starts and `main` runs; preflight fails on `kernel.krunfw` with `EPERM`. The denial is
`file-read-metadata /opt`: preflight canonicalizes every config path and checks that the result is unchanged
(`crates/minos-vmm/src/preflight.rs` line 159), and `realpath` stats each ancestor directory. A literal list of
ancestors would hard-code the libkrunfw version and the job directory, so ITER-3 uses Seatbelt's `path-ancestors`
filter (used by `/System/Library/Sandbox/Profiles/system.sb` line 93 for the same purpose) on the parameters whose
ancestors preflight walks: metadata only (stat; no file contents and no directory listing) of the ancestors of
`KRUNFW`, `KRUNFW_LINK`, and `CONFIG` (the job directory, shared by every job file).

### S05-ITER-3: ancestor metadata allowed

Requirement: SPIKE-05

Setup: ITER-2's profile plus the `path-ancestors` metadata rule. This run also shows that `(param ...)` works inside `path-ancestors`.

Profile text run (`crates/minos-vmm/minos-vmm.sb`, whole file):

```scheme
(version 1)
(deny default)
; Seatbelt profile for one minos-vmm helper (SPIKE-05). Every path arrives as a canonical -D parameter.
; Allowances below the starting set are added only from logged denials, each with a comment.
(allow process-exec (literal (param "VMM")))
(allow file-read* (literal "/") (subpath "/usr/lib") (subpath "/System/Library")
                  (subpath "/private/var/db/dyld") (literal "/dev/null") (literal "/dev/urandom"))
(allow file-read* (literal (param "VMM")) (literal (param "CONFIG"))
                  (literal (param "KRUNFW")) (literal (param "KRUNFW_LINK"))
                  (literal (param "ROOT_IMG")) (literal (param "INPUT_IMG")))
(allow file-read* file-write* (literal (param "SCRATCH_IMG")) (literal (param "CONSOLE_LOG")))
; AF_UNIX connect to the vsock route socket is a network-outbound operation (02-RESEARCH.md Pitfall 4).
(allow network-outbound (literal (param "SOCK")))
(allow sysctl-read (sysctl-name "hw.pagesize") (sysctl-name "hw.ncpu") (sysctl-name "hw.memsize")
                   (sysctl-name-prefix "hw.optional.arm.") (sysctl-name-prefix "kern.proc.pid."))
; S05-ITER-1: "deny(1) sysctl-read hw.pagesize_compat"; without it Rust std's startup got an invalid page size and
; aborted with "failed to allocate a guard page: Invalid argument (os error 22)" before main.
(allow sysctl-read (sysctl-name "hw.pagesize_compat"))
; S05-ITER-2: "deny(1) file-read-metadata /opt", then preflight failed with "kernel.krunfw ...: Operation not
; permitted". Preflight canonicalizes every config path, and realpath stats each ancestor directory. This allows
; metadata only (stat, no contents, no listing) of the ancestors of the paths the config names.
(allow file-read-metadata (path-ancestors (param "KRUNFW")) (path-ancestors (param "KRUNFW_LINK"))
                          (path-ancestors (param "CONFIG")))
```

Command: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest MINOS_KRUNFW=$(realpath /opt/homebrew/lib/libkrunfw.5.dylib) cargo test -p minos-backend-krun --test spike_seatbelt boot_under_seatbelt -- --ignored --exact --nocapture --test-threads=1`

Exit code: 101

```text

running 1 test
test boot_under_seatbelt ... == boot_under_seatbelt argv:
  /usr/bin/sandbox-exec
  -f
  ~/Repos/minos/crates/minos-vmm/minos-vmm.sb
  -D
  CONFIG=/private/tmp/mnsC8UzQ94D/vmm.json
  -D
  CONSOLE_LOG=/private/tmp/mnsC8UzQ94D/console.log
  -D
  INPUT_IMG=/private/tmp/mnsC8UzQ94D/input.img
  -D
  KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib
  -D
  KRUNFW_LINK=/opt/homebrew/lib/libkrunfw.5.dylib
  -D
  ROOT_IMG=/private/tmp/mnsC8UzQ94D/root.img
  -D
  SCRATCH_IMG=/private/tmp/mnsC8UzQ94D/scratch.img
  -D
  SOCK=/private/tmp/mnsC8UzQ94D/o.sock
  -D
  VMM=~/Repos/minos/target/debug/minos-vmm
  ~/Repos/minos/target/debug/minos-vmm
  /private/tmp/mnsC8UzQ94D/vmm.json
== boot_under_seatbelt: exit_code Some(66), signal None, killed_at_deadline false, elapsed 10.751833ms, connections 0
-- console (0 bytes, overflowed false):

-- minos-vmm stderr (112 bytes):
minos-vmm: error: preflight: vsock.socket /private/tmp/mnsC8UzQ94D/o.sock: Operation not permitted (os error 1)

-- minos-vmm stdout (0 bytes):

== end boot_under_seatbelt

thread 'boot_under_seatbelt' (2745519) panicked at crates/minos-backend-krun/tests/spike_seatbelt.rs:314:5:
assertion `left == right` failed
  left: Some(66)
 right: Some(0)
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
FAILED

failures:

failures:
    boot_under_seatbelt

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.42s

error: test failed, to rerun pass `-p minos-backend-krun --test spike_seatbelt`
```

Command: `/usr/bin/log show --start "<the local time just before the run>" --style compact --predicate 'eventMessage CONTAINS "minos-vmm" AND eventMessage CONTAINS "deny"'`

Exit code: 0

```text
Timestamp               Ty Process[PID:TID]
2026-10-10 11:44:17.215 E  kernel[0:29e4c1] (Sandbox) 1 duplicate report for Sandbox: minos-vmm(84820) deny(1) sysctl-read kern.bootargs
2026-10-10 11:44:17.215 E  kernel[0:29e4c1] (Sandbox) 1 duplicate report for Sandbox: minos-vmm(84820) deny(1) sysctl-read kern.bootargs
2026-10-10 11:44:17.215 E  kernel[0:29e4c1] (Sandbox) Sandbox: minos-vmm(84820) deny(1) file-read-metadata /System/Cryptexes/Rosetta
2026-10-10 11:44:17.216 E  kernel[0:29e4c1] (Sandbox) Sandbox: minos-vmm(84820) deny(1) file-read-data /dev/dtracehelper
2026-10-10 11:44:17.217 E  kernel[0:29e4c1] (Sandbox) Sandbox: minos-vmm(84820) deny(1) sysctl-read kern.osproductversion
2026-10-10 11:44:17.217 E  kernel[0:29e4c1] (Sandbox) Sandbox: minos-vmm(84820) deny(1) sysctl-read kern.iossupportversion
2026-10-10 11:44:17.217 E  kernel[0:29e4c1] (Sandbox) Sandbox: minos-vmm(84820) deny(1) sysctl-read kern.osproductversion
2026-10-10 11:44:17.217 E  kernel[0:29e4c1] (Sandbox) Sandbox: minos-vmm(84820) deny(1) file-read-metadata /var
2026-10-10 11:44:17.217 E  kernel[0:29e4c1] (Sandbox) Sandbox: minos-vmm(84820) deny(1) sysctl-read kern.osvariant_status
2026-10-10 11:44:17.217 E  kernel[0:29e4c1] (Sandbox) Sandbox: minos-vmm(84820) deny(1) sysctl-read hw.ephemeral_storage
2026-10-10 11:44:17.217 E  kernel[0:29e4c1] (Sandbox) Sandbox: minos-vmm(84820) deny(1) file-read-metadata /var
2026-10-10 11:44:17.217 E  kernel[0:29e278] (Sandbox) 2 duplicate reports for Sandbox: minos-vmm(84820) deny(1) file-read-metadata /var
2026-10-10 11:44:17.217 E  kernel[0:29e278] (Sandbox) Sandbox: minos-vmm(84820) deny(1) mach-lookup com.apple.system.notification_center
2026-10-10 11:44:17.217 E  kernel[0:29e422] (Sandbox) Sandbox: minos-vmm(84820) deny(1) mach-lookup com.apple.system.opendirectoryd.libinfo
2026-10-10 11:44:17.217 E  kernel[0:29e4c1] (Sandbox) Sandbox: minos-vmm(84820) deny(1) file-read-metadata /etc
2026-10-10 11:44:17.217 E  kernel[0:29e422] (Sandbox) 1 duplicate report for Sandbox: minos-vmm(84820) deny(1) file-read-metadata /etc
2026-10-10 11:44:17.217 E  kernel[0:29e422] (Sandbox) Sandbox: minos-vmm(84820) deny(1) mach-lookup com.apple.system.opendirectoryd.libinfo
2026-10-10 11:44:17.217 E  kernel[0:29e4c1] (Sandbox) Sandbox: minos-vmm(84820) deny(1) file-read-metadata /etc
2026-10-10 11:44:17.218 E  kernel[0:29e4c1] (Sandbox) 1 duplicate report for Sandbox: minos-vmm(84820) deny(1) file-read-metadata /etc
2026-10-10 11:44:17.218 E  kernel[0:29e4c1] (Sandbox) Sandbox: minos-vmm(84820) deny(1) file-read-metadata /private/tmp/mnsC8UzQ94D/o.sock
```

Reading: preflight passes the kernel and disk paths and stops at `vsock.socket` with `EPERM`; the one denial is
`file-read-metadata` of the socket path (preflight checks that it is a socket). The E-1 rule for the socket is
`network-outbound` only, which does not cover `lstat`. ITER-4 adds metadata of that one literal path.

### S05-ITER-4: socket metadata allowed (final profile)

Requirement: SPIKE-05

Setup: ITER-3's profile plus `(allow file-read-metadata (literal (param "SOCK")))`. This is the final profile.

Profile text run (`crates/minos-vmm/minos-vmm.sb`, whole file):

```scheme
(version 1)
(deny default)
; Seatbelt profile for one minos-vmm helper (SPIKE-05). Every path arrives as a canonical -D parameter.
; Allowances below the starting set are added only from logged denials, each with a comment.
(allow process-exec (literal (param "VMM")))
(allow file-read* (literal "/") (subpath "/usr/lib") (subpath "/System/Library")
                  (subpath "/private/var/db/dyld") (literal "/dev/null") (literal "/dev/urandom"))
(allow file-read* (literal (param "VMM")) (literal (param "CONFIG"))
                  (literal (param "KRUNFW")) (literal (param "KRUNFW_LINK"))
                  (literal (param "ROOT_IMG")) (literal (param "INPUT_IMG")))
(allow file-read* file-write* (literal (param "SCRATCH_IMG")) (literal (param "CONSOLE_LOG")))
; AF_UNIX connect to the vsock route socket is a network-outbound operation (02-RESEARCH.md Pitfall 4).
(allow network-outbound (literal (param "SOCK")))
(allow sysctl-read (sysctl-name "hw.pagesize") (sysctl-name "hw.ncpu") (sysctl-name "hw.memsize")
                   (sysctl-name-prefix "hw.optional.arm.") (sysctl-name-prefix "kern.proc.pid."))
; S05-ITER-1: "deny(1) sysctl-read hw.pagesize_compat"; without it Rust std's startup got an invalid page size and
; aborted with "failed to allocate a guard page: Invalid argument (os error 22)" before main.
(allow sysctl-read (sysctl-name "hw.pagesize_compat"))
; S05-ITER-2: "deny(1) file-read-metadata /opt", then preflight failed with "kernel.krunfw ...: Operation not
; permitted". Preflight canonicalizes every config path, and realpath stats each ancestor directory. This allows
; metadata only (stat, no contents, no listing) of the ancestors of the paths the config names.
(allow file-read-metadata (path-ancestors (param "KRUNFW")) (path-ancestors (param "KRUNFW_LINK"))
                          (path-ancestors (param "CONFIG")))
; S05-ITER-3: "deny(1) file-read-metadata /private/tmp/<job>/o.sock"; preflight checks that the route path is a
; socket (lstat). Metadata of that one path only; the connect itself is the network-outbound rule above.
(allow file-read-metadata (literal (param "SOCK")))
```

Command: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest MINOS_KRUNFW=$(realpath /opt/homebrew/lib/libkrunfw.5.dylib) cargo test -p minos-backend-krun --test spike_seatbelt boot_under_seatbelt -- --ignored --exact --nocapture --test-threads=1`

Exit code: 0

```text

running 1 test
test boot_under_seatbelt ... == boot_under_seatbelt argv:
  /usr/bin/sandbox-exec
  -f
  ~/Repos/minos/crates/minos-vmm/minos-vmm.sb
  -D
  CONFIG=/private/tmp/mnsS3RO8t0k/vmm.json
  -D
  CONSOLE_LOG=/private/tmp/mnsS3RO8t0k/console.log
  -D
  INPUT_IMG=/private/tmp/mnsS3RO8t0k/input.img
  -D
  KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib
  -D
  KRUNFW_LINK=/opt/homebrew/lib/libkrunfw.5.dylib
  -D
  ROOT_IMG=/private/tmp/mnsS3RO8t0k/root.img
  -D
  SCRATCH_IMG=/private/tmp/mnsS3RO8t0k/scratch.img
  -D
  SOCK=/private/tmp/mnsS3RO8t0k/o.sock
  -D
  VMM=~/Repos/minos/target/debug/minos-vmm
  ~/Repos/minos/target/debug/minos-vmm
  /private/tmp/mnsS3RO8t0k/vmm.json
== boot_under_seatbelt: exit_code Some(0), signal None, killed_at_deadline false, elapsed 293.337542ms, connections 1
-- console (3634 bytes, overflowed false):
[56 kernel console lines omitted by the evidence protocol]
MINOS-GUEST-BOOT-OK
minos-guest: cmdline reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0 root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1791647069733941000  --
minos-guest: osrelease 6.12.109
minos-guest: args /init
minos-guest: env HOME=/ KRUN_BOOT_START_NS=1791647069733941000 TERM=linux
minos-guest: net_ifaces lo
minos-guest: virtio_devices virtio0:0x0004 virtio1:0x0003 virtio2:0x0002 virtio3:0x0002 virtio4:0x0002 virtio5:0x0013 virtio6:0x4d47
MINOS-GUEST-FRAME-SENT 761 bytes
MINOS-GUEST-POWEROFF
[1 kernel console lines omitted by the evidence protocol]

-- minos-vmm stderr (18 bytes):
minos-vmm: exit 0

-- minos-vmm stdout (0 bytes):

-- connection 0 (761 bytes), frame:
{
  "job_error": null,
  "steps": [
    {
      "report_boot": {
        "args": [
          "/init"
        ],
        "cmdline": "reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0 root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1791647069733941000  --",
        "env": [
          [
            "HOME",
            "/"
          ],
          [
            "KRUN_BOOT_START_NS",
            "1791647069733941000"
          ],
          [
            "TERM",
            "linux"
          ]
        ],
        "marker": "MINOS-GUEST-BOOT-OK",
        "net_ifaces": [
          "lo"
        ],
        "osrelease": "6.12.109",
        "virtio_devices": [
          "virtio0:0x0004",
          "virtio1:0x0003",
          "virtio2:0x0002",
          "virtio3:0x0002",
          "virtio4:0x0002",
          "virtio5:0x0013",
          "virtio6:0x4d47"
        ]
      }
    },
    {
      "connect": {
        "results": [
          {
            "error": "Network unreachable (os error 101)",
            "ok": false,
            "raw_os_error": 101,
            "target": "1.1.1.1:443"
          }
        ]
      }
    }
  ],
  "v": 1
}
== end boot_under_seatbelt
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.71s

```

Command: `/usr/bin/log show --start "<the local time just before the run>" --style compact --predicate 'eventMessage CONTAINS "minos-vmm" AND eventMessage CONTAINS "deny"'`

Exit code: 0

```text
Timestamp               Ty Process[PID:TID]
2026-10-10 11:44:29.548 E  kernel[0:29e785] (Sandbox) Sandbox: minos-vmm(84915) deny(1) sysctl-read security.mac.lockdown_mode_state
2026-10-10 11:44:29.548 E  kernel[0:29e785] (Sandbox) Sandbox: minos-vmm(84915) deny(1) sysctl-read kern.bootargs
2026-10-10 11:44:29.548 E  kernel[0:29e785] (Sandbox) Sandbox: minos-vmm(84915) deny(1) file-read-metadata /System/Cryptexes/OS
2026-10-10 11:44:29.548 E  kernel[0:29e785] (Sandbox) Sandbox: minos-vmm(84915) deny(1) sysctl-read kern.bootargs
2026-10-10 11:44:29.548 E  kernel[0:29e785] (Sandbox) 1 duplicate report for Sandbox: minos-vmm(84915) deny(1) sysctl-read kern.bootargs
2026-10-10 11:44:29.548 E  kernel[0:29e785] (Sandbox) Sandbox: minos-vmm(84915) deny(1) file-read-data /System/Volumes/Preboot/Cryptexes/OS/System/Library/dyld
2026-10-10 11:44:29.548 E  kernel[0:29e785] (Sandbox) Sandbox: minos-vmm(84915) deny(1) sysctl-read kern.bootargs
2026-10-10 11:44:29.548 E  kernel[0:29e785] (Sandbox) 1 duplicate report for Sandbox: minos-vmm(84915) deny(1) sysctl-read kern.bootargs
2026-10-10 11:44:29.548 E  kernel[0:29e785] (Sandbox) Sandbox: minos-vmm(84915) deny(1) file-read-metadata /System/Cryptexes/OS
2026-10-10 11:44:29.548 E  kernel[0:29e785] (Sandbox) Sandbox: minos-vmm(84915) deny(1) sysctl-read kern.bootargs
2026-10-10 11:44:29.548 E  kernel[0:29e785] (Sandbox) Sandbox: minos-vmm(84915) deny(1) file-read-metadata /System/Cryptexes/Rosetta
2026-10-10 11:44:29.549 E  kernel[0:29e785] (Sandbox) Sandbox: minos-vmm(84915) deny(1) file-read-data /dev/dtracehelper
2026-10-10 11:44:29.550 E  kernel[0:29e785] (Sandbox) Sandbox: minos-vmm(84915) deny(1) sysctl-read kern.osproductversion
2026-10-10 11:44:29.550 E  kernel[0:29e785] (Sandbox) Sandbox: minos-vmm(84915) deny(1) sysctl-read kern.iossupportversion
2026-10-10 11:44:29.550 E  kernel[0:29e785] (Sandbox) Sandbox: minos-vmm(84915) deny(1) sysctl-read kern.osproductversion
2026-10-10 11:44:29.550 E  kernel[0:29e785] (Sandbox) Sandbox: minos-vmm(84915) deny(1) file-read-metadata /var
2026-10-10 11:44:29.550 E  kernel[0:29e785] (Sandbox) Sandbox: minos-vmm(84915) deny(1) sysctl-read kern.osvariant_status
2026-10-10 11:44:29.550 E  kernel[0:29e785] (Sandbox) Sandbox: minos-vmm(84915) deny(1) sysctl-read hw.ephemeral_storage
2026-10-10 11:44:29.550 E  kernel[0:29e785] (Sandbox) Sandbox: minos-vmm(84915) deny(1) file-read-metadata /var
2026-10-10 11:44:29.550 E  kernel[0:29e278] (Sandbox) 2 duplicate reports for Sandbox: minos-vmm(84915) deny(1) file-read-metadata /var
2026-10-10 11:44:29.550 E  kernel[0:29e278] (Sandbox) Sandbox: minos-vmm(84915) deny(1) mach-lookup com.apple.system.notification_center
2026-10-10 11:44:29.550 E  kernel[0:29e61a] (Sandbox) Sandbox: minos-vmm(84915) deny(1) mach-lookup com.apple.system.opendirectoryd.libinfo
2026-10-10 11:44:29.550 E  kernel[0:29e785] (Sandbox) Sandbox: minos-vmm(84915) deny(1) file-read-metadata /etc
2026-10-10 11:44:29.550 E  kernel[0:29e61a] (Sandbox) 1 duplicate report for Sandbox: minos-vmm(84915) deny(1) file-read-metadata /etc
2026-10-10 11:44:29.550 E  kernel[0:29e61a] (Sandbox) Sandbox: minos-vmm(84915) deny(1) mach-lookup com.apple.system.opendirectoryd.libinfo
2026-10-10 11:44:29.550 E  kernel[0:29e785] (Sandbox) Sandbox: minos-vmm(84915) deny(1) file-read-metadata /etc
2026-10-10 11:44:29.571 E  kernel[0:29adad] (Sandbox) 1 duplicate report for Sandbox: minos-vmm(84915) deny(1) file-read-metadata /etc
2026-10-10 11:44:29.733 E  kernel[0:29e785] (Sandbox) Sandbox: minos-vmm(84915) deny(1) system-info vfs.disk-space
2026-10-10 11:44:29.736 E  kernel[0:29e61a] (Sandbox) 2 duplicate reports for Sandbox: minos-vmm(84915) deny(1) system-info vfs.disk-space
2026-10-10 11:44:29.736 E  kernel[0:29e61a] (Sandbox) Sandbox: minos-vmm(84915) deny(1) mach-lookup com.apple.logd
2026-10-10 11:44:29.736 E  kernel[0:29e785] (Sandbox) Sandbox: minos-vmm(84915) deny(1) file-read-metadata /System/Cryptexes/App
2026-10-10 11:44:29.736 E  kernel[0:29e785] (Sandbox) Sandbox: minos-vmm(84915) deny(1) file-read-metadata /System/Cryptexes/OS
2026-10-10 11:44:29.736 E  kernel[0:29e796] (Sandbox) Sandbox: minos-vmm(84915) deny(1) sysctl-read kern.hv_vmm_present
2026-10-10 11:44:29.736 E  kernel[0:29e796] (Sandbox) Sandbox: minos-vmm(84915) deny(1) file-read-metadata /System/Cryptexes/App
2026-10-10 11:44:29.736 E  kernel[0:29e796] (Sandbox) Sandbox: minos-vmm(84915) deny(1) file-read-metadata /System/Cryptexes/OS
2026-10-10 11:44:29.737 E  kernel[0:29e796] (Sandbox) Sandbox: minos-vmm(84915) deny(1) file-read-metadata /System/Cryptexes/App
2026-10-10 11:44:29.737 E  kernel[0:29e796] (Sandbox) Sandbox: minos-vmm(84915) deny(1) file-read-metadata /System/Cryptexes/OS
```

Reading: the VM boots and runs under the profile. `MINOS-GUEST-BOOT-OK` is on the console, one connection carries a
valid v1 status frame (761 bytes, under the 8 KiB-per-connection limit of 02-08 DIAG-01), the guest's connect to
`1.1.1.1:443` fails with `ENETUNREACH` (101) as in 02-08 S02-EGRESS, and the helper exits 0 after `on_exit`. The
four denials left (`system-info vfs.disk-space`, `mach-lookup com.apple.logd`, `sysctl-read kern.hv_vmm_present`, and
the startup set) did not affect the run and stay denied. There is no `iokit-open`, `mach-lookup`, or other denial
that names Hypervisor.framework or `hv_vm_create`: VM creation needed no hypervisor-specific rule.

### S05-CTRL-SOCK: the final profile without the socket connect rule

Requirement: SPIKE-05

Setup: the final profile with the line `(allow network-outbound (literal (param "SOCK")))` removed, written to
`crates/minos-vmm/minos-vmm.sb` for this run only and then restored from a saved copy (`cmp` against the copy printed
nothing; the ITER-4 text is what is committed). Purpose: show that an AF_UNIX connect to the route socket is a
`network-outbound` operation under Seatbelt (02-RESEARCH.md Pitfall 4), so the profile's single network allowance is
needed and is the only path out.

Profile text run (the diff from the final profile):

```diff
13d12
< (allow network-outbound (literal (param "SOCK")))
```

Command: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest MINOS_KRUNFW=$(realpath /opt/homebrew/lib/libkrunfw.5.dylib) cargo test -p minos-backend-krun --test spike_seatbelt boot_under_seatbelt -- --ignored --exact --nocapture --test-threads=1`

Exit code: 101

```text
== boot_under_seatbelt: exit_code Some(0), signal None, killed_at_deadline false, elapsed 309.462625ms, connections 0
-- console (3680 bytes, overflowed false):
[56 kernel console lines omitted by the evidence protocol]
MINOS-GUEST-BOOT-OK
minos-guest: cmdline reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0 root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1791647093260216000  --
minos-guest: osrelease 6.12.109
minos-guest: args /init
minos-guest: env HOME=/ KRUN_BOOT_START_NS=1791647093260216000 TERM=linux
minos-guest: net_ifaces lo
minos-guest: virtio_devices virtio0:0x0004 virtio1:0x0003 virtio2:0x0002 virtio3:0x0002 virtio4:0x0002 virtio5:0x0013 virtio6:0x4d47
minos-guest: sending the status frame: Connection reset by peer (os error 104)
MINOS-GUEST-POWEROFF
[1 kernel console lines omitted by the evidence protocol]

-- minos-vmm stderr (18 bytes):
minos-vmm: exit 0

-- minos-vmm stdout (0 bytes):

== end boot_under_seatbelt
Error: "expected exactly one connection, got 0"
FAILED

failures:

failures:
    boot_under_seatbelt

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.74s

error: test failed, to rerun pass `-p minos-backend-krun --test spike_seatbelt`
```

Command: `/usr/bin/log show --start "<the local time just before the run>" --style compact --predicate 'eventMessage CONTAINS "minos-vmm" AND eventMessage CONTAINS "deny"'`

Exit code: 0

```text
Timestamp               Ty Process[PID:TID]
2026-10-10 11:44:53.259 E  kernel[0:29eb43] (Sandbox) Sandbox: minos-vmm(85077) deny(1) system-info vfs.disk-space
2026-10-10 11:44:53.262 E  kernel[0:29eac5] (Sandbox) 2 duplicate reports for Sandbox: minos-vmm(85077) deny(1) system-info vfs.disk-space
2026-10-10 11:44:53.262 E  kernel[0:29eac5] (Sandbox) Sandbox: minos-vmm(85077) deny(1) mach-lookup com.apple.logd
2026-10-10 11:44:53.263 E  kernel[0:29eb47] (Sandbox) Sandbox: minos-vmm(85077) deny(1) sysctl-read kern.hv_vmm_present
2026-10-10 11:44:53.359 E  kernel[0:29eb43] (Sandbox) Sandbox: minos-vmm(85077) deny(1) network-outbound /private/tmp/mnsDVISJl5p/o.sock
```

(The startup denials already shown in ITER-4 are filtered out of this log with `grep -vE`; the five lines above are
the rest.)

Reading: Seatbelt denied `network-outbound /private/tmp/<job>/o.sock`, the guest's frame send was reset
(`ECONNRESET`, 104), and the supervisor received no connection. Note for Phase 5: the helper still exited 0, so a
supervisor that trusted the exit code would miss a confinement failure on the output path; the frame requirement
(02-RESEARCH.md Pitfall 1) is what catches it.

### S05-BOOT: boot_under_seatbelt passes under the final profile

Requirement: SPIKE-05

Setup: the final profile (ITER-4 text, committed). All three ignored tests run in one invocation, which is also the
plan's VM spike re-run command.

Command: `MINOS_VMM=$(pwd)/target/debug/minos-vmm MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest MINOS_KRUNFW=$(realpath /opt/homebrew/lib/libkrunfw.5.dylib) cargo test -p minos-backend-krun --test spike_seatbelt -- --ignored --nocapture --test-threads=1`

Exit code: 0

```text

running 3 tests
test boot_under_seatbelt ... == boot_under_seatbelt argv:
  /usr/bin/sandbox-exec
  -f
  ~/Repos/minos/crates/minos-vmm/minos-vmm.sb
  -D
  CONFIG=/private/tmp/mns6wq5YnCy/vmm.json
  -D
  CONSOLE_LOG=/private/tmp/mns6wq5YnCy/console.log
  -D
  INPUT_IMG=/private/tmp/mns6wq5YnCy/input.img
  -D
  KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib
  -D
  KRUNFW_LINK=/opt/homebrew/lib/libkrunfw.5.dylib
  -D
  ROOT_IMG=/private/tmp/mns6wq5YnCy/root.img
  -D
  SCRATCH_IMG=/private/tmp/mns6wq5YnCy/scratch.img
  -D
  SOCK=/private/tmp/mns6wq5YnCy/o.sock
  -D
  VMM=~/Repos/minos/target/debug/minos-vmm
  ~/Repos/minos/target/debug/minos-vmm
  /private/tmp/mns6wq5YnCy/vmm.json
== boot_under_seatbelt: exit_code Some(0), signal None, killed_at_deadline false, elapsed 312.784375ms, connections 1
-- console (3634 bytes, overflowed false):
[56 kernel console lines omitted by the evidence protocol]
MINOS-GUEST-BOOT-OK
minos-guest: cmdline reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0 root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1791647128170951000  --
minos-guest: osrelease 6.12.109
minos-guest: args /init
minos-guest: env HOME=/ KRUN_BOOT_START_NS=1791647128170951000 TERM=linux
minos-guest: net_ifaces lo
minos-guest: virtio_devices virtio0:0x0004 virtio1:0x0003 virtio2:0x0002 virtio3:0x0002 virtio4:0x0002 virtio5:0x0013 virtio6:0x4d47
MINOS-GUEST-FRAME-SENT 761 bytes
MINOS-GUEST-POWEROFF
[1 kernel console lines omitted by the evidence protocol]

-- minos-vmm stderr (18 bytes):
minos-vmm: exit 0

-- minos-vmm stdout (0 bytes):

-- connection 0 (761 bytes), frame:
{
  "job_error": null,
  "steps": [
    {
      "report_boot": {
        "args": [
          "/init"
        ],
        "cmdline": "reboot=k panic=-1 panic_print=0 nomodule console=hvc0 rootfstype=virtiofs rw quiet no-kvmapf rootfstype=squashfs ro ignore_loglevel dummy.numdummies=0 root=/dev/root init=/init root=/dev/vda      KRUN_BOOT_START_NS=1791647128170951000  --",
        "env": [
          [
            "HOME",
            "/"
          ],
          [
            "KRUN_BOOT_START_NS",
            "1791647128170951000"
          ],
          [
            "TERM",
            "linux"
          ]
        ],
        "marker": "MINOS-GUEST-BOOT-OK",
        "net_ifaces": [
          "lo"
        ],
        "osrelease": "6.12.109",
        "virtio_devices": [
          "virtio0:0x0004",
          "virtio1:0x0003",
          "virtio2:0x0002",
          "virtio3:0x0002",
          "virtio4:0x0002",
          "virtio5:0x0013",
          "virtio6:0x4d47"
        ]
      }
    },
    {
      "connect": {
        "results": [
          {
            "error": "Network unreachable (os error 101)",
            "ok": false,
            "raw_os_error": 101,
            "target": "1.1.1.1:443"
          }
        ]
      }
    }
  ],
  "v": 1
}
== end boot_under_seatbelt
ok
test profile_denies_network ... == profile_denies_network argv:
  /usr/bin/sandbox-exec
  -f
  ~/Repos/minos/crates/minos-vmm/minos-vmm.sb
  -D
  CONFIG=/private/tmp/mnsE3lXqeFo/vmm.json
  -D
  CONSOLE_LOG=/private/tmp/mnsE3lXqeFo/console.log
  -D
  INPUT_IMG=/private/tmp/mnsE3lXqeFo/input.img
  -D
  KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib
  -D
  KRUNFW_LINK=/opt/homebrew/lib/libkrunfw.5.dylib
  -D
  ROOT_IMG=/private/tmp/mnsE3lXqeFo/root.img
  -D
  SCRATCH_IMG=/private/tmp/mnsE3lXqeFo/scratch.img
  -D
  SOCK=/private/tmp/mnsE3lXqeFo/o.sock
  -D
  VMM=/usr/bin/nc
  /usr/bin/nc
  -z
  -G
  2
  1.1.1.1
  443
-- profile_denies_network: status ExitStatus(unix_wait_status(256)), exit code Some(1)
-- stdout:

-- stderr:

ok
test profile_denies_unlisted_file ... == profile_denies_unlisted_file (unlisted) argv:
  /usr/bin/sandbox-exec
  -f
  ~/Repos/minos/crates/minos-vmm/minos-vmm.sb
  -D
  CONFIG=/private/tmp/mnsoPUla647/vmm.json
  -D
  CONSOLE_LOG=/private/tmp/mnsoPUla647/console.log
  -D
  INPUT_IMG=/private/tmp/mnsoPUla647/input.img
  -D
  KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib
  -D
  KRUNFW_LINK=/opt/homebrew/lib/libkrunfw.5.dylib
  -D
  ROOT_IMG=/private/tmp/mnsoPUla647/root.img
  -D
  SCRATCH_IMG=/private/tmp/mnsoPUla647/scratch.img
  -D
  SOCK=/private/tmp/mnsoPUla647/o.sock
  -D
  VMM=/bin/cat
  /bin/cat
  /private/tmp/mnsgFYrhelf/unlisted.txt
-- profile_denies_unlisted_file (unlisted): status ExitStatus(unix_wait_status(256)), exit code Some(1)
-- stdout:

-- stderr:
cat: /private/tmp/mnsgFYrhelf/unlisted.txt: Operation not permitted

== profile_denies_unlisted_file (listed ROOT_IMG) argv:
  /usr/bin/sandbox-exec
  -f
  ~/Repos/minos/crates/minos-vmm/minos-vmm.sb
  -D
  CONFIG=/private/tmp/mnsoPUla647/vmm.json
  -D
  CONSOLE_LOG=/private/tmp/mnsoPUla647/console.log
  -D
  INPUT_IMG=/private/tmp/mnsoPUla647/input.img
  -D
  KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib
  -D
  KRUNFW_LINK=/opt/homebrew/lib/libkrunfw.5.dylib
  -D
  ROOT_IMG=/private/tmp/mnsoPUla647/root.img
  -D
  SCRATCH_IMG=/private/tmp/mnsoPUla647/scratch.img
  -D
  SOCK=/private/tmp/mnsoPUla647/o.sock
  -D
  VMM=/bin/cat
  /bin/cat
  /private/tmp/mnsoPUla647/root.img
-- profile_denies_unlisted_file (listed ROOT_IMG): status ExitStatus(unix_wait_status(0)), exit code Some(0)
-- stdout:
minos-seatbelt-listed

-- stderr:

ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.17s

```

Command: `/usr/bin/log show --start "<the local time just before the run>" --style compact --predicate 'eventMessage CONTAINS "minos-vmm" AND eventMessage CONTAINS "deny"'` (piped through `sort | uniq -c` after removing the timestamp and thread columns and the PID)

Exit code: 0

```text
   2 E  (Sandbox) 1 duplicate report for Sandbox: minos-vmm(PID) deny(1) file-read-metadata /etc
   2 E  (Sandbox) 1 duplicate report for Sandbox: minos-vmm(PID) deny(1) sysctl-read kern.bootargs
   1 E  (Sandbox) 2 duplicate reports for Sandbox: minos-vmm(PID) deny(1) file-read-metadata /var
   1 E  (Sandbox) 2 duplicate reports for Sandbox: minos-vmm(PID) deny(1) system-info vfs.disk-space
   1 E  (Sandbox) Sandbox: minos-vmm(PID) deny(1) file-read-data /dev/dtracehelper
   2 E  (Sandbox) Sandbox: minos-vmm(PID) deny(1) file-read-metadata /etc
   3 E  (Sandbox) Sandbox: minos-vmm(PID) deny(1) file-read-metadata /System/Cryptexes/App
   3 E  (Sandbox) Sandbox: minos-vmm(PID) deny(1) file-read-metadata /System/Cryptexes/OS
   1 E  (Sandbox) Sandbox: minos-vmm(PID) deny(1) file-read-metadata /System/Cryptexes/Rosetta
   2 E  (Sandbox) Sandbox: minos-vmm(PID) deny(1) file-read-metadata /var
   1 E  (Sandbox) Sandbox: minos-vmm(PID) deny(1) mach-lookup com.apple.logd
   1 E  (Sandbox) Sandbox: minos-vmm(PID) deny(1) mach-lookup com.apple.system.notification_center
   2 E  (Sandbox) Sandbox: minos-vmm(PID) deny(1) mach-lookup com.apple.system.opendirectoryd.libinfo
   1 E  (Sandbox) Sandbox: minos-vmm(PID) deny(1) sysctl-read hw.ephemeral_storage
   1 E  (Sandbox) Sandbox: minos-vmm(PID) deny(1) sysctl-read kern.hv_vmm_present
   1 E  (Sandbox) Sandbox: minos-vmm(PID) deny(1) sysctl-read kern.iossupportversion
   2 E  (Sandbox) Sandbox: minos-vmm(PID) deny(1) sysctl-read kern.osproductversion
   1 E  (Sandbox) Sandbox: minos-vmm(PID) deny(1) sysctl-read kern.osvariant_status
   1 E  (Sandbox) Sandbox: minos-vmm(PID) deny(1) system-info vfs.disk-space
```

The `boot_under_seatbelt` section of this run is the S05-BOOT result: exit 0, not killed, `MINOS-GUEST-BOOT-OK`, one
valid frame, and `connect` to `1.1.1.1:443` with `ok` false. The two profile tests in the same run are S05-DENY-NET
and S05-DENY-FILE below, which also record their own denial logs.

### S05-DENY-NET: the profile denies an outbound TCP connection

Requirement: SPIKE-05

Setup: the final profile and the same parameters as a job, with `VMM=/usr/bin/nc` so the profile permits exec of
`nc` and nothing else changes. The job files hold marker bytes. First the unsandboxed control.

Command: `/usr/bin/nc -z -G 2 1.1.1.1 443` (unsandboxed control, outside any test)

Exit code: 0

```text
Connection to 1.1.1.1 port 443 [tcp/https] succeeded!
```

The host itself has network, so a failure under the profile comes from the profile.

Command: `MINOS_KRUNFW=$(realpath /opt/homebrew/lib/libkrunfw.5.dylib) cargo test -p minos-backend-krun --test spike_seatbelt profile_denies_network -- --ignored --exact --nocapture --test-threads=1`

Exit code: 0

```text

running 1 test
test profile_denies_network ... == profile_denies_network argv:
  /usr/bin/sandbox-exec
  -f
  ~/Repos/minos/crates/minos-vmm/minos-vmm.sb
  -D
  CONFIG=/private/tmp/mnsCLnFegtN/vmm.json
  -D
  CONSOLE_LOG=/private/tmp/mnsCLnFegtN/console.log
  -D
  INPUT_IMG=/private/tmp/mnsCLnFegtN/input.img
  -D
  KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib
  -D
  KRUNFW_LINK=/opt/homebrew/lib/libkrunfw.5.dylib
  -D
  ROOT_IMG=/private/tmp/mnsCLnFegtN/root.img
  -D
  SCRATCH_IMG=/private/tmp/mnsCLnFegtN/scratch.img
  -D
  SOCK=/private/tmp/mnsCLnFegtN/o.sock
  -D
  VMM=/usr/bin/nc
  /usr/bin/nc
  -z
  -G
  2
  1.1.1.1
  443
-- profile_denies_network: status ExitStatus(unix_wait_status(256)), exit code Some(1)
-- stdout:

-- stderr:

ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.20s

```

Command: `/usr/bin/log show --start "<the local time just before the run>" --style compact --predicate '(eventMessage CONTAINS "nc(" OR eventMessage CONTAINS "cat(") AND eventMessage CONTAINS "deny"'`

Exit code: 0

```text
Timestamp               Ty Process[PID:TID]
2026-10-10 11:45:03.534 E  kernel[0:29ec39] (Sandbox) Sandbox: nc(85152) deny(1) sysctl-read security.mac.lockdown_mode_state
2026-10-10 11:45:03.534 E  kernel[0:29ec39] (Sandbox) Sandbox: nc(85152) deny(1) sysctl-read kern.bootargs
2026-10-10 11:45:03.534 E  kernel[0:29ec39] (Sandbox) Sandbox: nc(85152) deny(1) file-read-metadata /System/Cryptexes/OS
2026-10-10 11:45:03.534 E  kernel[0:29ec39] (Sandbox) Sandbox: nc(85152) deny(1) sysctl-read kern.bootargs
2026-10-10 11:45:03.534 E  kernel[0:29ec39] (Sandbox) 1 duplicate report for Sandbox: nc(85152) deny(1) sysctl-read kern.bootargs
2026-10-10 11:45:03.534 E  kernel[0:29ec39] (Sandbox) Sandbox: nc(85152) deny(1) file-read-data /System/Volumes/Preboot/Cryptexes/OS/System/Library/dyld
2026-10-10 11:45:03.534 E  kernel[0:29ec39] (Sandbox) Sandbox: nc(85152) deny(1) sysctl-read kern.bootargs
2026-10-10 11:45:03.534 E  kernel[0:29ec39] (Sandbox) 1 duplicate report for Sandbox: nc(85152) deny(1) sysctl-read kern.bootargs
2026-10-10 11:45:03.534 E  kernel[0:29ec39] (Sandbox) Sandbox: nc(85152) deny(1) file-read-metadata /System/Cryptexes/OS
2026-10-10 11:45:03.534 E  kernel[0:29ec39] (Sandbox) Sandbox: nc(85152) deny(1) sysctl-read kern.bootargs
2026-10-10 11:45:03.534 E  kernel[0:29ec39] (Sandbox) Sandbox: nc(85152) deny(1) file-read-metadata /System/Cryptexes/Rosetta
2026-10-10 11:45:03.535 E  kernel[0:29ec39] (Sandbox) Sandbox: nc(85152) deny(1) file-read-data /dev/dtracehelper
2026-10-10 11:45:03.535 E  kernel[0:29ec39] (Sandbox) Sandbox: nc(85152) deny(1) sysctl-read kern.osproductversion
2026-10-10 11:45:03.535 E  kernel[0:29ec39] (Sandbox) Sandbox: nc(85152) deny(1) sysctl-read kern.iossupportversion
2026-10-10 11:45:03.535 E  kernel[0:29ec39] (Sandbox) Sandbox: nc(85152) deny(1) sysctl-read kern.osproductversion
2026-10-10 11:45:03.535 E  kernel[0:29ec39] (Sandbox) Sandbox: nc(85152) deny(1) file-read-metadata /var
2026-10-10 11:45:03.535 E  kernel[0:29ec39] (Sandbox) Sandbox: nc(85152) deny(1) sysctl-read kern.osvariant_status
2026-10-10 11:45:03.535 E  kernel[0:29ec39] (Sandbox) Sandbox: nc(85152) deny(1) sysctl-read hw.ephemeral_storage
2026-10-10 11:45:03.535 E  kernel[0:29ec39] (Sandbox) Sandbox: nc(85152) deny(1) file-read-metadata /var
2026-10-10 11:45:03.535 E  kernel[0:29ebcf] (Sandbox) 2 duplicate reports for Sandbox: nc(85152) deny(1) file-read-metadata /var
2026-10-10 11:45:03.535 E  kernel[0:29ebcf] (Sandbox) Sandbox: nc(85152) deny(1) mach-lookup com.apple.system.notification_center
2026-10-10 11:45:03.535 E  kernel[0:29e61a] (Sandbox) Sandbox: nc(85152) deny(1) mach-lookup com.apple.system.opendirectoryd.libinfo
2026-10-10 11:45:03.535 E  kernel[0:29ec39] (Sandbox) Sandbox: nc(85152) deny(1) file-read-metadata /etc
2026-10-10 11:45:03.535 E  kernel[0:29e61a] (Sandbox) 1 duplicate report for Sandbox: nc(85152) deny(1) file-read-metadata /etc
2026-10-10 11:45:03.535 E  kernel[0:29e61a] (Sandbox) Sandbox: nc(85152) deny(1) mach-lookup com.apple.system.opendirectoryd.libinfo
2026-10-10 11:45:03.535 E  kernel[0:29ec39] (Sandbox) Sandbox: nc(85152) deny(1) file-read-metadata /etc
2026-10-10 11:45:03.536 E  kernel[0:29ebcf] (Sandbox) 1 duplicate report for Sandbox: nc(85152) deny(1) file-read-metadata /etc
2026-10-10 11:45:03.536 E  kernel[0:29ebcf] (Sandbox) Sandbox: nc(85152) deny(1) mach-lookup com.apple.logd
2026-10-10 11:45:03.536 E  kernel[0:29ec39] (Sandbox) Sandbox: nc(85152) deny(1) file-read-metadata /System/Cryptexes/App
2026-10-10 11:45:03.536 E  kernel[0:29ec39] (Sandbox) Sandbox: nc(85152) deny(1) file-read-metadata /System/Cryptexes/OS
2026-10-10 11:45:03.536 E  kernel[0:29ec39] (Sandbox) Sandbox: nc(85152) deny(1) file-read-data /Library/Preferences/com.apple.networkd.plist
2026-10-10 11:45:03.536 E  kernel[0:29ec39] (Sandbox) 1 duplicate report for Sandbox: nc(85152) deny(1) file-read-data /Library/Preferences/com.apple.networkd.plist
2026-10-10 11:45:03.536 E  kernel[0:29ec39] (Sandbox) Sandbox: nc(85152) deny(1) file-read-metadata /System/Cryptexes/App
2026-10-10 11:45:03.536 E  kernel[0:29ec39] (Sandbox) Sandbox: nc(85152) deny(1) file-read-metadata /System/Cryptexes/OS
2026-10-10 11:45:03.536 E  kernel[0:29ec39] (Sandbox) Sandbox: nc(85152) deny(1) sysctl-read net.routetable.0.0.3.0
2026-10-10 11:45:03.536 E  kernel[0:29ec39] (Sandbox) Sandbox: nc(85152) deny(1) network-outbound remote:*:443
```

Reading: `nc` ran under the profile and exited 1 (with `-z` it prints nothing on failure). The log shows
`deny(1) network-outbound remote:*:443`, plus the denied `net.routetable` sysctl and `com.apple.networkd.plist` read
that precede it. The unsandboxed control succeeded a moment earlier.

### S05-DENY-FILE: the profile denies an unlisted file and permits a listed one

Requirement: SPIKE-05

Setup: the final profile, the job parameters of a marker job, and `VMM=/bin/cat`. The unlisted file is
`unlisted.txt` in a second `short_job_dir` (a sibling of the job directory under `/private/tmp`, not a parameter). The
listed file is the job's `ROOT_IMG`, which holds `minos-seatbelt-listed\n`.

Command: `MINOS_KRUNFW=$(realpath /opt/homebrew/lib/libkrunfw.5.dylib) cargo test -p minos-backend-krun --test spike_seatbelt profile_denies_unlisted_file -- --ignored --exact --nocapture --test-threads=1`

Exit code: 0

```text

running 1 test
test profile_denies_unlisted_file ... == profile_denies_unlisted_file (unlisted) argv:
  /usr/bin/sandbox-exec
  -f
  ~/Repos/minos/crates/minos-vmm/minos-vmm.sb
  -D
  CONFIG=/private/tmp/mnsU0RVyr76/vmm.json
  -D
  CONSOLE_LOG=/private/tmp/mnsU0RVyr76/console.log
  -D
  INPUT_IMG=/private/tmp/mnsU0RVyr76/input.img
  -D
  KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib
  -D
  KRUNFW_LINK=/opt/homebrew/lib/libkrunfw.5.dylib
  -D
  ROOT_IMG=/private/tmp/mnsU0RVyr76/root.img
  -D
  SCRATCH_IMG=/private/tmp/mnsU0RVyr76/scratch.img
  -D
  SOCK=/private/tmp/mnsU0RVyr76/o.sock
  -D
  VMM=/bin/cat
  /bin/cat
  /private/tmp/mns3ZKAxhMy/unlisted.txt
-- profile_denies_unlisted_file (unlisted): status ExitStatus(unix_wait_status(256)), exit code Some(1)
-- stdout:

-- stderr:
cat: /private/tmp/mns3ZKAxhMy/unlisted.txt: Operation not permitted

== profile_denies_unlisted_file (listed ROOT_IMG) argv:
  /usr/bin/sandbox-exec
  -f
  ~/Repos/minos/crates/minos-vmm/minos-vmm.sb
  -D
  CONFIG=/private/tmp/mnsU0RVyr76/vmm.json
  -D
  CONSOLE_LOG=/private/tmp/mnsU0RVyr76/console.log
  -D
  INPUT_IMG=/private/tmp/mnsU0RVyr76/input.img
  -D
  KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib
  -D
  KRUNFW_LINK=/opt/homebrew/lib/libkrunfw.5.dylib
  -D
  ROOT_IMG=/private/tmp/mnsU0RVyr76/root.img
  -D
  SCRATCH_IMG=/private/tmp/mnsU0RVyr76/scratch.img
  -D
  SOCK=/private/tmp/mnsU0RVyr76/o.sock
  -D
  VMM=/bin/cat
  /bin/cat
  /private/tmp/mnsU0RVyr76/root.img
-- profile_denies_unlisted_file (listed ROOT_IMG): status ExitStatus(unix_wait_status(0)), exit code Some(0)
-- stdout:
minos-seatbelt-listed

-- stderr:

ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.21s

```

Command: `/usr/bin/log show --start "<the local time just before the run>" --style compact --predicate 'eventMessage CONTAINS "cat(" AND eventMessage CONTAINS "deny"'` (startup denials filtered out as in S05-CTRL-SOCK)

Exit code: 0

```text
Timestamp               Ty Process[PID:TID]
2026-10-10 11:45:14.488 E  kernel[0:29ed25] (Sandbox) Sandbox: cat(85219) deny(1) file-read-data /private/tmp/mns3ZKAxhMy/unlisted.txt
```

Reading: under one profile, `cat` of the unlisted file failed with `Operation not permitted` (exit 1, nothing on
stdout) and the log names `file-read-data` of that path; `cat` of the listed root image exited 0 and printed its
bytes. The `path-ancestors` metadata rule did not open the sibling directory's contents.

### S05-RECIPE: dedicated unprivileged `_minos` user (not executed, D-10)

Requirement: SPIKE-05

Status: **not executed (D-10).** No user or group was created, no `dscl` write ran, no `sudo` command ran, and no
LaunchDaemon was installed. The recipe below is recorded for Phase 5 (`minos host setup`, KRUN-06); it is the
standard macOS service-account recipe from 02-RESEARCH.md Pattern 6 and is itself unverified on this host.

```text
sudo dscl . -create /Users/_minos UniqueID <an unused id below 500>
sudo dscl . -create /Users/_minos PrimaryGroupID <a matching _minos group id>
sudo dscl . -create /Users/_minos UserShell /usr/bin/false
sudo dscl . -create /Users/_minos NFSHomeDirectory /var/empty
sudo dscl . -create /Users/_minos IsHidden 1
```

Open issue for Phase 5: an unprivileged parent cannot start a process as `_minos` without `sudo` (for example a
sudoers NOPASSWD rule pinned to the exact helper path) or a LaunchDaemon; `minos host setup` must choose one and
`host doctor` must report it. Either choice also has to hand the job files to `_minos` (owner or ACL on the job
directory) and let `_minos` connect to the supervisor's socket, which the current 0700 job directory does not allow.

Proof that nothing was created:

Command: `dscl . -read /Users/_minos`

Exit code: 56

```text
<dscl_cmd> DS Error: -14136 (eDSRecordNotFound)
```

Command: `id _minos`

Exit code: 1

```text
id: _minos: no such user
```

### SPIKE-05 answer

**Yes.** A deny-default Seatbelt profile around `minos-vmm` still lets it create and run a VM. Under the final
profile (`crates/minos-vmm/minos-vmm.sb`, the ITER-4 text) `boot_under_seatbelt` passed: exit 0, the boot marker, one
valid status frame over the vsock route, and the guest's TCP connect failing (S05-BOOT). The same profile, with the
exec target swapped, denied `nc`'s outbound TCP connection while the unsandboxed control connected (S05-DENY-NET), and
denied `cat` an unlisted file while a listed file stayed readable (S05-DENY-FILE). Four iterations were used of the
eight allowed; three allowances were added after E-1, each from a logged denial that stopped the run.

`hv_vm_create` needed **no hypervisor-specific rule**: no `iokit-open`, no `mach-lookup`, and no other denial in any
iteration names Hypervisor.framework, and the VM was created with none added. This matches the BoxLite prior art in
02-RESEARCH.md Pattern 6. The only hypervisor-related denial, `sysctl-read kern.hv_vmm_present`, is a nested-VM query
that did not affect the run and stays denied.

Network: the profile has exactly one network allowance, `network-outbound` to the literal route socket path.
S05-CTRL-SOCK shows it is needed (without it the AF_UNIX connect is denied and no frame arrives) and S05-DENY-NET
shows nothing else gets out. There is no general network rule and no default allow.

Final profile allowances:

| # | Allowance | The denial or file it answers | Evidence |
|---|---|---|---|
| 1 | `process-exec` of `VMM` | `sandbox-exec` must exec the helper, and only the helper | E-1; S05-BOOT |
| 2 | `file-read*` of `/` (literal), `/usr/lib`, `/System/Library`, `/private/var/db/dyld`, `/dev/null`, `/dev/urandom` | dyld, libSystem, and the frameworks the helper links (Hypervisor.framework), plus the null and random devices | E-1; S05-BOOT |
| 3 | `file-read*` of `VMM`, `CONFIG`, `KRUNFW`, `KRUNFW_LINK`, `ROOT_IMG`, `INPUT_IMG` | the helper binary, its config, libkrunfw (Cellar target and Homebrew symlink), and the two read-only disks | E-1; S05-BOOT |
| 4 | `file-read*` and `file-write*` of `SCRATCH_IMG`, `CONSOLE_LOG` | the read-write scratch disk and the console file the helper creates | E-1; S05-BOOT |
| 5 | `network-outbound` to the literal `SOCK` path | the AF_UNIX connect of the vsock route (02-RESEARCH.md Pitfall 4) | E-1; S05-CTRL-SOCK denial `network-outbound /private/tmp/<job>/o.sock` |
| 6 | `sysctl-read` of `hw.pagesize`, `hw.ncpu`, `hw.memsize`, `hw.optional.arm.*`, `kern.proc.pid.*` | the starting set from 02-RESEARCH.md Pattern 6 | E-1 |
| 7 | `sysctl-read` of `hw.pagesize_compat` | `deny(1) sysctl-read hw.pagesize_compat`; std aborted with "failed to allocate a guard page" | S05-ITER-1 |
| 8 | `file-read-metadata` of `path-ancestors` of `KRUNFW`, `KRUNFW_LINK`, `CONFIG` | `deny(1) file-read-metadata /opt`; preflight's canonicalize stats each ancestor | S05-ITER-2 |
| 9 | `file-read-metadata` of the literal `SOCK` path | `deny(1) file-read-metadata /private/tmp/<job>/o.sock`; preflight checks the path is a socket | S05-ITER-3 |

Rows 1 to 6 are the E-1 starting set and were not minimized one by one; only row 5 was shown to be needed by removing
it. Rows 7 to 9 each have a `;` comment in the profile naming the denial. Every path in the profile is a `-D`
parameter or a system location (`grep -c '/Users/'` prints 0).

Denied and left denied (logged in S05-ITER-2 to S05-BOOT, harmless to the run): `kern.bootargs`, `kern.osproductversion`,
`kern.iossupportversion`, `kern.osvariant_status`, `hw.ephemeral_storage`, `kern.hv_vmm_present`,
`security.mac.lockdown_mode_state`, `system-info vfs.disk-space`, metadata of `/var`, `/etc`, and the
`/System/Cryptexes/{OS,App,Rosetta}` paths, data of `/dev/dtracehelper` and the Preboot Cryptex dyld path, and
`mach-lookup` of `com.apple.logd`, `com.apple.system.notification_center`, and
`com.apple.system.opendirectoryd.libinfo`.

Confinement layers Phase 5 can report as active in `BackendInfo` (KRUN-07) for a local libkrun run:

| Layer | Status after this spike | What Phase 5 records |
|---|---|---|
| Hardware virtualization (Hypervisor.framework via libkrun) | active (02-06) | always |
| No network device, no TSI, no virtio-fs | active (02-06, 02-08) | always |
| Seatbelt deny-default profile on `minos-vmm` | **can be active**: VM boots under it (S05-BOOT) | active only when the helper was launched through `/usr/bin/sandbox-exec` with this profile; record the profile's SHA-256 and `sandbox-exec`'s SHA-256. If `sandbox-exec` is missing or the profile fails, the run records Seatbelt as absent; never a silent fallback |
| Dedicated unprivileged `_minos` user | **not active**: recipe only (S05-RECIPE, D-10) | absent until `minos host setup` creates the user and solves the launch-as-user issue; until then every run records reduced local hardening (no dedicated user) in provenance |
| seccomp, cgroups | not available on macOS | absent on this platform; reported as such, not hidden |

Caveats for Phase 5: `sandbox-exec` is deprecated by Apple (09-00 man page text) though shipped and working on macOS
27.0, so `host doctor` should run a preflight equivalent to S05-DENY-NET and S05-DENY-FILE and refuse to claim the
layer if the denials do not hold. The helper exits 0 even when the profile breaks its output path (S05-CTRL-SOCK), so
the frame, not the exit code, decides success.

<!-- fragments-end -->

## Boot-path decision (D-18)

Decided by the user on 2026-10-09, after research, and recorded in 02-CONTEXT.md D-18. Stock libkrunfw 5.6.2 has `# CONFIG_BLK_DEV_INITRD is not set`, so the kernel cannot unpack an initramfs; the one literal attempt failed as predicted (S01-INITRAMFS: the kernel never ran `/init` from the initramfs and panicked with `VFS: Unable to mount root fs on "/dev/root" or unknown-block(0,0)`, and the helper still exited 0). The VM instead boots from the raw read-only SquashFS root on `/dev/vda`, with `/init` = the static musl `minos-guest` written in last by the trusted packer, no initramfs, and no virtio-fs device. `minos-vmm` carries `root=/dev/vda` past msb_krun's own `root=/dev/root` through `KernelBuilder::init_path`, and the boot test asserts the exact `/proc/cmdline` tokens (S01-ROOTDISK, S01-CMDLINE). By the user's decision in S01-NETIFACES ("option 2"), the fixed root-disk arguments also carry one Minos-originated token, `dummy.numdummies=0`, and the boot test asserts it with the rest. SPIKE-08: not triggered (S08).

Superseded on 2026-10-09 by this decision, in the same change: REQUIREMENTS.md SPIKE-01 ("from an initramfs"), KRUN-09 ("a root image and an initramfs"), and ROADMAP.md Phase 2 success criterion 1 and Phase 5 criterion 4. Superseded by D-11 in the same change: KRUN-08 ("An `xtask` signs the helper") and ROADMAP.md Phase 5 criterion 2.

## Research claims overturned (D-09)

The research files stay as written, because they are dated inputs (02-CONTEXT.md D-09); `git diff --quiet dbd1a87491d155fb6c4685ee4a30ce7a42d937c4 -- .planning/research` exits 0. Each row names a claim a spike or a spike-driven decision contradicted, the evidence, and where the design of record was corrected. A claim that held gets no row. Rows are sorted by file, then by section.

| Research file and section | Claim (quoted) | What the spike showed | Evidence | Where corrected |
|---|---|---|---|---|
| `.planning/phases/02-platform-spikes/02-RESEARCH.md` Pattern 3 (K7 table), row "Guest `/init` exits or panics" | "\"Attempted to kill init\" → kernel panic → `panic=-1` → PSCI `SYSTEM_RESET` → `Shutdown`" | libkrunfw patches a returning PID 1 into an orderly reboot (`Failed to start orderly reboot: forcing the issue`); no panic. The exit status is still 0, as predicted | K7-02 | CLAUDE.md Local backends (exit-code bullet) |
| `02-RESEARCH.md` Pattern 5 (egress probe) | "expect `ENETUNREACH` (101) and no TSI" | IPv4 targets fail with `ENETUNREACH` (101), but public IPv6 and `::1` fail with `EADDRNOTAVAIL` (99). TSI is compiled into the kernel (`CONFIG_TSI=y`); no egress holds only while inet hijack stays off | S02-EGRESS, H-02, SPIKE-02 and SPIKE-03 answers | CLAUDE.md Local backends (no-network bullet); `.claude/CLAUDE.md` decision 1b, K3 row, corrections section |
| `02-RESEARCH.md` Pattern 5 (vsock probe) | "write a deterministic pattern of N bytes with N > host cap ... The host reads `take(cap + 1)`, asserts it read exactly `cap` bytes ... The guest records the write error (EPIPE or ECONNRESET). For K4, time a 64 MiB transfer" | One connection carries at most 8192 bytes guest to host on the pinned `msb_krun_devices` 0.1.41: 8193 to 32768 bytes arrive cut to 8192, a single write over 32768 bytes is dropped, and a stream over 262144 bytes deadlocks. No stream reached the cap, so the EPIPE or ECONNRESET expectation is untested. The 64 MiB transfer never completed | S03-WITHIN, S03-OVER, K4-THROUGHPUT, 02-08 DEFECT-01, DIAG-01, S03-ANSWER | CLAUDE.md Local backends (output bullet); `.claude/CLAUDE.md` decision 1c, K4 row, Version Compatibility, corrections section |
| `02-RESEARCH.md` Pattern 7 (usage field names) | "`usage.{input_tokens, output_tokens, cache_creation_input_tokens, cache_read_input_tokens, output_tokens_details.thinking_tokens, server_tool_use, service_tier, cache_creation.{...}, iterations[]}`" | All present, plus `inference_geo`, `speed`, and `fallback_credit` | C-03, Research claims checked | `.claude/CLAUDE.md` Question 9 (corrections bullet) |
| `02-RESEARCH.md` Pattern 7 (`--disable-slash-commands`) | `--disable-slash-commands` should empty `skills`, `slash_commands`, and `agents` | It empties `slash_commands` and `skills`; `agents` is still listed (no Agent tool exists) | C-05, Research claims checked | `.claude/CLAUDE.md` Question 9 (corrections bullet) |
| `02-RESEARCH.md` Pattern 7 (unsatisfiable schema, expectation) | a schema the model fails shows `error_max_structured_output_retries` | `error_max_turns`, exit 1, `is_error` `true`, under `--max-turns` 1 and 3; the retries subtype did not appear | C-11, C-12, Research claims checked | `.claude/CLAUDE.md` Question 9 (corrections bullet) |
| `.planning/research/ARCHITECTURE.md` Pattern 5, device table row `vda` | "root image (user Dockerfile + minos final stage)" | There is no Docker final stage: the trusted packer writes `/init` into the image after every file from the user's image (decision D-18, booted in S06-BOOT) | S01-ROOTDISK, S06-BOOT | CLAUDE.md Environments and source (`/init` sentence) |
| `ARCHITECTURE.md` Pattern 5, device table row "vsock port `N`", and "Output stream: One tar" | "the guest writes a tar; the host reads it with caps" over one connection | One connection carries at most 8192 bytes; the user chose a chunked protocol of short connections, at most 4 KiB each | S03-WITHIN, S03-OVER, DIAG-01, S03-ANSWER | CLAUDE.md Local backends (output bullet) |
| `ARCHITECTURE.md` Pattern 5, paragraph after the device table (also Pattern 6 and the P0 Spikes row) | "Root uses `krun_set_root_disk_remount`. In v1.19.6 it boots libkrun's `init.krun` from a NullFs virtiofs with no host directory" | The VM boots directly from the SquashFS root on `/dev/vda` with `root=/dev/vda` placed last through `init_path`; no virtio-fs device of any kind (no `0x001a` in the guest's device list) | S01-ROOTDISK, S01-CMDLINE, S01-FRAME | CLAUDE.md Local backends (root-disk bullet); `.claude/CLAUDE.md` Stack Patterns, corrections section |
| `ARCHITECTURE.md` Pattern 5, "Budgets and exit", "Exit code" bullet (also the P0 Spikes row, "exit code propagates") | "libkrun `exit()`s the helper with the workload's exit code (125, 126, or 127 mean init failures)" | The helper's exit code never carries the guest's result: guest power-off, a returning init, and a host SIGTERM all exit 0. This holds only for libkrun's `init.krun` plus virtio-fs path. The status frame decides | K7-01, K7-02, K7-05, K7 table, Classification rule for Phase 5 | CLAUDE.md Local backends (exit-code bullet); `.claude/CLAUDE.md` K7 row, corrections section |
| `ARCHITECTURE.md` Suggested Build Order, P0 Spikes row | "`ip link` shows only `lo`" | The stock kernel creates `dummy0` (`CONFIG_DUMMY=y`, `numdummies` defaults to 1); by the user's decision ("option 2") the launcher passes `dummy.numdummies=0` and the boot test asserts no virtio-net device | S01-NETIFACES | CLAUDE.md Local backends (root-disk and no-network bullets); `.claude/CLAUDE.md` libkrunfw row |
| `.planning/research/PITFALLS.md` Pitfall 22 (the entitlement disappears on every rebuild), Prevention | "Add an `xtask` or Makefile step that signs the helper after build." | One documented `codesign` argv in CLAUDE.md Commands, run after every build; no xtask, script, `build.rs`, or alias (decision D-11), used for every spike run | S04-SIGN, K5 | CLAUDE.md Commands; REQUIREMENTS.md KRUN-08; ROADMAP.md Phase 5 criterion 2 |
| `.planning/research/STACK.md` Decisions at a glance 1a, Core Technologies libkrunfw row, Sources (libkrunfw config); also `.planning/research/SUMMARY.md` Recommended Stack | "libkrunfw 5.6.2 (Linux 6.12.91)"; "Guest Linux kernel (6.12.91)"; "kernel 6.12.91" | The guest's `osrelease` is 6.12.109; the committed config header is stale (02-RESEARCH.md Pitfall 7 predicted this) | S01-CMDLINE, H-02 | `.claude/CLAUDE.md` decision 1a, libkrunfw row, Sources, corrections section |
| `STACK.md` Decisions at a glance 11, macOS signing specifics | "Only `minos-vmm` needs `com.apple.security.hypervisor`. Ad-hoc sign it with `codesign --force -s - --entitlements minos-vmm.entitlements`." | Under the hardened runtime the helper also needs `com.apple.security.cs.disable-library-validation`: the Homebrew libkrunfw dylib is refused for "different Team IDs" | K5, S04-SIGN | CLAUDE.md Commands (signing paragraph); `.claude/CLAUDE.md` decision 11, plist bullet, K5 row, corrections section |
| `STACK.md` macOS signing specifics | "Hypervisor.framework refuses `hv_vm_create` (`HV_DENIED`)" | Through msb_krun the refusal surfaces as `VmSetup(VmCreate)` with the return code discarded, and `minos-vmm` exits 70; `HV_DENIED` is not observable | S04-UNSIGNED | CLAUDE.md Commands (signing paragraph); `.claude/CLAUDE.md` signing bullet |
| `STACK.md` Development Tools, `/usr/bin/codesign` row; macOS signing specifics ("Add an `xtask` (`cargo xtask build-vmm`)") | "Called from an `xtask` (argv array), not from `build.rs`" | No xtask (D-11); the documented argv in CLAUDE.md Commands signs every spike helper | S04-SIGN, K5 | `.claude/CLAUDE.md` codesign row, corrections section; CLAUDE.md Commands |
| `STACK.md` Question 1 launcher recipe, item 4 "No virtio-fs at all", and the What remains unverified K1 and K2 rows; Alternatives Considered; What NOT to Use (virtio-fs row) | "Boot from the initramfs, then mount `/dev/vda` from inside the guest ... With an initramfs present the kernel runs `/init` from the initramfs and does not mount `root=`." | Stock libkrunfw has `# CONFIG_BLK_DEV_INITRD is not set`; the literal initramfs attempt panicked (`VFS: Unable to mount root fs on "/dev/root"`). The root-disk boot (D-18) works | H-02, S01-INITRAMFS, S01-ROOTDISK, S01-CMDLINE | `.claude/CLAUDE.md` libkrunfw row, `cpio` row, K1, K2, Alternatives Considered, What NOT to Use, Stack Patterns, corrections section; CLAUDE.md Local backends; REQUIREMENTS.md SPIKE-01, KRUN-09; ROADMAP.md Phase 2 criterion 1, Phase 5 criterion 4 |
| `STACK.md` Question 1 launcher recipe, item 5 | "The parent reads one capped tar stream from it into `Untrusted<Bytes>`." | One connection carries at most 8192 bytes; output becomes a sequence of chunked connections | DIAG-01, S03-ANSWER | CLAUDE.md Local backends (output bullet) |
| `STACK.md` Question 6 testing, VM integration row | "`#[ignore]`d tests, or a feature-gated test crate run by `cargo xtask vm-test`" | The VM spikes are ignored integration tests in `crates/minos-backend-krun/tests/spike_*.rs`, run with `cargo test --ignored`; no xtask | S01-ROOTDISK (command lines), D-11 | `.claude/CLAUDE.md` testing row; CLAUDE.md Commands |
| `STACK.md` Sources, Claude Code docs bullet | "Local `claude --help` (2.1.295) ... flag interplay is unverified" | The spikes ran 2.1.296; `command -v claude` resolves to a cmux shim, and the canonical binary is under `~/.local/share/claude/versions/`; the flag interplay is verified | C-01, SPIKE-07 contract | `.claude/CLAUDE.md` Sources, decision 9, Question 9 |

## DOCS-05 checklist

Plan 02-11, 2026-10-10. Each 02-11-PLAN.md E-10 item, checked on CLAUDE.md after commit `235a1cf`. Output is shown after the arrow.

- Local-first build order up front: `sed -n '/^## Build order/{n;n;p;}' CLAUDE.md` -> `` Milestone v1 is local-first, on macOS arm64 (02-CONTEXT.md D-07; `.planning/ROADMAP.md`): `` (E-7's first line)
- `ModelProvider` decision, `claude` CLI provider as personal default: `grep -c 'ModelProvider' CLAUDE.md` -> `1`
- No tokio in v1: `grep -c '^- \*\*No tokio in v1.\*\*' CLAUDE.md` -> `1`
- Pico-dollar money: `grep -c 'pico-dollar' CLAUDE.md` -> `1`; `grep -c 'nano' CLAUDE.md` -> `0`
- `libkrun/krun` tap: `grep -c 'libkrun/krun' CLAUDE.md` -> `2`
- libkrun networking default and msb_krun counter-default: `grep -c 'inet_hijack' CLAUDE.md` -> `1` (merged into the existing no-network bullet; see 02-11-SUMMARY.md)
- `minos-loop` crate and loop-never-links-sandbox: `grep -c 'minos-loop/' CLAUDE.md` -> `1`; ``grep -n '^- `loop`' CLAUDE.md`` -> `` 102:- `loop` depends on `core`, `store`, and `propose`. It never links `sandbox` or a backend, so it never sees raw sandbox output. The orchestrator, when it arrives, follows the same rule. ``
- Every spike result in Verified facts with the date: `## Verified facts, local (as of 2026-10-10)` has 9 bullets: SPIKE-01 (three: kernel, boot, exit code), SPIKE-02, SPIKE-03 (the recorded limit), SPIKE-04, SPIKE-05, SPIKE-06, SPIKE-07; every bullet cites evidence IDs from this file
- Codesign line and musl clippy line in Commands: `grep -c 'codesign --force --sign - --options runtime' CLAUDE.md` -> `1`; `grep -c 'Phase 2 onward (02-CONTEXT.md D-03)' CLAUDE.md` -> `2`
- P0 open questions answered and removed; AWS questions deferred: `### Deferred to the AWS milestone` holds 6 bullets, the six BASE bullets verbatim; the BASE `## Open questions` held no local question, so none needed removing
- AWS content labelled, nothing removed (D-08): `grep -c '^## Durable execution rules (AWS milestone)$' CLAUDE.md` -> `1`; `grep -c '^## Verified facts, AWS (as of 2026-10-08)$' CLAUDE.md` -> `1`; each non-empty line of the BASE (`dbd1a87`) `## Durable execution rules` (with its platform limits table), `## Build order`, `## Open questions`, and `## Verified facts (as of 2026-10-08)` sections checked with `grep -cF` against CLAUDE.md -> `D-08 lines checked: 43, missing: 0`
- No slot marker left: `grep -nE '\{(H-|RI-|S0|K[0-9]|the |version|osrelease|from|each|anything|num_turns|SPIKE)' CLAUDE.md` -> (no output)
- No ironclad claim added: `grep -ci ironclad CLAUDE.md` -> `1`, equal to BASE (the existing do-not-claim sentence)

## Final gate (Phase 2)

Plan 02-11, 2026-10-10, HEAD `235a1cf5d652ecd6fb4d5b4bfc546b3a75b5e8cc`, run from the repository root. Only each command's closing summary lines are shown, verbatim. Home path shown as `~`.

Command: `cargo fmt --all --check`

Exit code: 0

```text
(no output)
```

Command: `cargo clippy --workspace --all-targets -- -D warnings`

Exit code: 0

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.09s
```

Command: `cargo test --workspace`

Exit code: 0

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running unittests src/lib.rs (target/debug/deps/minos_backend_krun-540d1d7b0c0c63d3)
test result: ok. 55 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 1.02s
     Running tests/spike_boot.rs (target/debug/deps/spike_boot-a9f449792644c36b)
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/spike_exits.rs (target/debug/deps/spike_exits-887a382d85f74a31)
test result: ok. 0 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/spike_net.rs (target/debug/deps/spike_net-9a0a538ebb732781)
test result: ok. 0 passed; 0 failed; 4 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/spike_rootimg.rs (target/debug/deps/spike_rootimg-c993db859c447c7f)
test result: ok. 0 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/spike_seatbelt.rs (target/debug/deps/spike_seatbelt-d68db829a28e468a)
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/main.rs (target/debug/deps/minos-48c6bf59dbe260aa)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/minos_core-c619c5be5a10c8c9)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/minos_guest-c85fab116ead6037)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running unittests src/main.rs (target/debug/deps/minos_guest-c1681213587d5b69)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/minos_loop-994e138925915d56)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/minos_propose-1a9a8fd769d6a47b)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/minos_runner-89243ff6043495d6)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/minos_sandbox-dcc21162d5b68e3e)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/minos_store-4edbb133c2d25261)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/main.rs (target/debug/deps/minos_vmm-6911070ecfa4068f)
test result: ok. 46 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests minos_backend_krun
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests minos_core
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests minos_guest
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests minos_loop
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests minos_propose
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests minos_runner
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests minos_sandbox
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests minos_store
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Command: `cargo deny check`

Exit code: 0. The warn-level duplicate-version reports (`multiple-versions = "warn"`, Cargo.toml comment) are listed, then the closing line.

```text
warning[duplicate]: found 2 duplicate entries for crate 'bitflags'
warning[duplicate]: found 2 duplicate entries for crate 'getrandom'
warning[duplicate]: found 2 duplicate entries for crate 'miniz_oxide'
warning[duplicate]: found 2 duplicate entries for crate 'nix'
warning[duplicate]: found 2 duplicate entries for crate 'rand_chacha'
warning[duplicate]: found 2 duplicate entries for crate 'rand_core'
warning[duplicate]: found 2 duplicate entries for crate 'syn'
advisories ok, bans ok, licenses ok, sources ok
```

Command: `cargo doc --workspace --no-deps`

Exit code: 0

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
   Generated ~/Repos/minos/target/doc/minos_backend_krun/index.html and 9 other files
```

Command: `cargo clippy -p minos-guest --all-targets --target aarch64-unknown-linux-musl -- -D warnings`

Exit code: 0

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.07s
```

Command (C-01): `for f in crates/*/Cargo.toml; do grep -A1 -xF '[lints]' "$f" | grep -qxF 'workspace = true' && echo "ok $f" || echo "MISSING $f"; done`

```text
ok crates/minos-backend-krun/Cargo.toml
ok crates/minos-cli/Cargo.toml
ok crates/minos-core/Cargo.toml
ok crates/minos-guest/Cargo.toml
ok crates/minos-loop/Cargo.toml
ok crates/minos-propose/Cargo.toml
ok crates/minos-runner/Cargo.toml
ok crates/minos-sandbox/Cargo.toml
ok crates/minos-store/Cargo.toml
ok crates/minos-vmm/Cargo.toml
```

Command (C-02): `grep -rnE '#!?\[allow\(' crates --include='*.rs'`

Exit code: 1

```text
(no output)
```

Command (no unsafe): `grep -rnE 'unsafe[[:space:]]*(\{|fn |impl|extern|trait)|unsafe_code' crates --include='*.rs'`

Exit code: 1

```text
(no output)
```

## Invariant check (Phase 2)

BASE is `dbd1a87491d155fb6c4685ee4a30ce7a42d937c4` (the 02-01 fragment's `HEAD at start`). Each check's output follows its `### I<n>` marker, verbatim.

Commands, in order:

- I1: `git diff dbd1a87 -- Cargo.toml` (expected: the panic comment only, 02-01 P-PANIC)
- I2: `git diff --quiet dbd1a87 -- deny.toml rustfmt.toml crates/minos-core/clippy.toml rules .planning/research; echo $?` (expected: `0`)
- I3: `git diff dbd1a87 -- .cargo/config.toml` (expected: the added musl table only)
- I3b: `awk '/^\[target\.aarch64-unknown-linux-musl\]/,0' .cargo/config.toml | grep -c rustflags` (expected: `0`)
- I3c: `grep -n '^\[build\]' -A3 .cargo/config.toml` (the `[build]` line, unchanged)
- I4: `grep -n 'unsafe_code\|unwrap_used\|expect_used\|indexing_slicing' Cargo.toml` (expected: all four at `forbid`)
- I5: `grep -n 'msb_krun' Cargo.toml` (expected: `features = ["blk"]` only)

```text
### I1
diff --git a/Cargo.toml b/Cargo.toml
index f6a8f21..c0a4329 100644
--- a/Cargo.toml
+++ b/Cargo.toml
@@ -25,7 +25,9 @@ publish = false          # v1 publishes nothing; also makes clippy::cargo_common
 [profile.release]
 # Every binary parses hostile input (sandbox output, diffs, job specs). Integer overflow in a size cap,
 # a length-prefixed hash, or pico-dollar accumulation must panic, not wrap, without relying on each call site.
-# panic = "abort" for minos-guest (PID 1 has nothing to unwind into) belongs to the phase that gives it a body.
+# panic = "abort" cannot be set for minos-guest alone: Cargo rejects `panic` in a [profile.*.package.*]
+# override (02-SPIKE-EVIDENCE.md P-PANIC). Unwinding stays. If the guest's PID 1 panics, the kernel panics and
+# the VM powers off without sending a status frame, which the host classifies as Crashed.
 overflow-checks = true
 
 [workspace.lints.rust]
### I2
0
### I3
diff --git a/.cargo/config.toml b/.cargo/config.toml
index 051a4d3..ac00e89 100644
--- a/.cargo/config.toml
+++ b/.cargo/config.toml
@@ -8,3 +8,9 @@
 # does not apply to the installed package itself); run tool installs from elsewhere.
 [build]
 rustflags = ["-Funsafe_code", "-Fclippy::unwrap_used", "-Fclippy::expect_used", "-Fclippy::indexing_slicing"]
+
+# The static guest (/init) links with rust-lld, which ships with the pinned toolchain (02-CONTEXT.md D-03).
+# This table holds the linker key only. A flags key here would replace the [build] backstop above for this
+# target (01-GATE-EVIDENCE.md G-02-CLOSED).
+[target.aarch64-unknown-linux-musl]
+linker = "rust-lld"
### I3b
0
### I3c
9:[build]
10-rustflags = ["-Funsafe_code", "-Fclippy::unwrap_used", "-Fclippy::expect_used", "-Fclippy::indexing_slicing"]
11-
12-# The static guest (/init) links with rust-lld, which ships with the pinned toolchain (02-CONTEXT.md D-03).
### I4
34:unsafe_code = "forbid"
47:unwrap_used = "forbid"
48:expect_used = "forbid"
49:indexing_slicing = "forbid"
55:# rules/lint-unsafe-doc.md (moot while unsafe_code is forbid, but kept as defense in depth)
### I5
64:# cargo: msb_krun, proptest, and backhand pull duplicate versions we cannot unify; cargo-deny multiple-versions = "warn" reports them
97:msb_krun     = { version = "=0.1.41", default-features = false, features = ["blk"] }
```

Result: Cargo.toml differs from BASE only in the `[profile.release]` panic comment (P-PANIC); deny.toml, rustfmt.toml, the core clippy.toml, `rules/`, and `.planning/research/` are unchanged; `.cargo/config.toml` gained only the musl `linker` table and its `[build]` rustflags line is unchanged; the four workspace lints are `forbid`; `msb_krun` has `features = ["blk"]` only. With the empty no-unsafe grep above, no unsafe exception exists anywhere in the workspace (SPIKE-08).

## Spike re-run (Phase 2)

D-02: the spikes re-run from the final tree. Summary-only record: each run's `test result:` line (and failures, of which there were none). Setup, from the repository root, after the gate above (which relinks `minos-vmm`):

- `cargo build -p minos-guest --release --target aarch64-unknown-linux-musl` exit 0; `cargo build -p minos-vmm` exit 0; `cp target/debug/minos-vmm $S/minos-vmm-unsigned` (`$S` is the session scratch directory).
- SHA-256: guest `10e59803c53ce60bb53445f95b86e07652a5029c5601170823f1fe156e4b3a89` (equal to 02-01 G-GUEST); helper and its unsigned copy `63173a3cd5d9379e74c460ccac9a8b023e57bf7257c4ac3bf01fffb8547523b3`.
- Signed with the 02-06 E-4 argv, `codesign --force --sign - --options runtime --entitlements crates/minos-vmm/minos-vmm.entitlements target/debug/minos-vmm` (exit 0, `replacing existing signature`). `codesign --display --entitlements - target/debug/minos-vmm` (exit 0) lists `com.apple.security.cs.disable-library-validation` and `com.apple.security.hypervisor`, both `true`. The entitlement count was re-checked before the later runs (2 keys each time); no spike run relinks the helper.
- Environment (02-06 E-1): `MINOS_VMM=$(pwd)/target/debug/minos-vmm`, `MINOS_GUEST=$(pwd)/target/aarch64-unknown-linux-musl/release/minos-guest`, `MINOS_KRUNFW=$(realpath /opt/homebrew/lib/libkrunfw.5.dylib)` (SHA-256 `59764391005357bbdd5218f3fd522dd8ab63722cbc96144752c7bddc2c8e5217`, H-01), plus `MINOS_VMM_UNSIGNED=$S/minos-vmm-unsigned` for `spike_boot`, `MINOS_DOCKER=$(command -v docker)` for `spike_rootimg` (server 29.6.1, already running), and `MINOS_SPIKE_HOST_IP=$(ipconfig getifaddr en0)` for `spike_net` (value not recorded, as in 02-08).

Command: `cargo test -p minos-backend-krun --test spike_boot -- --ignored --nocapture --test-threads=1`

Exit code: 0

```text
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.61s
```

Command: `cargo test -p minos-backend-krun --test spike_rootimg -- --ignored --nocapture --test-threads=1`

Exit code: 0 (the image again hashed `d1e93ef0334243de484b036aec053eed241b77abc8176bd2e3e672aa2eb0a4ae`, `images_match` true, `tars_match` false, as in RI-03 and RI-04)

```text
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.01s
```

Command: `cargo test -p minos-backend-krun --test spike_exits -- --ignored --nocapture --test-threads=1`

Exit code: 0. The run prints three `test result:` lines: the two `1 passed; ... 11 filtered out` lines come from the `orphan_launcher` child runs that `k7_07_parent_gone` and `k7_07b_parent_gone_after_boot` start; the last line is the file's own result.

```text
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 11 filtered out; finished in 0.00s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 11 filtered out; finished in 0.33s
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.82s
```

Command: `cargo test -p minos-backend-krun --test spike_seatbelt -- --ignored --nocapture --test-threads=1`

Exit code: 0

```text
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.13s
```

Command: `cargo test -p minos-backend-krun --test spike_net -- --ignored --test-threads=1 --skip vsock_`

Exit code: 0

```text
test egress_fails_closed ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.73s
```

`vsock_stream_within_cap`, `vsock_stream_over_cap`, and `vsock_throughput_64mib` were intentionally not re-run: they encode the single-connection contract that the pinned `msb_krun_devices` 0.1.41 cannot meet, a recorded limit and not a regression (DIAG-01, S03-ANSWER).

After every run: `/usr/bin/pgrep -x minos-vmm; echo "pgrep exit $?"` printed `pgrep exit 1` (no helper left running).

Result: five files, five file-level `test result: ok.` lines. `spike_boot` 3 of 3, `spike_rootimg` 2 of 2, `spike_exits` 12 of 12, `spike_seatbelt` 3 of 3, and `spike_net` 1 of 1 run (`egress_fails_closed`; the three vsock-limit tests filtered out as above).
