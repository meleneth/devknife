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

## Deferred Tooling

Some heavy desktop-specific Linux headers and runtime details may still be tuned when Phase 8 begins (desktop UI exploration). They are intentionally not treated as a hard runtime dependency of the product at bootstrap.

## Troubleshooting

- If Node or Rust versions appear missing, rebuild container: `Dev Containers: Rebuild Container`.
- If package caches become inconsistent, remove the named Docker volumes and rebuild.
- If extension recommendations do not apply, confirm both `.devcontainer/devcontainer.json` and `.vscode/extensions.json` are loaded in the container context.
