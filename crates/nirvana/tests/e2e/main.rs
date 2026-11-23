//! End-to-end tests of the release binary against Jira Cloud and Linear.
//!
//! Run with `mise run test-e2e`, which builds the release binary and passes
//! its path in `NIRVANA_E2E_BIN`. The other `NIRVANA_E2E_*` variables come
//! from the Infisical `/e2e` folder.

mod config;
mod connection;
mod credential;
mod helpers;
mod info;
mod logs;
mod profile;
mod publish;
mod serve;
mod settings;
mod ticket;
mod tracking;
