# The push relay as a static binary on a distroless image: no shell, no
# package manager, a non-root user. Build from the repository root, which the
# workspace's members and patches need:
#
#   docker buildx build -f crates/sheprd-push-relay/Dockerfile \
#     --platform linux/amd64,linux/arm64 -t sheprd-push-relay .
#
# Both base images are pinned by digest, the tag kept for reading, so a
# moved tag cannot change what is built. Dependabot proposes new digests
# for the distroless image; move the Rust one by hand with the toolchain,
# in step with crates/sheprd-relay/Dockerfile.

ARG RUST_IMAGE=rust:1.99.0-alpine3.22@sha256:c3a5ad77ff2e5ec99fffaf62cb518e61a9dcbd9e1d7526d281a380db0be3bb3b

FROM ${RUST_IMAGE} AS build
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p sheprd-push-relay \
 && cp target/release/sheprd-push-relay /sheprd-push-relay

# The static image carries the CA certificates the APNs connection checks.
FROM gcr.io/distroless/static-debian12:nonroot@sha256:afa5c872c891853ca7fcf1f12c3edb23f7eeef36189728842dd51042ff57f7ab
COPY --from=build /sheprd-push-relay /usr/local/bin/sheprd-push-relay
EXPOSE 8080
USER nonroot:nonroot
ENTRYPOINT ["/usr/local/bin/sheprd-push-relay"]
# Inside a container the relay listens on every interface; the host decides
# what is published. The secrets are a directory mounted read-only, owned by
# nonroot (65532) and private to it: the relay refuses a key others can read.
CMD ["--secrets", "/secrets", "--listen", "0.0.0.0:8080"]
