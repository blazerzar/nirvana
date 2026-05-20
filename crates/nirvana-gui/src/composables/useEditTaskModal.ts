import { computed, ref } from "vue";
import { formatDuration, useAllTasksStore } from "../stores/allTasks";
import { formatDateTimeInput, parseDateTimeInput } from "./dateTimeInputs";

const normalizeTicketKey = (value: string) => value.trim().toUpperCase();

export const useEditTaskModal = () => {
  const tasks = useAllTasksStore();
  const firstField = ref<HTMLInputElement | null>(null);
  const ticketKey = ref("");
  const start = ref("");
  const stop = ref("");
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

  const duration = computed(() => {
    const parsedStart = parseDateTimeInput(start.value);
    const parsedStop = stop.value ? parseDateTimeInput(stop.value) : null;
    if (!parsedStart || (stop.value && !parsedStop)) return "Invalid";

    const end = parsedStop ?? tasks.now;
    return formatDuration(end.getTime() - parsedStart.getTime());
  });

  const computedError = computed(() => {
    if (tasks.activeModal !== "edit") return "";
    if (readOnly.value) return "Published slots are read-only.";
    if (!normalizeTicketKey(ticketKey.value)) return "Ticket key is required.";

    const parsedStart = parseDateTimeInput(start.value);
    if (!parsedStart) return "Start time is required.";

    const wasRunning = entry.value?.session.end === null;
    if (!stop.value && !wasRunning) return "Stopped slots need a stop time.";

    if (stop.value) {
      const parsedStop = parseDateTimeInput(stop.value);
      if (!parsedStop) return "Stop time is invalid.";
      if (parsedStop.getTime() <= parsedStart.getTime()) {
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
      return;
    }

    ticketKey.value = selectedEntry.task.key;
    start.value = formatDateTimeInput(selectedEntry.session.start);
    stop.value = selectedEntry.session.end
      ? formatDateTimeInput(selectedEntry.session.end)
      : "";
    note.value = selectedEntry.session.note ?? "";
  };

  const submit = () => {
    error.value = computedError.value;
    const selectedEntry = entry.value;
    const parsedStart = parseDateTimeInput(start.value);
    const parsedStop = stop.value ? parseDateTimeInput(stop.value) : null;

    if (error.value || !selectedEntry || !parsedStart) return;

    const saved = tasks.updateSession({
      sessionId: selectedEntry.session.id,
      ticketKey: ticketKey.value,
      start: parsedStart,
      end: parsedStop,
      note: note.value,
    });

    if (!saved) {
      error.value = "This slot could not be saved.";
      return;
    }

    tasks.activeModal = null;
  };

  const deleteSession = () => {
    error.value = "";

    if (!tasks.deleteSelectedSession()) {
      error.value = "This slot cannot be deleted.";
      return;
    }

    tasks.activeModal = null;
  };

  return {
    computedError,
    deleteSession,
    duration,
    entry,
    error,
    firstField,
    knownTask,
    note,
    readOnly,
    reset,
    start,
    stop,
    submit,
    ticketKey,
  };
};
