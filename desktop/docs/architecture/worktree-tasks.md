# Worktree tasks, review and file checkpoints

Three parts of working with an agent on code: keep tasks apart, review what they did, take it back.

## Worktree tasks (`worktree.rs`)

The branch button on a Code project's sidebar row (`handleNewWorktree` → `newWorktreeTask`) runs `git worktree add -b npi/task-<base36 time>` from `HEAD` and opens a tab there. A tab's cwd is fixed at spawn, so this is an ordinary tab on another folder and omp knows nothing about it. omp's own `/wt` also runs in RPC mode, but it moves the session without telling the desktop: `live.js` keeps `entry.path`, so the git browser, terminal and checkpoints would stay on the old folder. Do not route this through `/wt` without reporting the move first.

- Location: `~/Documents/NPi/Code/.worktrees/<repo>-<hash>/<slug>`: worktrees hold the user's work, so they sit beside the Code projects. Not the command line's sibling `.agent-worktrees` (PR checkouts, task isolation), which `npi worktree clear` empties. Worktrees made by builds before 3.0.0.136 stay in `~/.local/share/npi/worktrees/` (Git records their paths, so they are not moved) and are no longer grouped as tasks.
- Uncommitted changes stay in the main checkout. Ignored files (`.env`, `node_modules`) are not copied: the agent sets them up.
- There is no registry. `worktree_info` answers from that folder and Git; a worktree the user made elsewhere is an ordinary project.
- `_startProjectSession` stores the answer as the tab's `worktree`. `project-nav.js` `homeOf` groups the tab under `worktree.main`, and it is not added to the recent projects. `findProjectTab` still matches exact paths, so a recent project never opens onto a worktree tab.
- The git browser head shows the state and acts on it. **Merge** runs `git merge` in the main checkout, into whatever branch it is on; it needs a committed worktree, and a conflict is aborted so the main checkout stays as it was. **Open PR** asks the Agent. **Remove** closes the tabs first, because a running omp must not outlive its folder, and asks only when uncommitted files or unmerged commits would be lost.

## Review (`design/git/file-view.jsx`, `app/review-comments.js`, `git_browser/hunk.rs`)

Hovering a line of the working-tree view shows `+` (comment) and, on the first line of a change in a modified file, `↩` (undo).

- Comments are per project in memory, surviving the overlay but not the app. **Send to Agent** formats them in file and line order and goes through `handleSend`, so a running turn queues it as any prompt. A comment shows only on the line it was written on: if the line changes it disappears from view, but it is still sent, with the code it quoted.
- Undo recomputes the view in Rust and requires the named line to still have the same numbers and text, so a stale screen refuses instead of undoing something else. It rewrites the work-tree file only (the index keeps what was staged) and keeps CRLF in a CRLF file. Added, deleted and renamed files have one change: **discard** handles them.
- Staging is per file. There is no per-change accept: the view is against `HEAD`, so a staged change would look the same.

## File checkpoints (omp `session/file-checkpoints.ts`)

Rewind on its own moves only the conversation. The kernel owns the snapshots, so the CLI and the desktop share them: before dispatching a user prompt, `AgentSession` copies the project's files into a content-addressed store under `<agent dir>/checkpoints/<hash of the folder>/` and appends a `file-checkpoint` custom entry just before the prompt.

- No Git process runs, and the project's `.git` is never used: omp allows Git only through its native `vcs` wrapper, which has no separate work tree. A folder without Git works the same.
- The snapshot is tied to the prompt by the session tree, not by time, so it follows the conversation through a branch or a fork. A prompt with none (sent during a turn, a slow snapshot) rewinds the conversation only.
- The prompt waits at most 2 s, holding the in-flight claim. A slower snapshot finishes in the background unused, and its manifest makes the next one quick through size and mtime.
- Off in Chat and Academic (`NPI_APP_MODE`), in subagents, without a session file, and with `checkpoints.files: false`.
- `handleRewind` asks omp with `plan_file_restore`; with changed files, `RewindDialog` offers conversation only or files too, focused on the choice that changes no file, and `restore_files` runs before `branch`. In the CLI, Esc Esc to a prompt rewinds first and then asks. `/tree` and `/branch` do not ask.
- A restore snapshots the present first, but nothing offers to undo it yet.
- Limits: files ignored by the project, `SKIPPED_DIRS`, symlinks and files over 10 MiB are not kept; a commit the agent made stays, only the files go back; several tabs on one folder share its files; 200 snapshots. A store that would pass 2 GiB stops for good (a `full` marker), so a run in a home folder cannot fill the disk; delete that folder's store to start again.
