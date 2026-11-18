# GitHub on the phone

Plan for native GitHub reads, and later actions, in the iOS app. Today the
phone never talks to GitHub: badges, checks and failing logs come from a
paired computer, either Sheprd's pull request cache through `ChangeAsks` or
the daemon's opt-in `gh` reads (`[remote] github_cli`, PR #1282). With
neither, the Changes screen can only say that standalone reads are off.

With this plan the phone calls GitHub itself, with a sign-in it gets from a
paired computer. The computer only reports which repository and branch a
workspace is on, and hands over a token. The phone can also sign in to
github.com itself, so it reads GitHub with no computer reachable.

## Decisions

- **One account per GitHub host**, shared by every computer. github.com and
  each Enterprise host have their own account; the phone does not route
  between several accounts on one host as the desktop's additional accounts
  do.
- **The token comes from a desktop, or from the phone's own sign-in.**
  Without signing in, the phone borrows a computer's token. Settings shows
  which computer each host's account came from, and offers its own sign-in
  for github.com.
- **The phone's own sign-in wins** for github.com; a lent token is the
  fallback when it is signed out or its sign-in fails. Enterprise hosts
  only come from a computer's `gh`: Sheprd's GitHub App exists on
  github.com alone.
- **GitHub Enterprise is supported**: Enterprise Server (`https://HOST/api/v3`,
  `https://HOST/api/graphql`) and GHE.com (`https://api.SUBDOMAIN.ghe.com`).
- **Falling back to the computer is quiet.** When a direct read fails (no
  token, no access to that organization, offline from GitHub), the phone
  asks the computer as it does today, and labels the answer "via COMPUTER".
- **v1 is read-only** (phases 1 to 4). Actions follow in their own change.

## Getting the token

Sheprd's GitHub App issues tokens that expire after 8 hours, with refresh
tokens that GitHub rotates: a refresh invalidates the old pair
(`crates/sheprd-github/src/token.rs`). If the phone held a copy of the
refresh token, the phone and the desktop would sign each other out. So:

- **Sheprd stays the only holder of its refresh token.** The phone gets a
  lease: the current access token and its expiry. When Sheprd renews, the
  old access token stops working; the phone gets a 401 and asks again.
- **`gh` tokens and pasted tokens do not expire.** These are copied as they
  are. This is the only token source for Enterprise hosts, since Sheprd's
  App exists only on github.com: `gh auth token --hostname HOST` on the
  computer.
- **Sources, in order, per host:** Sheprd's sign-in for that host's account
  (github.com only), then the `gh` sign-in for that host. Only the main
  account is shared; additional accounts stay on the desktop.
- **The daemon never refreshes.** When Sheprd is closed and its lease has
  expired, the daemon offers the `gh` token if there is one, and otherwise
  says so; the phone then falls back to the computer's reads. This avoids
  two processes rotating one refresh token without a lock between them.

### Wire

- `GET /api/v2/github/token?host=github.com` answers `{"host", "login",
  "token", "expires_at"?, "source": "sheprd"|"gh"}`, or 404 when this
  computer has no account for the host. Paired devices only, over the
  existing TLS or relay tunnel; never logged; rate-limited per device.
- `home` gains `github_hosts: [{"host", "login"}]` so the phone knows which
  computer can lend which account without asking each one.
- The daemon asks Sheprd through the control socket the way it does for
  changes: a `Request::GitHubLeaseAsks` held open while a phone is connected,
  answered with the current access token; Sheprd checks with GitHub that the
  token works before sending it. When Sheprd is closed, the daemon runs
  `gh auth token --hostname HOST` itself.

### Consent and revocation

PR #1282 promised that pairing never copies credentials; this change
replaces that promise, so the user has to opt in:

- The pairing approval on the computer gets a switch, **Share GitHub sign-in
  with this phone**, on by default, with the account shown. Settings >
  Remote control lists each phone with the same switch.
- Turning it off, or revoking the phone, stops new leases. A Sheprd lease
  dies within 8 hours anyway. A copied `gh` token does not expire: the phone
  forgets it when told to, and when it forgets the computer, but only
  `gh auth refresh` or GitHub's settings can revoke it; the switch says so.
- On the phone, tokens live in the Keychain as
  `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`: never in iCloud, the
  widgets, or the watch, and removed when no computer lends that host any
  more.

## Signing in on the phone

- GitHub's device flow with Sheprd's GitHub App (`Iv23liurUcwxPjrdIFYT`,
  the client ID the desktop uses; device flow needs no secret). The phone
  shows the code with a copy button and opens `github.com/login/device`
  in `ASWebAuthenticationSession`, then polls as GitHub asks
  (`authorization_pending`, `slow_down`, `expired_token`,
  `access_denied`).
