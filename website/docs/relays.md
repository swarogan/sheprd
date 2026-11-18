---
title: Relays
blurb: Run your own relay, so phones reach your computer from anywhere.
status: experimental
platforms: macOS, Linux, Windows
---

# Relays

A relay joins your phone, or another Sheprd, to your computer when they have no direct path: different networks, no Tailscale, a hotel Wi-Fi in between. Everything that crosses it is end-to-end encrypted, so a relay sees only ciphertext, timing, and sizes; it can never read your chats or terminals, and it keeps nothing on disk.

Relays are optional. Without one, phones keep reaching your computer [directly or over Tailscale](remote-control.md).

## Sheprd's relays and yours

Sheprd lists its own relays at [sheprd.org/relays.json](https://sheprd.org/relays.json). That list is empty for now: **no Sheprd relays are available for now**, so to use a relay, run your own. One small instance serves all your computers and phones, and you can run as many as you like, in different regions.

Your computers will then use the relays you name, and only those: with a private relay, Sheprd's relays are never contacted.

## Use your relay

In Sheprd, open **Settings > Remote control > Relays**, type your relay's address, such as `https://relay.example.org`, and choose **Add**. The daemon restarts and each relay's row shows a dot and a line for how it stands: green while it is listening, red with the reason when it failed. `sheprd remote status` reports the same.

The same section lists Sheprd's own relays once there are some, each with an **Add** button. Sheprd takes that list only when it is signed, and says who validated it.

Or name them in the config file:

```toml
[remote]
relays = ["https://relay.example.org"]
```

To tell relays apart, give one a name, a description, and tags. Write it as a table instead of an address:

```toml
[remote]
relays = [
  "https://relay.example.org",
  { endpoint = "https://fra3.example.org", name = "fra3",
    description = "not for public use", tags = ["eu", "private"] },
]
```

Settings, `sheprd remote status`, and the iPhone's relay lists then show the name in place of the address, with the description and tags below it. Adding or removing a relay in Settings keeps the labels of the others. A name is at most 64 characters, a description 256, and a tag 32, with at most 8 tags. The label stays between you, your computer, and your phones: the relay never sees it.

A paired phone learns of a new relay the next time it reaches the computer directly, and uses it after that whenever it has no direct path.

On the iPhone, **Settings > Relays** lists the relays it keeps, each with a dot for whether it answers. The relays your computers named are filled in and kept. To reach a computer that moved to a relay while the phone was away, type that relay's address there and choose **Add Relay**: the phone then tries it for every computer it is paired with. A computer not paired yet is still paired nearby or on its network first.

The computer listens on a relay over QUIC, so name a relay on a droplet or your own host. A relay on DigitalOcean App Platform offers no QUIC, and shows as failed.

## Deploy on DigitalOcean in one click

[![Deploy to DigitalOcean](https://www.deploytodo.com/do-btn-blue.svg)](https://sheprd.org/relay/)

The button, and **Deploy on DigitalOcean** in **Settings > Remote control > Relays**, open [sheprd.org/relay](https://sheprd.org/relay/), which creates one small droplet in each region you choose, with a relay serving both QUIC and WebSockets:

1. In DigitalOcean, open **API > Tokens** and generate a token that expires soon. Give it **Custom Scopes**: `droplet` create and read, `ssh_key` read, and `tag` create; or **Full Access**.
2. Paste it on the page, choose one or more regions near you and your computers, and optionally a size, your own domain, and the SSH keys to install.
3. Choose **Deploy to DigitalOcean**. Keep the tab open: each relay's row shows its address, such as `https://203-0-113-7.sslip.io`, and turns to **ready** once the relay answers, a few minutes later.
4. Copy each address into **Settings > Remote control > Relays** in Sheprd, then delete the token in DigitalOcean.

The token stays in the page: it is sent only to DigitalOcean's API, never stored, and forgotten when the tab closes. The page builds each droplet's setup from [`deploy/relay`](https://github.com/sheprd-labs/sheprd/tree/main/deploy/relay) on `main`, exactly as `create-droplet.sh` below does.

Rather not paste a token? Choose **Show the user data instead**, copy it, and create the droplet yourself in DigitalOcean: **Create > Droplets**, a region, **Ubuntu 24.04**, a size, then under **Advanced options** check **Add initialization scripts** and paste it.

One `s-1vcpu-1gb` droplet is plenty for a household; DigitalOcean bills it monthly, see its [droplet pricing](https://www.digitalocean.com/pricing/droplets).

## Deploy on DigitalOcean from the command line

With [`doctl`](https://docs.digitalocean.com/reference/doctl/) signed in (`doctl auth init`), from a checkout of the repository:

```sh
deploy/relay/create-droplet.sh fra1            # one region
deploy/relay/create-droplet.sh nyc3 fra1 sgp1  # one droplet in each
```

It ends by printing each relay's address, ready to add in Settings. These change what it creates:

| Variable | What it does |
| --- | --- |
| `RELAY_SIZE` | The droplet size; `s-1vcpu-1gb` by default |
| `RELAY_SSH_KEYS` | Key ids or fingerprints from `doctl compute ssh-key list`, comma-separated, to reach the droplets over SSH; without them, use DigitalOcean's web console |
| `RELAY_DOMAIN_SUFFIX` | Your own names: `relay.example.org` serves the relay in `fra1` at `fra1.relay.example.org` |

## What a relay droplet runs

Each droplet runs Ubuntu 24.04, is tagged `sheprd-relay`, and opens only SSH, HTTP, HTTPS, and QUIC (UDP 443). At first boot it installs Docker and starts two containers: the relay, serving QUIC on UDP 443 with its own pinned certificate, and Caddy, serving HTTPS with a Let's Encrypt certificate for the WebSocket fallback, the relay's `/v1/info`, and its dashboard.

Without your own domain, the relay's name is its address with dashes, at [sslip.io](https://sslip.io), which resolves to the droplet itself, so the certificate comes with no DNS to set up.

It runs only a relay image signed by this repository's relay image workflow on `main`, by the exact digest that signature covers. When the check fails, nothing starts, or, on an update, the relay already running keeps running.

## Your own domain

Give the page a domain, or set `RELAY_DOMAIN_SUFFIX`, and each relay is named after its region under it, such as `fra1.relay.example.org`. Once the droplet has an address, add an `A` record for that name pointing at it, at your DNS provider or with `doctl compute domain records create`. Caddy gets the certificate on the first request after the name resolves; until then the relay's row in Sheprd stays red.

## The dashboard and the droplet

The relay's dashboard, at `https://ADDRESS/dashboard`, shows sessions, listeners, throughput, and refusals, read-only. Its access token is in `/root/sheprd-relay-login.txt` on the droplet:

```sh
doctl compute ssh sheprd-relay-fra1 --ssh-command 'cat /root/sheprd-relay-login.txt'
```

Without an SSH key, open the droplet in DigitalOcean and choose **Access > Launch Droplet Console**. `https://ADDRESS/healthz` answers `ok`, for an uptime check.

## Update or remove a relay

To move a droplet to the newest signed relay, run its start script again; it verifies the image before changing anything. Open sessions drop, and computers and phones reconnect within seconds:

```sh
doctl compute ssh sheprd-relay-fra1 --ssh-command /opt/sheprd-relay/start.sh
```

To remove a relay, take it out of **Settings > Remote control > Relays** first, then delete the droplet in DigitalOcean, or:

```sh
doctl compute droplet delete sheprd-relay-fra1
```

## When a relay does not answer

- **Still installing.** The first boot takes a few minutes. Its log is `/var/log/cloud-init-output.log` on the droplet; a line starting `sheprd-relay:` says why the relay did not start.
- **Red in Settings with a certificate error.** With your own domain, the name must resolve to the droplet before Caddy can get its certificate. Check with `dig +short fra1.relay.example.org`.
- **The relay's own log.** On the droplet: `cd /opt/sheprd-relay && docker compose --env-file compose.env logs relay`. It holds no addresses, routes, or payloads.
- **Phones connect but the computer does not listen.** The computer needs QUIC, on UDP 443. A network that blocks outgoing UDP keeps the computer off the relay, though phones there still reach it over WebSockets.

## DigitalOcean App Platform (WebSockets only)

[![Deploy to DigitalOcean](https://www.deploytodo.com/do-btn-blue.svg)](https://cloud.digitalocean.com/apps/new?repo=https://github.com/sheprd-labs/sheprd/tree/main)

This button creates an App Platform app from the published image, with no server to manage. App Platform serves HTTPS but not UDP, so this relay offers only its WebSocket transport, and **computers cannot listen on it yet**: they listen over QUIC, and show it as failed. Use a droplet above for now.

Its address is the app's domain, such as `https://sheprd-relay-abc12.ondigitalocean.app`. For its dashboard, add a `RELAY_DASHBOARD_TOKEN` of 24 or more characters as an encrypted environment variable in the app's settings. App Platform cannot check the image's signature: it runs whatever `:latest` points at when it deploys.

## Any host with Docker

```sh
cd deploy/relay
cp env.example .env
mkdir -p bans && touch bans/list
RELAY_DOMAIN=relay.example.org docker compose up -d
```

Point `relay.example.org` at the host first: Caddy gets its certificate on the first request. The image, `ghcr.io/sheprd-labs/sheprd-relay`, runs on amd64 and arm64.

## Keeping it safe

A relay needs no accounts and no setup to stay safe:

- Only devices a computer has paired can reach that computer; anyone else hears that it is offline.
- Connections, sessions, and bandwidth are limited per address and in total. Every limit is a `RELAY_*` setting with a safe default.
- Addresses that misbehave are banned automatically, for ten minutes, then an hour, then a day. To ban one yourself, add it to `bans/list`; the relay rereads it every 30 seconds.
- `RELAY_LISTEN_TOKEN` keeps a relay to your own computers.

The full list of settings is in [`deploy/relay/README.md`](https://github.com/sheprd-labs/sheprd/blob/main/deploy/relay/README.md), and the design in [`docs/relay-plan.md`](https://github.com/sheprd-labs/sheprd/blob/main/docs/relay-plan.md).
