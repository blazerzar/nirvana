import { computed, nextTick, ref, watch } from "vue";
import { formatClock, useAllTasksStore } from "../stores/allTasks";
import { startOfDay } from "../tasks/time";
import { Task } from "../types/types";
import {
  applyTimeParts,
  formatDurationInput,
  formatTimeInput,
  formatTimeParts,
  parseDurationInput,
  parseTimeParts,
  wrapTimePart,
} from "./dateTimeInputs";

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
  const start = ref("");
  const stop = ref("");
  const durationInput = ref("");
  const localError = ref("");
  const searchOpen = ref(false);
  const highlightedResultIndex = ref(0);

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

  const parsedStart = computed(() => {
    const time = parseTimeParts(start.value);
    if (!time) return null;

    return applyTimeParts(startOfDay(tasks.selectedDate), time);
  });

  const parsedStop = computed(() => {
    const time = parseTimeParts(stop.value);
    if (!time) return null;

    return applyTimeParts(startOfDay(tasks.selectedDate), time);
  });

  const parsedDurationMs = computed(() => parseDurationInput(durationInput.value));

  const overlapsExistingSlot = computed(() => {
    const proposedStart = parsedStart.value;
    const proposedStop = parsedStop.value;
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
    if (!start.value.trim()) return "Start time is required.";
    if (!stop.value.trim()) return "Stop time is required.";
    if (!parsedStart.value) return "Start time is invalid.";
    if (!parsedStop.value) return "Stop time is invalid.";
    if (!durationInput.value.trim()) return "Duration is required.";
    if (!parsedDurationMs.value || parsedDurationMs.value <= 0) {
      return "Duration is invalid.";
    }
    if (parsedStop.value.getTime() <= parsedStart.value.getTime()) {
      return "Stop must be after start.";
    }
    if (
      parsedStart.value.getTime() > tasks.now.getTime() ||
      parsedStop.value.getTime() > tasks.now.getTime()
    ) {
      return "Slot time cannot be in the future.";
    }
    if (overlapsExistingSlot.value) {
      return "Time range overlaps an existing slot.";
    }
    return "";
  });

  const statusText = computed(() => {
    const key = normalizeTicketKey(ticketKey.value);

    if (key && parsedStart.value && parsedStop.value && !computedError.value) {
      return `Adding ${knownTask.value?.key ?? key} from ${formatClock(parsedStart.value)} to ${formatClock(parsedStop.value)}.`;
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
    start.value = formatTimeInput(fallbackStart);
    stop.value = formatTimeInput(fallbackStop);
    durationInput.value = formatDurationInput(
      fallbackStop.getTime() - fallbackStart.getTime(),
    );
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
    searchOpen.value = true;
    highlightedResultIndex.value = 0;
  });

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

  const normalizeTime = (value: typeof start) => {
    const time = parseTimeParts(value.value);
    if (!time) return;

    value.value = formatTimeParts(time.hours, time.minutes);
  };

  const syncDurationFromTimes = () => {
    if (!parsedStart.value || !parsedStop.value) return;

    durationInput.value = formatDurationInput(
      parsedStop.value.getTime() - parsedStart.value.getTime(),
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
    const fallback = value.value || (value === stop ? start.value : stop.value);
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

  const normalizeStartTime = () => {
    normalizeTime(start);
    applyDurationToStop();
  };
  const normalizeStopTime = () => {
    normalizeTime(stop);
    syncDurationFromTimes();
  };
  const handleStartKeydown = (event: KeyboardEvent) =>
    handleTimeKeydown(event, start);
  const handleStopKeydown = (event: KeyboardEvent) =>
    handleTimeKeydown(event, stop);

  const submit = async () => {
    localError.value = computedError.value;
    if (localError.value || !parsedStart.value || !parsedStop.value) return;

    const saved = await tasks.createSession({
      ticketKey: ticketKey.value,
      note: note.value,
      start: parsedStart.value,
      end: parsedStop.value,
    });

    if (!saved) {
      localError.value = tasks.error || "This slot could not be added.";
    }
  };

  const slotCountLabel = (task: Task) =>
    `${task.sessions.length} ${task.sessions.length === 1 ? "slot" : "slots"}`;

  return {
    computedError,
    durationInput,
    firstField,
    handleNoteKeydown,
    handleDurationKeydown,
    handleStartKeydown,
    handleStopKeydown,
    handleTicketKeydown,
    highlightedResultIndex,
    knownTask,
    localError,
    note,
    noteField,
    applyDurationToStop,
    normalizeStartTime,
    normalizeStopTime,
    normalizeDuration,
    reset,
    searchOpen,
    searchResults,
    selectSearchResult,
    shouldShowSearch,
    slotCountLabel,
    start,
    statusText,
    stop,
    submit,
    ticketKey,
  };
};
