import { computed, nextTick, ref, watch } from "vue";
import { formatClock, useAllTasksStore } from "../stores/allTasks";
import { isSameDay, startOfDay } from "../tasks/time";
import { Task } from "../types/types";

type TicketSearchResult = {
  task: Task;
  score: number;
};

const normalizeTicketKey = (value: string) => value.trim().toUpperCase();

const formatTimeInput = (date: Date) =>
  `${date.getHours().toString().padStart(2, "0")}:${date
    .getMinutes()
    .toString()
    .padStart(2, "0")}`;

const parseTimeParts = (value: string) => {
  const match = value.trim().match(/^(\d{1,2}):(\d{2})(?::(\d{2}))?$/);
  if (!match) return null;

  const hours = Number(match[1]);
  const minutes = Number(match[2]);
  const seconds = match[3] ? Number(match[3]) : 0;

  if (
    !Number.isInteger(hours) ||
    !Number.isInteger(minutes) ||
    !Number.isInteger(seconds) ||
    hours < 0 ||
    hours > 23 ||
    minutes < 0 ||
    minutes > 59 ||
    seconds < 0 ||
    seconds > 59
  ) {
    return null;
  }

  return { hours, minutes, seconds };
};

const formatTimeParts = (hours: number, minutes: number) =>
  `${hours.toString().padStart(2, "0")}:${minutes
    .toString()
    .padStart(2, "0")}`;

const wrapTimePart = (value: number, maxExclusive: number) =>
  ((value % maxExclusive) + maxExclusive) % maxExclusive;

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

export const useStartTaskModal = () => {
  const tasks = useAllTasksStore();
  const firstField = ref<HTMLInputElement | null>(null);
  const noteField = ref<HTMLInputElement | null>(null);
  const ticketKey = ref("");
  const note = ref("");
  const start = ref("");
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
      tasks.activeModal === "start" &&
      searchOpen.value &&
      searchResults.value.length > 0,
  );

  const activeTaskWarning = computed(() => {
    if (
      !tasks.activeTask ||
      tasks.activeTask.key === normalizeTicketKey(ticketKey.value)
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

    const date = startOfDay(tasks.selectedDate);
    date.setHours(time.hours, time.minutes, time.seconds, 0);
    return date;
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
      return `Starting ${knownTask.value?.key ?? key} at ${formatClock(parsedStart.value)}`;
    }

    return key
      ? `Select a ticket or press Enter to create ${key}.`
      : "Select a ticket or type a new key.";
  });

  const error = computed(() => {
    if (tasks.activeModal !== "start") return "";
    if (tasks.error) return tasks.error;
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

  const reset = () => {
    ticketKey.value = tasks.selectedTask?.key ?? "";
    note.value = "";
    start.value = formatTimeInput(new Date(tasks.now));
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

  const submit = async () => {
    if (error.value) return;

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

  const slotCountLabel = (task: Task) =>
    `${task.sessions.length} ${task.sessions.length === 1 ? "slot" : "slots"}`;

  return {
    activeTaskWarning,
    error,
    firstField,
    handleNoteKeydown,
    handleStartKeydown,
    handleTicketKeydown,
    highlightedResultIndex,
    knownTask,
    note,
    noteField,
    normalizeStartTime,
    reset,
    searchOpen,
    searchResults,
    selectSearchResult,
    shouldShowSearch,
    slotCountLabel,
    start,
    statusText,
    submit,
    ticketKey,
  };
};
