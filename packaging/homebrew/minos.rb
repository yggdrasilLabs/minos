# The Homebrew formula for Minos, as a template. The release workflow (.github/workflows/release.yml) fills
# @VERSION@ and @SHA256@ from the pushed tag and writes the result to Formula/minos.rb in yggdrasilLabs/homebrew-tap.
# Edit this file, never the copy in the tap.
class Minos < Formula
  desc "Lab for autonomous experiment loops in sandboxed VMs"
  homepage "https://github.com/yggdrasilLabs/minos"
  url "https://github.com/yggdrasilLabs/minos/archive/refs/tags/v@VERSION@.tar.gz"
  sha256 "@SHA256@"
  license "MIT"
  head "https://github.com/yggdrasilLabs/minos.git", branch: "main"

  # Minos boots VMs through Hypervisor.framework and libkrun, which the libkrun tap supports on arm64 only.
  depends_on "rustup" => :build
  depends_on arch: :arm64
  depends_on "libkrun/krun/libkrunfw"
  depends_on :macos

  def install
    # The repository pins its toolchain in rust-toolchain.toml: Rust 1.97.1 with the aarch64-unknown-linux-musl
    # target, which the static guest binary needs. Homebrew's own rust formula ships no musl target, so rustup
    # installs the pinned toolchain into the build directory instead.
    ENV["RUSTUP_HOME"] = buildpath/"rustup-home"
    ENV["CARGO_HOME"] = buildpath/"cargo-home"
    ENV.prepend_path "PATH", formula_opt_bin("rustup")
    system "rustup", "toolchain", "install"

    system "cargo", "build", "--release", "--locked", "--jobs", ENV.make_jobs.to_s,
           "-p", "minos-cli", "-p", "minos-vmm"
    system "cargo", "build", "--release", "--locked", "--jobs", ENV.make_jobs.to_s,
           "-p", "minos-guest", "--target", "aarch64-unknown-linux-musl"

    bin.install "target/release/minos", "target/release/minos-vmm"
    bin.install "target/aarch64-unknown-linux-musl/release/minos-guest"

    # `minos host setup` signs the helper ad hoc with the hardened runtime and the committed entitlements
    # (the hypervisor, and loading the Homebrew libkrunfw), then verifies both keys. It is the same code path a
    # source checkout uses, so the installed helper carries exactly the reviewed entitlements.
    system bin/"minos", "host", "setup"
  end

  def caveats
    <<~EOS
      Run `minos host doctor` once. It must end with `result: every check passed`.

      Minos loads libkrunfw from its versioned Cellar path and checks the file against a pinned SHA-256, so it
      works only with the libkrunfw release it was built against (5.6.2). Pin it so `brew upgrade` does not
      move it until a Minos release does:
        brew pin libkrunfw

      `minos env build` needs Docker with buildx (for example Docker Desktop), and the `single_shot` proposer
      needs your own logged-in `claude` CLI. See the README.
    EOS
  end

  test do
    assert_match "minos #{version}", shell_output("#{bin}/minos --version")
    # The helper keeps both entitlements after installation.
    entitlements = shell_output("codesign --display --entitlements - --xml #{bin}/minos-vmm 2>&1")
    assert_match "com.apple.security.hypervisor", entitlements
    assert_match "com.apple.security.cs.disable-library-validation", entitlements
    # The guest is the static Linux binary, not a Mach-O.
    assert_match "ELF", shell_output("file #{bin}/minos-guest")
  end
end
