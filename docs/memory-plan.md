# Plan: long-term agent memory

Agents working in Sheprd remember what they learned across sessions: the
user's preferences, a repository's conventions, decisions made last week.
They search those memories, save new ones, and start each session with the
important ones already in context. Read `AGENTS.md` first; its rules apply
throughout.

The design comes from Moltis (`~/code/moltis`, `crates/memory`,
`crates/agents/src/silent_turn.rs`, `docs/src/memory.md`). This plan keeps
the parts that worked there and changes one thing that cannot carry over:
Moltis runs the agent loop and its tools in-process, but Sheprd hosts
agents it does not own (Claude, Codex, Pi, and others through ACP or in a
terminal). Memory therefore reaches agents through an MCP server, the
`sheprd` CLI, the managed skill, and the session start points `sheprd-acp`
already controls, rather than through tools compiled into a loop.

## What Moltis does, and what we keep

| Moltis | Here |
| --- | --- |
| Markdown files are the source of truth (`MEMORY.md`, `memory/<topic>.md`, `memory/YYYY-MM-DD.md`); the database is a rebuildable index | Keep. People can read, edit, and version their memories with any editor. |
| SQLite index: `files`, `chunks`, `embedding_cache`, an external-content FTS5 table kept in sync by triggers | Keep, with `rusqlite` (bundled, FTS5 already compiled in) instead of sqlx. |
| Sync by mtime+size, then file sha256, then per-chunk sha256 embedding cache; file watcher plus periodic full sync | Keep the three-level skip. Sync on write and on read when stale (mtime scan); a watcher only if measurements ask for it. |
| Line-based markdown chunking, 400 words with 80 overlap | Keep, as a small pure function with tests. |
| Hybrid search: FTS5 (tokens sanitized and quoted) plus cosine scan, merged by RRF (k=60) | Keep. Keyword-only first; vectors in phase 3. |
| Tools `memory_search`, `memory_get`, `memory_save`, `memory_delete`, `memory_forget` | Keep the first four as MCP tools and CLI commands. `memory_forget` (LLM-planned deletion) is dropped: an agent can search then delete. |
| `MEMORY.md` injected into the system prompt as `## Long-Term Memory`, capped at 8,000 characters, with usage instructions | Keep, through each agent's own system-prompt or instruction channel. |
| Per-turn prefetch: top 3 results appended as `<recalled_context>` | Keep for chats `sheprd-acp` runs, scoped and budgeted separately from `MEMORY.md`. |
| Silent-turn extraction before compaction, every 5 turns, and on session reset | Adapt: Sheprd has no model of its own, so extraction asks the agent itself, or runs the agent's own CLI headlessly, and is opt-in. |
| Write-target validation (`MEMORY.md`, `memory/<name>.md`, no `..`, absolute paths, or dot files), 50 KB per save, checkpoint before every mutation | Keep. Checkpoints become a copy kept beside the file (or Git, when the memory folder is a repository). |

### Moltis mistakes not to repeat

Found while reading it; each becomes a test here.

- Prefetch ignored the agent's scope; scoped tools filtered by path, prefetch did not. Here every read takes a `Scope` and the store refuses an unscoped query.
- The recalled block was appended before the 8,000-character cut, so a long `MEMORY.md` pushed it out. Here each part has its own budget.
- `vector_weight`/`keyword_weight` and the reranker, batch embedder, and session exporter were configurable or documented but never wired. Ship no setting or module without a caller and a test.
- A custom embedding model reported a hard-coded 1,536 dimensions. Here dimensions come from the first response and are stored with each vector; a mismatch re-embeds.
- One embedding request carried every cache miss for a file. Here requests are sub-batched and bounded.

## Decisions

