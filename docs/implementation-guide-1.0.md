# Nirvana 1.0 - implementation guide

Companion to [roadmap-1.0.md](roadmap-1.0.md) (the checklist). This file holds
the context, decisions, design summary and analysis needed to implement 1.0
across many sessions. Keep both files up to date as decisions change.

## 1. Working mode

- The maintainer writes the code by hand. Agents review, guide, answer
  questions and keep this file and the roadmap current. Agents do not write
  implementation code unless explicitly asked.
- Each module is worked on individually, in roadmap order.
- Migration from `v0.3.0-alpha` to `v1.0.0` must stay fully compatible (DB and
  `config.toml`).
- Conventional commits from Prep onward.
- All commits and tags are signed (SSH key, `commit.gpgsign` and
  `tag.gpgsign` on). Never create unsigned commits.

## 2. Target shape of 1.0

One `nirvana` binary:

- **CLI** (clap): start, stop, resume, switch, edit, delete, list, publish,
  export, connections, profiles.
- **Desktop GUI** (Iced): a small tracker (960×540). It only does start, stop,
  resume, switch, publish, overview and its own machine settings. Everything
  else opens in the browser.
- **Local web** (Leptos) at `localhost:5167`, served by an embedded axum
  server. It covers timesheets, editing logs, profiles, connections, profile
  settings and first-run setup.
- No TUI in 1.0.

Later (stage 3): **Nirvana Portal**, a standalone multi-user server for storing
worklogs.

## 3. Decisions

