# devknife desktop

Initial Tauri + Vue + shadcn-vue shell for devknife.

## Commands

```sh
npm install
npm run build
npm run dev:web
npm run dev:tauri
```

`dev:web` runs the Vue shell in a browser and falls back to scaffold data when Tauri commands are unavailable.

`dev:tauri` runs the desktop shell and invokes Rust commands from `src-tauri`, backed by `devknife-core`.
The shell includes a read-only source inspector for workflow YAML; source paths are resolved and
confined to the repository by the Tauri backend.

On Linux, Tauri requires WebKit/GTK development packages. See `../../docs/008-devcontainer-and-tooling.md`.
