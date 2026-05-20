import { computed, nextTick, ref, watch } from "vue";
import { formatClock, useAllTasksStore } from "../stores/allTasks";
import { Task } from "../types/types";

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

export const useStartTaskModal = () => {
  const tasks = useAllTasksStore();
  const firstField = ref<HTMLInputElement | null>(null);
  const noteField = ref<HTMLInputElement | null>(null);
  const ticketKey = ref("");
  const note = ref("");
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

  const statusText = computed(() => {
    if (activeTaskWarning.value) return activeTaskWarning.value;
    if (knownTask.value) return `Starting ${knownTask.value.key}`;

    const key = normalizeTicketKey(ticketKey.value);
    return key
      ? `Select a ticket or press Enter to create ${key}.`
      : "Select a ticket or type a new key.";
  });

  const error = computed(() => {
    if (tasks.activeModal !== "start") return "";
    return normalizeTicketKey(ticketKey.value) ? "" : "Ticket key is required.";
  });

  const reset = () => {
    ticketKey.value = tasks.selectedTask?.key ?? "";
    note.value = "";
    searchOpen.value = true;
    highlightedResultIndex.value = 0;
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

  const submit = () => {
    if (error.value) return;

    tasks.startTaskFromInput({
      ticketKey: ticketKey.value,
      note: note.value,
    });
    tasks.activeModal = null;
  };

  const handleNoteKeydown = (event: KeyboardEvent) => {
    if (event.key !== "Enter" || event.isComposing) return;

    event.preventDefault();
    submit();
  };

  const slotCountLabel = (task: Task) =>
    `${task.sessions.length} ${task.sessions.length === 1 ? "slot" : "slots"}`;

  return {
    activeTaskWarning,
    error,
    firstField,
    handleNoteKeydown,
    handleTicketKeydown,
    highlightedResultIndex,
    knownTask,
    note,
    noteField,
    reset,
    searchOpen,
    searchResults,
    selectSearchResult,
    shouldShowSearch,
    slotCountLabel,
    statusText,
    submit,
    ticketKey,
  };
};
