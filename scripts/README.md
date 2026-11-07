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
```

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
