#!/usr/bin/env bash
# Creates one relay droplet per region with doctl, each zero-config:
#
#   deploy/relay/create-droplet.sh nyc3 fra1 sgp1
#
# Needs `doctl auth init` first. Optional environment:
#   RELAY_SIZE     droplet size (default s-1vcpu-1gb)
#   RELAY_SSH_KEYS comma-separated SSH key ids or fingerprints from
#                  `doctl compute ssh-key list` (default: none, console only)
#   RELAY_DOMAIN_SUFFIX  e.g. relay.example.org gives nyc3.relay.example.org;
#                  point those names at the droplets yourself. Without it,
#                  each relay is served at IP-WITH-DASHES.sslip.io.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
if [ "$#" -eq 0 ]; then
  echo "usage: $0 REGION [REGION...]" >&2
  exit 2
fi
command -v doctl >/dev/null || { echo "doctl is not installed" >&2; exit 1; }

size=${RELAY_SIZE:-s-1vcpu-1gb}
compose=$(base64 < "$here/compose.yml" | tr -d '\n')
caddyfile=$(base64 < "$here/Caddyfile" | tr -d '\n')
data=$(mktemp)
trap 'rm -f "$data"' EXIT

# The relay droplets there already, by id: a region created again has two
# droplets of one name, and only this run's are named at the end.
before=$(doctl compute droplet list --tag-name sheprd-relay --format ID --no-header)

for region in "$@"; do
  domain=""
  if [ -n "${RELAY_DOMAIN_SUFFIX:-}" ]; then
    domain="$region.$RELAY_DOMAIN_SUFFIX"
  fi
  sed -e "s|__COMPOSE_B64__|$compose|" \
      -e "s|__CADDYFILE_B64__|$caddyfile|" \
      -e "s|__RELAY_DOMAIN__|$domain|" \
      "$here/cloud-init.yml" > "$data"
  keys=()
  if [ -n "${RELAY_SSH_KEYS:-}" ]; then
    keys=(--ssh-keys "$RELAY_SSH_KEYS")
  fi
  doctl compute droplet create "sheprd-relay-$region" \
    --region "$region" \
    --size "$size" \
    --image ubuntu-24-04-x64 \
    --enable-monitoring \
    --tag-name sheprd-relay \
    --user-data-file "$data" \
    "${keys[@]}" \
    --wait \
    --format Name,PublicIPv4,Region
done

cat <<'NOTE'

Each droplet starts its relay within a few minutes. The dashboard login is in
/root/sheprd-relay-login.txt on each droplet (`doctl compute ssh NAME`, or the
web console).

Add each relay in Sheprd, under Settings > Remote control > Relays:
NOTE

# The addresses, as the droplets serve them: the certificate is for the name
# with dashes, so the IP with dots would be refused.
if [ -n "${RELAY_DOMAIN_SUFFIX:-}" ]; then
  for region in "$@"; do
    echo "  https://$region.$RELAY_DOMAIN_SUFFIX"
  done
  echo "Point each name at its droplet's address above first."
else
  listed=$(doctl compute droplet list --tag-name sheprd-relay \
    --format ID,Name,PublicIPv4 --no-header) || listed=""
  # An older droplet of the same name may serve another name: skip it.
  addresses=$(awk -v before="$before" '
    BEGIN { split(before, ids, "\n"); for (i in ids) old[ids[i]] = 1 }
    $3 != "" && !($1 in old) { print $3 }' <<<"$listed")
  if [ -z "$addresses" ]; then
    echo "  https://IP-WITH-DASHES.sslip.io (the new droplets' addresses could not be read)"
  else
    while read -r address; do
      echo "  https://${address//./-}.sslip.io"
    done <<<"$addresses"
  fi
fi
