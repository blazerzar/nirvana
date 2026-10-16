# Roadmap to 1.0

Stage-by-stage checklist for 1.0. Decisions, design summary and analysis live
in [implementation-guide-1.0.md](implementation-guide-1.0.md).

Main goals that need to be achieved before shipping 1.0:

- rewrite and refactor core (async + SeaORM), introduce tests, new features,
- replace Tauri with Iced,
- add Leptos into nirvana binary for local web and API server,
- have automatic releases and testing using CICD.

To achieve this, we will first start clearing old stuff, then we will modify
some and finally start adding. Each module will be worked on individually. We
must keep migration from v0.3.0-alpha to v1.0.0 fully compatible (DB and
config.toml).

We will replace some tools:

- Tombi with dprint (TOML, YAML, JSON),
- Justfile with Mise tasks (mise.toml already exists, cross platform),
- perhaps some more if we find more appropriate tools.

TUI is dropped for 1.0; the binary is CLI + GUI + local web server.

We will be coding this by hand, using an agent only for review, guidance and
question answering.

## Prep

- [x] Get CI going at least for tests (fmt, clippy, test on macOS, Windows, Linux).
- [x] Get git hooks using hk for linting and formatting.
- [x] Use conventional commits from now on (commit-msg check in hk, PR title
      check in CI).
- [x] Sign commits and tags with SSH key (commit.gpgsign, tag.gpgsign).
- [x] Set GitHub repository settings and rulesets for main and dev-1.0
      (implementation guide §12).
- [x] Move justfile recipes to mise tasks.
- [x] Use dprint for formatting.
- [x] Update AGENTS.md with new tooling.

## Cleanup

- [x] Remove not needed config files.
- [x] Remove GUI crate - idle detection will be taken from main later, tray and
      menu bar code will be rewritten.
- [x] Remove TUI stub with ratatui and crossterm.
- [x] Remove not needed git attributes.
- [x] Remove Zed local settings - we may introduce this back later.
- [x] Remove the agent.md pointer if not needed.
- [x] Update local deps.

## Logging

Created before we start changing code, so DB and settings migrations are
logged from the start.

- [ ] Add logging module.

## Database

- [ ] Back up the DB and config.toml before running migrations.
- [ ] Make core async (tokio, async reqwest).
- [ ] Add SeaORM.
- [ ] Bridge existing user_version migrations (v1, v2) into SeaORM baseline
      that detects 0.3 DB.
- [ ] Add profiles table - single DB, existing data becomes one profile with
      all connections.
- [ ] Add settings to connections - publish and squash columns, replacing
      global publish_squashed_worklogs.
- [ ] Add profile settings (week start, time format, workday, expected hours,
      weekends, portal toggle + url).
- [ ] Add uuid to slots (needed for idempotent publishing to portal).
- [ ] Add publish entries table (target, remote id, status, error) with slot
      link - covers worklog id, multiple targets and per log errors.
- [ ] Migrate config.toml: active connection obsolete, squash flag into
      connections, machine settings for desktop app.
- [ ] Remove note column (decide if notes get exported first).
- [ ] Add DB migration tests: fixture DB created by 0.3.0-alpha (connections,
      credentials, tickets, slots incl. running and published) migrates
      correctly to the new schema without data loss.
- [ ] Add config migration tests: legacy flat and 0.3.0-alpha sectioned
      config.toml fixtures migrate correctly (squash flag into connections,
      active connection dropped, machine settings kept).
- [ ] Keep these fixtures in the repo and run them in CI so every future
      migration is tested against today's data.
- [ ] Check if we need SQL formatter (without bringing back Node).

## Keychain

- [ ] Add support for storing connection tokens in keychain (keyring crate).
- [ ] Support macOS, Windows and Linux.
- [ ] Migrate plaintext tokens from credentials table into keyring and delete them.
- [ ] Env override NIRVANA_TOKEN_<NAME> for CI and headless use.

## Tickets

- [ ] Make sure start, stop, resume, switch, edit, delete are working and tested.
- [ ] Get these commands working in CLI and start integration testing as well.
- [ ] Decide how owning connection is resolved when profile has multiple
      connections (proposal in the implementation guide).
