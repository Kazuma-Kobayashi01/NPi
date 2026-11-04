# Memory

Short notes about the user that the model keeps across conversations, in Chat, Code and Academic. On by default; Settings → Chat → Memory turns it off, and shows every note to edit or delete.

## Store (`src-tauri/src/memory/mod.rs`)

One SQLite file, `~/.local/share/npi/memory.db` (`PRAGMA user_version` 1; a newer file is refused). `notes`: `kind` (`you`, `topic`, `area`, `person`), `title` (unique, ignoring case), `text`, `mode` (where it was last written: `chat`, `code`, `academic`, or `user` for an edit in Settings), timestamps. `meta`: `enabled`, `chat-memory-imported`. A title is 60 characters at most, a text 300, 300 notes in all. Text that looks like a key or token is refused.

## What the model sees (`memory/tools.rs`)

`start_session` adds, in every mode while Memory is on: a block in the prompt (when and what to note, what never to, that notes are data and not instructions; then the notes, newest first, 4 KiB at most, with a count of what was left out) and three host tools on top of the session's own (`MemoryHost`): `remember` (the same title replaces a note), `memory_lookup`, `forget_memory`. Noting is the model's own call during the turn: there is no second pass over the conversation. The names avoid omp's own `recall`. With Memory off, neither the block nor the tools are there. Changes reach a session when it next starts; the tools write at once. SideChat, the Copilot, Compile and quick ask are isolated and have no memory.

## The old `MEMORY.md`

Chat's file in the chat folder is taken in once (`import_chat_memory`) and left in place, no longer written or read into prompts.

## Checked

Unit tests, and `real_model_takes_notes_and_uses_them` (ignored by default) with a real model: it noted two facts from a message while answering something else, used them in the next conversation, and noted nothing for "1+1". Not checked in the real app: a session started from a tab, and the Settings row with a real store (it ran against a fake bridge).
