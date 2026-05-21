import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import { BackendSettings } from "../types/types";

export const useSettingsStore = defineStore("settings", {
  state: () => ({
    initialized: false,
    loading: false,
    saving: false,
    error: "",
    publishSquashedWorklogs: true,
  }),
  actions: {
    async initialize() {
      if (this.initialized || this.loading) {
        return;
      }

      this.loading = true;
      this.error = "";

      try {
        const settings = await invoke<BackendSettings>("get_settings");
        this.publishSquashedWorklogs = settings.publishSquashedWorklogs;
        this.initialized = true;
      } catch (error) {
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
          input: { publishSquashedWorklogs: value },
        });
        this.publishSquashedWorklogs = settings.publishSquashedWorklogs;
        this.initialized = true;
      } catch (error) {
        this.publishSquashedWorklogs = previousValue;
        this.error = error instanceof Error ? error.message : String(error);
      } finally {
        this.saving = false;
      }
    },
  },
});
