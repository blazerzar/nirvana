<script setup lang="ts">
import { nextTick, onMounted } from "vue";
import { useStartTaskModal } from "../../composables/useStartTaskModal";
import { useAllTasksStore } from "../../stores/allTasks";
import TicketKeyCombobox from "../TicketKeyCombobox.vue";
import ModalShell from "./ModalShell.vue";

const tasks = useAllTasksStore();
const startModal = useStartTaskModal();
const {
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
  slotCountLabel,
  statusText,
  submit,
  ticketKey,
  validationError,
} = startModal;

onMounted(async () => {
  reset();
  await nextTick();
  firstField.value?.focus();
  firstField.value?.select();
});
</script>

<template>
  <ModalShell
    labelled-by="start-modal-title"
    width-class="w-[min(520px,100%)]"
    @close="tasks.closeModal()"
    @submit="submit"
  >
    <template #title>
      <h2 id="start-modal-title" class="m-0 text-[13px] font-bold text-(--text)">Start tracking</h2>
    </template>

    <div class="flex flex-col gap-3 p-4">
      <label class="flex min-w-0 flex-col gap-1.5">
        <span class="text-[10px] font-bold uppercase tracking-[0.04em] text-(--faint)">Ticket</span>
        <TicketKeyCombobox
          ref="firstField"
          v-model="ticketKey"
          v-model:search-open="searchOpen"
          v-model:highlighted-result-index="highlightedResultIndex"
          results-id="start-ticket-results"
          :search-results="searchResults"
          :handle-ticket-keydown="handleTicketKeydown"
          :select-search-result="selectSearchResult"
          :slot-count-label="slotCountLabel"
        />
      </label>

      <div
        v-if="knownTask"
        class="flex min-w-0 items-center gap-2 px-0 pt-0.5 text-[11px] leading-none text-(--faint)"
      >
        <span class="shrink-0 text-[10px] font-bold uppercase tracking-[0.04em] text-(--very-faint)">Title</span>
        <strong class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap font-medium text-(--muted)">{{ knownTask.title }}</strong>
      </div>

      <label class="flex min-w-0 flex-col gap-1.5">
        <span class="text-[10px] font-bold uppercase tracking-[0.04em] text-(--faint)">Note</span>
        <input
          ref="noteField"
          v-model="note"
          class="min-h-[34px] w-full rounded-md border border-(--border) bg-(--input-bg) px-2.5 py-[7px] text-xs text-(--text) tabular-nums outline-none transition-[border-color,box-shadow] duration-150 ease-[var(--ease)] placeholder:text-(--very-faint) focus:border-(--input-focus) focus:shadow-[0_0_0_2px_var(--input-focus-ring)]"
          placeholder="What are you working on?"
          autocomplete="off"
          @keydown="handleNoteKeydown"
        />
      </label>

      <p
        class="mt-0.5 mb-0 min-h-[18px] rounded-md border px-2.5 py-2 text-[11px]"
        :class="activeTaskWarning ? 'border-[rgba(210,192,113,0.2)] bg-[rgba(210,192,113,0.08)] text-(--warning)' : 'border-(--border) bg-(--surface) text-(--muted)'"
      >
        {{ statusText }}
      </p>
      <p class="m-0 min-h-4 text-[11px] text-(--danger)">{{ error }}</p>
    </div>

    <template #footer>
      <footer class="flex min-h-[46px] items-center justify-end gap-2 border-t border-(--border) bg-(--surface-inset) px-3.5 py-2.5 max-[760px]:flex-wrap">
        <span class="mr-auto text-[11px] text-(--faint) max-[760px]:mr-0">
          <kbd class="mr-1 rounded border border-(--accent) bg-(--surface-selected) px-[5px] py-px font-[family-name:var(--font-mono)] text-[10px] text-(--accent)">esc</kbd>
          cancel
        </span>
        <button
          class="min-h-[30px] rounded-md border border-(--border) bg-(--surface) px-3 py-1.5 text-[11px] text-(--muted) transition-[color,background,border-color] duration-150 ease-[var(--ease)] hover:bg-(--surface-strong) hover:text-(--text)"
          type="button"
          @click="tasks.closeModal()"
        >Cancel</button>
        <button
          class="min-h-[30px] rounded-md border border-(--accent) bg-(--accent) px-3 py-1.5 text-[11px] font-bold text-(--bg) transition-[color,background,border-color] duration-150 ease-[var(--ease)] disabled:cursor-default disabled:opacity-[0.42]"
          type="submit"
          :disabled="Boolean(validationError) || tasks.loading"
        >{{ tasks.loading ? "Starting..." : "Start" }}</button>
      </footer>
    </template>
  </ModalShell>
</template>
