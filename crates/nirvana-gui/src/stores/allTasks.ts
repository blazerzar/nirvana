import { defineStore } from "pinia";
import { createTasksData } from "../tasksData";
import {
  deleteSession as deleteSessionOperation,
  ensureTaskForKey as ensureTaskForKeyOperation,
  findTaskByKey as findTaskByKeyOperation,
  nextSessionId as nextSessionIdOperation,
  nextTaskId as nextTaskIdOperation,
  publishUnpublishedForDay,
  startTaskSession,
  stopActiveTask as stopActiveTaskOperation,
  updateSession as updateSessionOperation,
} from "../tasks/operations";
import {
  activeSessionFor,
  activeTaskFor,
  previousTaskFor,
  selectedSessionEntryFor,
  selectedTaskFor,
  summariesForDay,
  timelineSessionsForDay,
  totalDurationForDay,
  unpublishedDurationForDay,
} from "../tasks/selectors";
import {
  addDays,
  formatClock,
  formatDayLabel,
  formatDuration,
  startOfDay,
} from "../tasks/time";
import {
  EditSessionInput,
  ModalKind,
  StartTaskInput,
  Task,
  TaskSession,
  TaskSessionEntry,
  TaskSummary,
  TaskTimelineSession,
  ViewMode,
} from "../types/types";

export { formatClock, formatDayLabel, formatDuration };

