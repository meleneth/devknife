# devknife (working title)

This repository defines a file-backed, cross-platform service workflow runner for developers. The core model is event-native execution: workflows start from seed events, handlers produce effects, effects produce observations, observations may emit more typed events, and every run yields a causal trace explaining what happened and why.

Status: Bootstrap and planning only.

## What Exists In This Repository

- Product thesis and domain model docs.
- Explicit system invariants and non-goals.
- Architecture decision records (ADRs).
- Draft roadmap from bootstrap to v1.
- Devcontainer setup for Rust + Node toolchains.

## What Does Not Exist Yet

- No workflow engine implementation.
- No CLI implementation.
- No desktop UI.
- No Tauri application.
- No plugin system.
- No hosted service.

## Open In Devcontainer

1. Install Docker Desktop (or another compatible container runtime).
2. Install VS Code with the Dev Containers extension.
3. Open this folder in VS Code.
4. Run: Dev Containers: Reopen in Container.
5. Verify toolchains:
   - `rustc --version`
   - `cargo --version`
   - `rustfmt --version`
   - `cargo clippy -V`
   - `node --version`
   - `pnpm --version`

See docs/008-devcontainer-and-tooling.md for details and troubleshooting.

## Read Next

1. docs/000-product-thesis.md
2. docs/002-invariants.md
3. docs/003-execution-model.md
4. docs/009-roadmap-bootstrap-to-v1.md
5. docs/010-open-questions.md

## Syntax Warning

Any syntax examples in this repository are illustrative draft shapes, not final language or schema commitments.
