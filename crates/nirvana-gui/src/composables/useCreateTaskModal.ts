import { computed, nextTick, ref, watch } from "vue";
import { useAllTasksStore } from "../stores/allTasks";
import { startOfDay } from "../tasks/time";
import { Task } from "../types/types";
import { formatDurationInput } from "./dateTimeInputs";
import {
  formatCompletedRangePreview,
  useSessionRangeEditor,
} from "./useSessionRangeEditor";

type TicketSearchResult = {
  task: Task;
  score: number;
};

const normalizeTicketKey = (value: string) => value.trim().toUpperCase();

const fuzzyScore = (query: string, candidate: string) => {
  const normalizedQuery = query.trim().toLowerCase();
  const normalizedCandidate = candidate.toLowerCase();

  if (!normalizedQuery) return 1;
  if (normalizedCandidate === normalizedQuery) return 1000;
  if (normalizedCandidate.startsWith(normalizedQuery)) {
    return 800 - normalizedCandidate.length;
  }
  if (normalizedCandidate.includes(normalizedQuery)) {
    return 600 - normalizedCandidate.indexOf(normalizedQuery);
  }

  let score = 0;
  let searchIndex = 0;
  let consecutive = 0;

  for (const character of normalizedQuery) {
    const foundIndex = normalizedCandidate.indexOf(character, searchIndex);
    if (foundIndex === -1) return 0;

    consecutive = foundIndex === searchIndex ? consecutive + 1 : 0;
    score += 12 + consecutive * 8 - foundIndex;
    searchIndex = foundIndex + 1;
  }

  return Math.max(1, score);
};

export const useCreateTaskModal = () => {
  const tasks = useAllTasksStore();
  const firstField = ref<HTMLInputElement | null>(null);
  const noteField = ref<HTMLInputElement | null>(null);
  const ticketKey = ref("");
  const note = ref("");
  const localError = ref("");
  const searchOpen = ref(false);
  const highlightedResultIndex = ref(0);

  const clearSubmitError = () => {
    localError.value = "";
    tasks.error = "";
  };

  const range = useSessionRangeEditor({
    allowRunningStop: false,
    clearSubmitError,
    now: () => tasks.now,
  });

  const knownTask = computed(
    () =>
      tasks.tasks.find(
        (task) => task.key.toUpperCase() === normalizeTicketKey(ticketKey.value),
      ) ?? null,
  );

  const searchResults = computed<TicketSearchResult[]>(() => {
    const query = ticketKey.value.trim();

    return tasks.tasks
      .map((task) => {
        const keyScore = fuzzyScore(query, task.key) * 2;
        const titleScore = fuzzyScore(query, task.title);
        return {
          task,
          score: Math.max(keyScore, titleScore),
        };
      })
      .filter((result) => result.score > 0)
      .sort((left, right) => {
        if (right.score !== left.score) return right.score - left.score;
        return left.task.key.localeCompare(right.task.key);
      })
      .slice(0, 6);
  });

  const shouldShowSearch = computed(
    () =>
      tasks.activeModal === "create" &&
      searchOpen.value &&
      searchResults.value.length > 0,
  );

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

      return `${knownTask.value?.key ?? key} · ${preview}`;
    }

    return key
      ? `Add completed work for ${knownTask.value?.key ?? key}.`
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
    searchOpen.value = true;
    highlightedResultIndex.value = 0;
    tasks.error = "";
  };

  watch(searchResults, (results) => {
    if (highlightedResultIndex.value >= results.length) {
      highlightedResultIndex.value = Math.max(0, results.length - 1);
    }
  });

  watch(ticketKey, () => {
    clearSubmitError();
    searchOpen.value = true;
    highlightedResultIndex.value = 0;
  });
  watch(
    [note, range.startDateInput, range.stopDateInput, range.start, range.stop, range.durationInput],
    clearSubmitError,
  );

  const selectSearchResult = async (task: Task) => {
    ticketKey.value = task.key;
    highlightedResultIndex.value = 0;
    await nextTick();
    searchOpen.value = false;
    noteField.value?.focus();
  };

  const handleTicketKeydown = async (event: KeyboardEvent) => {
    if (event.key === "Escape" && searchOpen.value) {
      event.preventDefault();
      searchOpen.value = false;
      return;
    }

    if (event.key === "ArrowDown" && searchResults.value.length > 0) {
      event.preventDefault();
      searchOpen.value = true;
      highlightedResultIndex.value =
        (highlightedResultIndex.value + 1) % searchResults.value.length;
      return;
    }

    if (event.key === "ArrowUp" && searchResults.value.length > 0) {
      event.preventDefault();
      searchOpen.value = true;
      highlightedResultIndex.value =
        (highlightedResultIndex.value - 1 + searchResults.value.length) %
        searchResults.value.length;
      return;
    }

    if (
      event.key === "Enter" &&
      shouldShowSearch.value &&
      searchResults.value[highlightedResultIndex.value]
    ) {
      event.preventDefault();
      await selectSearchResult(searchResults.value[highlightedResultIndex.value].task);
    }
  };

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

  const slotCountLabel = (task: Task) =>
    `${task.sessions.length} ${task.sessions.length === 1 ? "slot" : "slots"}`;

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
    handleTicketKeydown,
    highlightedResultIndex,
    knownTask,
    localError,
    note,
    noteField,
    normalizeStartTime: range.normalizeStartTime,
    normalizeStopTime: range.normalizeStopTime,
    normalizeDuration: range.normalizeDuration,
    reset,
    rangePreview,
    searchOpen,
    searchResults,
    selectSearchResult,
    shouldShowSearch,
    slotCountLabel,
    startDateInput: range.startDateInput,
    start: range.start,
    stopDayLabel: range.stopDayLabel,
    stopDayOffset: range.stopDayOffset,
    stopDateInput: range.stopDateInput,
    stop: range.stop,
    submit,
    ticketKey,
    toggleAdvancedStopDate: range.toggleAdvancedStopDate,
  };
};