| # | Decision | Why |
|---|---|---|
| 1 | **Single SQLite DB with a `profiles` table.** Every profile-scoped table gets `profile_id`. | Maintainer's choice. Departs from the design, which shows one `.db` file per profile: the web profile card cannot show a per-profile DB path, so show the shared DB path in Settings → data instead. |
| 2 | **0.3 migration: the existing DB becomes one profile** (e.g. `default`) holding all existing connections. Tickets keep their `connection_id`; a slot's connection comes from its ticket (verify `slots.connection_id` always matches before dropping it). In 0.3 slots of different connections can overlap, so the web lets users remove a connection with all its tickets and their slots (Webserver stage). | Simple, lossless. Splitting into more profiles can be done by hand later. Most users have a single connection, so one profile changes nothing for them. |
| 3 | **Async core on tokio + SeaORM**, async reqwest. Use SeaORM as much as possible: the raw SQL migration files are replaced by SeaORM migrations, with `0001` and `0002` rewritten as the baseline. | axum, Leptos and Iced tasks are async anyway, and SeaORM is async-only. |
| 4 | **TUI dropped for 1.0.** Remove the ratatui stub and the ratatui/crossterm deps. | No TUI tasks or v6 screens exist. Can come back after 1.0. |
| 5 | Tickets keep a `connection_id` FK. **Open:** how the owner is picked when a profile has several connections (see §9). | |
| 6 | **No rounding anywhere.** Jira rejects worklogs under 60 s, so a sub-minute entry **fails that entry with a visible error** and stays unpublished. | Design product rule: never silently alter time. |
| 7 | Git hooks with **hk** (+ mise). **Mise tasks** replace the justfile. **dprint** (TOML, YAML, JSON) replaces Tombi and sql-formatter, so Node is no longer needed. Tool versions are pinned in `mise.toml`. | Cross-platform, one tool manager, one formatter. |
| 8 | **Idle detection** is restored from `main` (`crates/nirvana-gui/src-tauri/src/idle/{mod,macos,windows}.rs`) when the Iced app needs it. **Tray and menu-bar code is rewritten** for Iced. The GUI crate can be deleted without porting anything first. | |
| 9 | **Portal stack: Leptos vs Phoenix, decided at the Stage 2 review** (see §8). Until then the Portal contract stays language-neutral. | |
| 10 | Local web: **Leptos CSR (Trunk) + axum REST API**, assets embedded with `rust-embed`. *Recommended, confirm when stage 2 starts.* | Simpler single-binary build than SSR/hydration. The explicit API matches the roadmap's API section. |
| 11 | **Migration tests are mandatory**: 0.3.0-alpha DB and config fixtures are checked in, never edited, and run in CI (§5). | Guarantees the compatibility promise. |
| 12 | **Linear history only, no merge commits.** Feature PRs are squash-merged into `dev-1.0`, `main` is only fast-forwarded to `dev-1.0`. Code review is done by agents on the PR (§12). | |
| 13 | **Logging with `tracing`** to a single `nirvana.log`, **no rotation**. On macOS the log lives in `~/Library/Logs/nirvana/` (other locations in `paths.rs` module docs). Levels: `info` for user issued commands and state changes (start, stop, publish, DB and config migrated, config `X` changed from `a` to `b`), `warn`, `error`, `debug` for everything else, `trace` for dev only logs (compiled out in release with `release_max_level_debug`). Only our crates are logged, at `info` by default (`trace` in dev mode). `[log] level` in `config.toml` (new in schema version 2, written only when set) sets the level for our crates only and overrides the dev mode default. `NIRVANA_LOG` (full filter directive) overrides both and is the only way to log dependencies (sqlx, sea-orm). Info events live in `Nirvana` so every frontend logs the same. `nirvana-core` owns the log file, format and filters and exposes them as a `Layer` (`logging::file_layer`). The `nirvana` binary only installs it and sets the max level feature, as `tracing` recommends for libraries. Stdout output for GUI dev comes later. | macOS convention. Level scheme follows `tracing` verbosity order. |
| 14 | **One running slot per profile**, enforced by `unique (profile_id) where stopped_at is null`. Getting, starting and stopping the running slot works on the active connection's profile, so stopping needs an active connection. | Easier to show the state to the user than one running slot across profiles. |
| 15 | **Legacy detection + backup.** Before applying pending migrations to an existing DB, the DB is copied to `data_dir/backup` with `VACUUM INTO`, a consistent single-file snapshot that includes rows still in the `-wal` file. The baseline migrations `m0001` and `m0002` skip their work when `user_version` is 2, which every released 0.x DB has. Every step is logged. The config does the same in the Config stage. | Never lose user data on upgrade. |
| 16 | **Plaintext credentials stay supported** (probably needed on Linux). The `credentials` table and `connections.secret_store` stay in the Database stage. The keyring comes in the Keychain stage. | |
| 17 | **Canonical SeaORM layout in two crates**: `nirvana-entity` (one file per table in the 2.0 `#[sea_orm::model]` format, plus `prelude.rs`) and `nirvana-migration` (`Migrator`, one file per migration, no `cli` feature or binary). `nirvana-core` depends on both and owns opening the DB, the backup and running `Migrator::up`. `nirvana` depends only on `nirvana-core`. No repository layer: services call entities directly. Table rows are entity `Model`s, which `nirvana-core` re-exports as `nirvana_core::entity`. `domain.rs` holds only types that are not a table row (inputs, results, `Change<T>`, `Connection`). Connections are exposed as `domain::Connection`: the `connections` row plus `ConnectionKind`, which interprets the `kind` column and carries that kind's settings. Slot services return `(slot::Model, ticket::Model)` and frontends build the issue link with `Nirvana::issue_urls`. Shared queries are entity methods that return a `Select`, and timestamps are set in `ActiveModelBehavior::before_save`. | Matches SeaORM's canonical layout. Migrations are only run by the application, never from a CLI. |
| 18 | **`profile_id` columns are `not null references profiles(id)` with no default, added with a table rebuild.** The migration returns `use_transaction() -> Some(false)` and skips itself when its work is already done. It turns `PRAGMA foreign_keys` off, then in one transaction: creates `<table>_new`, copies the rows with the `default` profile id, drops the old table, renames `<table>_new` to `<table>`, recreates the indexes and fails if `PRAGMA foreign_key_check` returns rows. After the commit (and on error) it turns `foreign_keys` back on. Tests check that the rows of every child table survive. | SQLite cannot add a `NOT NULL` column without a default, nor a `REFERENCES` column with a non-NULL default. A default would silently put rows in one profile. `PRAGMA foreign_keys` is ignored inside a transaction, and with it on `DROP TABLE` deletes the rows first, which fails on `tickets` and `slots` and cascades into `credentials`. This is the procedure from sqlite.org/lang_altertable.html. |
| 19 | **Keychain.** `secret_store` moves from `connections` to `credentials` (`not null default 'plaintext'`) and is `plaintext`, `keyring` or `env`. `credentials.credential` holds the token for `plaintext`, the keyring account `nirvana_<profile>_<connection>_<key>` with a random 8 hex digit key for `keyring` and the name of the environment variable holding the token for `env`. The token input is required for every store, and for `env` it is the variable name, which must start with `NIRVANA_`. Core rejects other names when adding and when reading a connection, so a foreign DB cannot send an arbitrary environment variable to its host. `keyring` stores the token under service `nirvana` and that account, through `keyring-core` with `apple-native-keyring-store` (macOS), `windows-native-keyring-store` (Windows) and `zbus-secret-service-keyring-store` with `rt-tokio-crypto-rust` (Linux, Secret Service: gnome-keyring, KWallet on Plasma 6, KeePassXC). `env` is for CLI integration tests, which run the built release binary with Jira and Linear tokens from GitHub secrets, so the tokens never reach the DB. It is available in all builds and hidden from `--help` and the interactive picker. `nirvana info` creates the keyring store without touching any item and shows the backend or why it is unavailable. Plaintext tokens are not migrated: users move them into the keyring themselves. Tokens are `secrecy::SecretString`, keyring calls run in `spawn_blocking`. Core takes the store as a factory, called inside every `spawn_blocking` keyring call. Tests use a factory that returns one shared `keyring_core` mock store, and `#[ignore]` tests against the real keyring are run manually on full machines. | The store and the credential are read together, and moving a token updates one row. The profile and connection names make entries readable in the keyring UI, and the random key keeps dev and production DBs apart in one keyring. The account is stored, not rebuilt from names, so renaming does not break reading. The Secret Service covers both GNOME and KDE. `keyring-core` is synchronous and providers can show unlock dialogs. The store uses the zbus blocking API, and with `rt-tokio` zbus runs it on its own tokio runtime, so creating, using or dropping the store from inside our runtime panics. Every store call, including creating and dropping it, runs in `spawn_blocking`, so `Nirvana` cannot hold the store and holds the factory instead. A new mock store is empty, so the test factory returns clones of one store to keep entries between calls. |
| 20 | **`connections.identity` is nullable.** It holds the Jira Cloud email, used for basic auth. Jira DC authenticates with the PAT alone, so it has no identity and the CLI does not ask for a username. The profiles migration, which already rebuilds `connections`, creates `identity` as nullable and sets it to NULL for `jira-dc` rows. | A username that is never used for auth is not semantically an identity. |
| 21 | **Errors: `thiserror` in `nirvana-core`, `anyhow::Result` in the `nirvana` CLI.** `main` prints the whole chain with `{e:#}`, so a `#[from]` variant does not repeat its source with `{0}` in its message (or is `#[error(transparent)]`). Domain rules and their errors (future times, missing active connection) live in core, not in the CLI. CLI errors are fixed command by command, when that command is reworked. | The CLI only displays errors and never matches on them. A source printed in the message and again by the chain shows up several times. Rules in core apply to every frontend. |

