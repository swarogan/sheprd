// sheprd.org/relay: creates relay droplets on DigitalOcean from the browser.
//
// The user data is deploy/relay/cloud-init.yml with compose.yml and the
// Caddyfile inlined, exactly as deploy/relay/create-droplet.sh renders it
// (scripts/test_relay_deploy.py checks the two agree). It is read from the
// repository's main branch, so this page and the script deploy the same
// relay. The DigitalOcean token stays in this page's memory: it is sent only
// to api.digitalocean.com, never stored, and gone when the tab closes.
'use strict';

const SOURCE = 'https://raw.githubusercontent.com/sheprd-labs/sheprd/main/deploy/relay/';
const API = 'https://api.digitalocean.com/v2';
const IMAGE = 'ubuntu-24-04-x64';
const TAG = 'sheprd-relay';
// One label per name part, as DNS allows; nothing else reaches the YAML.
const DOMAIN = /^(?=.{1,253}$)([a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z][a-z0-9-]{0,61}[a-z0-9]$/;

function base64(text) {
  const bytes = new TextEncoder().encode(text);
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}

// create-droplet.sh's sed substitution: the three placeholders, filled.
function render(cloudInit, compose, caddyfile, domain) {
  return cloudInit
    .split('__COMPOSE_B64__').join(base64(compose))
    .split('__CADDYFILE_B64__').join(base64(caddyfile))
    .split('__RELAY_DOMAIN__').join(domain);
}

// The address Sheprd is given: the name the certificate is for.
function relayAddress(ip, region, suffix) {
  return suffix ? `https://${region}.${suffix}` : `https://${ip.split('.').join('-')}.sslip.io`;
}

function normalizedSuffix(value) {
  const suffix = value.trim().toLowerCase().replace(/^\.+|\.+$/g, '');
  if (suffix && !DOMAIN.test(suffix)) return null;
  return suffix;
}

if (typeof module !== 'undefined') module.exports = { render, relayAddress, normalizedSuffix };

if (typeof document !== 'undefined') {
  const $ = id => document.getElementById(id);
  const form = $('rd-form');
  const status = $('rd-status');
  const results = $('rd-results');
  let sources = null;

  async function readSources() {
    if (sources) return sources;
    const names = ['cloud-init.yml', 'compose.yml', 'Caddyfile'];
    const texts = await Promise.all(names.map(async name => {
      const response = await fetch(SOURCE + name, { cache: 'no-store' });
      if (!response.ok) throw new Error(`Could not read ${name} from GitHub (HTTP ${response.status}).`);
      return response.text();
    }));
    sources = { cloudInit: texts[0], compose: texts[1], caddyfile: texts[2] };
    return sources;
  }

  async function api(token, path, options = {}) {
    const response = await fetch(API + path, {
      ...options,
      headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
    });
    const body = response.status === 204 ? null : await response.json().catch(() => null);
    if (!response.ok) {
      const reason = body && body.message ? body.message : `HTTP ${response.status}`;
      throw new Error(`DigitalOcean refused: ${reason}`);
    }
    return body;
  }

  function say(text, kind) {
    status.textContent = text;
    status.dataset.kind = kind || '';
  }

  function chosenRegions() {
    return [...form.querySelectorAll('input[name="region"]:checked')].map(input => input.value);
  }

  function suffixOrSay() {
    const suffix = normalizedSuffix($('rd-suffix').value);
    if (suffix === null) say('That domain is not a valid name, such as relay.example.org.', 'error');
    return suffix;
  }

  // The token whose keys the list shows. Set only once they load, so a
  // failed request can be retried by pasting the same token again.
  let keysFor = null;
  const noKeysHint = $('rd-keys-none').textContent;
  async function loadKeys() {
    const token = $('rd-token').value.trim();
    const list = $('rd-keys');
    if (!token || token === keysFor) return;
    keysFor = null;
    list.replaceChildren();
    // A request for an earlier token may finish after this one: only the
    // answer for the token still in the field may fill the list.
    const current = () => $('rd-token').value.trim() === token;
    try {
      const body = await api(token, '/account/keys?per_page=200');
      if (!current()) return;
      const keys = body.ssh_keys || [];
      list.replaceChildren();
      $('rd-keys-none').textContent = noKeysHint;
      $('rd-keys-none').hidden = keys.length > 0;
      for (const key of keys) {
        const label = document.createElement('label');
        const box = document.createElement('input');
        box.type = 'checkbox';
        box.name = 'key';
        box.value = String(key.id);
        label.append(box, ` ${key.name}`);
        list.append(label);
      }
      keysFor = token;
    } catch (error) {
      if (!current()) return;
      $('rd-keys-none').hidden = false;
      $('rd-keys-none').textContent = `SSH keys could not be listed (${error.message}). The droplets will be reachable from DigitalOcean's web console.`;
    }
  }

  function resultRow(region) {
    const item = document.createElement('li');
    item.innerHTML = '<strong class="rd-region"></strong> <span class="rd-state"></span>' +
      '<div class="rd-address" hidden><code></code> <button type="button" class="rd-copy">Copy</button></div>' +
      '<div class="rd-more"></div>';
    item.querySelector('.rd-region').textContent = region;
    item.querySelector('.rd-copy').addEventListener('click', () =>
      navigator.clipboard.writeText(item.querySelector('code').textContent));
    results.append(item);
    return {
      state: text => { item.querySelector('.rd-state').textContent = text; },
      address: text => {
        item.querySelector('code').textContent = text;
        item.querySelector('.rd-address').hidden = false;
      },
      more: (text, href) => {
        const more = item.querySelector('.rd-more');
        more.textContent = text;
        if (href) {
          const link = document.createElement('a');
          link.href = href;
          link.textContent = 'Open the droplet';
          link.rel = 'noopener';
          link.target = '_blank';
          more.append(' ', link);
        }
      },
    };
  }

  const pause = ms => new Promise(resolve => setTimeout(resolve, ms));

  async function follow(token, droplet, row, region, suffix) {
    let ip = '';
    for (let tries = 0; tries < 60 && !ip; tries++) {
      await pause(5000);
      const body = await api(token, `/droplets/${droplet.id}`);
      const network = (body.droplet.networks.v4 || []).find(net => net.type === 'public');
      if (body.droplet.status === 'active' && network) ip = network.ip_address;
    }
    if (!ip) {
      row.state('created, but no address yet: see it in DigitalOcean.');
      return;
    }
    const address = relayAddress(ip, region, suffix);
    row.address(address);
    row.more(`Droplet ${droplet.name} at ${ip}. The dashboard's access token is in /root/sheprd-relay-login.txt on it.`,
      `https://cloud.digitalocean.com/droplets/${droplet.id}`);
    if (suffix) {
      row.state(`point ${region}.${suffix} at ${ip}; the relay answers once the name resolves.`);
      return;
    }
    row.state('installing (a few minutes)…');
    // Only the relay's own `ok` counts: Caddy answers 502 while the relay
    // is down, and a refusal or certificate error rejects, so keep trying
    // for a quarter of an hour. The Caddyfile allows this one cross-origin
    // read.
    for (let tries = 0; tries < 90; tries++) {
      try {
        const response = await fetch(`${address}/healthz`, { cache: 'no-store' });
        if (response.ok && (await response.text()).trim() === 'ok') {
          row.state('ready: add it in Sheprd.');
          return;
        }
      } catch {
        // Not answering yet.
      }
      await pause(10000);
    }
    row.state('not answering yet: see /var/log/cloud-init-output.log on the droplet.');
  }

  form.addEventListener('submit', async event => {
    event.preventDefault();
    const token = $('rd-token').value.trim();
    const regions = chosenRegions();
    const suffix = suffixOrSay();
    if (suffix === null) return;
    if (!token) return say('Paste a DigitalOcean token first.', 'error');
    if (regions.length === 0) return say('Choose at least one region.', 'error');
    const button = $('rd-deploy');
    button.disabled = true;
    results.replaceChildren();
    try {
      say('Reading the relay files…');
      const { cloudInit, compose, caddyfile } = await readSources();
      const keys = [...form.querySelectorAll('input[name="key"]:checked')].map(box => Number(box.value));
      const size = $('rd-size').value;
      const created = [];
      for (const region of regions) {
        say(`Creating the droplet in ${region}…`);
        const row = resultRow(region);
        try {
          const body = await api(token, '/droplets', {
            method: 'POST',
            body: JSON.stringify({
              name: `sheprd-relay-${region}`,
              region,
              size,
              image: IMAGE,
              monitoring: true,
              tags: [TAG],
              ssh_keys: keys,
              user_data: render(cloudInit, compose, caddyfile, suffix ? `${region}.${suffix}` : ''),
            }),
          });
          row.state('created, starting…');
          created.push(follow(token, body.droplet, row, region, suffix).catch(error => row.state(error.message)));
        } catch (error) {
          row.state(error.message);
        }
      }
      say(created.length ? 'Droplets created. Keep this tab open to see when each relay answers.' : 'No droplet was created.',
        created.length ? '' : 'error');
      await Promise.all(created);
      if (created.length) say('Done. Add each address in Sheprd under Settings > Remote control > Relays, then delete the token if you made it for this.');
    } catch (error) {
      say(error.message, 'error');
    } finally {
      button.disabled = false;
    }
  });

  $('rd-token').addEventListener('change', loadKeys);

  $('rd-show').addEventListener('click', async () => {
    const suffix = suffixOrSay();
    if (suffix === null) return;
    const region = chosenRegions()[0] || 'REGION';
    try {
      const { cloudInit, compose, caddyfile } = await readSources();
      $('rd-userdata').value = render(cloudInit, compose, caddyfile, suffix ? `${region}.${suffix}` : '');
      $('rd-manual').hidden = false;
    } catch (error) {
      say(error.message, 'error');
    }
  });

  $('rd-copy-userdata').addEventListener('click', () => navigator.clipboard.writeText($('rd-userdata').value));
}
