import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import { IdlePeriod } from "../types/types";
import { useAllTasksStore } from "./allTasks";

export const useIdleStore = defineStore("idle", {
  state: () => ({
    idlePeriods: [] as IdlePeriod[],
    wasWorking: null as boolean | null,
    continueTracking: null as boolean | null,
  }),
  getters: {
    currentPeriod(state): IdlePeriod | null {
      return state.idlePeriods[0] ?? null;
    },
  },
  actions: {
    async fetchIdlePeriods() {
      try {
        this.idlePeriods = await invoke<IdlePeriod[]>("get_idle_periods");
      } catch {
        this.idlePeriods = [];
      }
    },
    async resolve() {
      const period = this.currentPeriod;
      if (!period || this.wasWorking === null || this.continueTracking === null) {
        return;
      }

      try {
        await invoke("resolve_idle", {
          input: {
            idleStart: period.from,
            idleEnd: period.to,
            wasWorking: this.wasWorking,
            continueTracking: this.continueTracking,
          },
        });
      } catch {
        return;
      }

      this.wasWorking = null;
      this.continueTracking = null;

      const tasks = useAllTasksStore();
      await Promise.all([tasks.refreshRunningSlot(), tasks.loadSelectedDate()]);
      await this.fetchIdlePeriods();
    },
    dismiss() {
      this.idlePeriods = [];
      this.wasWorking = null;
      this.continueTracking = null;
    },
    drain() {
      void invoke("get_idle_periods");
    },
  },
});