## 4. Current codebase (0.3.0-alpha) - what exists

- Workspace: `crates/nirvana` (CLI + TUI stub), `crates/nirvana-core`,
  `crates/nirvana-gui` (Tauri v2 + Vue 3, to be removed).
- `nirvana-core`: sync `rusqlite`, blocking reqwest. `Nirvana { paths, config, db }`
  in `src/service/mod.rs`. Modules: `service/` (tracking, publish, edit, delete, slots,
  connection, settings, info), `storage/` (connection, ticket and slot repos;
  migrations `0001_init.sql`, `0002_tickets_slots.sql` run via `user_version`),
  `integration/` (Jira cloud/dc via `kind` = `jira-cloud`/`jira-dc`, REST v2),
  `credentials/` (**tokens stored in plaintext in the `credentials` table**),
  `config.rs` (sectioned `config.toml` with `[core] active_connection,
  publish_squashed_worklogs`, `[gui] font_scale, theme, show_tray_icon`,
  `[idle] enabled, methods, threshold_secs`, plus the 0.1 flat format),
  `paths.rs` (platform dirs via `directories`, `NIRVANA_*_DIR` overrides,
  dev mode = exe dir).
- Schema today: `connections(id, name, kind, host, identity, secret_store,
  created_at, updated_at)`, `credentials(connection_id, credential)`,
  `tickets(id, ticket_key, summary, connection_id, last_worked_at)`,
  `slots(id, ticket_id, connection_id, note, started_at, stopped_at,
  published_at)` with a unique index enforcing one running slot.
- Publishing: squash per ticket per local day (global flag), Jira worklog POST,
  no stored worklog id, a single `published_at`.
- `scripts/jira/*.sh`: manual Jira API exploration scripts.

## 5. Target data model (sketch, refine in the Database stage)

```
profiles(id, name unique, created_at, updated_at)
profile_settings(profile_id, week_start, clock_24h, workday_start, workday_end,
                 expected_minutes_per_day, show_weekends,
                 portal_enabled, portal_url)
connections(id, profile_id, name, kind {jira-cloud, jira-dc, linear}, host,
            identity NULL, created_at, updated_at)
            -- identity: the Jira Cloud email, NULL for jira-dc
            -- unique (profile_id, name)
            -- deleting a profile cascades
credentials(connection_id, secret_store {plaintext, keyring, env},
            credential)
            -- plaintext: the token, keyring: nirvana_<profile>_<connection>_<key>,
            -- env: the environment variable name
jira_settings(connection_id, publish bool default true,
              squash bool default false)
            -- one settings table per connection kind
tickets(id, profile_id, connection_id NULL, ticket_key, title,
        last_worked_at, synced_at)   -- NULL = owner not resolved yet
            -- no unique key besides id
            -- deleting a profile cascades
slots(id, uuid, profile_id, ticket_id, started_at, stopped_at)
            -- one running slot per profile
publish_entries(slot_id, connection_id, published_at NULL, error NULL)
            -- primary key (slot_id, connection_id), one row per slot per target
            -- pending: both NULL, ok: published_at set, failed: error set
            -- deleting a slot cascades, deleting a connection does not
            -- the Portal will probably be a connection
```

- The machine config (`config.toml`) keeps machine-only state: active profile,
  desktop settings (launch at login, close to menu bar, idle, reminder,
  shortcuts, web server on/off + port, theme light/dark/system), log level.
- 0.3 DB migration (Database stage): back up the DB to `data_dir/backup`,
  create profile `default`. Set every connection's `profile_id` and give
  every Jira connection a `jira_settings` row with `publish = true` and
  `squash = true`. New Jira connections get the column defaults
  (`publish = true`, `squash = false`). Add a uuid to every slot (rebuild
  `slots` with a NOT NULL `uuid` BLOB column and a unique `slots_by_uuid`
  index). For slots that already have `published_at`, create an ok
  `publish_entries` row with the slot's `connection_id` and `published_at`.
  Fail if a slot's `connection_id` differs from its ticket's, then replace it
  with the ticket's `profile_id`. Changing a slot's ticket uses the connection
  of the slot's current ticket and fails if that ticket has none.
  `credentials` and `secret_store` stay (plaintext support).
