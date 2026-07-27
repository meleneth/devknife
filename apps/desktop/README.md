# devknife desktop

Initial Tauri + Vue + shadcn-vue shell for devknife.

## Commands

```sh
npm install
npm run build
npm run dev:web
npm run dev:tauri
```

`dev:web` runs the Vue shell in a browser, but repository operations require the Tauri backend.
Backend failures are shown explicitly rather than replaced with synthetic workflow data.

`dev:tauri` runs the desktop shell and invokes Rust commands from `src-tauri`, backed by `devknife-core`.
The shell includes a workflow YAML editor with validation, guarded saving, and protection against
overwriting files that changed on disk. Writable source paths are confined to
`examples/workflows` by the Tauri backend.
Saves are staged in the workflow directory and flushed before the live file is replaced.
Unsaved editor changes also trigger the platform close/navigation warning.
Validation distinguishes YAML syntax failures from semantic workflow errors and reports source
locations when the parser provides them.
Desktop runs that request write-capable effects require explicit confirmation before execution.
The confirmed capability IDs are passed to the engine as an exact per-run allowlist.
Completed runs open their trace automatically, with free-text filtering across event, effect, and
payload details.
Desktop runs persist the same `runs/<run_id>.trace.json` artifacts as CLI runs.
The recent-runs panel can reopen persisted CLI or desktop reports in the trace inspector.
The environment selector chooses repository-confined runtime bindings while showing only service,
value, and secret-reference counts. Changing it also preflights the visible run plan against those
bindings. Runs remain disabled until that backend plan succeeds for the currently selected workflow
and environment; failed preflights do not display synthetic fallback plan data.
Unsaved editor changes invalidate that plan until the workflow is saved, validated, and planned
again, and stale plan details are cleared from the UI immediately.
Refreshing workflow or environment lists preserves the current selection while it still exists.
Concurrent startup and planning loads keep controls disabled until every active operation finishes.
Environment discovery failures are shown beside the selector instead of silently appearing as an
empty environment list.
Workflow source reads ignore stale responses from earlier selections and lock the editor until the
current file finishes loading.
Validation and saving operate on an exact source snapshot; editing or switching workflows cannot
apply a stale validation result to different content.

On Linux, Tauri requires WebKit/GTK development packages. See `../../docs/008-devcontainer-and-tooling.md`.
