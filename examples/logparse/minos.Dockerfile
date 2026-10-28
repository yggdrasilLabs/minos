# The build environment for the logparse sample.
#
# Minos runs every stage (build, run, judge) as uid 64000 with exactly these four variables:
#   PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
#   HOME=/scratch/home
#   TMPDIR=/tmp
#   LANG=C
# ENV lines in this file never reach a stage, and USER is ignored. A toolchain must therefore be reachable on that
# PATH and readable by any user.
#
# The base image is rust:1.97.1-slim-trixie, pinned by its index digest; buildx resolves the linux/arm64 image
# from it. Minos refuses a tag beside the digest, because the digest must be the only identity of the image.
FROM rust@sha256:8e8cf8f7fd54a2d23d5a743b3a03f56e26b6c774276c33fa0595111704ebb15c

# The image's cargo and rustc are rustup proxies in /usr/local/cargo/bin, which is not on the stage PATH, and rustup
# would look for its toolchains under HOME. Link the toolchain's own binaries into /usr/local/bin instead, so no
# rustup proxy ever runs in a stage. rustc finds its sysroot through the resolved link.
RUN ln -s /usr/local/rustup/toolchains/1.97.1-aarch64-unknown-linux-gnu/bin/cargo /usr/local/bin/cargo \
 && ln -s /usr/local/rustup/toolchains/1.97.1-aarch64-unknown-linux-gnu/bin/rustc /usr/local/bin/rustc
