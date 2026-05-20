import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import { normalizeTicketKey, normalizedNote } from "../tasks/operations";
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
  BackendSlot,
  BackendPublishResult,
  BackendTicket,
  EditSessionInput,
  ModalKind,
  StartTaskInput,
  Task,
  TaskSession,
  TaskSessionEntry,
  TaskStatus,
  TaskSummary,
  TaskTimelineSession,
  ViewMode,
} from "../types/types";

export { formatClock, formatDayLabel, formatDuration };

const taskIdForKey = (ticketKey: string) => {
  let hash = 0;
  const key = normalizeTicketKey(ticketKey);

  for (let index = 0; index < key.length; index += 1) {
    hash = (hash * 31 + key.charCodeAt(index)) | 0;
  }

  return Math.abs(hash) || 1;
};

const dateRangeSeconds = (selectedDate: Date) => {
  const fromDate = startOfDay(selectedDate);
  const toDate = startOfDay(addDays(fromDate, 1));

  return {
    from: Math.floor(fromDate.getTime() / 1000),
    to: Math.floor(toDate.getTime() / 1000),
  };
};

const dateKey = (date: Date) => startOfDay(date).toISOString();

const buildTasksFromBackend = (
  slots: BackendSlot[],
  tickets: BackendTicket[],
): Task[] => {
  const tasksByKey = new Map<string, Task>();

  const ensureTask = (
    ticketKey: string,
    title?: string | null,
    url?: string | null,
  ) => {
    const key = normalizeTicketKey(ticketKey);
    const existing = tasksByKey.get(key);

    if (existing) {
      if (title && existing.title === existing.key) {
        existing.title = title;
      }
      if (url) {
        existing.url = url;
      }
      return existing;
    }

    const task: Task = {
      id: taskIdForKey(key),
      key,
      title: title || key,
      status: TaskStatus.Idle,
      url: url ?? undefined,
      sessions: [],
    };

    tasksByKey.set(key, task);
    return task;
  };

  tickets.forEach((ticket) => {
    ensureTask(ticket.ticket_key, ticket.summary, ticket.issue_url);
  });

  slots.forEach((slot) => {
    const task = ensureTask(slot.ticket_key, slot.summary, slot.issue_url);
    const session: TaskSession = {
      id: slot.id,
      taskId: task.id,
      start: new Date(slot.started_at * 1000),
      end: slot.stopped_at ? new Date(slot.stopped_at * 1000) : null,
      note: slot.note ?? undefined,
      publishState: slot.published_at === null ? "unpublished" : "published",
    };

    task.sessions.push(session);
    if (session.end === null) {
      task.status = TaskStatus.Running;
    }
  });

  return Array.from(tasksByKey.values()).sort((left, right) =>
    left.key.localeCompare(right.key),
  );
};