- Its authorization is its own token pair, separate from the desktop's,
  so renewing one never signs the other out. The phone keeps the access
  and refresh tokens in its Keychain (this device only, not synced),
  renews shortly before expiry or on a 401, and saves the rotated pair
  before using it, as `sheprd-github`'s `token.rs` does.
- Sign out deletes both tokens; revoking the authorization itself is on
  GitHub (Settings > Applications), which the screen links to.
- An organization that has not installed the App still reads public
  repositories only; the screen says so and links to the installation,
  as the desktop's `installation.rs` does.

## Phases

### 1. The computer names the repository (Rust)

- Each workspace in `home` gains `github: {"host", "owner", "repo",
  "branch", "head", "head_owner"}`, from the remote `native/changes/local.rs`
  already reads; left out when the remote is not a GitHub host. A fork with
  an `upstream` remote names the upstream repository, where its pull request
  lives, and `head_owner` names the fork's owner (the branch's tracked
  remote, else `origin`) only when it differs from `owner`; `head` is the
  commit checked out.
- Enterprise hosts are recognized from the hosts `gh` is signed in to and
  from `[github] hosts` in the config, not guessed from the URL.
- Tests: SSH, HTTPS and `ssh://` URLs, Enterprise and GHE.com hosts, forks,
  detached HEAD, no remote, a non-GitHub remote.

### 2. Leases (Rust and Swift)

- Daemon: the endpoint, the `home` field, the control socket ask, and the
  pairing switch, with tests for each, the bounds, and redaction (no token
  in logs, errors, or `Debug`).
- Sheprd: answers lease asks from its credential store on a background
  thread; shows the switch in the pairing dialog and in Settings.
- Phone (the `SheprdGitHub` target in `ios/SheprdRemoteCore`, which only
  the app links): `TokenStore` (Keychain), `Lease`
  (expiry, refetch on a 401, the computer it came from), and the choice of
  computer when several lend the same host: the most recently reachable one.

### 3. Direct reads (Swift)

- One GraphQL query per repository resolves branch → pull request: state,
  draft, review decision, mergeability, and the check rollup. Queries for
  visible workspaces are batched.
- REST only for a failing Actions job's log. The excerpt is cut on the phone
  as `check_log` does on the computer (8 KiB, ending on the last error,
  without colors or timestamps); shared vectors keep the Rust and Swift
  versions in step, as `sheprd-tunnel`'s do.
- ETags, the rate limit's reset time, and backoff; a refresh when a screen
  appears and every 60 s while it stays on screen, nothing in the
  background.
- When the phone has no lease for a host, or a read fails, it asks the
  computer as it does today.

### 4. Screens (mockups first)

- Changes > Branch: a pull request header with title, state, review
  decision, checks, and the existing failing-log → chat flow. The "not
  enabled" note goes when a lease answers.
- Pull request detail: description (`MarkdownView`), reviewers, the
  conversation and review comments (read-only), and files changed in the
  existing diff view.
- A Pull requests list in the Workspaces tab: mine, and review requested,
  including repositories no computer has a workspace for.
- Settings > GitHub: one row per host with the login, avatar, and where
  its token comes from (this iPhone's sign-in, or a computer), with Sign
  in and Sign out for github.com, and the device-code sheet.

### 5. Actions (later)

Following the desktop's `pr_actions`, each behind a confirmation: re-run
failed jobs, approve, request changes, comment, mark ready, and merge with a
method the repository allows. Opening a pull request for unpushed commits
(`unopened_commits`) still pushes from the computer; the phone does no Git.
A Sheprd lease with the App's permissions is enough; a `gh` token needs the
`repo` scope, which the phone reads from `X-OAuth-Scopes` before offering
an action.

### 6. Notifications (later)

First, what changed since the app was last opened (a check failed, a review
arrived). Pushes from GitHub would need webhooks through the APNs relay and
are a separate design.

## Verification

- Rust: `home` and URL parsing, leases, consent, bounds, and redaction, with
  focused `cargo test -p sheprd-remote-control --features server` runs.
- Swift: `swift test` in `ios/SheprdRemoteCore` against recorded GraphQL and
  REST fixtures: 200, 304, 401 then a new lease, 403 for SSO and for the
  rate limit, 404, Enterprise base URLs, and the log excerpt vectors.
- Native: the simulator loop for the screens; a real device for the
  Keychain after a reboot, a lease expiring while the phone is away, and an
  Enterprise host signed in with `gh`.
