import { computed, nextTick, ref } from "vue";
import { useAllTasksStore } from "../stores/allTasks";
import {
  applyTimeParts,
  formatDurationInput,
  formatTimeInput,
  formatTimeParts,
  parseDurationInput,
  parseTimeParts,
  wrapTimePart,
} from "./dateTimeInputs";

const normalizeTicketKey = (value: string) => value.trim().toUpperCase();

export const useEditTaskModal = () => {
  const tasks = useAllTasksStore();
  const firstField = ref<HTMLInputElement | null>(null);
  const ticketKey = ref("");
  const start = ref("");
  const stop = ref("");
  const durationInput = ref("");
  const note = ref("");
  const error = ref("");

  const entry = computed(() => tasks.selectedSessionEntry);

  const knownTask = computed(
    () =>
      tasks.tasks.find(
        (task) => task.key.toUpperCase() === normalizeTicketKey(ticketKey.value),
      ) ?? null,
  );

  const readOnly = computed(
    () => entry.value?.session.publishState === "published",
  );

  const parsedStart = computed(() => {
    const selectedEntry = entry.value;
    const time = parseTimeParts(start.value);
    if (!selectedEntry || !time) return null;

    return applyTimeParts(selectedEntry.session.start, time);
  });

  const parsedStop = computed(() => {
    const selectedEntry = entry.value;
    const time = stop.value ? parseTimeParts(stop.value) : null;
    if (!selectedEntry || !time) return null;

    return applyTimeParts(
      selectedEntry.session.end ?? selectedEntry.session.start,
      time,
    );
  });

  const parsedDurationMs = computed(() => parseDurationInput(durationInput.value));

  const computedError = computed(() => {
    if (tasks.activeModal !== "edit") return "";
    if (readOnly.value) return "Published slots are read-only.";
    if (!normalizeTicketKey(ticketKey.value)) return "Ticket key is required.";

    if (!start.value.trim()) return "Start time is required.";
    if (!parsedStart.value) return "Start time is invalid.";
    if (!durationInput.value.trim()) return "Duration is required.";
    if (!parsedDurationMs.value || parsedDurationMs.value <= 0) {
      return "Duration is invalid.";
    }

    if (stop.value) {
      if (!parsedStop.value) return "Stop time is invalid.";
      if (parsedStop.value.getTime() <= parsedStart.value.getTime()) {
        return "Stop must be after start.";
      }
    }

    return "";
  });

  const reset = () => {
    const selectedEntry = tasks.selectedSessionEntry;
    error.value = "";

    if (!selectedEntry) {
      ticketKey.value = "";
      start.value = "";
      stop.value = "";
      note.value = "";
      durationInput.value = "";
      return;
    }

    ticketKey.value = selectedEntry.task.key;
    start.value = formatTimeInput(selectedEntry.session.start);
    stop.value = selectedEntry.session.end
      ? formatTimeInput(selectedEntry.session.end)
      : "";
    durationInput.value = formatDurationInput(
      (selectedEntry.session.end ?? tasks.now).getTime() -
        selectedEntry.session.start.getTime(),
    );
    note.value = selectedEntry.session.note ?? "";
  };

  const submit = async () => {
    error.value = computedError.value;
    const selectedEntry = entry.value;
    const nextStart = parsedStart.value;
    const nextStop = stop.value ? parsedStop.value : null;

    if (error.value || !selectedEntry || !nextStart) return;

    const saved = await tasks.updateSession({
      sessionId: selectedEntry.session.id,
      ticketKey: ticketKey.value,
      start: nextStart,
      end: nextStop,
      note: note.value,
    });

    if (!saved) {
      error.value = tasks.error || "This slot could not be saved.";
      return;
    }

    tasks.closeModal();
  };

  const normalizeStartTime = () => {
    const time = parseTimeParts(start.value);
    if (!time) return;

    start.value = formatTimeParts(time.hours, time.minutes);
    if (stop.value) {
      syncDurationFromTimes();
    }
  };

  const normalizeStopTime = () => {
    if (!stop.value.trim()) return;

    const time = parseTimeParts(stop.value);
    if (!time) return;

    stop.value = formatTimeParts(time.hours, time.minutes);
    syncDurationFromTimes();
  };

  const syncDurationFromTimes = () => {
    if (!parsedStart.value) return;

    const end = parsedStop.value ?? tasks.now;
    durationInput.value = formatDurationInput(
      end.getTime() - parsedStart.value.getTime(),
    );
  };

  const applyDurationToStop = () => {
    if (!parsedStart.value || !parsedDurationMs.value) return;

    stop.value = formatTimeInput(
      new Date(parsedStart.value.getTime() + parsedDurationMs.value),
    );
  };

  const normalizeDuration = () => {
    if (!parsedDurationMs.value) return;

    durationInput.value = formatDurationInput(parsedDurationMs.value);
    applyDurationToStop();
  };

  const handleDurationKeydown = (event: KeyboardEvent) => {
    if (event.key !== "ArrowUp" && event.key !== "ArrowDown") return;

    event.preventDefault();

    const direction = event.key === "ArrowUp" ? 1 : -1;
    const currentMs = parsedDurationMs.value ?? 30 * 60 * 1000;
    const stepMs = 5 * 60 * 1000;
    const nextMs = Math.max(60 * 1000, currentMs + direction * stepMs);

    durationInput.value = formatDurationInput(nextMs);
    applyDurationToStop();
  };

  const handleTimeKeydown = async (
    event: KeyboardEvent,
    value: typeof start,
  ) => {
    if (event.key !== "ArrowUp" && event.key !== "ArrowDown") return;

    event.preventDefault();

    const input = event.currentTarget as HTMLInputElement;
    const direction = event.key === "ArrowUp" ? 1 : -1;
    const fallback = value === stop && !value.value ? start.value : value.value;
    const time = parseTimeParts(value.value) ?? parseTimeParts(fallback);
    if (!time) return;

    const separatorIndex = value.value.indexOf(":");
    const editingMinutes =
      separatorIndex !== -1 &&
      input.selectionStart !== null &&
      input.selectionStart > separatorIndex;
    const nextHours = editingMinutes
      ? time.hours
      : wrapTimePart(time.hours + direction, 24);
    const nextMinutes = editingMinutes
      ? wrapTimePart(time.minutes + direction, 60)
      : time.minutes;

    value.value = formatTimeParts(nextHours, nextMinutes);
    await nextTick();

    if (editingMinutes) {
      input.setSelectionRange(3, 5);
    } else {
      input.setSelectionRange(0, 2);
    }
  };

  const handleStartKeydown = (event: KeyboardEvent) =>
    handleTimeKeydown(event, start);

  const handleStopKeydown = (event: KeyboardEvent) =>
    handleTimeKeydown(event, stop);

  const deleteSession = async () => {
    error.value = "";

    if (!(await tasks.deleteSelectedSession())) {
      error.value = tasks.error || "This slot cannot be deleted.";
      return;
    }

    tasks.closeModal();
  };

  return {
    computedError,
    deleteSession,
    durationInput,
    entry,
    error,
    firstField,
    handleDurationKeydown,
    handleStartKeydown,
    handleStopKeydown,
    knownTask,
    note,
    applyDurationToStop,
    normalizeDuration,
    normalizeStartTime,
    normalizeStopTime,
    readOnly,
    reset,
    start,
    stop,
    submit,
    ticketKey,
  };
};
