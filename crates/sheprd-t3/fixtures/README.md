# Recorded T3 Code frames

Server frames from a sandboxed T3 Code 0.1.7 server (`t3 serve` with a
private home, loopback only, telemetry off), driven by a fake ACP agent that
echoes each prompt and makes one read tool call. No real agent, account, or
user data is involved; tokens and tickets were never written here.

- `snapshots.json`: the thread list and one finished thread, as
  `orchestration.subscribeShell` (request 1) and
  `orchestration.subscribeThread` (request 2) first answer.
- `live-turn.json`: the same two subscriptions while a second message is
  dispatched (request 3): every event of that turn, then the thread list's
  updates.

Re-record them against the version `sheprd-t3` pins when it changes.
