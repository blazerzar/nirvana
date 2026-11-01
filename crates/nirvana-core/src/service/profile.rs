//! Profiles.
//!
//! All profiles live in one database. Connections, tickets and slots belong
//! to a profile. The active profile is machine state, stored by name as
//! `[core] profile` in `config.toml`.

use nirvana_entity::connection::Entity as ConnectionEntity;
use nirvana_entity::profile::Entity as ProfileEntity;
use nirvana_entity::{connection, credential, jira_settings, profile};
use sea_orm::ActiveValue::Set;
use sea_orm::{
    ActiveModelTrait, DatabaseConnection, EntityTrait, ModelTrait, QueryFilter, QueryOrder, SqlErr,
};

use crate::domain::Connection;
use crate::errors::{DomainError, NirvanaError};
use crate::secrets::{SecretStore, delete_from_keychain};
use crate::service::{ActiveProfile, Nirvana, Profile};

impl Nirvana {
    /// Makes the profile named `name` active and saves the config.
    ///
    /// # Errors
    ///
    /// Returns an error if the profile does not exist, the config is
    /// unavailable or cannot be saved, or the database query fails.
    pub async fn switch_profile(&mut self, name: String) -> Result<(), NirvanaError> {
        match ProfileEntity::find_by_name(&name).one(&self.db).await? {
            Some(profile) => {
                self.config_mut()?.core.profile = Some(profile.name);
                self.config()?.save(&self.paths)?;
                Ok(())
            }
            None => Err(DomainError::ProfileNotFound(name).into()),
        }
    }

    /// Returns all profiles, oldest first.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn list_profiles(&self) -> Result<Vec<Profile>, NirvanaError> {
        Ok(ProfileEntity::find()
            .order_by_asc(profile::COLUMN.id)
            .all(&self.db)
            .await?)
    }

    /// Creates a profile named `name` without making it active.
    ///
    /// # Errors
    ///
    /// Returns an error if the name is empty, a profile with that name already
    /// exists or the database insert fails.
    pub async fn create_profile(&self, name: String) -> Result<Profile, NirvanaError> {
        profile::ActiveModel {
            name: Set(name.clone()),
            ..Default::default()
        }
        .insert(&self.db)
        .await
        .map_err(|e| match e.sql_err() {
            Some(SqlErr::UniqueConstraintViolation(_)) => DomainError::ProfileExists(name).into(),
            _ => e.into(),
        })
    }

    /// Deletes the profile named `name` with its connections, tickets and
    /// slots, and removes the keyring entries of its connections.
    ///
    /// If it was the active profile, the oldest remaining profile becomes
    /// active, or none if no profile is left. A keyring entry that cannot be
    /// removed is only logged.
    ///
    /// # Errors
    ///
    /// Returns an error if the profile does not exist, the config is
    /// unavailable or cannot be saved, or a database query fails.
    pub async fn delete_profile(&mut self, name: String) -> Result<(), NirvanaError> {
        let Some(profile) = ProfileEntity::find_by_name(&name).one(&self.db).await? else {
            return Err(DomainError::ProfileNotFound(name).into());
        };
        let deletes_active = self.config()?.core.profile.as_ref() == Some(&name);

        let keyring_accounts: Vec<String> = credential::Entity::find()
            .inner_join(connection::Entity)
            .filter(connection::COLUMN.profile_id.eq(profile.id))
            .all(&self.db)
            .await?
            .into_iter()
            .filter(|credential| credential.secret_store == SecretStore::Keyring)
            .map(|credential| credential.credential)
            .collect();
        profile.delete(&self.db).await?;
        for account in keyring_accounts {
            delete_from_keychain(&self.keyring, account).await;
        }

        if deletes_active {
            self.config_mut()?.core.profile = resolve(&self.db, None).await?;
            self.config()?.save(&self.paths)?;
        }
        Ok(())
    }

    /// Returns the active profile with its connections, oldest first.
    ///
    /// # Errors
    ///
    /// Returns an error if no profile is active, the config is unavailable,
    /// a connection has invalid settings or the database query fails.
    pub async fn active_profile(&self) -> Result<ActiveProfile, NirvanaError> {
        let profile = self.find_active_profile().await?;
        let connections = ConnectionEntity::find()
            .find_also_related(jira_settings::Entity)
            .filter(connection::COLUMN.profile_id.eq(profile.id))
            .order_by_asc(connection::COLUMN.id)
            .all(&self.db)
            .await?
            .into_iter()
            .map(|(model, jira_settings)| Connection::new(model, jira_settings))
            .collect::<Result<_, _>>()?;
        Ok(ActiveProfile {
            profile,
            connections,
        })
    }

    pub(super) async fn find_active_profile(&self) -> Result<Profile, NirvanaError> {
        let name = self
            .config()?
            .core
            .profile
            .as_ref()
            .ok_or(DomainError::NoActiveProfile)?;
        Ok(ProfileEntity::find_by_name(name)
            .one(&self.db)
            .await?
            .ok_or(DomainError::NoActiveProfile)?)
    }
}

