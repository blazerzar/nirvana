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
| 2 | **0.3 migration: the existing DB becomes one profile** (e.g. `default`) holding all existing connections. Tickets keep their `connection_id`; a slot's connection comes from its ticket (verify `slots.connection_id` always matches before dropping it). | Simple, lossless. Splitting into more profiles can be done by hand later. |
| 3 | **Async core on tokio + SeaORM**, async reqwest. Bridge the existing `PRAGMA user_version` migrations (v1, v2) into SeaORM migrations with a baseline that detects a 0.3 DB. | axum, Leptos and Iced tasks are async anyway, and SeaORM is async-only. |
| 4 | **TUI dropped for 1.0.** Remove the ratatui stub and the ratatui/crossterm deps. | No TUI tasks or v6 screens exist. Can come back after 1.0. |
| 5 | Tickets keep a `connection_id` FK. **Open:** how the owner is picked when a profile has several connections (see §9). | |
| 6 | **No rounding anywhere.** Jira rejects worklogs under 60 s, so a sub-minute entry **fails that entry with a visible error** and stays unpublished. | Design product rule: never silently alter time. |
| 7 | Git hooks with **hk** (+ mise). **Mise tasks** replace the justfile. **Taplo** replaces Tombi. | Cross-platform, one tool manager. |
| 8 | **Idle detection** is restored from `main` (`crates/nirvana-gui/src-tauri/src/idle/{mod,macos,windows}.rs`) when the Iced app needs it. **Tray and menu-bar code is rewritten** for Iced. The GUI crate can be deleted without porting anything first. | |
| 9 | **Portal stack: Leptos vs Phoenix, decided at the Stage 2 review** (see §8). Until then the Portal contract stays language-neutral. | |
| 10 | Local web: **Leptos CSR (Trunk) + axum REST API**, assets embedded with `rust-embed`. *Recommended, confirm when stage 2 starts.* | Simpler single-binary build than SSR/hydration. The explicit API matches the roadmap's API section. |
| 11 | **Migration tests are mandatory**: 0.3.0-alpha DB and config fixtures are checked in, never edited, and run in CI (§5). | Guarantees the compatibility promise. |

## 4. Current codebase (0.3.0-alpha) - what exists

- Workspace: `crates/nirvana` (CLI + TUI stub), `crates/nirvana-core`,
  `crates/nirvana-gui` (Tauri v2 + Vue 3, to be removed).
- `nirvana-core`: sync `rusqlite`, blocking reqwest. `NirvanaApi { paths, config, db }`
  in `src/api/mod.rs`. Modules: `api/` (tracking, publish, edit, delete, slots,
  connection, settings, info), `storage/` (connection, ticket and slot repos;
  migrations `0001_init.sql`, `0002_tickets_slots.sql` run via `user_version`),
  `integration/` (Jira cloud/dc via `kind` = `jira-cloud`/`jira-dc`, REST v2),
  `credentials/` (**tokens stored in plaintext in the `credentials` table**),
  `config.rs` (sectioned `config.toml` with `[core] active_connection,
  publish_squashed_worklogs`, `[gui] font_scale, theme, show_tray_icon`,
  `[idle] enabled, methods, threshold_secs`, plus the legacy flat format),
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
            identity, publish bool, squash bool, created_at, updated_at)
            -- token lives in the OS keyring, never in the DB
tickets(id, profile_id, connection_id NULL, ticket_key, title, description,
        last_worked_at, synced_at)   -- NULL = owner not resolved yet
slots(id, uuid, profile_id, ticket_id, started_at, stopped_at)
publish_entries(id, target_kind {connection, portal}, connection_id NULL,
                remote_id, status {pending, ok, failed}, error, attempted_at)
publish_entry_slots(entry_id, slot_id)   -- a squash maps many slots to one entry
```

- The machine config (`config.toml`) keeps machine-only state: active profile,
  desktop settings (launch at login, close to menu bar, idle, reminder,
  shortcuts, web server on/off + port, theme light/dark/system).
- 0.3 migration: create profile `default`. Set every connection's
  `profile_id`, plus `publish = true` and `squash` from the global
  `core.publish_squashed_worklogs`. Move the `credentials` rows into the
  keyring, then delete them. Add a uuid to every slot. For slots that already
  have `published_at`, create a synthetic ok `publish_entry` (no remote_id).
  `core.active_connection` becomes obsolete (all connections are active).
  The 0.3 `gui.*` settings (font scale, tray icon, theme) map to the new
  machine settings where an equivalent exists; old theme names map to
  light/dark.
- The note column is dropped (no UI uses it). Decide whether old notes get
  exported first.
- **Migration tests** (required before the Stage 1 review), run in CI:
  - DB: a fixture `nirvana.db` produced by 0.3.0-alpha (`user_version` 2) with
    several connections, credentials, tickets, closed, running and published
    slots. It must migrate to the latest schema with no data loss: one
    `default` profile, connection settings set, tokens moved to the keyring
    (mock the keyring in tests), uuids assigned, publish entries created for
    published slots. Also cover a fresh DB and a DB at `user_version` 1.
  - Config: fixtures for the legacy flat format and the 0.3.0-alpha sectioned
    format (`[core]`, `[gui]`, `[idle]`). Check that the squash flag moves into
    connections, `active_connection` is dropped, and machine/idle settings are
    kept.
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
- Publishing to Jira/Linear is a per-connection setting (`publish`, `squash`).
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
- **New profile modal**: name (lowercase, unique) and "start with" (default
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
  external target is ok**.
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
   sync or at publish time.
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

## 10. Security notes

- The local server manages tokens, so: bind to `127.0.0.1` only, validate the
  `Host` header (against DNS rebinding), check `Origin` / use CSRF tokens on
  mutating requests, and never return tokens over the API.
- Keyring: service `nirvana`, account per connection (use a stable id, not a
  renameable name). Env override `NIRVANA_TOKEN_<NAME>` (connection name,
  upper-cased) for CI/headless.
  Without a stable macOS Developer ID signature the keychain prompts after
  every update, so signing is part of CICD.
- Tokens in memory: `secrecy::SecretString`. Redact them in logs.

## 11. Stage order (details in the roadmap)

1. **Prep**: CI, hk hooks, conventional commits, mise tasks, Taplo.
2. **Cleanup**: remove the GUI crate, TUI stub, Zed settings, unneeded
   configs/attributes. Update deps and AGENTS.md.
3. **Database**: async + SeaORM, 0.3 baseline, profiles, connection
   settings, publish entries, uuid, config migration, drop notes, migration
   tests.
4. **Keychain**.
5. **Tickets**: commands + tests, owner resolution, async title/description
   sync.
6. **Linear** integration.
7. **Publishing**: fan-out, squash, per-entry errors, retry, sub-minute rule,
   ranges.
8. **Core API review**, then **CICD** (first standalone release). Then the
   **Stage 1 review**.
9. **Design tokens**, then the **local API server**, then the **Iced
   desktop**, then the **Leptos web**, then **CICD** (signed bundles). Then the
   **Stage 2 review** (decides the Portal stack).
10. **Portal**.
