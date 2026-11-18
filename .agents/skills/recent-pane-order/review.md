# Recent-pane review and verification

The full PR was reviewed from merge base
`f4b9066771f8c0c1a53011f95239a675e3f155f9` to committed head
`f63d437a5c0bb0c4fbf945d2c7e8997408a6802a`, including command registration,
shortcuts, localization, documentation, endpoint switching, polling, window
input, the reducer, and tests. The subsequent correction and verifier are committed at
`9430b824a0ce688f19a8f617a6099124475d64b2`.
GitHub PR metadata was unavailable; these findings concern the local branch.

## Original counterexample

The original unfenced observer treated a matching pane identity as proof that
all queued focus requests had finished. The bounded checker found a failure
without out-of-order messages:

1. Start focused on `c`, with MRU `[c,b,a]`.
2. Hold Ctrl and press Tab three times before snapshots arrive. The window
   queues `b`, `a`, then `b`.
3. Release Ctrl. The old reducer promotes `b`, producing `[b,c,a]`.
4. The first `b` report is mistaken for the final `b` request's completion.
5. Subsequent reports for `a` and `b` become independent visits, producing
   `[b,a,c]`.

The next Ctrl-Tab chooses `a` instead of `c`. A headless GPUI test reproduced
that exact request sequence and wrong next destination. The original run had
66 passing tests and these two failing counterexamples.

Serializing legacy requests is insufficient by itself. Legacy success responses
have no projection revision, and target snapshots may be coalesced. Pane identity,
a generic success response, or a timeout cannot establish the needed ordering.
Independent design reviews traced the client and the read-only Herdr source
before choosing the existing surface activation contract.

The other original gap was backend coverage. The old observer read only Herdr 1
snapshot identities. Core-only windows had no usable history, and mixed windows
could navigate away from a visible core pane using hidden Herdr history.

## Corrected contract

One recent selection may be pending. Herdr completion uses the existing
`SurfaceActivation`; a small identity ticket binds it to the originating
endpoint inbox, activation request, and any deferred navigation. It does not
copy the activation's success state. Connection replacement, activation
replacement, and failed deferred dispatch cancel the pending selection.
Deferred dispatch compares the actual target with the ticket before adopting
its activation. Replacing deferred target X with Y cancels X's selection.

The reducer freezes order during a hold and retains its cursor by pane identity.
Release waits for confirmation. Cancellation keeps the last confirmed visit;
coalesced independent focus becomes the newest visit. The destination-queue
heuristic and unfenced observation mode are removed.

Herdr versions without surface activation are explicitly unsupported for recent
switching. The command shows a localized notice. Supported sources skip those
destinations. This is an intentional compatibility restriction, documented in
the crate README.

Typed core targets include saved tab panes and the fallback shell. Local focus
is observed at command and release boundaries, so rapid clicks do not depend
on the next polling tick. Core navigation checks the owning workspace and uses
its normal navigation when necessary. A stale core view remains remembered but
is skipped until available again.

Independent implementation review also identified deferred core focus after its
workspace ceased to be visible, a generic workspace reveal overriding the shell,
and shell selection in a secondary editor group. All three now have passing
headless regressions alongside the reducer check. A four-pane test uses actual
rendered Ctrl-Tab and modifier events and checks intermediate selections, so
missing key delivery or a missing release cannot pass by returning to the
initial pane accidentally.

## Verification method

The checker executes the production Rust reducer rather than translating it
into another language. This keeps the finite proof tied to the code being
changed and uses the existing pinned Rust toolchain.

Its domain is one surface-capable endpoint, three panes, one continuous hold,
and up to three attempted presses in either direction. Every admitted selection
may complete or be cancelled before gaining focus. A completion may expose any
pane as the newest coalesced focus. Release may precede or follow completion.
The checker exhausts those event schedules and checks exact final MRU order and
single-selection admission. See [the skill](SKILL.md) for the command, coverage
mapping, bounds, and fault-injection procedure.

This is a bounded guarantee. It does not prove arbitrary history sizes,
multiple holds, endpoint or boot transitions, core navigation, native keyboard
delivery, or platform-specific behavior. Separate headless tests exercise those
adapter and lifecycle paths where practical. Native UI and full cross-platform
CI remain separate checks.

Model the Domain led to typed Herdr/core identities. Laziness Protocol led to
removing destination inference and reusing the existing surface fence. Prove It
Works led to the production-code schedule checker and headless reproductions.

Full local verification output and exit statuses are saved under
`.audit/recent-pane-*`. The final handoff records the completed checks.

## Observed results

On the corrected Linux source at the sync commit with Rust 1.99.0:

| Command | Result |
| --- | --- |
| `cargo test --locked -p sheprd-gpui --lib recent -- --nocapture` | Exit 0. 90 passed, including 64 recent-pane tests, 12 core recent-pane tests, and 14 other tests matched by the filter. |
| `just format-check` | Exit 0. |
| `just check-file-size` | Exit 0. |
| `just check-sorted` | Exit 0. |
| `just check-i18n` | Exit 0. 6,040 messages valid and used. |
| `git diff --check` | Exit 0. |

The bounded checker exhausted 1,168 complete schedules with at most three
attempted presses. Its separate one-press run exhausted 16 schedules, a subset
of that larger domain. This count is generated by the breadth-first event
enumeration, not sampled or a claim about native event delivery.

Removing `release`'s call to `finish` failed the ordering assertion. A trace
selected `b`, released Ctrl, then completed with independent focus on `a`;
the broken code kept `[c,b,a]` instead of `[a,b,c]`. Separately removing
`peek`'s pending guard failed the admission assertion. Both mutations were
restored byte-for-byte before the final passing run. The restored reducer's
SHA-256 is `27730a4c6c2efb49aac01502acb50f0b6eb1fa17b4ede6c7b24585e174247e85`.

The deferred-replacement regression also failed its cancellation assertion
before the target-identity fix and passed afterwards. That earlier combined
run additionally exposed an unrelated test fixture that failed to keep its
surface size and inbox snapshot coherent; the fixture was corrected.

Final test output is `.audit/recent-pane-final-verified.log`. Fault-injection
output is `.audit/recent-pane-mutation-release.log` and
`.audit/recent-pane-mutation-admission.log`. Each has a sibling `.status` file.
Independent source reviews found no remaining blockers. Five added redundant
comment lines were removed; ownership and call-order contracts were retained.
There are no open comment holds or constraint-encoding requests.

Native desktop input, macOS/Windows builds, all-feature compilation, and full
workspace CI were not run. Audit logs remain local and are not included
in the commits.