/// Returns `configured` if a profile with that name exists, otherwise the
/// name of the oldest profile, or `None` if there are no profiles.
pub(crate) async fn resolve(
    db: &DatabaseConnection,
    configured: Option<String>,
) -> Result<Option<String>, NirvanaError> {
    if let Some(name) = configured
        && ProfileEntity::find_by_name(&name).one(db).await?.is_some()
    {
        return Ok(Some(name));
    }
    Ok(ProfileEntity::find()
        .order_by_asc(profile::COLUMN.id)
        .one(db)
        .await?
        .map(|profile| profile.name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::domain::ConnectionData;
    use crate::service::test_support::test_service;

    async fn profile_names(service: &Nirvana) -> Vec<String> {
        service
            .list_profiles()
            .await
            .unwrap()
            .into_iter()
            .map(|profile| profile.name)
            .collect()
    }

    #[tokio::test]
    async fn create_profile_adds_profile() {
        let service = test_service().await;

        let created = service.create_profile("work".to_string()).await.unwrap();

        assert_eq!(created.name, "work");
        assert_eq!(profile_names(&service).await, ["test", "work"]);
    }

    #[tokio::test]
    async fn delete_profile_removes_keyring_entries_and_switches_active() {
        let mut service = test_service().await;
        service.create_profile("work".to_string()).await.unwrap();
        service
            .add_connection(ConnectionData {
                name: "Keyring Jira".to_string(),
                kind: "jira-cloud".to_string(),
                host: "https://other.atlassian.net".to_string(),
                identity: "tester@example.com".to_string(),
                secret_store: SecretStore::Keyring,
                token: "keyring-token".into(),
            })
            .await
            .unwrap();

        service.delete_profile("test".to_string()).await.unwrap();

        assert_eq!(profile_names(&service).await, ["work"]);
        assert!(
            service
                .active_profile()
                .await
                .unwrap()
                .connections
                .is_empty()
        );
        let store = (service.keyring)().unwrap();
        assert!(
            store
                .search(&[("service", "nirvana")].into())
                .unwrap()
                .iter()
                .all(|entry| entry.get_password().is_err())
        );
        assert_eq!(
            service.config().unwrap().core.profile.as_deref(),
            Some("work")
        );
    }

    #[tokio::test]
    async fn delete_profile_rejects_missing_profile() {
        let mut service = test_service().await;

        let err = service
            .delete_profile("missing".to_string())
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::ProfileNotFound(name)) if name == "missing"
        ));
    }

    #[tokio::test]
    async fn create_profile_rejects_duplicate_name() {
        let service = test_service().await;

        let err = service
            .create_profile("test".to_string())
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::ProfileExists(name)) if name == "test"
        ));
    }

    #[tokio::test]
    async fn switch_profile_saves_active_profile() {
        let mut service = test_service().await;
        service.create_profile("work".to_string()).await.unwrap();

        service.switch_profile("work".to_string()).await.unwrap();

        assert_eq!(
            service.config().unwrap().core.profile.as_deref(),
            Some("work")
        );
        let saved = AppConfig::load(&service.paths).unwrap();
        assert_eq!(saved.core.profile.as_deref(), Some("work"));
    }

    #[tokio::test]
    async fn switch_profile_rejects_missing_profile() {
        let mut service = test_service().await;

        let err = service
            .switch_profile("missing".to_string())
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            NirvanaError::Domain(DomainError::ProfileNotFound(name)) if name == "missing"
        ));
        assert_eq!(
            service.config().unwrap().core.profile.as_deref(),
            Some("test")
        );
    }

    #[tokio::test]
    async fn delete_profile_keeps_active_profile_when_deleting_another() {
        let mut service = test_service().await;
        service.create_profile("work".to_string()).await.unwrap();

        service.delete_profile("work".to_string()).await.unwrap();

        assert_eq!(profile_names(&service).await, ["test"]);
        assert_eq!(service.active_profile().await.unwrap().connections.len(), 1);
        assert_eq!(
            service.config().unwrap().core.profile.as_deref(),
            Some("test")
        );
    }

    #[tokio::test]
    async fn active_profile_loads_its_connections() {
        let mut service = test_service().await;
        service.create_profile("work".to_string()).await.unwrap();

        let profile = service.active_profile().await.unwrap();
        assert_eq!(profile.connections.len(), 1);
        assert_eq!(profile.connections[0].model.name, "Test Jira");

        service.switch_profile("work".to_string()).await.unwrap();
        assert!(
            service
                .active_profile()
                .await
                .unwrap()
                .connections
                .is_empty()
        );
    }

    #[tokio::test]
    async fn delete_last_profile_clears_active_profile() {
        let mut service = test_service().await;

        service.delete_profile("test".to_string()).await.unwrap();

        assert!(profile_names(&service).await.is_empty());
        assert_eq!(service.config().unwrap().core.profile, None);
        let saved = AppConfig::load(&service.paths).unwrap();
        assert_eq!(saved.core.profile, None);
    }

    #[tokio::test]
    async fn resolve_keeps_configured_profile() {
        let service = test_service().await;
        service.create_profile("work".to_string()).await.unwrap();

        let resolved = resolve(&service.db, Some("work".to_string()))
            .await
            .unwrap();

        assert_eq!(resolved.as_deref(), Some("work"));
    }

    #[tokio::test]
    async fn resolve_falls_back_to_first_profile() {
        let service = test_service().await;
        service.create_profile("work".to_string()).await.unwrap();

        let missing = resolve(&service.db, Some("missing".to_string()))
            .await
            .unwrap();
        let unset = resolve(&service.db, None).await.unwrap();

        assert_eq!(missing.as_deref(), Some("test"));
        assert_eq!(unset.as_deref(), Some("test"));
    }

    #[tokio::test]
    async fn resolve_without_profiles_is_none() {
        let mut service = test_service().await;
        service.delete_profile("test".to_string()).await.unwrap();

        let resolved = resolve(&service.db, Some("test".to_string()))
            .await
            .unwrap();

        assert_eq!(resolved, None);
    }
}
