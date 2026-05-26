import { computed, ref, watch } from "vue";
import { formatClock, useAllTasksStore } from "../stores/allTasks";
import { normalizeTicketKey, useTicketKeySearch } from "./useTicketKeySearch";

export const useStartTaskModal = () => {
  const tasks = useAllTasksStore();
  const firstField = ref<HTMLInputElement | null>(null);
  const noteField = ref<HTMLInputElement | null>(null);
  const ticketKey = ref("");
  const note = ref("");

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

  const statusText = computed(() => {
    if (activeTaskWarning.value) return activeTaskWarning.value;

    const key = normalizeTicketKey(ticketKey.value);
    if (key) {
      return `Starting ${ticketSearch.knownTask.value?.key ?? key} now.`;
    }

    return "Select a ticket or type a new key.";
  });

  const validationError = computed(() => {
    if (tasks.activeModal !== "start") return "";
    if (!normalizeTicketKey(ticketKey.value)) return "Ticket key is required.";
    return "";
  });

  const error = computed(() => tasks.error || validationError.value);

  const reset = () => {
    ticketKey.value = "";
    note.value = "";
    ticketSearch.searchOpen.value = true;
    ticketSearch.highlightedResultIndex.value = 0;
    tasks.error = "";
  };

  watch(note, clearSubmitError);

  const submit = async () => {
    if (validationError.value) return;

    await tasks.startTaskFromInput({
      ticketKey: ticketKey.value,
      note: note.value,
    });
  };

  const handleNoteKeydown = (event: KeyboardEvent) => {
    if (event.key !== "Enter" || event.isComposing) return;

    event.preventDefault();
    submit();
  };

  return {
    activeTaskWarning,
    error,
    firstField,
    handleNoteKeydown,
    handleTicketKeydown: ticketSearch.handleTicketKeydown,
    highlightedResultIndex: ticketSearch.highlightedResultIndex,
    knownTask: ticketSearch.knownTask,
    note,
    noteField,
    reset,
    searchOpen: ticketSearch.searchOpen,
    searchResults: ticketSearch.searchResults,
    selectSearchResult: ticketSearch.selectSearchResult,
    shouldShowSearch: ticketSearch.shouldShowSearch,
    slotCountLabel: ticketSearch.slotCountLabel,
    statusText,
    submit,
    ticketKey,
    validationError,
  };
};
