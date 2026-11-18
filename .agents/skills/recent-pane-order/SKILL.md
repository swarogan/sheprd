---
name: recent-pane-order
description: Check Ctrl-Tab MRU ordering after changes to recent_panes, navigation admission, surface fences, core pane focus, or modifier release. Exhaustively explore bounded schedules against production Rust and check the window adapter with headless commands.
---

# Check recent-pane ordering

A held cycle must promote only its last confirmed selection and any subsequent
independent focus, after Ctrl is released. Otherwise the next Ctrl-Tab can
return to an intermediate pane instead of the pane the user left.

## Run the check

1. Read the coverage record and [review](review.md). Compare covered source
   with the recorded sync commit and confirm the event assumptions still match.
2. From the repository root, run:

   ```sh
   cargo test --locked -p sheprd-gpui --lib recent -- --nocapture
   ```

   Save full output and exit status. `order::tests::model_check` runs the
   exhaustive check. Other recent-pane tests cover the window adapter,
   lifecycle, backend selection, and core keyboard focus.
3. Read a failure as a sequence of attempted presses, confirmed selections,
   cancellation, and release. Compare actual and expected MRU order. Check
   whether the adapter permits that sequence before classifying a product bug.
4. Preserve reachable counterexamples. Change product behavior only within the
   user's request. When admission changes intentionally, reconcile the event
   generator and keep the ordering property intact.
5. After checking correspondence with changed code, update the coverage record.
   Advance the sync commit only when that code is committed. A test pass alone
   does not establish correspondence.

The breadth-first checker calls production `RecentPanes` methods with the real
snapshot and modifier types. It replays prefixes instead of copying the reducer
or adding verification-only production APIs. Rust is pinned to 1.99.0 in
`rust-toolchain.toml`; dependencies are pinned by `Cargo.lock`. An initial build
can take minutes; the bounded exploration itself is small.

## Scope and adapter contract

The exhaustive domain has one connected, surface-capable Herdr endpoint, one
boot, three open panes, initial MRU `[c,b,a]`, one Ctrl hold, and one to three
attempted presses in either direction. A pending selection refuses further
presses. Each accepted selection either completes or is cancelled before it
gains focus. Completion can expose any of the three panes as the latest focus,
representing a confirmed target followed by coalesced independent focus.
Release can occur before or after completion. Cancellation ends the cycle.
A complete schedule has at most seven events.

The independent expected order starts with `[c,b,a]`, promotes the last
confirmed selection, then promotes the actual final focus. It also checks
that a pending selection admits no additional press and that a completed
schedule has no pending selection. The search stops at the shortest failure.

`RecentPanes::observe` receives a focus only when the adapter has established
completion. For Herdr, that evidence is the existing `SurfaceActivation` from
the original inbox and activation request. A generic response or matching pane
identity is insufficient. Herdr versions without surface activation are
explicitly unsupported for recent-pane switching. Core focus completes locally
after showing the typed pane, or after its owning Herdr workspace fence settles.

The exhaustive claim excludes arbitrary history sizes, multiple holds,
endpoint changes, boots, closures, native keyboard delivery, and core navigation.
Headless example tests cover several of those boundaries; they do not extend
the finite proof. Platform-specific builds and native UI still need their own
checks.

## Check that the verifier can fail

Preserve the exact working source before each mutation. On a scratch change,
remove the `self.finish()` call inside `RecentPanes::release`, then run:

```sh
cargo test --locked -p sheprd-gpui --lib window::recent_panes::order::tests::model_check -- --nocapture
```

Require an ordering assertion failure. Restore the source. Separately remove
the pending guard inside `RecentPanes::peek` and require the bounded check to
fail its pending-admission assertion. Compilation or setup failures do not
count. Restore the source exactly and rerun the full `recent` command.

## Coverage record

Last confirmed production sync commit:
`9430b824a0ce688f19a8f617a6099124475d64b2`.

The correction and verifier were reconciled together at this commit. The
original PR baseline was `f63d437a5c0bb0c4fbf945d2c7e8997408a6802a`; its
legacy counterexample and the resulting contract change are in the review.

| File under `crates/sheprd-gpui/src/` | Boundary | Coverage |
| --- | --- | --- |
| `window/recent_panes/order.rs` | `peek`, `begin`, `observe`, `release`, `cancel`, `finish` | Bounded production-code exploration |
| `window/recent_panes/order/tests/model_check.rs` | `replay`, `check_schedules` | Event generation and independent expected order |
| `window/recent_panes.rs` | Command/release observation, navigation identity and completion | Source correspondence and headless examples |
| `window/recent_panes/tests/` | Legacy refusal, delayed focus, lifecycle, endpoint switching | Headless commands and mock peers |
| `core_terminals/recent.rs` | Membership, focus, composition, navigation | Source correspondence and core headless examples |
| `core_terminals/tests/recent.rs`, `core_terminals/tests/recent/` | Core focus, mixed backends, shell groups, composition | Headless commands and rendered keyboard events |
| `endpoint/polling.rs`, `endpoint.rs` | Deferred dispatch, activation replacement, reset | Source correspondence and lifecycle examples |
| `window/commands.rs`, `core_terminals/tabs.rs` | Observe local focus before commands/poll | Headless focus examples |
| `window/construct.rs` | Window deactivation finishes the hold | Source correspondence with `finish` |

Observed at the sync commit: the command passed 90 tests,
including all 1,168 schedules with up to three attempted presses. The separate
one-press check passed its 16 schedules. Each scratch mutation failed the
intended assertion, and the exact reducer source was restored before the
passing run. The review records commands and log locations.

Use `/maintain-formal-verification-skill` after covered code changes without
rechecking this correspondence.
