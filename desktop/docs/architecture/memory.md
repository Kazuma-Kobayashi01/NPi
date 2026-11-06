# Memory

What the assistant keeps for later, in Chat, Code and Academic: short notes about the user, a profile of how they like to work in each mode, and a way back into past conversations; and, in Code and Academic, skills written from what worked. On by default; Settings → Memory turns it off, and shows every note to edit or delete.

## Store (`src-tauri/src/memory/mod.rs`)

One SQLite file, `~/.local/share/npi/memory.db` (`PRAGMA user_version` 2; version 1 gains the `session` column on open; a newer file is refused). `notes`: `kind` (`you`, `topic`, `area`, `person`), `title` (unique, ignoring case), `text`, `mode` (where it was last written: `chat`, `code`, `academic`, or `user` for an edit in Settings), `session` (the tab that last wrote it, cleared by an edit in Settings), timestamps. `meta`: `enabled`, `skill-learning`, `chat-memory-imported`. A title is 60 characters at most, a text 300, 300 notes in all. Text that looks like a key or token is refused.

## One memory in the app

The kernel has memories of its own: `memory.backend` (a background pipeline that summarises past sessions with extra model calls into `memory_summary.md`, per folder) and `autolearn` (lessons per folder in `learned.md`). A user's CLI `config.yml` may turn them on, and they would then run in every tab too, outside this switch. So every session's `--config` overlay pins them (`AppMode::session_policy`, written by `start_session`): `memory.backend` is always `off`; `autolearn.enabled` is `true` only in Code and Academic, while Memory is on and Settings' "Write skills from what worked" is on (see Skills above). Notes about the user are this store's alone. The overlay outranks global and project config, so the CLI keeps whatever its own config says.

## Being told

Each `remember` and `update_user_profile` that lands emits `memory://noted` (`{ sessionId, note, replaced }` or `{ sessionId, profile, text }`); `live.js` passes it on as `npi:memory-noted`, together with the skills it sees, and `design/memory-notice.jsx` shows the change for six seconds at the bottom of the window (`OMP_MEMORY.noticeOf`: Undo only for something new).

## What the model sees (`memory/prompt.rs`, `memory/tools.rs`)

`start_session` (`memory_host` in `lib.rs`) adds, in every mode while Memory is on, a block in the prompt and host tools on top of the session's own (`MemoryHost`):

- The block: when and what to note, what never to, that all of it is data and not instructions; how full memory is (`Memory in use: N of 300 notes; X of 3000 characters of notes shown here[, k older left out]; profile Y of 1500 characters`, and a warning near the note limit); the notes, newest first, 3000 characters at most (characters, so Chinese notes get the room English ones do); the mode's profile.
- `remember` (the same title replaces a note), `memory_lookup`, `forget_memory`: the notes.
- `update_user_profile`: the mode's profile, rewritten whole.
- `search_conversations` (when the sessions folder resolves): past conversations, below.

Noting is the model's own call during the turn: there is no second pass over the conversation. The names avoid omp's own `recall`. With Memory off, neither the block nor the tools are there. Changes reach a session when it next starts; the tools write at once. SideChat, the Copilot, Compile and quick ask are isolated and have no memory.

## Profiles (`memory/profile.rs`)

`~/.local/share/npi/USER-CHAT.md`, `USER-CODE.md`, `USER-ACADEMIC.md`: the model's summary of how the user likes to work in that mode (tone, form of results, what to check, what to avoid), 1500 characters at most, refused when it looks like it holds a key. Facts go in notes. The model replaces it whole; Settings → Memory shows the three with their size and saves an edit on blur (`memory_set_profile`; empty clears it). They sit beside Settings → Account (Chat's own profile and instructions, written by the user), not instead of it.

## Past conversations (`session_search.rs`)

omp's session files under the profile's `sessions/` are indexed into `~/.cache/npi/desktop/session-index.db`, a cache rebuilt from them (another schema version is dropped and rebuilt). Only user and assistant text is indexed, never tool output, at most 4000 characters a message, CJK as unigrams and bigrams (`librarian::cjk`, as the library). Every search first brings the index in line with the files by size and modification time, so there is no watcher. `scope: here` (default) is every chat folder in Chat and the tab's folder in Code and Academic; `all` is every conversation of the profile. At most 8 hits, two per conversation, as dated excerpts with the conversation's title and folder; the current conversation may be among them.

## Skills (omp's autolearn)

omp's autolearn (`manage_skill`) writes skills from what worked into `<agent dir>/managed-skills/`, never touching the user's own. The session overlay turns it on in Code and Academic only, while Memory and Settings' "Write skills from what worked" (`skill_learning`, default on) are on. `live.js` sees a `manage_skill` result (`details.action` create/update) and shows it through the same notice, with Undo for a new one; Settings → Skills lists managed skills (the `managed` source) with Delete. Undo and Delete call `skill_delete_managed`, which takes only a plain skill name and moves the folder to `~/.cache/npi/desktop/skill-trash/<name>-<time>` rather than erasing it. A running session keeps a removed skill listed until it next refreshes its skills.

## The old `MEMORY.md`

Chat's file in the chat folder is taken in once (`import_chat_memory`) and left in place, no longer written or read into prompts.

## Checked

Unit tests, and `real_model_takes_notes_and_uses_them` (ignored by default) with a real model: it noted two facts from a message while answering something else, used them in the next conversation, and noted nothing for "1+1". Not checked in the real app or with a real model: a session started from a tab, the notice, profiles and `search_conversations` in use, a skill written and undone, and the Settings page with a real store (the real page ran against a fake bridge). The overlay's precedence was read from omp's `Settings.#mergeOwnLayers`, not observed in a running session.