- Config migration (Config stage): back up `config.toml` to
  `data_dir/backup`. `core.active_connection` and
  `core.publish_squashed_worklogs` are dropped, missing sections (0.2.x has
  no `[idle]`) get the 0.3 defaults,
  `[log]` is not written, since its default `level` is unset. A parse error
  fails instead of falling back to defaults. The `config/` module has one
  file per old format (0.3 sectioned). Each only parses TOML and returns the
  latest config interface.
  A flat file (no sections) without `schema_version` is 0.1 and is replaced
  with the defaults. An empty file gives the defaults and is not rewritten. A
  file with sections but no `schema_version` fails.
  Every file older than the current schema (0.1 included) is backed up and
  rewritten on load with only non-default values, through a temporary file
  renamed over `config.toml`. Every migration step is logged at info. A
  missing file is not written. A failed backup or rewrite returns an error. Saving the config
  and using a profile fail on it, every other command only warns.
- Desktop settings migration (GUI stage): the 0.3 `gui.*` and `idle.*`
  settings map to the new machine settings where an equivalent exists, unused
  keys (font scale, tray icon) are removed, old theme names map to
  light/dark/system.
- Keychain stage: plaintext tokens stay where they are. Users move them into
  the keyring themselves (decision 19).
- The note column is dropped (no UI uses it). Notes are dropped without an
  export, and the pre-migration backup keeps them.
- **Migration tests** (required before the Stage 1 review), run in CI:
  - DB: a fixture `nirvana.db` produced by 0.3.0-alpha (`user_version` 2) with
    several connections, credentials, tickets, closed, running and published
    slots. It must migrate to the latest schema with no data loss: one
    `default` profile, connection settings set, uuids assigned, publish
    entries created for published slots, backup created. Also cover a fresh
    DB, and check that a fresh DB and a migrated one end with the same
    schema.
  - Config: a fixture for the 0.3.0-alpha sectioned
    format (`[core]`, `[gui]`, `[idle]`). Check that `active_connection` is
    dropped and machine/idle settings are kept. A 0.2.x fixture (no
    `[idle]`) gets the 0.3 idle defaults, a 0.1 flat fixture gives the
    defaults.
  - Fixtures are checked into the repo and never edited. Each new migration
    adds tests on top of them.

## 6. Design summary (claude.ai design project "Nirvana v6")

Source: project `019de3df-d808-77cf-9f0f-bf2e8f5860a1`, main file
`Nirvana v6.html`. Fetch files with the DesignSync tool (`get_file`). Relevant
files: `v6-theme.jsx` (tokens, logo, press states), `v6-desktop.jsx`,
`v6-web.jsx`, `v6-web-pages.jsx`, `v6-web-narrow.jsx`, `v6-web-setup.jsx`,
`v6-states.jsx`, `v6-canvas.jsx` (artboard list = screen inventory),
`v6-data.js` (sample data), `v4-theme.jsx` (font stacks only), and
`nv-tokens.js`, `nv-type.js`, `nv-spacing.js` (token, type and spacing
sources). `legacy/` is frozen and ignored. `uploads/nirvana-spec.md` is an old
TUI-era spec: useful background, but superseded wherever it conflicts with the
decisions here (rounding, single active connection, …).

### 6.1 Product rules (from the design project's CLAUDE.md)

- A profile has multiple connections, all active. Never name "the" connection
  in profile UI.
- A Jira or Linear connection is required (ticket names + descriptions).
- No rounding anywhere.
- Publishing is a per-connection setting. Each connection kind has its own
  settings (Jira: `publish`, `squash`).
  Nirvana server publishing has its own toggle + url in web settings.
- Default port **5167**.
- Linear has no worklogs, so time is posted as a comment on the issue.
- Tokens and keys live in the OS keyring, never in the DB.
- Publishing happens from the desktop app (and CLI). The web only edits.
- Published logs are read-only.

### 6.2 Desktop screens (960×540)

- **Header**: logo + "nirvana", profile pill (status dot + name), buttons
  `[v] overview`, `[o] open web ↗`, `[,] settings`.