- [ ] Let's update ticket name and description when interacting with it (async).
- [ ] Validate overlaps on edit; published slots are read-only.

## Linear

- [ ] Add scripts for obtaining Linear ticket information.
- [ ] Rewrite scripts for JIRA and Linear and clean them up.
- [ ] Add Linear integration - GraphQL, ticket name + description, workspace
      and user detected from key.
- [ ] Publish time to Linear as issue comment.

## Publishing

- [ ] Make sure publishing works.
- [ ] Publish to every connection with publish enabled, squash per ticket per
      day when enabled.
- [ ] Store ID of the worklog / comment, nirvana marker to avoid duplicates on
      retry.
- [ ] No rounding - Jira rejects worklogs under 60 s, such entry fails with
      error and stays unpublished.
- [ ] Partial publish state (published to some targets) derived from publish
      entries, retry is just publish again (see the implementation guide).
- [ ] Get publishing ready in CLI (--target).
- [ ] Make sure we are able to publish from any range (today, all).
- [ ] Need error checking for each log.

## Nirvana API

- [ ] Check if we should rework the API interface of the core.
- [ ] The same goes for DB and config API.
- [ ] Separate profile settings from machine settings.
- [ ] Export logs to CSV and JSON.
- [ ] Connection health checks (ping every 60 s, latency, auth status).
- [ ] Change notification so CLI, desktop and web stay in sync.

## CICD

- [ ] Need to start releasing automatically (release-plz or git-cliff from
      conventional commits).
- [ ] Sign release tags in CICD.
- [ ] This is the first time we are having a full standalone result.

## Stage 1 review

This will be the first milestone: complete core and CLI. We will do a
thorough review to make sure we are not bringing any problems into the next stage.

## Design tokens

- [ ] Single Rust source of v6 tokens (light, dark) used by Iced theme and
      generated web CSS variables.
- [ ] Bundle JetBrains Mono and Inter fonts.

## API

- [ ] Need to start building the API that will be used to control Nirvana from web.
- [ ] Analyze design if we should put it in nirvana or core.
- [ ] Axum server bound to 127.0.0.1, Host and Origin checks, never return tokens.
- [ ] Default port 5167, configurable.

## GUI

- [ ] Create a GUI application using Iced.
- [ ] We need to support CLI and GUI in the same binary.
- [ ] We need to make sure such binary works on macOS (.dmg), Windows and Linux.
- [ ] Start building screens: main (tracking, not tracking), start / switch
      form, publish modal, overview, settings, first launch.
- [ ] Menu bar support (macOS).
- [ ] Tray menu support (Windows).
- [ ] Global shortcuts, launch at login, close to menu bar.
- [ ] Idle detection (take from main), not tracking reminder.
- [ ] GUI will have its own settings that are related to the GUI application.

## Webserver

- [ ] Create web page using Leptos (decide CSR or SSR, assets embedded in binary).
- [ ] Make it possible to toggle it from the settings.
- [ ] Start building screens: setup, week, month, edit drawer, profiles and
      connections, add connection, new profile, settings.
- [ ] Responsive layouts (below 1200 px and 720 px).

## CICD

- [ ] Continue with deployment goals.
- [ ] macOS signing and notarization, Windows signing, Linux artifact.

## Stage 2 review

This will be the second milestone: finished GUI and web page. We will do a
thorough review to make sure we are not bringing any problems into the next
stage. We also decide the Portal stack here (Leptos or Phoenix).

## Nirvana Portal

We start building a standalone application for storing worklogs. This will be
a big project that will tie everything together:

- [ ] Possible to store Linear worklogs.
- [ ] Admin is able to manage users, see their logs, create and download reports.
- [ ] Users can tag special days (vacations, sick).
- [ ] Users can create templates for sending monthly emails to staff.
- [ ] Users can login using username/password or Microsoft SSO.
- [ ] Application needs to be containerized.

Leptos or Phoenix, decided at the stage 2 review (analysis in the
implementation guide). The
API contract stays language neutral (OpenAPI, upserts by slot uuid) so either
works.
