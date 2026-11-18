#!/usr/bin/env bash
# Copies a push relay's secrets, made by make-secrets.sh, to its droplet over
# SSH and (re)starts the relay there:
#
#   deploy/push-relay/install-secrets.sh ~/push-sandbox root@203.0.113.7
#
# They replace the droplet's secrets as a whole, once its start.sh has
# checked them: it refuses a missing file, an APNs key for the other
# environment, or any key the relay cannot load, and keeps the old secrets
# until the relay answers with the new ones.
set -euo pipefail

[ "$#" -eq 2 ] || { echo "usage: $0 DIR USER@HOST" >&2; exit 2; }
dir=$1 host=$2
files=(apns/config.json apns/AuthKey.p8 relay-key.pem credential-secret)
for file in "${files[@]}"; do
  [ -f "$dir/$file" ] || { echo "no $dir/$file; make them with make-secrets.sh" >&2; exit 2; }
done
# The key before a rotation, so handles sealed to it still open.
if [ -f "$dir/relay-key-previous.pem" ]; then
  files+=(relay-key-previous.pem)
fi

# start.sh unpacks them into a fresh directory under its lock, so a dropped
# connection or a second install leaves the old secrets in place, and swaps
# them in only once the relay's own --check accepts them. macOS tar adds no
# AppleDouble files with COPYFILE_DISABLE.
COPYFILE_DISABLE=1 tar -C "$dir" -cf - "${files[@]}" |
  ssh "$host" NEW_SECRETS=1 /opt/sheprd-push-relay/start.sh
