# Running the push relay

`sheprd-push-relay` forwards sealed pushes from each user's
`sheprd-remote-control` daemon to Apple, so the TestFlight and App Store app
can get Live Activities and alerts without the APNs key leaving our hands.
`docs/apns-relay-plan.md` has the design.

One relay serves one APNs environment, and the daemon knows each by a name
compiled into it (`crates/sheprd-apns/src/relay.rs`), so there are exactly
two:

| Environment | Name | For |
| --- | --- | --- |
| `production` | `push.sheprd.org` | TestFlight and App Store builds |
| `sandbox` | `push-sandbox.sheprd.org` | Builds from Xcode |

Until App Attest and `POST /v1/attest` exist (plan, phase 2), nothing issues
the credentials handles carry, so a relay answers `/healthz` but has no phone
to push to. Bringing up the sandbox relay now still rehearses the deploy and
the recovery below.

## The image

CI (`.github/workflows/push-relay-image.yml`) builds the image natively for
amd64 and arm64, checks that each starts on throwaway secrets and answers,
and on every change to `main` publishes one multi-architecture image, signed
with cosign and with build provenance:

```text
ghcr.io/sheprd-labs/sheprd-push-relay:latest
ghcr.io/sheprd-labs/sheprd-push-relay:sha-COMMIT
```

Before the first deploy, make the package public once (GitHub > the
organization > Packages > sheprd-push-relay > Package settings > Change
visibility), or droplets cannot pull it without credentials.

To check an image by hand, verify its signature against the workflow that
signs it, and deploy the digest it prints, never the tag:

```sh
cosign verify ghcr.io/sheprd-labs/sheprd-push-relay:latest \
  --certificate-identity https://github.com/sheprd-labs/sheprd/.github/workflows/push-relay-image.yml@refs/heads/main \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  | jq -r '.[0].critical.image."docker-manifest-digest"'
```

`scripts/push-relay-image.sh amd64` builds and checks it locally, as CI does.
The pinned base images, Caddy, BuildKit, and cosign follow
`deploy/relay/README.md` ("Pinned images"); move them together.

## Secrets

The relay reads one directory, mounted read-only, and refuses any key file
others can read:

| File | What |
| --- | --- |
| `apns/AuthKey.p8` | The APNs key from the Apple Developer site, for this environment |
| `apns/config.json` | `team_id`, `key_id`, `topic` (`so.pen.herdr.remote`), and `sandbox` |
| `relay-key.pem` | The relay's P-256 key (PKCS#8 PEM); phones seal handles to its public half |
| `relay-key-previous.pem` | Optional: the key before a rotation, so older handles still open |
| `credential-secret` | 32 random bytes as hex, keying the device credentials |

Make them on your own machine, once per environment:

```sh
deploy/push-relay/make-secrets.sh sandbox ~/Downloads/AuthKey_XYZ9876543.p8 ABCDE12345 XYZ9876543 ~/sheprd-push-sandbox
```

It refuses a directory that exists, and prints the relay's public key, which
the offline root key signs once attestation is built.

**Keep that directory offline**, in a password manager or on an encrypted
disk. `relay-key.pem` and `credential-secret` must survive the droplet: a
relay with new ones answers `refused` to every handle phones made, and each
phone repairs itself only when it next opens. None of them go in user data,
which the droplet's metadata service and DigitalOcean's API both hand out.

## DigitalOcean

```sh
doctl auth init
doctl compute ssh-key list                    # your key's id
PUSH_RELAY_SSH_KEYS=12345678 deploy/push-relay/create-droplet.sh sandbox fra1
```

It makes one `s-1vcpu-1gb` droplet, `sheprd-push-relay-sandbox`, tagged
`sheprd-push-relay`, with only ports 22, 80, and 443 open, and a reserved IP
for it, so the name keeps pointing at a rebuilt droplet. It refuses to make
a second droplet for an environment. Then:

1. Add an A record for `push-sandbox.sheprd.org` to the reserved IP it
   printed. Caddy keeps asking Let's Encrypt for a certificate until the name
   resolves.
