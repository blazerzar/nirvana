# Scripts

Manual API exploration scripts, run as mise tasks (`mise tasks` lists them,
`mise run <task> --help` shows the arguments).

```sh
mise run jira:issue ME-498
mise run jira:issues ME-498 ME-499
mise run jira:worklog ME-498 "1h 30m" --comment "I did some work here."
mise run jira:worklog ME-498 "1h 30m" --dc
mise run linear:issue ENG-123
mise run linear:issues ENG-123 ENG-124
mise run jira:seed ME
mise run linear:seed ENG
mise run jira:connect keyring
mise run jira:connect plaintext --dc --bin target/release/nirvana
mise run linear:connect keyring
mise run dev:setup keyring
mise run dev:slots
mise run dev:gui-errors
mise run dev:gui-errors bad-config
```

The `seed` tasks create 100 issues in a project or team. The `connect` tasks
add a connection to Nirvana, run with `cargo run` unless `--bin` is given.
`dev:setup` creates a profile and adds the Jira Cloud and Linear connections.
`dev:slots`, run after `dev:setup`, adds about 8 hours of slots on random
test tickets to today and each of the 4 workdays before it (`--days` changes
the count). Today only gets the slots that already ended, since slots cannot
be in the future.
In dev mode the DB lives next to the binary (`target/debug/nirvana.db`), so it
never touches your real data. Dev mode is on in debug builds or with
`NIRVANA_DEV=1`, so a release binary given with `--bin` uses your real DB unless
`NIRVANA_DEV=1` is set.

`dev:gui-errors` opens the GUI in each error state it can reach from the
outside, one after another, and prints what to check and click before each
one. Every case runs with dev mode off against its own temporary directory, so
neither the dev DB nor your real data is touched.

## Secrets

Secrets come from [Infisical](https://infisical.com). Log in with
`infisical login`. The project is set in `.infisical.yaml` in the repository
root. Each connection kind has its own folder, and the e2e tests have `/e2e`:

| Folder        | Secrets                      |
| ------------- | ---------------------------- |
| `/jira-cloud` | `HOST`, `EMAIL`, `API_TOKEN` |
| `/jira-dc`    | `HOST`, `PAT`                |
| `/linear`     | `API_KEY`                    |
| `/e2e`        | `NIRVANA_E2E_*`, see below   |

`HOST` is the instance host without a scheme, e.g. `yourcompany.atlassian.net`.
Jira Cloud API tokens are generated at
https://id.atlassian.com/manage-profile/security/api-tokens. Linear API keys
are generated at https://linear.app/settings/account/security.

`/e2e` holds the secrets of the e2e tests (`mise run test-e2e`):

- `NIRVANA_E2E_JIRA_HOST`, `NIRVANA_E2E_JIRA_EMAIL` and
  `NIRVANA_E2E_JIRA_TOKEN`: the same as in `/jira-cloud`.
- `NIRVANA_E2E_LINEAR_TOKEN`: the same as `API_KEY` in `/linear`.
- `NIRVANA_E2E_JIRA_PROJECT` and `NIRVANA_E2E_LINEAR_TEAM`: the Jira project
  key and the Linear team key the tests create their issues in.

## Adding scripts

Add an executable `scripts/<kind>/<name>.sh` with `#MISE` and `#USAGE`
headers. It becomes the `<kind>:<name>` task. Shared code goes in
non-executable `lib.sh` files, and `load_secrets <folder>` from
`scripts/lib.sh` exports the secrets of an Infisical folder.