- **Main left pane**: when tracking, shows "tracking · since HH:MM" with a
  pulse dot, the ticket key (40px, accent), the ticket title (or "ticket name
  loads on next sync"), a big h:mm:ss timer (68px), and "today Xh · ⌃⌥X stops
  from anywhere". When not tracking, it shows 0:00:00 dimmed, the last ticket +
  stopped time, recent-ticket chips, and the copy "logs stay on this machine
  until you publish".
- **Start / switch form** (replaces the left pane): ticket input with a `›`
  prefix, a recent list (key + title), and for a switch a dashed row "current
  ticket · stops at HH:MM · elapsed". Buttons `[esc] cancel`, `[↵] start` /
  `stop & start`.
- **Right panel (400px)**: scope segmented control (today / unpublished · N
  days) and view segmented control (by time / by ticket). The list rows are:
  a `►` running row, then `●` unpublished / `✓` published marks, time range,
  key + title, duration. "By ticket" groups rows with a slot count. Unpublished
  scope shows sticky day headers, newest first. Footer: "unpublished · days ·
  total" and "edit logs and see your week in nirvana web ↗". A `«`/`»`
  control expands the list to full width.
- **Bottom bar**: `[s] start`, `[x] stop`, `[r] resume <key>`,
  `[w] switch ⇄ <prevKey>`, spacer, `[p] publish <unpub total>`. Disabled
  states follow availability.
- **Publish modal** (640px): targets + log count + days, a table ticket / logs
  / logged, the total in accent, the note "X is still running and won't be
  published", `[esc] cancel`, `[↵] publish Xh`.
- **Toast** (inverse bg) after stop, switch and publish.
- **Overview**: 3 stat cards (today, week, month against a target derived from
  the profile's expected hours per day; the design shows 8h / 40h / 21×8h), each
  with a published (neutral) / unpublished (accent) bar, then a this-week
  bar chart by day with an 8h dashed line, and a link to web.
- **Desktop settings** ("this machine only"). Window: launch at login
  (no/yes/minimized), close to menu bar. Tracking: detect idle time, idle
  after (1–30 min), detect by (lock / sleep / no input), remind me when not
  tracking (every 15/30/60 min) + toggle. Global shortcuts: ⌃⌥S start/switch,
  ⌃⌥X stop, ⌃⌥O open web. Local web server: run toggle (status) and port.
- **First launch** (no profile): welcome copy, `[↵] open nirvana web ↗`, then
  a waiting state ("waiting for nirvana web…") with `open again`. A 3-step
  side panel (create profile, connect Jira or Linear, start tracking). The
  start/stop buttons stay disabled. The desktop picks up the new profile by
  itself.

### 6.3 Web screens (localhost:5167)

- **Shell**: sidebar 232px (≥1200px). Logo, profile switcher (avatar letter,
  name, status dot + first host; the dropdown has a checkmark, connection
  count and "+ new profile"), nav (Timesheet, Profiles & connections,
  Settings), and a "desktop app" card showing the running ticket + minutes.
  Under 1200px it becomes a top bar with nav + profile dropdown. Under 720px
  it becomes a top bar with ☰, and the menu opens as a full-height sheet.
- **Timesheet week** (`/timesheet/week`): page head with "profile X ·
  publishes to <conn> ●" and "● Xh unpublished · publish from the desktop
  app", plus `+ add log`. Controls: week/month segmented, ‹ label ›, today,
  and a legend (published / unpublished / running). The grid has 52px hour
  gutter + 7 day columns, 08:00–20:00 at 38px/hour, and workday shading
  09:00–17:00 on weekdays. Each day header shows its total and "●
  unpublished" or ✓. Block styles: published = neutral fill; unpublished =
  accent-subtle with a 3px accent left border; running = accent fill. A red
  now-line runs across. Hovering empty space shows a dashed ghost "+ HH:00",
  and clicking it opens the add drawer. Clicking a block opens the edit drawer.
- **Edit/add drawer** (400px, full width on small screens): "log #id" or "add
  log", the date, fields ticket (uppercase, hint = title or "not a recent
  ticket · checked when published"), day select, start, stop, and the
  computed duration. Validation: "use hh:mm", "stop must be after start".
  Actions: delete log, cancel, save / add log. A published log shows "✓
  published to <conn>. published logs are read-only" and close.
- **Month** (`/timesheet/month`): 5×7 cells + a week-total column. Each cell
  has its date (today highlighted), a ● unpublished dot, the total, and a
  bar against 8h. Footer shows the month total. Clicking a cell goes to that
  week. Small screens get a compact month and a day list (day chips + cards +
  "+ add log to <day>").
- **Profiles & connections** (`/profiles`): profile list (active badge) +
  detail ("active on this machine" or "switch to this profile"). Connections
  are listed with "status checked every 60 s" and "+ add connection". A
  connection card shows the name + provider tag, host · user · auth, a
  status badge (ok "connected · 142 ms", slow "slow · 2.4 s", checking,
  offline "unreachable · retrying in 30 s", auth "token rejected (401) ·
  update it to publish"), and the mode ("publishing · squashed per day" /
  "one per slot" / "ticket names only · not publishing"). Buttons: settings
  ▾ (expands the publish + squash toggles), test (or "update token" on
  auth), remove. Test results: testing… → "✓ signed in as … · can read
  issues and add worklogs · 135 ms" or "× couldn't reach … timed out after
  10 s".
- **Add connection modal**: service Jira/Linear. Jira fields: name, auth (API
  token · cloud / PAT · server), base url, email/username, token. Linear
  fields: name, personal API key (the workspace and user are detected from
  the key). Connection settings toggles (default: Jira publish on, Linear
  publish off with squash on). Test, then save. The note says "tokens and
  keys are stored in your OS keyring, never in the database".
- **New profile modal**: name (any case, unique) and "start with" (default
  settings / copy settings from <active>).
- **Settings** (per profile). Timesheet: week starts on (Mon/Sun), time format
  (14:30 / 2:30 pm), workday start–end, expected hours per day, show
  weekends. Publishing: publish to nirvana server toggle + server url. Data:
  database path + log count (reveal), export CSV/JSON.
- **Setup** (`/setup`): 3-step wizard. Create profile, then connect (required;
  "save and continue" only enabled after a passing test), then done ("open
  timesheet", "add another connection").

### 6.4 Theme tokens (from `v6-theme.jsx`)

Brand `#FFCB50`. Letter spacing: caps `0.12em`, heading `-0.02em`, display
`-0.03em`, everything else 0.

| token | light | dark |
|---|---|---|
| accent | #BE7E12 | #F0B33A |
| bgDefault | #FFFCF7 | #181715 |
| bgSubtle | #FBF7F0 | #1C1B19 |
| bgMuted | #F3EDE3 | #272522 |
| bgFrame | #F5F0E8 | #121110 |
| bgRaised | #FFFFFF | #211F1C |
| bgInverse | #1F1A14 | #F2EFEA |
| textPrimary | #1F1A14 | #F2EFEA |
| textSecondary | #57534C | #BDB8B0 |
| textTertiary | #706B63 | #A09B93 |
| textDisabled | #BEB8AE | #4F4B45 |
| textAccent | #A35300 | #FFCB50 |
| textOnAccent | #1F1A14 | #FFCB50 |
| textInverse | #FFFCF7 | #181715 |
| borderDefault | #E6E0D6 | #34312C |
| borderAccent | #F0BF4B | #77612C |
| fillAccent | #FFDB85 | #45391C |
| fillAccentSubtle | #FDF1D3 | #3A311C |
| fillNeutral | #F1ECE3 | #2A2825 |
| fillNeutralStrong | #969188 | #6E6A62 |
| fillHover | #F3EDE3 | #272522 |
| fillPressed | #E6E0D6 | #34312C |
| fillAccentHover | #FFCB50 | #52431F |
| fillAccentPressed | #F0BF4B | #5E4D23 |
| statusSuccess | #2F7A3F | #8FCB7A |
| statusWarning | #9A5F08 | #F0B86E |
| statusDanger | #B03A22 | #E66C5C |
| bgOverlay | rgba(31,26,20,.18) | rgba(18,17,16,.6) |
| shadowSm | 0 1px 2px rgba(40,30,20,.12) | 0 1px 2px rgba(0,0,0,.4) |
| shadowLg | 0 24px 60px rgba(40,30,20,.16), 0 4px 14px rgba(40,30,20,.08) | 0 30px 80px rgba(0,0,0,.55), 0 6px 20px rgba(0,0,0,.35) |

borderFocus = accent. The logo is a flat sun/stopwatch SVG (6 rays, crown,
button), drawn in brand color. Geometry is in `V4Logo` in `v6-theme.jsx`.

### 6.5 Type, spacing, states

- Fonts: **JetBrains Mono** (UI, the default 13px) and **Inter** (headings and
  the wordmark, weight 700). Bundle both. Do not load them from Google Fonts at
  runtime.
- Sizes in use: 68, 40, 30, 24, 18, 16, 14, 13, 12, 11, 10.
- Line heights are unitless tokens: none 1, tight 1.25, snug 1.4, default 1.5,
  relaxed 1.6. They follow size: 68–30 → none, Inter headings → tight, 16–14 →
  snug, 13–12 → default, 11–10 → snug. Only paragraphs (relaxed) and kbd
  (default) differ. Every text element sets its line height explicitly.
- Tabular numbers for all times and durations. Labels are 10px uppercase with
  caps tracking.
- Radii: 4 (inputs, chips, small), 6 (buttons, cards), 8 (panels, modals), 10
  (window), 9999 (pills).
- Buttons: 38px tall (30 small). Desktop selects are 26px, web selects 30px.
  Inputs are 36px tall.
- Interaction: hover and pressed change only the fill (fillHover/fillPressed;
  for primary, fillAccentHover/fillAccentPressed). Size, border and text stay
  put. Timeline blocks get a 1px accent ring on hover and 2px when pressed or
  selected. Focus is a 2px accent outline at offset 2, keyboard only.
  Disabled is 38% opacity with the arrow cursor. Every pressable uses the
  pointing-hand cursor.
- Key hint chips (`[s]`) use the accent-subtle fill with borderAccent and
  textAccent.

### 6.6 Keyboard

Desktop: `s` start (switch while tracking), `x` stop, `r` resume, `w` switch
back, `p` publish, `v` overview, `o` open web, `,` settings, `esc` cancel /
done, `↵` confirm. Global: ⌃⌥S start/switch, ⌃⌥X stop, ⌃⌥O open web.

## 7. Analysis: partially published slots

**Model.** A slot is published per *target*. A target is each profile
connection with `publish = true`, plus the Portal when enabled. The slot's
state is **derived** from `publish_entries`, never stored:

- **unpublished**: no ok entry.
- **partial**: at least one ok entry, but some target that was in scope **at
  publish time** is still pending or failed.
- **published**: every target in scope at publish time is ok.

When a publish starts, the in-scope targets are snapshotted as `pending`
entries. Turning on a new target later (e.g. the Portal) therefore doesn't
mark the whole history partial. Backfilling old logs to a new target is a
separate, explicit action ("send history to nirvana server").

**Showing it (existing tokens only):**

- Markers: `●` accent = unpublished, `✓` statusSuccess = published,
  **`◐` statusWarning = partial**.
- Desktop lists and web day cards: ◐ plus "partially published". The footer
  and the publish button count partial slots as pending
  (`publish · 2 retry`).
- Week grid block: published styling with a statusWarning left border.
- Edit drawer: a per-target list, e.g. `✓ acme · Jira worklog 10423` and
  `× nirvana server · unreachable`. A slot becomes **read-only as soon as any
  external target is ok**. Deleting a published slot is allowed, and the
  frontend warns before it.
- Publish modal: one section per target. Partial slots are listed only under
  the targets they're missing. If a target's health check is failing, the
  modal warns before publishing ("nirvana server unreachable: these logs stay
  partial") and still allows publishing to the rest.

**Publishing again.** Publish is idempotent per (slot, target): it only sends
what has no ok entry for that target. Retrying is just "publish" again. CLI:
`nirvana publish [--target <name>]`.

- Jira: store the worklog id. Add a `[nirvana:<slot ids>]` marker to the
  comment. On retry after a timeout or unknown result, list the issue's
  worklogs and match the marker before posting again (to avoid duplicates).
- Linear: store the comment id. Use the same marker approach.
- Portal: send raw slots keyed by `slots.uuid`, and the Portal upserts on it.
  Retries, and even later edits, are safe. Optionally, the desktop retries
  Portal-only failures in the background once the health check recovers.
  Jira/Linear retries stay manual.
- Squash + retry: rebuild the entry from that ticket and day's slots that
  have no ok entry for that target. Slots added to an already-published day
  create a new worklog. Never merge them into an existing one.

## 8. Analysis: Portal stack (Leptos vs Phoenix)

Portal needs: multi-user auth (password + Microsoft SSO/OIDC), admin CRUD
(users, their logs), tagged days (vacation, sick), reports (CSV/XLSX), monthly
email templates, Postgres, a container. It also stores Linear worklogs.

**Leptos (Rust)**
- For: the local web is already Leptos, so components, tokens and DTO types
  are reused (a shared `nirvana-portal-api` crate used by the desktop
  publisher). One language for a one-person team. A single static binary
  makes a tiny container. The libraries exist: axum-login, openidconnect,
  lettre, rust_xlsxwriter, SeaORM on Postgres, plus tokio-cron or apalis for
  jobs.
- Against: auth, admin, email and job plumbing is hand-written. Leptos has had
  breaking minor releases. Long compile times.

**Phoenix (Elixir)**
- For: DB-heavy CRUD + jobs is its sweet spot. Ecto handles schemas,
  migrations and reporting queries. `mix phx.gen.auth` gives password auth
  immediately; Microsoft SSO comes via `assent`/`ueberauth` (OIDC). LiveView
  builds admin screens and live dashboards with almost no JS. **Oban** handles
  scheduled monthly emails, report generation and retries, and Swoosh sends
  email. `mix release` gives a small container, and BEAM supervision gives
  resilience. Admin scaffolding: `phx.gen.live`, Backpex.
- Against: a second language, runtime and CI toolchain. No shared Rust types,
  so the contract must be language-neutral. The design system gets rebuilt in
  HEEx/Tailwind. No reuse of Leptos components.

**Rails**: the most admin gems (Devise/OmniAuth/ActiveAdmin), but it loses to
Phoenix on real-time views, jobs (Oban vs Sidekiq + Redis) and image size.
It's a distant third.

**Recommendation:** decide between Leptos and Phoenix at the Stage 2 review.
If the local web went well in Leptos and the Portal stays small (store
worklogs, simple admin, CSV), use Leptos. If admin, reporting and scheduled
email become the bulk of the work, use Phoenix. Either way, keep both
possible now:

- Define the Portal API as a language-neutral contract: an OpenAPI spec
  generated from Rust DTOs (e.g. `utoipa`), with idempotent upserts keyed by
  slot uuid.
- Have the design-token source also emit plain CSS variables.

## 9. Open questions

1. **Ticket owner resolution** with several connections in a profile.
   Proposed: ask every connection (async), cache the first hit in
   `tickets.connection_id`, and ask the user if several answer. A ticket with
   no owner is allowed ("checked when published") and resolved on the next
   sync or at publish time. Tickets have no unique key besides `id`.
2. Leptos CSR vs SSR for the local web (CSR recommended, §3 #10).
3. The per-profile DB path in the design versus the single-DB decision:
   adjust the copy on the profile card, Settings → data and setup ("logs are
   stored in …").
4. Does the web server run only inside the desktop app, or also headless
   (`nirvana serve`)? How does the desktop detect profile creation done in
   web (shared process, or DB change notification)?
5. Notes: export them before dropping the column?
6. Linux packaging format (AppImage / deb / Flatpak) and Linux tray support in
   Iced.
7. Change propagation between CLI, desktop and web: poll
   `PRAGMA data_version`, or a notify channel inside the process.
8. Branch flow after 1.0: keep `dev-1.0` (or a `dev` branch) or PR straight
   into `main`. Release PRs (release-plz) that land on `main` must not break
   the fast-forward from the dev branch.
9. Logging to the terminal when running with `cargo run` (dev mode): stdout
   or stderr, and should it replace or add to the log file?
10. Release boundary: the first standalone release (Stage 1 CICD) freezes the
    DB and config schema it ships. Later changes then need migrations and
    fixtures from that release too. Until then, 1.0 migrations can still be
    edited.

## 10. Security notes

- The local server manages tokens, so: bind to `127.0.0.1` only, validate the
  `Host` header (against DNS rebinding), check `Origin` / use CSRF tokens on
  mutating requests, and never return tokens over the API.
- Keyring: service `nirvana`, account =
  `nirvana_<profile>_<connection>_<key>` stored in `credentials`. The key is
  random so a dev and a production DB never overwrite each other's entries. The `env` store reads
  the token from the `NIRVANA_` environment variable named in `credentials`.
  Without a stable macOS Developer ID signature the keychain prompts after
  every update, so signing is part of CICD.
- Tokens in memory: `secrecy::SecretString`. Redact them in logs.

## 11. Stage order (details in the roadmap)

1. **Prep**: CI, hk hooks, conventional commits, GitHub settings (§12),
   mise tasks, dprint.
2. **Cleanup**: remove the GUI crate, TUI stub, Zed settings, unneeded
   configs/attributes. Update deps and AGENTS.md.
3. **Logging**: logging module, in place before any code changes so DB and
   settings migrations are logged.
4. **Database**: 0.3 fixtures, async + SeaORM, 0.3 detection + backup +
   baseline, profiles, connection settings, publish entries, uuid, slots and
   tickets changes, drop notes, migration tests.
5. **Config**: 0.3 detection + backup, schema_version 2, active profile,
   `[log] level`, migration tests.
6. **Keychain**: `secret_store` moved to `credentials`, keyring on macOS,
   Windows and Linux, injectable store with mock tests, manual keyring
   tests, Windows and Linux arm64 CI.
7. **Profiles and connections CLI**: CLI rework, nullable identity,
   user-driven token move, `env` store.
8. **Tickets**: commands + tests, owner resolution, async title/description
   sync.
9. **Linear** integration.
10. **Publishing**: fan-out, squash, per-entry errors, retry, sub-minute rule,
    ranges.
11. **Core API review**, then **CICD** (first standalone release). Then the
    **Stage 1 review**.
12. **Design tokens**, then the **local API server**, then the **Iced
    desktop**, then the **Leptos web**, then **CICD** (signed bundles). Then
    the **Stage 2 review** (decides the Portal stack).
13. **Portal**.

## 12. Git and GitHub workflow

History is linear everywhere. There are no merge commits.

### 12.1 Branches

- `main` is the released state. Until 1.0 nothing is committed to `main`
  directly, so `dev-1.0` is always a fast-forward of `main`.
- `dev-1.0` is the integration branch. Changes land only through PRs.
- Feature branches come off `dev-1.0`, one per roadmap item, named
  `<stage>/<topic>` (e.g. `prep/ci`, `cleanup/remove-gui`). They are
  short-lived and deleted after merge.

### 12.2 Pull requests

- One roadmap checkbox per PR. The PR ticks its checkbox in the roadmap.
- Open PRs as drafts early so CI runs. Mark ready when it is ready for review.
- The PR title is a conventional commit (`feat(core): ...`). It becomes the
  squash commit message, so CI checks the PR title. Commits on the feature
  branch do not need to be clean.
- Keep the feature branch up to date by rebasing on `dev-1.0` and force
  pushing the feature branch (never `dev-1.0` or `main`).
- What a review checks is listed in `AGENTS.md` (Reviewing PRs).

### 12.3 Code review

GitHub does not allow approving your own PR, so required approvals are 0 and
reviews come from agents:

1. Before marking ready: `/code-review` locally on the branch.
2. On the PR: `/code-review <PR#> --comment` posts findings as inline
   comments. Each comment is fixed or answered, then resolved. Unresolved
   comments block the merge.
3. Stage 1 and Stage 2 reviews: `/code-review ultra` on `dev-1.0`, plus a
   manual read of the diff since the previous stage.

### 12.4 Merging

- Feature branch into `dev-1.0`: **squash merge** in the GitHub UI. GitHub
  signs the squash commit with its own key, which shows as Verified.
- `dev-1.0` into `main`: **fast-forward** from a local clone after `ci-ok` is
  green on `dev-1.0`:

  ```sh
  git fetch origin
  git push origin origin/dev-1.0:main
  ```

  The push is rejected if it is not a fast-forward. Optionally open a
  `dev-1.0` → `main` PR first for visibility; GitHub marks it merged when the
  push lands.

### 12.5 GitHub settings

Account (Settings → SSH and GPG keys):

- The SSH signing key is added as a **Signing key** (not only as an
  authentication key), otherwise commits do not show as Verified.

Repository → Settings → General → Pull Requests:

- Allow merge commits: **off**.
- Allow squash merging: **on**, default commit message **Pull request title**.
- Allow rebase merging: **off**.
- Always suggest updating pull request branches: **on**.
- Automatically delete head branches: **on**.

Repository → Settings → Actions → General:

- Workflow permissions: **Read repository contents and packages permissions**.

Repository → Settings → Rules → Rulesets → New branch ruleset **`dev-1.0`**:

- Enforcement status: **Active**. Target branches: `dev-1.0`. Bypass list:
  empty.
- Restrict deletions: **on**.
- Require linear history: **on**.
- Require signed commits: **on**.
- Require a pull request before merging: **on**
  - Required approvals: **0**.
  - Require conversation resolution before merging: **on**.
  - Allowed merge methods: **Squash** only.
- Require status checks to pass: **on**
  - Require branches to be up to date before merging: **on**.
  - Status checks: **`ci-ok`** and **`pr-title`** (add them after CI has run
    once).
- Block force pushes: **on**.

Repository → Settings → Rules → Rulesets → New branch ruleset **`main`**:

- Enforcement status: **Active**. Target branches: `main`. Bypass list:
  **Repository admin**, mode **Always** (needed for the fast-forward push).
- Restrict deletions: **on**.
- Restrict updates: **on** (only the bypass list can push).
- Require linear history: **on**.
- Require signed commits: **on**.
- Require status checks to pass: **on**, status check **`ci-ok`**.
- Block force pushes: **on**.
