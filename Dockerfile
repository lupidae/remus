# The image is a packaging step, not a build: it carries the exact static musl
# binaries the release ships, so a bug reported against the image and one
# reported against the tarball are the same bytes. Compiling here instead meant
# building Rust twice, the arm64 half under QEMU, for forty minutes a release.
#
# So this needs the binaries beside it, which `just image` and the release
# workflow both arrange. TARGETARCH is amd64 or arm64, set by buildx per platform.
FROM scratch

ARG TARGETARCH
COPY remus-${TARGETARCH} /remus

# No base image, no shell, nothing to prompt at: the CLI is the whole container.
ENTRYPOINT ["/remus"]