| Question | Decision |
| --- | --- |
| Crate | New `crates/sheprd-memory`: files, writer validation, chunking, the SQLite index, search, and (phase 3) embeddings. No GPUI, no Tokio. Root `Error`/`Result` per `AGENTS.md`. |
| Scopes | `User` (follows the person everywhere) and `Repository(repo_key)` (one checkout's conventions, shared by its worktrees), using the orchestrator's existing repository key. Searches read both by default; saves name one. *(Proposed; confirm.)* |
| Folder | `<state dir>/memory/MEMORY.md` and `memory/*.md` for `User`; `<state dir>/repositories/<uuid5>/memory/` for `Repository`, beside the orchestrator's `state.sqlite3`. The index is `<state dir>/memory/index.sqlite3`. *(Proposed; confirm. An alternative is the config root, so people find and sync them more easily.)* |
| Concurrency | Many agents read and write at once, from several processes. WAL, a busy timeout, writes through one function that takes an advisory lock on the target file, appends atomically, then syncs that file's index rows. |
| How agents reach it | 1. An MCP server, `sheprd-acp memory-mcp` (stdio, blocking, no runtime), given to chats `sheprd-acp` starts and installable into terminal agents' configs. 2. `sheprd memory search|get|save|delete|list` for scripts and agents without MCP. 3. A section in the managed skill telling agents when to search and save. |
| CLI path | The `memory` commands open the store directly rather than going through the running app, so memory works when the app is closed (as `sheprd-automate` does). They still take `--json` and follow `docs/cli.md`'s exit codes. *(This departs from "every command talks to the app"; confirm.)* |
| Session start | `MEMORY.md` for both scopes, capped and labelled, through: Claude's `--append-system-prompt`; Codex's `thread/start` instructions; ACP agents in general, a first-turn embedded resource block; terminal agents, the `SessionStart` hooks `session-install` already writes. |
| Automatic extraction | Off by default. When on: at Claude's `PreCompact` hook and at chat end, ask the running agent to save what it learned (a queued prompt in a chat, the hook's instruction for terminal agents). A headless extraction with the agent's own CLI over the transcript is a later option, because it spends the user's model quota. *(Proposed; confirm.)* |
| Embeddings | Phase 3. An OpenAI-compatible endpoint over the workspace's `ureq`, plus Ollama when it answers locally. With none, search is keyword-only and says so. No bundled local model. |
| UI | A Memory tab (full-page view as a tab), listing files per scope, search, and open-in-editor; a settings section for the `[memory]` table. Phase 4. |

## Phases

### 1. Store and search, keyword only

- `sheprd-memory`:
  - `path`: scope roots and write-target validation (Moltis `writer.rs`), with tests for `..`, absolute paths, backslashes, dot files, and non-`.md` names.
  - `chunk`: line-based markdown chunking, 400/80.
  - `index`: schema v1 with `files`, `chunks`, `chunks_fts` and triggers; `user_version` migrations; the three-level sync skip; removal of deleted files.
  - `search`: FTS5 query sanitizing, rank normalization, scope filtering inside the query, `limit` clamped to 1..50.
  - `write`: save (append by default, 50 KB cap), delete a snippet or a file, with a copy kept before each change.
- Tests in topic files under `tests/`: sync skipping, concurrent writers in two processes, scope isolation, FTS edge cases (punctuation, empty query, non-ASCII), and the limits.

### 2. Agents read and write

- `sheprd-acp memory-mcp`: a stdio MCP server with `memory_search`, `memory_get`, `memory_save`, `memory_delete`, and JSON schemas copied from Moltis. Scope comes from the `cwd` it is started in.
- `sheprd-acp` passes it in `mcpServers` on `session/new` and `session/load`, Claude's `--mcp-config`, and Codex's `thread/start`, when `[memory] enabled = true`.
- `MEMORY.md` injection at session start (above), capped at 8,000 characters per scope.
- Per-turn prefetch in `send_prompt` for chats: top 3, 300 characters each, as a separate block with its own budget; skipped for slash commands and short prompts.
- `sheprd memory …` commands, `docs/cli.md`, the skill's command table and `description`.
- `session-install` gains an opt-in memory section: the MCP server entry and the `SessionStart` hook for terminal agents.

### 3. Extraction and embeddings

- Opt-in extraction at compaction and chat end, as decided above, writing to `memory/YYYY-MM-DD.md` with the date fixed by Sheprd, not the model.
- Embeddings: provider config, per-chunk cache keyed by provider, model, and text hash, bounded batches, vectors stored as little-endian `f32` with their dimension, cosine scan, RRF merge. A bounded cache eviction (Moltis: 50,000 rows).

### 4. UI

- Memory tab and settings section; a `sheprd memory open` command that focuses it.
- Native QA with screenshots per `AGENTS.md`.

## Config

```toml
[memory]
enabled = false            # MCP server, injection, prefetch
inject = true              # MEMORY.md at session start
prefetch = 3               # 0 turns it off
extract = "off"            # off | ask-agent
provider = "none"          # none | openai | ollama (phase 3)
base_url = ""
model = ""
```

Every key has a caller and a test in the phase that adds it.

## Open questions

1. Scopes: are `User` and `Repository` enough, or does each agent kind (Claude vs Codex) need its own, as Moltis has per agent?
2. Folder: state dir (private, out of the way) or config root (easy to find, sync, and version)?
3. Should `sheprd memory` bypass the running app, as proposed, or stay behind the control socket like every other command?
4. Extraction: is asking the running agent to save at compaction and chat end acceptable, or is a headless extraction pass worth the quota?
5. Do we import existing agent memories (Claude's `~/.claude/projects/*/memory/`, `AGENTS.md` files) into the index read-only, so search covers them?
