#!/usr/bin/env bash
# Builds the push relay's container image for one architecture, checks that
# it starts on throwaway secrets and answers, and with --push publishes it
# by digest and prints the digest. CI runs it once per architecture on
# native runners; it runs the same way locally:
#
#   scripts/push-relay-image.sh amd64                          # build and check
#   scripts/push-relay-image.sh arm64 ghcr.io/o/sheprd-push-relay --push
#
# A registry IMAGE also turns on its layer cache (IMAGE:buildcache-ARCH).
set -euo pipefail

arch=${1:?usage: push-relay-image.sh amd64|arm64 [IMAGE] [--push]}
image=${2:-sheprd-push-relay}
push=${3:-}
case $arch in amd64 | arm64) ;; *) echo "unknown architecture: $arch" >&2; exit 2 ;; esac

build=(--platform "linux/$arch" -f crates/sheprd-push-relay/Dockerfile --provenance=false)
# A mirror of the Rust base image, when Docker Hub is out of reach.
if [[ -n ${RUST_IMAGE:-} ]]; then
  build+=(--build-arg "RUST_IMAGE=$RUST_IMAGE")
fi
if [[ $image == */* ]]; then
  cache="type=registry,ref=$image:buildcache-$arch"
  build+=(--cache-from "$cache")
  if [[ $push == --push ]]; then
    build+=(--cache-to "$cache,mode=max")
  fi
fi

check="$image:check-$arch"
docker buildx build "${build[@]}" --load -t "$check" . >&2

docker run --rm "$check" --help >/dev/null

# Throwaway secrets in the layout deploy/push-relay/make-secrets.sh writes.
# Starting reads them all but reaches no one, so a made-up APNs key will do.
secrets=$(mktemp -d)
container=""
cleanup() {
  [[ -n $container ]] && docker rm -f "$container" >/dev/null
  rm -rf "$secrets"
}
trap cleanup EXIT
(
  umask 077
  mkdir "$secrets/apns"
  openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out "$secrets/apns/AuthKey.p8"
  openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out "$secrets/relay-key.pem"
  openssl rand -hex 32 > "$secrets/credential-secret"
  printf '{"team_id":"ABCDE12345","key_id":"XYZ9876543","topic":"so.pen.herdr.remote","sandbox":true}\n' \
    > "$secrets/apns/config.json"
)
# The image runs as the deploy files run it: read-only, no capabilities, and
# as the owner of its secrets (here this user, as on a droplet 65532).
container=$(docker run -d --read-only --cap-drop ALL --security-opt no-new-privileges:true \
  --user "$(id -u):$(id -g)" -v "$secrets:/secrets:ro" -p 127.0.0.1::8080 "$check")
port=$(docker port "$container" 8080/tcp | head -n 1 | sed 's/.*://')
for _ in $(seq 1 30); do
  curl -fsS "http://127.0.0.1:$port/healthz" >/dev/null 2>&1 && break
  sleep 1
done
if [[ $(curl -fsS "http://127.0.0.1:$port/healthz") != ok ]]; then
  docker logs "$container" >&2
  echo "$arch image did not answer /healthz" >&2
  exit 1
fi
echo "$arch image starts and answers" >&2

if [[ $push == --push ]]; then
  metadata=$(mktemp)
  # Every layer is cached from the build above, so this only pushes.
  docker buildx build "${build[@]}" --metadata-file "$metadata" \
    --output "type=image,name=$image,push-by-digest=true,name-canonical=true,push=true" . >&2
  jq -r '."containerimage.digest"' "$metadata"
fi
