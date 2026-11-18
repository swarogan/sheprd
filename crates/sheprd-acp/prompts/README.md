# Chat prompt templates

The built-in texts `sheprd-acp` sends an agent ahead of the person's words
(see `docs/rich-chat.md`, What the agent is told). To change one, copy it to
`~/.config/herdr/chat-prompts/` (under `XDG_CONFIG_HOME` when set,
`%APPDATA%\herdr\chat-prompts\` on Windows) on the machine the agent runs on
and edit the copy. `instructions.md` there, which has no built-in text, holds
your instructions for every chat.

| File | Sent when the person is typing |
| --- | --- |
| `intro.md` | Always first: says the note comes from Sheprd |
| `desktop.md` | In the desktop app on the agent's computer |
| `terminal.md` | In the chat view of the agent's terminal pane |
| `remote.md` | Anywhere else that cannot open the agent's files, such as the desktop app over SSH |
| `phone.md` | In the iPhone and iPad app or the phone web app |

`{{ name }}` is replaced by its value; an unknown name stays as written
and the chat warns about it. See `docs/rich-chat.md` for the variables.
