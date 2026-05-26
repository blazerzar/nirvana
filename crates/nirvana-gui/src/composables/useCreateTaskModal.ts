import { computed, ref, watch } from "vue";
import { useAllTasksStore } from "../stores/allTasks";
import { startOfDay } from "../tasks/time";
import { formatDurationInput } from "./dateTimeInputs";
import {
  formatCompletedRangePreview,
  useSessionRangeEditor,
} from "./useSessionRangeEditor";
import { normalizeTicketKey, useTicketKeySearch } from "./useTicketKeySearch";

export const useCreateTaskModal = () => {
  const tasks = useAllTasksStore();
  const firstField = ref<HTMLInputElement | null>(null);
  const noteField = ref<HTMLInputElement | null>(null);
  const ticketKey = ref("");
  const note = ref("");
  const localError = ref("");

  const clearSubmitError = () => {
    localError.value = "";
    tasks.error = "";
  };

  const range = useSessionRangeEditor({
    allowRunningStop: false,
    clearSubmitError,
    now: () => tasks.now,
  });

  const ticketSearch = useTicketKeySearch({
    activeModal: "create",
    ticketKey,
    focusAfterSelect: noteField,
    onTicketInput: clearSubmitError,
  });

  const overlapsExistingSlot = computed(() => {
    const proposedStart = range.parsedStart.value;
    const proposedStop = range.parsedStop.value;
    if (!proposedStart || !proposedStop) return false;

    const proposedStartMs = proposedStart.getTime();
    const proposedStopMs = proposedStop.getTime();

    return tasks.timelineSessions.some(({ session }) => {
      const sessionEndMs = (session.end ?? tasks.now).getTime();

      return (
        session.start.getTime() < proposedStopMs &&
        sessionEndMs > proposedStartMs
      );
    });
  });

  const computedError = computed(() => {
    if (tasks.activeModal !== "create") return "";
    if (!normalizeTicketKey(ticketKey.value)) return "Ticket key is required.";
    if (!range.startDateInput.value.trim()) return "Start date is required.";
    if (!range.parsedStartDate.value) return "Start date is invalid.";
    if (!range.stopDateInput.value.trim()) return "Stop date is required.";
    if (!range.parsedStopDate.value) return "Stop date is invalid.";
    if (!range.start.value.trim()) return "Start time is required.";
    if (!range.stop.value.trim()) return "Stop time is required.";
    if (!range.parsedStart.value) return "Start time is invalid.";
    if (!range.parsedStop.value) return "Stop time is invalid.";
    if (range.parsedStop.value.getTime() <= range.parsedStart.value.getTime()) {
      return "Stop must be after start.";
    }
    if (!range.durationInput.value.trim()) return "Duration is required.";
    if (!range.parsedDurationMs.value || range.parsedDurationMs.value <= 0) {
      return "Duration is invalid.";
    }
    if (
      range.parsedStart.value.getTime() > tasks.now.getTime() ||
      range.parsedStop.value.getTime() > tasks.now.getTime()
    ) {
      return "Slot time cannot be in the future.";
    }
    if (overlapsExistingSlot.value) {
      return "Time range overlaps an existing slot.";
    }
    return "";
  });

  const rangePreview = computed(() => {
    const key = normalizeTicketKey(ticketKey.value);

    if (key && range.parsedStart.value && range.parsedStop.value && !computedError.value) {
      const preview = formatCompletedRangePreview(
        range.parsedStart.value,
        range.parsedStop.value,
        formatDurationInput(range.parsedDurationMs.value ?? 0),
        range.stopDayOffset.value,
      );

      return `${ticketSearch.knownTask.value?.key ?? key} · ${preview}`;
    }

    return key
      ? `Add completed work for ${ticketSearch.knownTask.value?.key ?? key}.`
      : "Select a ticket or type a new key.";
  });

  const reset = () => {
    const now = new Date(tasks.now);
    const fallbackStop =
      startOfDay(tasks.selectedDate).getTime() === startOfDay(now).getTime()
        ? now
        : new Date(startOfDay(tasks.selectedDate).getTime() + 17 * 60 * 60 * 1000);
    const fallbackStart = new Date(fallbackStop.getTime() - 30 * 60 * 1000);

    ticketKey.value = tasks.selectedTask?.key ?? "";
    note.value = "";
    range.setRangeInputs({
      start: fallbackStart,
      stop: fallbackStop,
      durationMs: fallbackStop.getTime() - fallbackStart.getTime(),
    });
    localError.value = "";
    ticketSearch.searchOpen.value = true;
    ticketSearch.highlightedResultIndex.value = 0;
    tasks.error = "";
  };

  watch(
    [note, range.startDateInput, range.stopDateInput, range.start, range.stop, range.durationInput],
    clearSubmitError,
  );

  const handleNoteKeydown = (event: KeyboardEvent) => {
    if (event.key !== "Enter" || event.isComposing) return;

    event.preventDefault();
    submit();
  };

  const normalizeStartDate = async () => {
    const date = range.normalizeStartDate();
    if (!date) return;

    await tasks.setSelectedDate(date);
  };

  const submit = async () => {
    localError.value = computedError.value;
    if (localError.value || !range.parsedStart.value || !range.parsedStop.value) return;

    const saved = await tasks.createSession({
      ticketKey: ticketKey.value,
      note: note.value,
      start: range.parsedStart.value,
      end: range.parsedStop.value,
    });

    if (!saved) {
      localError.value = tasks.error || "This slot could not be added.";
    }
  };

  return {
    advancedStopDateVisible: range.advancedStopDateVisible,
    computedError,
    durationInput: range.durationInput,
    firstField,
    normalizeStartDate,
    normalizeStopDate: range.normalizeStopDate,
    handleDurationInput: range.handleDurationInput,
    handleNoteKeydown,
    handleStartDateInput: range.handleStartDateInput,
    handleDurationKeydown: range.handleDurationKeydown,
    handleStartTimeInput: range.handleStartTimeInput,
    handleStartKeydown: range.handleStartKeydown,
    handleStopDateInput: range.handleStopDateInput,
    handleStopTimeInput: range.handleStopTimeInput,
    handleStopKeydown: range.handleStopKeydown,
    handleTicketFocusout: ticketSearch.handleTicketFocusout,
    handleTicketKeydown: ticketSearch.handleTicketKeydown,
    highlightedResultIndex: ticketSearch.highlightedResultIndex,
    knownTask: ticketSearch.knownTask,
    localError,
    note,
    noteField,
    normalizeStartTime: range.normalizeStartTime,
    normalizeStopTime: range.normalizeStopTime,
    normalizeDuration: range.normalizeDuration,
    reset,
    rangePreview,
    searchOpen: ticketSearch.searchOpen,
    searchResults: ticketSearch.searchResults,
    selectSearchResult: ticketSearch.selectSearchResult,
    shouldShowSearch: ticketSearch.shouldShowSearch,
    slotCountLabel: ticketSearch.slotCountLabel,
    startDateInput: range.startDateInput,
    start: range.start,
    stopDayLabel: range.stopDayLabel,
    stopDayOffset: range.stopDayOffset,
    stopDateInput: range.stopDateInput,
    stop: range.stop,
    submit,
    ticketKey,
    ticketSearchRoot: ticketSearch.ticketSearchRoot,
    toggleAdvancedStopDate: range.toggleAdvancedStopDate,
  };
};