2. Copy the secrets over, which starts the relay:

   ```sh
   deploy/push-relay/install-secrets.sh ~/sheprd-push-sandbox root@RESERVED_IP
   ```

3. `curl https://push-sandbox.sheprd.org/healthz` answers `ok`.

Do the same with `production` for `push.sheprd.org`, with the production APNs
key and its own secrets.

Adding a DigitalOcean Cloud Firewall in front is worth it: inbound TCP 80 and
443, and 22 from your own address only. Outbound needs DNS, APNs
(`17.0.0.0/8`, TCP 443), Let's Encrypt, GHCR, and Ubuntu's mirrors.

### What a droplet runs

`/opt/sheprd-push-relay/start.sh`, which the user data writes. One run at a
time holds its lock, from staging new secrets to the relay answering; a
second run while one is going refuses to start.

1. It checks the secrets: every file is there, and `apns/config.json` is for
   the environment this droplet serves. New secrets from
   `install-secrets.sh` arrive on stdin and are unpacked into `secrets.new`.
2. Like the relay's droplets, it installs cosign by pinned SHA-256, verifies
   `ghcr.io/sheprd-labs/sheprd-push-relay:latest` against
   `push-relay-image.yml` on `main`, and writes the digest the signature
   covers to `compose.env`. A missing or foreign signature leaves the running
   relay alone.
3. New secrets then go through that image's `sheprd-push-relay --check`,
   which loads them exactly as the relay does, so an empty or malformed key
   is refused here. Refused ones are deleted and nothing else changes.
   Accepted ones become `secrets`, and the old ones wait in `secrets.old`.
4. It gives the secrets and `data/` to the image's user (65532), private to
   it, recreates the relay container, which reads its secrets only as it
   starts, and makes sure Caddy runs.
5. It asks the relay for `/healthz` from Caddy's container for up to 30
   seconds. With an answer, `secrets.old` goes; without one, the old secrets
   come back and the relay is recreated on them.

At first boot it stops at step 1, with `no secrets/apns/config.json` in
`/var/log/cloud-init-output.log`, until `install-secrets.sh` runs it.

The relay keeps revocations in SQLite at `/opt/sheprd-push-relay/data`, so
they survive restarts (`--store`, plan "Storage"). Nothing there is useful
to a thief: no tokens, payloads, or secrets.

## Updating

```sh
ssh root@RESERVED_IP /opt/sheprd-push-relay/start.sh
```

It verifies `:latest` and moves to its digest. To run a digest you verified
yourself, set `PUSH_RELAY_IMAGE=ghcr.io/sheprd-labs/sheprd-push-relay@sha256:…`
in front. Run compose there only with `--env-file compose.env`, which holds
the digest last verified.

A restart costs nothing a push depends on: each push carries its own handle,
so the next one works at once.

## Monitoring

- `https://DOMAIN/healthz` answers `ok`, for an uptime check.
- `docker compose --env-file compose.env logs relay` on the droplet. Logs
  hold no tokens, payloads, or addresses.
- Counters are not built yet (plan, phase 2).

## Recovery

If the host, the APNs key, or the relay key may be in someone else's hands.
Try it once on the sandbox relay before production is offered to anyone.

1. Revoke the APNs key in the Apple Developer site (Certificates, Identifiers
   & Profiles > Keys), and make a new one for the same environment.
2. Make new secrets with `make-secrets.sh` into a new directory.
3. Delete the droplet, and make a clean one in the same region, keeping its
   reserved IP so DNS needs no change:
   `PUSH_RELAY_RESERVED_IP=IP PUSH_RELAY_SSH_KEYS=… deploy/push-relay/create-droplet.sh ENVIRONMENT REGION`.
4. `install-secrets.sh` the new secrets.
5. Sign the new relay key with the offline root key, and publish the
   signature where phones fetch it (with attestation, not built yet).
6. Phones pick up the new key at their next attest. Old handles stop
   working, and daemons drop them as stale.

The offline root key never touches the server or CI.
