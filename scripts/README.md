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
```

The `seed` tasks create 100 issues in a project or team. The `connect` tasks
add a connection to Nirvana, run with `cargo run` unless `--bin` is given.
`dev:setup` creates a profile and adds the Jira Cloud and Linear connections.
In dev mode the DB lives next to the binary (`target/debug/nirvana.db`), so it
never touches your real data. Dev mode is on in debug builds or with
`NIRVANA_DEV=1`, so a release binary given with `--bin` uses your real DB unless
`NIRVANA_DEV=1` is set.

## Secrets

Secrets come from [Infisical](https://infisical.com). Log in with
`infisical login`. The project is set in `.infisical.yaml` in the repository
root. Each connection kind has its own folder:

| Folder        | Secrets                      |
| ------------- | ---------------------------- |
| `/jira-cloud` | `HOST`, `EMAIL`, `API_TOKEN` |
| `/jira-dc`    | `HOST`, `PAT`                |
| `/linear`     | `API_KEY`                    |

`HOST` is the instance host without a scheme, e.g. `yourcompany.atlassian.net`.
Jira Cloud API tokens are generated at
https://id.atlassian.com/manage-profile/security/api-tokens. Linear API keys
are generated at https://linear.app/settings/account/security.

## Adding scripts

Add an executable `scripts/<kind>/<name>.sh` with `#MISE` and `#USAGE`
headers. It becomes the `<kind>:<name>` task. Shared code goes in
non-executable `lib.sh` files, and `load_secrets <folder>` from
`scripts/lib.sh` exports the secrets of an Infisical folder.
