use crate::api::domain::AppSettings;
use crate::api::{NirvanaApi, NirvanaError};

impl NirvanaApi {
    pub fn get_settings(&self) -> AppSettings {
        AppSettings {
            publish_squashed_worklogs: self.config.publish_squashed_worklogs,
        }
    }

    pub fn update_settings(&mut self, settings: AppSettings) -> Result<AppSettings, NirvanaError> {
        self.config.publish_squashed_worklogs = settings.publish_squashed_worklogs;
        self.config.save(&self.paths)?;
        Ok(self.get_settings())
    }
}
