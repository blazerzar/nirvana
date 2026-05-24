import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import { BackendSettings, ThemeId } from "../types/types";

export type ThemeDefinition = {
  id: ThemeId;
  name: string;
  colorScheme: "dark" | "light";
};

const DEFAULT_FONT_SCALE = 1;
const MIN_FONT_SCALE = 0.9;
const MAX_FONT_SCALE = 1.25;
const DEFAULT_THEME: ThemeId = "high-contrast-dark";

export const themeOptions: ThemeDefinition[] = [
  {
    id: "nirvana-dark",
    name: "Nirvana dark",
    colorScheme: "dark",
  },
  {
    id: "high-contrast-dark",
    name: "High contrast",
    colorScheme: "dark",
  },
  {
    id: "soft-light",
    name: "Soft light",
    colorScheme: "light",
  },
];

const themeFor = (theme: string): ThemeDefinition =>
  themeOptions.find((option) => option.id === theme) ??
  themeOptions.find((option) => option.id === DEFAULT_THEME) ??
  themeOptions[0];

const normalizeFontScale = (value: number) =>
  Number.isFinite(value)
    ? Math.min(MAX_FONT_SCALE, Math.max(MIN_FONT_SCALE, value))
    : DEFAULT_FONT_SCALE;

const applyAppearance = (theme: string, fontScale: number) => {
  const root = document.documentElement;
  const themeDefinition = themeFor(theme);

  root.dataset.theme = themeDefinition.id;
  root.style.colorScheme = themeDefinition.colorScheme;
  root.style.setProperty("--app-font-scale", `${normalizeFontScale(fontScale)}`);
};

export const useSettingsStore = defineStore("settings", {
  state: () => ({
    initialized: false,
    loading: false,
    saving: false,
    error: "",
    publishSquashedWorklogs: true,
    fontScale: DEFAULT_FONT_SCALE,
    theme: DEFAULT_THEME as ThemeId,
    showTrayIcon: false,
    savedFontScale: DEFAULT_FONT_SCALE,
    savedTheme: DEFAULT_THEME as ThemeId,
    savedShowTrayIcon: false,
  }),
  getters: {
    fontScalePercent(state): number {
      return Math.round(state.fontScale * 100);
    },
    activeTheme(state): ThemeDefinition {
      return themeFor(state.theme);
    },
  },
  actions: {
    applyCurrentAppearance() {
      applyAppearance(this.theme, this.fontScale);
    },
    applySettings(settings: BackendSettings) {
      this.publishSquashedWorklogs = settings.publishSquashedWorklogs;
      this.fontScale = normalizeFontScale(settings.fontScale);
      this.theme = themeFor(settings.theme).id;
      this.showTrayIcon = settings.showTrayIcon;
      this.savedFontScale = this.fontScale;
      this.savedTheme = this.theme;
      this.savedShowTrayIcon = this.showTrayIcon;
      this.applyCurrentAppearance();
    },
    async initialize() {
      if (this.initialized || this.loading) {
        return;
      }

      this.loading = true;
      this.error = "";

      try {
        const settings = await invoke<BackendSettings>("get_settings");
        this.applySettings(settings);
        this.initialized = true;
      } catch (error) {
        this.applyCurrentAppearance();
        this.error = error instanceof Error ? error.message : String(error);
      } finally {
        this.loading = false;
      }
    },
    async setPublishSquashedWorklogs(value: boolean) {
      if (this.saving || value === this.publishSquashedWorklogs) {
        return;
      }

      const previousValue = this.publishSquashedWorklogs;
      this.publishSquashedWorklogs = value;
      this.saving = true;
      this.error = "";

      try {
        const settings = await invoke<BackendSettings>("update_settings", {
          input: {
            publishSquashedWorklogs: value,
            fontScale: this.fontScale,
            theme: this.theme,
            showTrayIcon: this.showTrayIcon,
          },
        });
        this.applySettings(settings);
        this.initialized = true;
      } catch (error) {
        this.publishSquashedWorklogs = previousValue;
        this.applyCurrentAppearance();
        this.error = error instanceof Error ? error.message : String(error);
      } finally {
        this.saving = false;
      }
    },
    async setFontScale(value: number) {
      const nextValue = normalizeFontScale(value);
      if (this.saving) {
        return;
      }

      const previousValue = this.savedFontScale;
      this.fontScale = nextValue;
      this.applyCurrentAppearance();
      this.saving = true;
      this.error = "";

      try {
        const settings = await invoke<BackendSettings>("update_settings", {
          input: {
            publishSquashedWorklogs: this.publishSquashedWorklogs,
            fontScale: nextValue,
            theme: this.theme,
            showTrayIcon: this.showTrayIcon,
          },
        });
        this.applySettings(settings);
        this.initialized = true;
      } catch (error) {
        this.fontScale = previousValue;
        this.applyCurrentAppearance();
        this.error = error instanceof Error ? error.message : String(error);
      } finally {
        this.saving = false;
      }
    },
    async setTheme(theme: ThemeId) {
      const nextTheme = themeFor(theme).id;
      if (this.saving || nextTheme === this.theme) {
        return;
      }

      const previousTheme = this.savedTheme;
      this.theme = nextTheme;
      this.applyCurrentAppearance();
      this.saving = true;
      this.error = "";

      try {
        const settings = await invoke<BackendSettings>("update_settings", {
          input: {
            publishSquashedWorklogs: this.publishSquashedWorklogs,
            fontScale: this.fontScale,
            theme: nextTheme,
            showTrayIcon: this.showTrayIcon,
          },
        });
        this.applySettings(settings);
        this.initialized = true;
      } catch (error) {
        this.theme = previousTheme;
        this.applyCurrentAppearance();
        this.error = error instanceof Error ? error.message : String(error);
      } finally {
        this.saving = false;
      }
    },
    async setShowTrayIcon(value: boolean) {
      if (this.saving || value === this.showTrayIcon) {
        return;
      }

      const previousValue = this.showTrayIcon;
      this.showTrayIcon = value;
      this.saving = true;
      this.error = "";

      try {
        const settings = await invoke<BackendSettings>("update_settings", {
          input: {
            publishSquashedWorklogs: this.publishSquashedWorklogs,
            fontScale: this.fontScale,
            theme: this.theme,
            showTrayIcon: value,
          },
        });
        this.applySettings(settings);
        this.initialized = true;
      } catch (error) {
        this.showTrayIcon = previousValue;
        this.error = error instanceof Error ? error.message : String(error);
      } finally {
        this.saving = false;
      }
    },
  },
});