export const useAllTasksStore = defineStore("allTasks", {
  state: () => ({
    tasks: createTasksData(),
    selectedTaskId: 12 as number | null,
    expandedTaskIds: [12] as number[],
    selectedSessionId: null as number | null,
    selectedDate: startOfDay(new Date()),
    viewMode: "day" as ViewMode,
    activeModal: null as ModalKind | null,
    now: new Date(),
  }),
  getters: {
    activeTask(state): Task | null {
      return activeTaskFor(state.tasks);
    },
    activeSession(state): TaskSession | null {
      return activeSessionFor(state.tasks);
    },
    previousTask(state): Task | null {
      const activeTask = activeTaskFor(state.tasks);
      return previousTaskFor(state.tasks, activeTask?.id ?? null);
    },
    summaries(state): TaskSummary[] {
      return summariesForDay(state.tasks, state.selectedDate, state.now);
    },
    timelineSessions(state): TaskTimelineSession[] {
      return timelineSessionsForDay(state.tasks, state.selectedDate, state.now);
    },
    selectedDateLabel(state): string {
      return formatDayLabel(state.selectedDate);
    },
    selectedDateTotalMs(state): number {
      return totalDurationForDay(state.tasks, state.selectedDate, state.now);
    },
    selectedDateUnpublishedTotalMs(state): number {
      return unpublishedDurationForDay(state.tasks, state.selectedDate, state.now);
    },
    selectedTask(state): Task | null {
      return selectedTaskFor(state.tasks, state.selectedTaskId);
    },
    selectedSessionEntry(state): TaskSessionEntry | null {
      return selectedSessionEntryFor(state.tasks, state.selectedSessionId);
    },
  },
  actions: {
    setViewMode(viewMode: ViewMode) {
      this.viewMode = viewMode;
    },
    previousDay() {
      this.selectedDate = startOfDay(addDays(this.selectedDate, -1));
      this.selectedSessionId = null;
    },
    nextDay() {
      this.selectedDate = startOfDay(addDays(this.selectedDate, 1));
      this.selectedSessionId = null;
    },
    goToToday() {
      this.selectedDate = startOfDay(new Date(this.now));
      this.selectedSessionId = null;
    },
    tick() {
      this.now = new Date();
    },
    selectTask(taskId: number) {
      this.selectedTaskId = taskId;
      this.selectedSessionId = null;
    },
    selectSession(sessionId: number) {
      const entry = this.tasks
        .flatMap((task) =>
          task.sessions.map((session) => ({
            taskId: task.id,
            sessionId: session.id,
          })),
        )
        .find((candidate) => candidate.sessionId === sessionId);

      if (!entry) {
        return;
      }

      this.selectedTaskId = entry.taskId;
      this.selectedSessionId = sessionId;
    },
    navigateSelection(direction: -1 | 1) {
      if (this.viewMode === "day") {
        const entries = this.timelineSessions;
        if (entries.length === 0) return;

        const currentIndex = entries.findIndex(
          (entry) => entry.session.id === this.selectedSessionId,
        );
        const nextIndex =
          currentIndex === -1
            ? direction > 0
              ? 0
              : entries.length - 1
            : Math.min(entries.length - 1, Math.max(0, currentIndex + direction));

        this.selectSession(entries[nextIndex].session.id);
        return;
      }

      const summaries = this.summaries;
      if (summaries.length === 0) return;

      const currentIndex = summaries.findIndex(
        (summary) => summary.task.id === this.selectedTaskId,
      );
      const nextIndex =
        currentIndex === -1
          ? direction > 0
            ? 0
            : summaries.length - 1
          : Math.min(summaries.length - 1, Math.max(0, currentIndex + direction));

      this.selectTask(summaries[nextIndex].task.id);
    },
    openStartModal() {
      this.activeModal = "start";
    },
    openEditModal() {
      if (!this.selectedSessionEntry) {
        return;
      }

      this.activeModal = "edit";
    },
    closeModal() {
      this.activeModal = null;
    },
    toggleExpanded(taskId: number) {
      if (this.expandedTaskIds.includes(taskId)) {
        this.expandedTaskIds = this.expandedTaskIds.filter((id) => id !== taskId);
        return;
      }

      this.expandedTaskIds = [...this.expandedTaskIds, taskId];
    },
    isExpanded(taskId: number) {
      return this.expandedTaskIds.includes(taskId);
    },
    stopActiveTask() {
      stopActiveTaskOperation(this.tasks, this.now);
    },
    nextTaskId() {
      return nextTaskIdOperation(this.tasks);
    },
    nextSessionId() {
      return nextSessionIdOperation(this.tasks);
    },
    findTaskByKey(ticketKey: string) {
      return findTaskByKeyOperation(this.tasks, ticketKey);
    },
    ensureTaskForKey(ticketKey: string, title?: string) {
      return ensureTaskForKeyOperation(this.tasks, ticketKey, title);
    },
    startTask(taskId?: number | null, note?: string) {
      const nextTaskId = taskId ?? this.selectedTaskId;
      if (nextTaskId === null) {
        return;
      }

      const started = startTaskSession(this.tasks, nextTaskId, this.now, note);
      if (!started) {
        return;
      }

      this.selectedDate = startOfDay(new Date(this.now));
      this.selectedTaskId = started.task.id;
      this.selectedSessionId = started.session.id;
      this.expandedTaskIds = Array.from(new Set([...this.expandedTaskIds, started.task.id]));
    },
    startTaskFromInput(input: StartTaskInput) {
      const task = this.ensureTaskForKey(input.ticketKey, input.title);
      this.startTask(task.id, input.note);
      this.activeModal = null;
    },
    switchToPreviousTask() {
      if (!this.previousTask || !this.activeTask) {
        return;
      }

      this.startTask(this.previousTask.id);
    },
    switchToSelectedTask() {
      this.switchToPreviousTask();
    },
    updateSession(input: EditSessionInput) {
      const updated = updateSessionOperation(this.tasks, input);
      if (!updated) {
        return false;
      }

      this.selectedTaskId = updated.task.id;
      this.selectedSessionId = updated.session.id;
      this.expandedTaskIds = Array.from(new Set([...this.expandedTaskIds, updated.task.id]));
      this.activeModal = null;
      return true;
    },
    deleteSelectedSession() {
      const task = deleteSessionOperation(this.tasks, this.selectedSessionId);
      if (!task) {
        return false;
      }

      this.selectedSessionId = null;
      this.selectedTaskId = task.id;
      this.activeModal = null;
      return true;
    },
    publishUnpublished() {
      publishUnpublishedForDay(this.tasks, this.selectedDate);
    },
    runShortcut(key: string) {
      if (this.activeModal) {
        return;
      }

      if (key === "s") {
        this.openStartModal();
      }

      if (key === "x") {
        this.stopActiveTask();
      }

      if (key === "w") {
        this.switchToPreviousTask();
      }

      if (key === "p") {
        this.publishUnpublished();
      }

      if (key === "e") {
        this.openEditModal();
      }
    },
  },
});
