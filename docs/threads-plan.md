# Plan: threads in `sheprd-acp`

A chat today is one agent's session in one pane. Change the agent after the
first prompt and the conversation is gone; close the pane and the chat
survives only as a restore record. This plan makes the conversation the
thing that lasts: a **thread** that keeps its id and transcript while its
agent, model, and process change underneath, and that the app can list,
search, archive, and continue. Read `AGENTS.md` first; its rules apply
throughout.

T3 Code (`pingdotgg/t3code`) is the reference for what users ask for here:
"easy to keep threads", and switching models and agents in the middle of
one. Its server owns threads in an event-sourced SQLite store. We keep our
shape instead: `sheprd-acp` in a Herdr pane is still the only client of its
agent, and nothing new runs while the app is closed.

## Where it starts

PR #1230 (`feat/chat-agent-choice`) already does the hard part for a new
chat:

- `sheprd-acp --choose` lists the installed agents in `Transcript::harnesses`;
  `Command::SetAgent` switches until the first prompt.
- A switch stops the old agent on its own thread, fences its late replies
  by a harness id, starts the new one in the same folder, and sends every
  client a snapshot of a new epoch.
- Protocol revision 2 adds `Change::Harnesses`; refusals `AgentFixed` and
  `AgentMissing`.
- The app's composer picks the agent, and `sheprd_usage::pick` chooses one
  with room on its plan.
- A Herdr 2 chat's restore record follows the switch to the agent that now
  runs (`Update::Started`), so a restore never takes the first agent's end
  for the running one's.

Step 1 of this plan is to merge it. Everything below extends it rather than
replacing it.

Changing the model inside one agent already works through
`Command::SetOption` and the agent's ACP config options. Threads do not
touch it.

## Decisions

| Question | Decision |
| --- | --- |
| What a thread is | A `ThreadId` (UUID) and a list of **segments**, each one agent session: the agent, its native session id, the model it last ran, and the first transcript entry it saw. The transcript stays one list across segments. |
| Who owns it | `sheprd-acp`, as it owns the chat. The thread record replaces the restore record's single `agent` and `session` with segments; the record outlives the process when the thread is kept, not only when it was killed. |
| Switching after the first prompt | `SetAgent` stays allowed while the agent is idle. `AgentFixed` now means only "a turn is running". Switching mid-turn is refused rather than cancelling silently. |
| What the new agent learns | A **handoff**: the selection below, sent ahead of the user's next prompt as attributed context, never as the user's words. No model writes a summary; selected items keep their full text. |
| Switching back | Resume the earlier agent's own session (`--resume` for Claude and Codex, `session/load` for ACP agents) and hand it only what happened since it last ran. Its native context is better than any handoff. When it cannot resume (the agent lacks ACP's `loadSession` capability, its session is gone, or loading fails), the switch starts a new session as for any other agent, with the full handoff, and the transcript says the earlier session could not be continued. A delta is never sent to a session that does not hold what came before it. |
| What is left out | Listed by entry id in the handoff, and readable through a `thread_read` tool the chat gives the agent (below). |
| Forks | A fork is a new thread in a new pane, starting from a chosen entry, with a handoff to its first agent. Merge-back sends the parent a handoff of the fork's new entries. Phase 5. |
| Thread list | The app lists thread records per host, beside workspaces in `sheprd-session`. Archive hides a record; delete removes it. A thread with no pane is continued by starting `sheprd-acp --thread ID` in a new one. |
| Storage | One JSON record per thread, as restore records are today, plus the thread's own transcript file beside it (below). No database; no daemon. |
| Where history is read from | The thread's transcript file, never the attach window, which stays trimmed as today. The handoff and `thread_read` read the same file, so what one names the other can return. Agents' native logs are not a source: their entries have no thread entry ids. |

## The handoff

Selection follows what T3 Code found works
(`packages/provider-core/src/server/handoffBudget.ts` there):

1. Always the thread's first prompt, the latest prompt, and the latest reply.
   The transcript file pins the first prompt (below), so it is always there.
