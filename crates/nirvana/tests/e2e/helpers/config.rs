use std::env;

pub struct Config {
    pub jira_host: String,
    pub jira_email: String,
    pub jira_token: String,
    pub jira_project: String,
    pub linear_token: String,
    pub linear_team: String,
    pub linear_workspace: String,
}

impl Config {
    pub fn from_env() -> Self {
        let var = |name: &str| env::var(name).unwrap_or_else(|_| panic!("{name} is not set"));
        Self {
            jira_host: var("NIRVANA_E2E_JIRA_HOST"),
            jira_email: var("NIRVANA_E2E_JIRA_EMAIL"),
            jira_token: var("NIRVANA_E2E_JIRA_TOKEN"),
            jira_project: var("NIRVANA_E2E_JIRA_PROJECT"),
            linear_token: var("NIRVANA_E2E_LINEAR_TOKEN"),
            linear_team: var("NIRVANA_E2E_LINEAR_TEAM"),
            linear_workspace: var("NIRVANA_E2E_LINEAR_WORKSPACE"),
        }
    }
}
