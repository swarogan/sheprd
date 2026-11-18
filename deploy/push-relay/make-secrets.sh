#!/usr/bin/env bash
# Makes one push relay's secrets in a new directory, on your own machine:
#
#   deploy/push-relay/make-secrets.sh sandbox ~/Downloads/AuthKey_XYZ9876543.p8 ABCDE12345 XYZ9876543 ~/push-sandbox
#
# Arguments: production or sandbox, the APNs key Apple gave you, your team
# id, the key's id, and a directory that does not exist yet. It writes:
#
#   apns/AuthKey.p8     the APNs key, copied
#   apns/config.json    team, key, topic, and environment
#   relay-key.pem       the relay's P-256 key, which phones seal handles to
#   credential-secret   32 random bytes as hex, keying device credentials
#
# Keep the directory offline (a password manager or an encrypted disk):
# a rebuilt droplet needs the same relay-key.pem and credential-secret, or
# every handle phones have made stops working. It prints the relay's public
# key, which the offline root key signs (docs/apns-relay-plan.md).
set -euo pipefail

usage="usage: $0 production|sandbox AUTHKEY.p8 TEAM_ID KEY_ID DIR"
[ "$#" -eq 5 ] || { echo "$usage" >&2; exit 2; }
environment=$1 key=$2 team=$3 key_id=$4 dir=$5
case $environment in
  production) sandbox=false ;;
  sandbox) sandbox=true ;;
  *) echo "$usage" >&2; exit 2 ;;
esac
apple_id='^[A-Z0-9]{10}$'
[[ $team =~ $apple_id ]] || { echo "TEAM_ID is ten uppercase letters or digits" >&2; exit 2; }
[[ $key_id =~ $apple_id ]] || { echo "KEY_ID is ten uppercase letters or digits" >&2; exit 2; }
[ -f "$key" ] || { echo "no APNs key at $key" >&2; exit 2; }
# Refuses an existing directory rather than replacing keys phones rely on.
[ ! -e "$dir" ] || { echo "$dir exists; choose a new directory" >&2; exit 1; }

umask 077
mkdir -p "$dir/apns"
cp "$key" "$dir/apns/AuthKey.p8"
chmod 600 "$dir/apns/AuthKey.p8"
printf '{"team_id":"%s","key_id":"%s","topic":"so.pen.herdr.remote","sandbox":%s}\n' \
  "$team" "$key_id" "$sandbox" > "$dir/apns/config.json"
openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out "$dir/relay-key.pem" 2>/dev/null
openssl rand -hex 32 > "$dir/credential-secret"

echo "Wrote the $environment relay's secrets to $dir. Its public key:" >&2
openssl pkey -in "$dir/relay-key.pem" -pubout
