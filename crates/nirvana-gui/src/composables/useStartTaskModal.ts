import { computed, nextTick, ref, watch } from "vue";
import { formatClock, useAllTasksStore } from "../stores/allTasks";
import { isSameDay, startOfDay } from "../tasks/time";
import {
  applyTimeParts,
  formatTimeInput,
  formatTimeParts,
  parseTimeParts,
  wrapTimePart,
} from "./dateTimeInputs";
import { normalizeTicketKey, useTicketKeySearch } from "./useTicketKeySearch";

export const useStartTaskModal = () => {
  const tasks = useAllTasksStore();
  const firstField = ref<HTMLInputElement | null>(null);
  const noteField = ref<HTMLInputElement | null>(null);
  const ticketKey = ref("");
  const note = ref("");
  const start = ref("");

  const clearSubmitError = () => {
    tasks.error = "";
  };

  const ticketSearch = useTicketKeySearch({
    activeModal: "start",
    ticketKey,
    focusAfterSelect: noteField,
    onTicketInput: clearSubmitError,
    sortByRecent: true,
  });

  const activeTaskWarning = computed(() => {
    const key = normalizeTicketKey(ticketKey.value);

    if (
      !key ||
      !tasks.activeTask ||
      tasks.activeTask.key === key
    ) {
      return "";
    }

    const since = tasks.activeSession ? formatClock(tasks.activeSession.start) : "";
    return `${tasks.activeTask.key} has been running${since ? ` since ${since}` : ""} and will be stopped.`;
  });

  const parsedStart = computed(() => {
    const time = parseTimeParts(start.value);
    if (!time) return null;

    if (
      isSameDay(tasks.selectedDate, tasks.now) &&
      start.value === formatTimeInput(tasks.now)
    ) {
      return new Date(tasks.now);
    }

    return applyTimeParts(startOfDay(tasks.selectedDate), time);
  });

  const overlapsExistingSlot = computed(() => {
    const proposedStart = parsedStart.value;
    if (!proposedStart) return false;

    const proposedStartMs = proposedStart.getTime();
    const proposedEndMs = tasks.now.getTime();

    if (
      tasks.activeSession &&
      proposedStartMs <= tasks.activeSession.start.getTime()
    ) {
      return true;
    }

    return tasks.timelineSessions.some(({ session }) => {
      if (session.end === null) return false;

      return (
        session.start.getTime() < proposedEndMs &&
        session.end.getTime() > proposedStartMs
      );
    });
  });

  const statusText = computed(() => {
    if (activeTaskWarning.value) return activeTaskWarning.value;

    const key = normalizeTicketKey(ticketKey.value);
    if (key && parsedStart.value) {
      return `Starting ${ticketSearch.knownTask.value?.key ?? key} at ${formatClock(parsedStart.value)}`;
    }

    return key
      ? `Select a ticket or press Enter to create ${key}.`
      : "Select a ticket or type a new key.";
  });

  const validationError = computed(() => {
    if (tasks.activeModal !== "start") return "";
    if (!normalizeTicketKey(ticketKey.value)) return "Ticket key is required.";
    if (!start.value.trim()) return "Start time is required.";
    if (!parsedStart.value) return "Start time is invalid.";
    if (parsedStart.value.getTime() > tasks.now.getTime()) {
      return "Start time cannot be in the future.";
    }
    if (overlapsExistingSlot.value) {
      return "Start time overlaps an existing slot.";
    }
    return "";
  });

  const error = computed(() => tasks.error || validationError.value);

  const reset = () => {
    ticketKey.value = "";
    note.value = "";
    start.value = formatTimeInput(new Date(tasks.now));
    ticketSearch.searchOpen.value = true;
    ticketSearch.highlightedResultIndex.value = 0;
    tasks.error = "";
  };

  watch([note, start], clearSubmitError);

  const submit = async () => {
    if (validationError.value) return;

    await tasks.startTaskFromInput({
      ticketKey: ticketKey.value,
      note: note.value,
      start: parsedStart.value ?? undefined,
    });
  };

  const handleNoteKeydown = (event: KeyboardEvent) => {
    if (event.key !== "Enter" || event.isComposing) return;

    event.preventDefault();
    submit();
  };

  const normalizeStartTime = () => {
    const time = parseTimeParts(start.value);
    if (!time) return;

    start.value = formatTimeParts(time.hours, time.minutes);
  };

  const handleStartKeydown = async (event: KeyboardEvent) => {
    if (event.key !== "ArrowUp" && event.key !== "ArrowDown") return;

    event.preventDefault();

    const input = event.currentTarget as HTMLInputElement;
    const direction = event.key === "ArrowUp" ? 1 : -1;
    const time = parseTimeParts(start.value) ?? parseTimeParts(formatTimeInput(tasks.now));
    if (!time) return;

    const editingMinutes =
      input.selectionStart !== null && input.selectionStart > start.value.indexOf(":");
    const nextHours = editingMinutes
      ? time.hours
      : wrapTimePart(time.hours + direction, 24);
    const nextMinutes = editingMinutes
      ? wrapTimePart(time.minutes + direction, 60)
      : time.minutes;

    start.value = formatTimeParts(nextHours, nextMinutes);
    await nextTick();

    if (editingMinutes) {
      input.setSelectionRange(3, 5);
    } else {
      input.setSelectionRange(0, 2);
    }
  };

  return {
    activeTaskWarning,
    error,
    firstField,
    handleNoteKeydown,
    handleStartKeydown,
    handleTicketKeydown: ticketSearch.handleTicketKeydown,
    highlightedResultIndex: ticketSearch.highlightedResultIndex,
    knownTask: ticketSearch.knownTask,
    note,
    noteField,
    normalizeStartTime,
    reset,
    searchOpen: ticketSearch.searchOpen,
    searchResults: ticketSearch.searchResults,
    selectSearchResult: ticketSearch.selectSearchResult,
    shouldShowSearch: ticketSearch.shouldShowSearch,
    slotCountLabel: ticketSearch.slotCountLabel,
    start,
    statusText,
    submit,
    ticketKey,
    validationError,
  };
};
