# AGENTS.md

This file provides guidance to Codex when working with code in this repository.

## Commands

All recipes use `just` as the task runner (`cargo install just`).

| Task | Command |
|------|---------|
| List all recipes | `just` |
| Build (debug) | `just build` |
| Build (release) | `just build-release` |
| Run CLI | `just run` |
| Run CLI (release) | `just run-release` |
| Test all | `just test` |
| Type-check all | `just check` |
| Lint | `just lint` |
| Format check | `just fmt` |
| Clean | `just clean` |

## Architecture

This is a Cargo workspace with three crates:

```
crates/
  nirvana-core/   — shared domain logic, SQLite persistence (rusqlite), config, paths
  nirvana/        — CLI (clap) + TUI (ratatui) binary
  nirvana-gui/    — Tauri v2 desktop GUI (Vue 3 frontend)
```

**`nirvana-core`** owns the data layer: `NirvanaApi` struct, `Connection` domain type, SQLite via `rusqlite`, TOML config, and platform-aware paths. All other crates depend on this.

**`nirvana`** is the terminal app. No args → launches the Ratatui TUI. With args → Clap CLI (subcommands: `info`, `connection add/list/use`).

**`nirvana-gui`** is the Tauri desktop app. Vue 3 + TypeScript frontend under `src/`, Tauri Rust crate under `src-tauri/`. Currently uses in-memory seed data; will be wired to `nirvana-core` once the backend domain model is complete.

## nirvana-gui (Desktop GUI)

Located at `crates/nirvana-gui/`. Requires `bun` for JS package management.

| Task | Command |
|------|---------|
| Install JS deps (once) | `just gui-install` |
| Frontend dev server only | `just gui-dev` |
| Full Tauri dev (recommended) | `just gui-tauri-dev` |
| Build desktop app bundle | `just gui-build` |
| Type-check frontend | `just gui-typecheck` |

The Vite dev server runs on port 1420 (strict — fails if unavailable). `gui-tauri-dev` starts Vite automatically via Tauri's `beforeDevCommand`.

**Frontend stack:** Vue 3 (Composition API, `<script setup>`), Pinia, TypeScript, Tailwind CSS v4 (via Vite plugin).

**Rust crate** (`src-tauri/`) is a workspace member. Currently exposes one Tauri command: `get_app_info`.
