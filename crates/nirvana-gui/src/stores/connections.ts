import { defineStore } from "pinia";
import { ConnectionSetupStep, ConnectionType, GuiConnection } from "../types/types";

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
    activeConnection: null as GuiConnection | null,
    setupStep: "details" as ConnectionSetupStep,
    draftType: "jira-cloud" as ConnectionType,
    draftHostname: "",
    draftUsername: "",
    draftToken: "",
  }),
  getters: {
    normalizedDraftHostname(state) {
      return normalizeHostname(state.draftHostname);
    },
    isDetailsValid(): boolean {
      return Boolean(this.draftType && this.normalizedDraftHostname);
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
    setConnectionType(type: ConnectionType) {
      this.draftType = type;
    },
    nextSetupStep() {
      if (!this.isDetailsValid) return;
      this.draftHostname = this.normalizedDraftHostname;
      this.setupStep = "credentials";
    },
    previousSetupStep() {
      this.setupStep = "details";
    },
    saveConnection() {
      if (!this.isDetailsValid || !this.isCredentialsValid) return false;

      this.activeConnection = {
        type: this.draftType,
        hostname: this.normalizedDraftHostname,
        username: this.draftUsername.trim(),
        token: this.draftToken,
      };
      this.draftToken = "";
      return true;
    },
    resetSetup() {
      this.setupStep = "details";
      this.draftType = "jira-cloud";
      this.draftHostname = "";
      this.draftUsername = "";
      this.draftToken = "";
    },
  },
});