2. Then entries newest first, until a budget of about 16,000 tokens,
   counted by a byte estimate (no tokenizer dependency).
3. Kept entries keep their full text: prompts, replies, tool calls with
   their command and exit status, file paths changed, plans. Output beyond a
   per-entry cap is cut with a marker.
4. Anything not kept is named by entry id, oldest and newest first, with a
   line telling the agent to read it with `thread_read` when it needs it.
   Entries the transcript file has already trimmed are not named as
   readable: the handoff says how many were trimmed and that they are gone.

On a switch back that resumes the target's own session, the selection
covers only entries after the target's last segment, and step 1 uses the
latest prompt and reply only. A switch back that cannot resume uses the full
selection above.

The handoff text is built in `sheprd-chat-protocol` from the thread's
entries and its trimmed ranges, so it is pure and tested without an agent;
`sheprd-acp` only sends it.

## The transcript file

Each thread appends its entries, with their ids, to one file beside its
record, separate from the attach window clients see:

- The first prompt is pinned: trimming never removes it.
- Past the cap (below), the oldest entries after the first prompt are
  removed, and the file records the removed id range, so a read of it
  answers "trimmed" rather than nothing.
- Promises narrow to what the file keeps: the pinned first prompt and every
  retained entry. Nothing older is fetched from anywhere else.

## `thread_read`

ACP's `session/new` and `session/load` take `mcpServers`. The chat passes
one stdio server, `sheprd-acp thread-mcp --thread ID`, which opens its own
thread's transcript file read-only and offers:

- `thread_read { from, to }`: entries by id range, bounded per call; ids
  in a trimmed range come back as trimmed, not as an error or silence.
- `thread_search { query }`: entry ids whose text matches, bounded.

The server reads only its own thread, cannot send commands, and ends with
the chat. Agents whose harness takes no MCP servers get the handoff without
the tool, and the handoff says so instead of naming it.

## Protocol

Revision 3 of the attach wire:

- `Transcript::thread`: the `ThreadId` and the segments, so every client
  shows where each agent began.
- `Entry::Handoff { from, to, kept, omitted }`: drawn as a divider
  ("Continued in Codex: 42 entries handed over, 17 by reference"), never
  as a user message.
- `Command::Fork { at: EntryId }` (phase 5).

Older clients get a `Notice` entry in place of `Handoff` and no `thread`
field, as revision 2 did for `Change::Harnesses`.

## Phases

Each phase is its own pull request.

1. **Land #1230.**
2. **Switch after the first prompt.** Segments in the record, `SetAgent`
   while idle, the handoff builder with its tests, `Entry::Handoff`, switch
   back by resume. Desktop composer and phone keep the same picker; it no
   longer disappears after the first prompt.
3. **`thread_read`.** The stdio MCP server and its bounds.
4. **Thread list.** Records kept past the pane; a Threads section in the
   activity bar listing every host's threads, searchable; archive, delete,
   continue in a new pane; `sheprd thread list|open|archive` commands and
   `docs/cli.md`.
5. **Fork and merge-back.**
6. **Phone.** Thread list and switching in Sheprd Remote, through the
   remote control daemon's existing chat routes.

## Bounds

- Handoff: at most 16,000 estimated tokens and 1 MiB.
- Transcript file per thread: at most 64 MiB, trimmed from the oldest
  entry after the pinned first prompt, with the trimmed ranges recorded.
- Segments per thread: capped (64); the oldest are folded into one.
- `thread_read`: at most 64 entries or 256 KiB per call.

## Not in scope

- Moving a thread between hosts. Teleport already moves the worktree and
  its agents; carrying thread records with it is a follow-up there.
- Picking the model by plan usage (separate from picking the agent).
- An LLM-written summary as the handoff.

## Open questions

- Should a switch with a queued prompt send the handoff and the prompt as
  one turn (T3 does) or the handoff first? Proposal: one turn.
- Do kept threads count against the restore record directory's existing
  listing bounds, or get their own directory? Proposal: their own.
