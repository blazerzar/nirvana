import { defineStore } from "pinia";
import { Task } from "../types/types";

export const useActiveTaskStore = defineStore("activeTask", {
  state: () => ({
    task: null as null | Task,
  }),
  actions: {
    setTask(task: Task) {
      this.task = task;
    },

    clearTask() {
      this.task = null;
    },
  },
});
