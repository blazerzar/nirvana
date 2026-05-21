import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import {
  ConnectionSetupStep,
  ConnectionType,
  CreateConnectionInput,
  GuiConnection,
} from "../types/types";

export const connectionTypeLabels: Record<ConnectionType, string> = {
  "jira-cloud": "Jira Cloud",
  "jira-dc": "Jira Data Center",
};

export const normalizeHostname = (value: string) => {
  const trimmed = value.trim();
  const withoutProtocol = trimmed
    .replace(/^https?:\/\//i, "")
    .replace(/\/+$/g, "");
  return withoutProtocol;
};

export const useConnectionsStore = defineStore("connections", {
  state: () => ({
    connections: [] as GuiConnection[],
    activeConnection: null as GuiConnection | null,
    initialized: false,
    loading: false,
    error: "",
    setupStep: "details" as ConnectionSetupStep,
    draftType: "jira-cloud" as ConnectionType,
    draftName: "",
    draftHostname: "",
    draftUsername: "",
    draftToken: "",
  }),
  getters: {
    normalizedDraftHostname(state) {
      return normalizeHostname(state.draftHostname);
    },
    isDetailsValid(): boolean {
      return Boolean(
        this.draftType &&
          this.draftName.trim() &&
          this.normalizedDraftHostname,
      );
    },
    isCredentialsValid(state): boolean {
      return Boolean(state.draftUsername.trim() && state.draftToken.trim());
    },
    identityLabel(state): string {
      return state.draftType === "jira-cloud" ? "Email" : "Username";
    },
    connectionTypeLabel(state): string {
      return connectionTypeLabels[state.draftType];
    },
  },
  actions: {
    async initialize() {
      this.loading = true;
      this.error = "";

      try {
        await this.refreshConnections();
      } catch (error) {
        this.error =
          error instanceof Error ? error.message : String(error);
      } finally {
        this.loading = false;
        this.initialized = true;
      }
    },
    async refreshConnections() {
      const [connections, activeConnection] = await Promise.all([
        invoke<GuiConnection[]>("list_connections"),
        invoke<GuiConnection | null>("get_active_connection"),
      ]);
      this.connections = connections;
      this.activeConnection = activeConnection;
    },
    setConnectionType(type: ConnectionType) {
      this.draftType = type;
    },
    nextSetupStep() {
      if (!this.isDetailsValid) return;
      this.error = "";
      this.draftName = this.draftName.trim();
      this.draftHostname = this.normalizedDraftHostname;
      this.setupStep = "credentials";
    },
    previousSetupStep() {
      this.error = "";
      this.setupStep = "details";
    },
    async saveConnection() {
      if (!this.isDetailsValid || !this.isCredentialsValid) return false;

      this.loading = true;
      this.error = "";

      const input: CreateConnectionInput = {
        name: this.draftName.trim(),
        type: this.draftType,
        hostname: this.normalizedDraftHostname,
        username: this.draftUsername.trim(),
        token: this.draftToken.trim(),
      };

      try {
        this.activeConnection = await invoke<GuiConnection>("create_connection", {
          input,
        });
        await this.refreshConnections();
        this.draftToken = "";
        this.resetSetup();
        return true;
      } catch (error) {
        this.error =
          error instanceof Error ? error.message : String(error);
        return false;
      } finally {
        this.loading = false;
      }
    },
    resetSetup() {
      this.setupStep = "details";
      this.draftType = "jira-cloud";
      this.draftName = "";
      this.draftHostname = "";
      this.draftUsername = "";
      this.draftToken = "";
      this.error = "";
    },
    async setActiveConnection(connectionId: number) {
      if (this.activeConnection?.id === connectionId || this.loading) {
        return true;
      }

      this.loading = true;
      this.error = "";

      try {
        this.activeConnection = await invoke<GuiConnection | null>(
          "set_active_connection",
          { input: { connectionId } },
        );
        await this.refreshConnections();
        return true;
      } catch (error) {
        this.error =
          error instanceof Error ? error.message : String(error);
        return false;
      } finally {
        this.loading = false;
      }
    },
    async deleteConnection(connectionId: number) {
      if (this.loading) {
        return false;
      }

      this.loading = true;
      this.error = "";

      try {
        this.activeConnection = await invoke<GuiConnection | null>(
          "delete_connection",
          { input: { connectionId } },
        );
        await this.refreshConnections();
        return true;
      } catch (error) {
        this.error =
          error instanceof Error ? error.message : String(error);
        return false;
      } finally {
        this.loading = false;
      }
    },
  },
});
