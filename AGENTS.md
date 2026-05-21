# AGENTS.md

This is the canonical agent guidance file for the Nirvana repository. Other
agent-specific files should point here instead of duplicating these instructions.

## Core Rules

- Do not hardcode variables, credentials, paths, ports, user data, or prompt
  values when production code should derive them from configuration, inputs, or
  the domain model.
- Do not leave placeholders, TODO-driven implementations, fake data, or
  scaffolding in production paths unless the task explicitly asks for a stub.
- Respect the existing worktree. Do not revert user changes, and keep edits
  scoped to the request.
- Clean after yourself: remove temporary files, debug output, local screenshots,
  generated scratch artifacts, and unused dependencies before finishing.
- Prefer project conventions over new abstractions. Add abstractions only when
  they reduce real complexity or match an established pattern.

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

```text
crates/
  nirvana-core/   - shared domain logic, SQLite persistence, config, paths
  nirvana/        - CLI (clap) and Ratatui TUI binary
  nirvana-gui/    - Tauri v2 desktop GUI with a Vue 3 frontend
```

`nirvana-core` owns shared domain and data behavior: `NirvanaApi`, connection
and ticket domain types, SQLite storage through `rusqlite`, TOML config,
credentials, integrations, and platform-aware paths. CLI, TUI, and GUI backend
work should use this crate instead of duplicating business logic.

`nirvana` is the terminal app. Running without arguments launches the Ratatui
TUI. With arguments it exposes Clap subcommands for app info, connections,
publishing, task timing, listing, editing, starting, stopping, and deleting.

`nirvana-gui` is the desktop app. The frontend lives under
`crates/nirvana-gui/src/`; the Tauri Rust crate lives under
`crates/nirvana-gui/src-tauri/` and is a workspace member.

## Rust Guidance

- Use workspace-level commands from the repository root unless a narrower command
  is clearly better.
- Keep persistent data access, migrations, domain rules, config, credentials, and
  integration logic in `nirvana-core`.
- Keep CLI argument parsing and terminal presentation in `nirvana`; delegate
  shared behavior to `nirvana-core`.
- Keep Tauri commands thin. They should validate/translate app inputs, call
  shared domain APIs, and return frontend-friendly results.
- Avoid panics in normal user-facing flows. Prefer typed errors and clear
  propagation through existing error patterns.
- When changing storage behavior, update migrations and repository code together
  and test the behavior through public APIs where possible.

## GUI Guidance

`crates/nirvana-gui/` uses Bun for JavaScript package management.

| Task | Command |
|------|---------|
| Install JS deps | `just gui-install` |
| Frontend dev server only | `just gui-dev` |
| Full Tauri dev | `just gui-tauri-dev` |
| Build desktop app bundle | `just gui-build` |
| Type-check frontend | `just gui-typecheck` |

- The frontend stack is Vue 3 with Composition API and `<script setup>`, Pinia,
  TypeScript, and Tailwind CSS v4 through the Vite plugin.
- The Vite dev server runs on strict port `1420`. If the port is unavailable,
  fix the conflicting process or choose an explicit approved workflow rather
  than silently changing the project default.
- Keep stateful UI behavior in Pinia stores or composables when it is shared.
  Keep components focused on presentation and local interaction.
- For GUI bugs, use Playwright/browser tooling to inspect the rendered app and
  browser logs before guessing at runtime failures. Reproduce the issue, inspect
  the logs, then fix and verify in the browser.
- Do not add visible instructional text, placeholder panels, or fake flows in
  the production UI unless the user explicitly asks for them.

## Verification

- Run the narrowest relevant check first while iterating.
- For Rust-only changes, prefer `just check`, then `just test` or `just lint`
  when behavior or warnings are affected.
- For frontend TypeScript/Vue changes, run `just gui-typecheck`. Run
  `just gui-build` when bundling, Tauri integration, or production output could
  be affected.
- For cross-crate changes, run `just check` and the relevant targeted tests.
- For docs-only changes, confirm file contents and `git status --short`; builds
  are not required unless settings or generated files were touched.

## Git And Generated Files

- Keep generated outputs out of commits unless they are intentionally tracked.
- Do not commit `target/`, `node_modules/`, `dist/`, `.vite/`, temporary browser
  artifacts, local screenshots, or debug logs.
- If dependencies change, update the appropriate lockfile with the package
  manager used by that part of the repo.
- Before finishing, check `git status --short` and make sure only intended files
  changed.
