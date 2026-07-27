# 008 Devcontainer And Tooling

Status: Draft

## Purpose

The devcontainer is a development environment only. It is not the runtime model of the product.

## Included Tooling

- Rust stable toolchain (`rustc`, `cargo`)
- Rust components: `rustfmt`, `clippy`
- Node.js LTS
- `pnpm`
- Git, curl, build utilities, SSL headers
- Common editor extensions for Rust/TOML/YAML/Markdown/Vue

## Open Project In Devcontainer

1. Install Docker Desktop (or equivalent container runtime).
2. Install VS Code extensions:
   - Dev Containers
   - GitHub Copilot Chat (optional but typical for this repo)
3. Open the repository folder in VS Code.
4. Run command: `Dev Containers: Reopen in Container`.
5. Wait for image build and container startup.

## Verify Toolchain

Run inside the container:

- `rustc --version`
- `cargo --version`
- `rustfmt --version`
- `cargo clippy -V`
- `node --version`
- `pnpm --version`

## Caching

The devcontainer uses named volumes for:

- cargo registry
- cargo git index
- rustup toolchains
- cargo target directory (via `CARGO_TARGET_DIR`)

This reduces repeated download/build cost.

## Desktop Tooling

The desktop app lives in `apps/desktop` and uses Tauri + Vue + shadcn-vue.

Useful frontend checks:

- `npm --prefix apps/desktop run build`
- `npm --prefix apps/desktop run dev:web`
- `npm --prefix apps/desktop run dev:tauri`

On Linux, Tauri requires native WebKit/GTK development packages. If `cargo check` reports missing
`pkg-config`, D-Bus, GTK, or WebKit packages, install the platform prerequisites before building.
For Debian/Ubuntu-style systems, Tauri documents:

```sh
sudo apt update
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

See the official Tauri prerequisites: https://tauri.app/start/prerequisites/

## Troubleshooting

- If Node or Rust versions appear missing, rebuild container: `Dev Containers: Rebuild Container`.
- If package caches become inconsistent, remove the named Docker volumes and rebuild.
- If extension recommendations do not apply, confirm both `.devcontainer/devcontainer.json` and `.vscode/extensions.json` are loaded in the container context.

## Current Implementation Checks

The initial Rust workspace can be checked with:

- `cargo fmt`
- `cargo test`
- `cargo clippy --all-targets --all-features`
- `cargo run -p devknife-cli -- run examples/workflows/bootstrap.workflow.yaml`
- `npm --prefix apps/desktop run build`
- `docker compose -f testbed/docker-compose.yml config`

The Docker Compose testbed is a development fixture for future protocol adapters, not a product runtime requirement.

## Sandbox Notes

The devcontainer uses relaxed Docker security options (`seccomp=unconfined` and `apparmor=unconfined`) so tools that rely on Linux user namespaces, including `bwrap`-based sandboxes, can create nested namespaces. After changing these settings, rebuild the devcontainer rather than only reloading the window.
