import { computed, ref, watch } from "vue";
import { useAllTasksStore } from "../stores/allTasks";
import { formatDurationInput } from "./dateTimeInputs";
import {
  formatCompletedRangePreview,
  formatRunningRangePreview,
  useSessionRangeEditor,
} from "./useSessionRangeEditor";
import { normalizeTicketKey, useTicketKeySearch } from "./useTicketKeySearch";

export const useEditTaskModal = () => {
  const tasks = useAllTasksStore();
  const firstField = ref<HTMLInputElement | null>(null);
  const noteField = ref<HTMLInputElement | null>(null);
  const ticketKey = ref("");
  const note = ref("");
  const error = ref("");

  const clearSubmitError = () => {
    error.value = "";
    tasks.error = "";
  };

  const range = useSessionRangeEditor({
    allowRunningStop: true,
    clearSubmitError,
    now: () => tasks.now,
  });

  const entry = computed(() => tasks.selectedSessionEntry);

  const ticketSearch = useTicketKeySearch({
    activeModal: "edit",
    ticketKey,
    focusAfterSelect: noteField,
    onTicketInput: clearSubmitError,
  });

  const readOnly = computed(
    () => entry.value?.session.publishState === "published",
  );

  const rangePreview = computed(() => {
    const key = ticketSearch.knownTask.value?.key ?? normalizeTicketKey(ticketKey.value);
    if (!key || !range.parsedStart.value) return "Select a ticket or type a new key.";

    const duration = range.parsedDurationMs.value
      ? formatDurationInput(range.parsedDurationMs.value)
      : range.durationInput.value;
    if (!range.parsedStop.value) {
      return `${key} · ${formatRunningRangePreview(range.parsedStart.value)}`;
    }

    const preview = formatCompletedRangePreview(
      range.parsedStart.value,
      range.parsedStop.value,
      duration,
      range.stopDayOffset.value,
    );

    return `${key} · ${preview}`;
  });

  const computedError = computed(() => {
    if (tasks.activeModal !== "edit") return "";
    if (readOnly.value) return "Published slots are read-only.";
    if (!normalizeTicketKey(ticketKey.value)) return "Ticket key is required.";

    if (!range.startDateInput.value.trim()) return "Start date is required.";
    if (!range.parsedStartDate.value) return "Start date is invalid.";
    if (!range.start.value.trim()) return "Start time is required.";
    if (!range.parsedStart.value) return "Start time is invalid.";

    if (range.stop.value.trim()) {
      if (!range.stopDateInput.value.trim()) return "Stop date is required.";
      if (!range.parsedStopDate.value) return "Stop date is invalid.";
      if (!range.parsedStop.value) return "Stop time is invalid.";
      if (range.parsedStop.value.getTime() <= range.parsedStart.value.getTime()) {
        return "Stop must be after start.";
      }
    }

    if (!range.durationInput.value.trim()) return "Duration is required.";
    if (!range.parsedDurationMs.value || range.parsedDurationMs.value <= 0) {
      return "Duration is invalid.";
    }

    return "";
  });

  const reset = () => {
    const selectedEntry = tasks.selectedSessionEntry;
    error.value = "";

    if (!selectedEntry) {
      ticketKey.value = "";
      range.setRangeInputs({ start: null, stop: null, durationMs: null });
      note.value = "";
      return;
    }

    ticketKey.value = selectedEntry.task.key;
    range.setRangeInputs({
      start: selectedEntry.session.start,
      stop: selectedEntry.session.end,
      durationMs:
        (selectedEntry.session.end ?? tasks.now).getTime() -
        selectedEntry.session.start.getTime(),
    });
    note.value = selectedEntry.session.note ?? "";
    ticketSearch.searchOpen.value = true;
    ticketSearch.highlightedResultIndex.value = 0;
  };

  watch(
    [note, range.startDateInput, range.stopDateInput, range.start, range.stop, range.durationInput],
    clearSubmitError,
  );

  const submit = async () => {
    error.value = computedError.value;
    const selectedEntry = entry.value;
    const nextStart = range.parsedStart.value;
    const nextStop = range.stop.value.trim() ? range.parsedStop.value : null;

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
    advancedStopDateVisible: range.advancedStopDateVisible,
    deleteSession,
    durationInput: range.durationInput,
    entry,
    error,
    firstField,
    handleDurationKeydown: range.handleDurationKeydown,
    handleDurationInput: range.handleDurationInput,
    handleStartDateInput: range.handleStartDateInput,
    handleStartTimeInput: range.handleStartTimeInput,
    handleStartKeydown: range.handleStartKeydown,
    handleStopDateInput: range.handleStopDateInput,
    handleStopTimeInput: range.handleStopTimeInput,
    handleStopKeydown: range.handleStopKeydown,
    handleTicketFocusout: ticketSearch.handleTicketFocusout,
    handleTicketKeydown: ticketSearch.handleTicketKeydown,
    highlightedResultIndex: ticketSearch.highlightedResultIndex,
    knownTask: ticketSearch.knownTask,
    note,
    noteField,
    normalizeDuration: range.normalizeDuration,
    normalizeStartDate: range.normalizeStartDate,
    normalizeStartTime: range.normalizeStartTime,
    normalizeStopDate: range.normalizeStopDate,
    normalizeStopTime: range.normalizeStopTime,
    readOnly,
    rangePreview,
    reset,
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