export const useAllTasksStore = defineStore("allTasks", {
  state: () => ({
    tasks: [] as Task[],
    selectedTaskId: null as number | null,
    expandedTaskIds: [] as number[],
    selectedSessionId: null as number | null,
    selectedDate: startOfDay(new Date()),
    loadedDateKey: "",
    loading: false,
    error: "",
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
    publishableSessions(state): TaskTimelineSession[] {
      return timelineSessionsForDay(state.tasks, state.selectedDate, state.now)
        .filter(
          (entry) =>
            entry.session.publishState === "unpublished" &&
            entry.session.end !== null,
        );
    },
  },
  actions: {
    setViewMode(viewMode: ViewMode) {
      this.viewMode = viewMode;
    },
    previousDay() {
      this.selectedDate = startOfDay(addDays(this.selectedDate, -1));
      this.selectedSessionId = null;
      void this.loadSelectedDate();
    },
    nextDay() {
      this.selectedDate = startOfDay(addDays(this.selectedDate, 1));
      this.selectedSessionId = null;
      void this.loadSelectedDate();
    },
    goToToday() {
      this.selectedDate = startOfDay(new Date(this.now));
      this.selectedSessionId = null;
      void this.loadSelectedDate();
    },
    tick() {
      this.now = new Date();
    },
    async loadSelectedDate() {
      const { from, to } = dateRangeSeconds(this.selectedDate);
      this.loading = true;
      this.error = "";

      try {
        const [slots, tickets] = await Promise.all([
          invoke<BackendSlot[]>("list_slots", {
            input: { from, to, sort: "started" },
          }),
          invoke<BackendTicket[]>("list_recent_tickets"),
        ]);

        const previousSelectedTaskId = this.selectedTaskId;
        const previousSelectedSessionId = this.selectedSessionId;

        this.tasks = buildTasksFromBackend(slots, tickets);
        this.loadedDateKey = dateKey(this.selectedDate);

        const selectedSessionExists =
          previousSelectedSessionId !== null &&
          this.tasks.some((task) =>
            task.sessions.some(
              (session) => session.id === previousSelectedSessionId,
            ),
          );
        const selectedTaskExists =
          previousSelectedTaskId !== null &&
          this.tasks.some((task) => task.id === previousSelectedTaskId);

        this.selectedSessionId = selectedSessionExists
          ? previousSelectedSessionId
          : null;
        this.selectedTaskId = selectedSessionExists
          ? this.selectedSessionEntry?.task.id ?? null
          : selectedTaskExists
            ? previousSelectedTaskId
            : this.timelineSessions[0]?.task.id ?? this.tasks[0]?.id ?? null;

        this.expandedTaskIds = Array.from(
          new Set([
            ...this.expandedTaskIds.filter((id) =>
              this.tasks.some((task) => task.id === id),
            ),
            ...(this.activeTask ? [this.activeTask.id] : []),
          ]),
        );
      } catch (error) {
        this.error = error instanceof Error ? error.message : String(error);
        this.tasks = [];
        this.selectedSessionId = null;
        this.selectedTaskId = null;
      } finally {
        this.loading = false;
      }
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
      this.error = "Editing database slots is not supported yet.";
    },
    openPublishModal() {
      if (this.publishableSessions.length === 0) {
        return;
      }

      this.error = "";
      this.activeModal = "publish";
    },
    closeModal() {
      this.activeModal = null;
      this.error = "";
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
    async stopActiveTask() {
      this.loading = true;
      this.error = "";

      try {
        await invoke<BackendSlot | null>("stop_slot");
        await this.loadSelectedDate();
      } catch (error) {
        this.error = error instanceof Error ? error.message : String(error);
      } finally {
        this.loading = false;
      }
    },
    findTaskByKey(ticketKey: string) {
      const key = normalizeTicketKey(ticketKey);
      return this.tasks.find((task) => task.key.toUpperCase() === key) ?? null;
    },
    async startTask(ticketKey?: string | null, note?: string, start?: Date) {
      const key =
        ticketKey ??
        (this.selectedTaskId
          ? this.tasks.find((task) => task.id === this.selectedTaskId)?.key
          : null);

      if (!key) return false;

      this.loading = true;
      this.error = "";

      try {
        const slot = await invoke<BackendSlot>("start_slot", {
          input: {
            ticketKey: key,
            note: normalizedNote(note) ?? null,
            startedAt: start ? Math.floor(start.getTime() / 1000) : null,
          },
        });
        this.selectedDate = startOfDay(new Date(slot.started_at * 1000));
        await this.loadSelectedDate();
        this.selectSession(slot.id);
        this.expandedTaskIds = Array.from(
          new Set([
            ...this.expandedTaskIds,
            this.selectedSessionEntry?.task.id,
          ].filter((id): id is number => typeof id === "number")),
        );
        return true;
      } catch (error) {
        this.error = error instanceof Error ? error.message : String(error);
        return false;
      } finally {
        this.loading = false;
      }
    },
    async startTaskFromInput(input: StartTaskInput) {
      const started = await this.startTask(input.ticketKey, input.note, input.start);
      if (started) {
        this.activeModal = null;
      }
      return started;
    },
    async switchToPreviousTask() {
      if (!this.previousTask || !this.activeTask) {
        return false;
      }

      return this.startTask(this.previousTask.key);
    },
    switchToSelectedTask() {
      return this.switchToPreviousTask();
    },
    updateSession(input: EditSessionInput) {
      void input;
      this.error = "Editing database slots is not supported yet.";
      return false;
    },
    deleteSelectedSession() {
      this.error = "Deleting database slots is not supported yet.";
      return false;
    },
    async confirmPublishUnpublished() {
      const { from, to } = dateRangeSeconds(this.selectedDate);
      this.loading = true;
      this.error = "";

      try {
        const result = await invoke<BackendPublishResult>("publish_slots", {
          input: { from, to },
        });
        await this.loadSelectedDate();

        if (result.failed.length > 0) {
          this.error = `${result.failed.length} ${result.failed.length === 1 ? "slot" : "slots"} failed to publish.`;
        } else {
          this.activeModal = null;
        }
      } catch (error) {
        this.error = error instanceof Error ? error.message : String(error);
      } finally {
        this.loading = false;
      }
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
        this.openPublishModal();
      }

      if (key === "e") {
        this.openEditModal();
      }
    },
  },
});
