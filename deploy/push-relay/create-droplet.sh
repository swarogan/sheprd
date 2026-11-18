#!/usr/bin/env bash
# Creates the push relay droplet for one APNs environment with doctl, and a
# reserved IP for it, so its DNS name outlives a rebuilt droplet:
#
#   PUSH_RELAY_SSH_KEYS=12345678 deploy/push-relay/create-droplet.sh sandbox fra1
#
# Needs `doctl auth init` first. Arguments: production or sandbox, then an
# optional region (default nyc3). Environment:
#   PUSH_RELAY_SSH_KEYS  required: comma-separated SSH key ids or fingerprints
#                        from `doctl compute ssh-key list`. install-secrets.sh
#                        copies the secrets over SSH, since user data is no
#                        place for them.
#   PUSH_RELAY_SIZE      droplet size (default s-1vcpu-1gb)
#   PUSH_RELAY_RESERVED_IP  an existing reserved IP to move to the new droplet,
#                        as recovery does, instead of making a new one; DNS
#                        already points at it.
#   PUSH_RELAY_DOMAIN    the name to serve (default push.sheprd.org, or
#                        push-sandbox.sheprd.org): the daemon reaches only
#                        those two, so change it only for a trial.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
environment=${1:-}
region=${2:-nyc3}
case $environment in
  production) default_domain=push.sheprd.org ;;
  sandbox) default_domain=push-sandbox.sheprd.org ;;
  *) echo "usage: $0 production|sandbox [REGION]" >&2; exit 2 ;;
esac
domain=${PUSH_RELAY_DOMAIN:-$default_domain}
command -v doctl >/dev/null || { echo "doctl is not installed" >&2; exit 1; }
if [ -z "${PUSH_RELAY_SSH_KEYS:-}" ]; then
  echo "set PUSH_RELAY_SSH_KEYS to your key ids from \`doctl compute ssh-key list\`: the secrets go over SSH" >&2
  exit 2
fi

name="sheprd-push-relay-$environment"
if doctl compute droplet list --tag-name sheprd-push-relay --format Name --no-header | grep -qx "$name"; then
  echo "$name exists already; delete it first, or update it with its start.sh" >&2
  exit 1
fi

# A reserved IP moves only within its region: check before making a droplet
# it could not reach.
if [ -n "${PUSH_RELAY_RESERVED_IP:-}" ]; then
  reserved_region=$(doctl compute reserved-ip get "$PUSH_RELAY_RESERVED_IP" --format Region --no-header)
  if [ "$reserved_region" != "$region" ]; then
    echo "$PUSH_RELAY_RESERVED_IP is in ${reserved_region:-no region}; create the droplet there: $0 $environment $reserved_region" >&2
    exit 2
  fi
fi

data=$(mktemp)
trap 'rm -f "$data"' EXIT
compose=$(base64 < "$here/compose.yml" | tr -d '\n')
caddyfile=$(base64 < "$here/Caddyfile" | tr -d '\n')
sed -e "s|__COMPOSE_B64__|$compose|" \
    -e "s|__CADDYFILE_B64__|$caddyfile|" \
    -e "s|__PUSH_RELAY_DOMAIN__|$domain|" \
    -e "s|__PUSH_RELAY_ENVIRONMENT__|$environment|" \
    "$here/cloud-init.yml" > "$data"

id=$(doctl compute droplet create "$name" \
  --region "$region" \
  --size "${PUSH_RELAY_SIZE:-s-1vcpu-1gb}" \
  --image ubuntu-24-04-x64 \
  --enable-monitoring \
  --tag-name sheprd-push-relay \
  --ssh-keys "$PUSH_RELAY_SSH_KEYS" \
  --user-data-file "$data" \
  --wait \
  --format ID --no-header)
if [ -n "${PUSH_RELAY_RESERVED_IP:-}" ]; then
  address=$PUSH_RELAY_RESERVED_IP
  doctl compute reserved-ip-action assign "$address" "$id" >/dev/null
  dns="$domain already points at $address, now this droplet's; check it does."
else
  address=$(doctl compute reserved-ip create --droplet-id "$id" --format IP --no-header)
  dns="Point $domain at it: an A record for $address."
fi

cat <<NOTE

$name is up in $region, at the reserved IP $address.

1. $dns
2. Make its secrets, once, on your own machine, and keep a copy offline:
     deploy/push-relay/make-secrets.sh $environment AuthKey_KEYID.p8 TEAM_ID KEY_ID DIR
3. Copy them over; this starts the relay:
     deploy/push-relay/install-secrets.sh DIR root@$address
4. Check it: curl https://$domain/healthz
NOTE
