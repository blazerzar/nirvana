# AGENTS.md

This is the agent guidance file for the Nirvana repository.

## 1.0 development

Work toward 1.0 is tracked in two files under `docs/`:

- `docs/roadmap-1.0.md` - the stage-by-stage checklist.
- `docs/implementation-guide-1.0.md` - decisions, design summary (claude.ai
  design "Nirvana v6"), target data model, analyses and open questions.

Read both before working on 1.0 and keep them up to date as decisions change.
The maintainer writes the code by hand; agents review, guide and answer
questions, and do not write implementation code unless explicitly asked.

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

## Tooling, hooks and formatting

Tool versions are pinned in `mise.toml` (`mise install`). Git hooks run through
[hk](https://hk.jdx.dev), configured in `hk.pkl`.

| Task | Command |
|------|---------|
| Install git hooks | `mise run setup` |
| Run all checks | `mise run lint` |
| Fix formatting | `mise run fix` |

- Formatting: `cargo +$RUSTFMT_TOOLCHAIN fmt` (Rust, pinned nightly rustfmt
  for import grouping, installed by `mise run setup`), dprint (TOML, YAML, JSON, `dprint.json`),
  `pkl format` (Pkl). Indentation and line endings follow `.editorconfig`.
- Linting: clippy with `-D warnings`, jactionlint (GitHub workflows, actions
  pinned to commit SHAs), editorconfig-checker.
- `pre-commit` fixes formatting, `pre-push` runs all checks, `commit-msg`
  enforces conventional commits. CI runs the same `hk check`, and the
  `pr-title` workflow checks that the PR title is a conventional commit.

## Architecture

This is a Cargo workspace with four crates:

```text
crates/
  nirvana-core/        - shared domain logic, SQLite persistence, config, paths
  nirvana-entity/      - SeaORM entities, one file per table
  nirvana-migration/   - SeaORM migrations (Migrator)
  nirvana/             - CLI (clap) binary
```

`nirvana-core` owns shared domain and data behavior: `Nirvana`, connection
and ticket domain types, SQLite storage through SeaORM, TOML config,
credentials, integrations, and platform-aware paths. CLI work should use this
crate instead of duplicating business logic.

`nirvana-entity` and `nirvana-migration` hold the schema. Entities also hold
shared queries (methods returning a `Select`) and row hooks in
`ActiveModelBehavior::before_save` (timestamps, row validation). `nirvana-core`
opens the database, backs it up and runs the migrations. Migrations are only
run by the application.

`nirvana` is the terminal app. It exposes Clap subcommands for app info,
connections, publishing, task timing, listing, editing, starting, stopping,
and deleting.

## Rust Guidance

- Use workspace-level commands from the repository root unless a narrower command
  is clearly better.
- Keep the schema, shared queries and row hooks in `nirvana-entity` and
  `nirvana-migration`. Keep the rest of data access, domain rules, config,
  credentials, and integration logic in `nirvana-core`. Only `nirvana-core` depends on the entity and migration
  crates.
- Keep CLI argument parsing and terminal presentation in `nirvana`; delegate
  shared behavior to `nirvana-core`.
- Avoid panics in normal user-facing flows. Prefer typed errors and clear
  propagation through existing error patterns.
- When changing storage behavior, update migrations and entities together
  and test the behavior through public APIs where possible.

## Verification

- Run the narrowest relevant check first while iterating.
- For Rust changes, prefer `cargo check`, then `cargo test` or `mise run lint`
  when behavior or warnings are affected.
- For cross-crate changes, run `cargo check` and the relevant targeted tests.
- For docs-only changes, confirm file contents and `git status --short`; builds
  are not required unless settings or generated files were touched.

## Git And Generated Files

- Keep generated outputs out of commits unless they are intentionally tracked.
- Do not commit `target/`, temporary browser artifacts, local screenshots, or
  debug logs.
- If dependencies change, update the appropriate lockfile with the package
  manager used by that part of the repo.
- Before finishing, check `git status --short` and make sure only intended files
  changed.

## Reviewing PRs

Besides correctness, check that:

- The PR title is a conventional commit (`type(scope): summary`).
- The PR covers one roadmap item and ticks its checkbox in
  `docs/roadmap-1.0.md`.
- Changes are tested.
- Changes affecting 0.3.0-alpha DB or config compatibility update the
  fixtures and migration tests.
- `docs/roadmap-1.0.md` and `docs/implementation-guide-1.0.md` are updated if
  a decision changed.
