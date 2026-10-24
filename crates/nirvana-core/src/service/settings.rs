use crate::config::{
    normalize_font_scale, normalize_idle_methods, normalize_idle_threshold_secs, normalize_theme,
};
use crate::domain::AppSettings;
use crate::errors::NirvanaError;
use crate::service::Nirvana;

impl Nirvana {
    pub fn get_settings(&self) -> AppSettings {
        AppSettings {
            font_scale: normalize_font_scale(self.config.gui.font_scale),
            theme: normalize_theme(&self.config.gui.theme),
            show_tray_icon: self.config.gui.show_tray_icon,
            idle_enabled: self.config.idle.enabled,
            idle_methods: normalize_idle_methods(&self.config.idle.methods),
            idle_threshold_secs: normalize_idle_threshold_secs(self.config.idle.threshold_secs),
        }
    }

    pub fn update_settings(&mut self, settings: AppSettings) -> Result<AppSettings, NirvanaError> {
        self.config.gui.font_scale = normalize_font_scale(settings.font_scale);
        self.config.gui.theme = normalize_theme(&settings.theme);
        self.config.gui.show_tray_icon = settings.show_tray_icon;
        self.config.idle.enabled = settings.idle_enabled;
        self.config.idle.methods = normalize_idle_methods(&settings.idle_methods);
        self.config.idle.threshold_secs =
            normalize_idle_threshold_secs(settings.idle_threshold_secs);
        self.config.save(&self.paths)?;
        Ok(self.get_settings())
    }
}
