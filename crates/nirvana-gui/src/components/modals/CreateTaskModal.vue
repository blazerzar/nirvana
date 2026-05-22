<script setup lang="ts">
import { nextTick, onMounted } from "vue";
import { useCreateTaskModal } from "../../composables/useCreateTaskModal";
import { useAllTasksStore } from "../../stores/allTasks";
import ModalShell from "./ModalShell.vue";

const tasks = useAllTasksStore();
const createModal = useCreateTaskModal();
const {
  computedError,
  dateInput,
  durationInput,
  firstField,
  handleDurationKeydown,
  handleNoteKeydown,
  handleStartKeydown,
  handleStopKeydown,
  handleTicketKeydown,
  highlightedResultIndex,
  knownTask,
  localError,
  note,
  noteField,
  applyDurationToStop,
  normalizeDate,
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
} = createModal;

onMounted(async () => {
  reset();
  await nextTick();
  firstField.value?.focus();
  firstField.value?.select();
});
</script>

<template>
  <ModalShell
    labelled-by="create-modal-title"
    width-class="w-[min(660px,100%)]"
    @close="tasks.closeModal()"
    @submit="submit"
  >
    <template #title>
      <h2 id="create-modal-title" class="m-0 text-[13px] font-bold text-(--text)">
        Add completed work
      </h2>
    </template>

    <div class="flex flex-col gap-3 p-4">
      <div class="grid grid-cols-[minmax(0,1fr)_auto] gap-3 max-[760px]:grid-cols-1">
        <label class="flex min-w-0 flex-col gap-1.5">
          <span class="text-[10px] font-bold uppercase tracking-[0.04em] text-(--faint)">Ticket</span>
          <div class="relative">
            <input
              ref="firstField"
              v-model="ticketKey"
              class="min-h-[34px] w-full rounded-md border border-(--border) bg-(--input-bg) px-2.5 py-[7px] text-xs text-(--text) tabular-nums outline-none transition-[border-color,box-shadow] duration-150 ease-[var(--ease)] placeholder:text-(--very-faint) focus:border-(--input-focus) focus:shadow-[0_0_0_2px_var(--input-focus-ring)]"
              placeholder="NIR-12"
              autocomplete="off"
              aria-autocomplete="list"
              :aria-expanded="shouldShowSearch"
              aria-controls="create-ticket-results"
              @focus="searchOpen = true"
              @keydown="handleTicketKeydown"
            />

            <div
              v-if="shouldShowSearch"
              id="create-ticket-results"
              class="absolute top-[calc(100%+5px)] right-0 left-0 z-40 max-h-[min(238px,48vh)] overflow-auto rounded-[7px] border border-(--border) bg-(--panel) shadow-[0_16px_36px_rgba(0,0,0,0.36)]"
              role="listbox"
              aria-label="Matching tickets"
            >
              <button
                v-for="(result, index) in searchResults"
                :key="result.task.id"
                class="grid min-h-[34px] w-full grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-[9px] border-b border-(--border) px-[9px] py-[7px] text-left text-[11px] last:border-b-0 hover:bg-(--surface-selected) hover:text-(--text) max-[760px]:grid-cols-[auto_minmax(0,1fr)]"
                :class="index === highlightedResultIndex ? 'bg-(--surface-selected) text-(--text)' : 'text-(--muted)'"
                type="button"
                role="option"
                :aria-selected="index === highlightedResultIndex"
                @mousedown.prevent="selectSearchResult(result.task)"
                @mouseenter="highlightedResultIndex = index"
              >
                <span class="whitespace-nowrap font-[family-name:var(--font-mono)] font-bold text-(--accent)">{{ result.task.key }}</span>
                <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-inherit">{{ result.task.title }}</span>
                <span class="whitespace-nowrap text-[10px] text-(--faint) max-[760px]:hidden">
                  {{ slotCountLabel(result.task) }}
                  <span v-if="result.task.status === 'running'">· running</span>
                </span>
              </button>
            </div>
          </div>
        </label>

        <label class="flex min-w-[118px] flex-col gap-1.5">
          <span class="text-[10px] font-bold uppercase tracking-[0.04em] text-(--faint)">Duration</span>
          <input
            v-model="durationInput"
            class="min-h-[34px] w-full rounded-md border border-(--border) bg-(--input-bg) px-2.5 py-[7px] text-xs text-(--text) tabular-nums outline-none transition-[border-color,box-shadow] duration-150 ease-[var(--ease)] placeholder:text-(--very-faint) focus:border-(--input-focus) focus:shadow-[0_0_0_2px_var(--input-focus-ring)]"
            type="text"
            inputmode="text"
            placeholder="30m"
            autocomplete="off"
            @input="applyDurationToStop"
            @blur="normalizeDuration"
            @keydown="handleDurationKeydown"
          />
        </label>
      </div>

      <div
        v-if="knownTask"
        class="flex min-w-0 items-center gap-2 px-0 pt-0.5 text-[11px] leading-none text-(--faint)"
      >
        <span class="shrink-0 text-[10px] font-bold uppercase tracking-[0.04em] text-(--very-faint)">Title</span>
        <strong class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap font-medium text-(--muted)">{{ knownTask.title }}</strong>
      </div>

      <div class="grid grid-cols-[minmax(0,0.9fr)_minmax(0,1fr)_minmax(0,1fr)] gap-3 max-[760px]:grid-cols-1">
        <label class="flex min-w-0 flex-col gap-1.5">
          <span class="text-[10px] font-bold uppercase tracking-[0.04em] text-(--faint)">Date</span>
          <input
            v-model="dateInput"
            class="min-h-[34px] w-full rounded-md border border-(--border) bg-(--input-bg) px-2.5 py-[7px] text-xs text-(--text) tabular-nums outline-none transition-[border-color,box-shadow] duration-150 ease-[var(--ease)] placeholder:text-(--very-faint) focus:border-(--input-focus) focus:shadow-[0_0_0_2px_var(--input-focus-ring)]"
            type="date"
            autocomplete="off"
            :disabled="tasks.loading"
            @change="normalizeDate"
            @blur="normalizeDate"
          />
        </label>

        <label class="flex min-w-0 flex-col gap-1.5">
          <span class="text-[10px] font-bold uppercase tracking-[0.04em] text-(--faint)">Start time</span>
          <input
            v-model="start"
            class="min-h-[34px] w-full rounded-md border border-(--border) bg-(--input-bg) px-2.5 py-[7px] text-xs text-(--text) tabular-nums outline-none transition-[border-color,box-shadow] duration-150 ease-[var(--ease)] placeholder:text-(--very-faint) focus:border-(--input-focus) focus:shadow-[0_0_0_2px_var(--input-focus-ring)]"
            type="text"
            inputmode="numeric"
            pattern="[0-9]{1,2}:[0-9]{2}"
            placeholder="14:30"
            autocomplete="off"
            @blur="normalizeStartTime"
            @keydown="handleStartKeydown"
          />
        </label>

        <label class="flex min-w-0 flex-col gap-1.5">
          <span class="text-[10px] font-bold uppercase tracking-[0.04em] text-(--faint)">Stop time</span>
          <input
            v-model="stop"
            class="min-h-[34px] w-full rounded-md border border-(--border) bg-(--input-bg) px-2.5 py-[7px] text-xs text-(--text) tabular-nums outline-none transition-[border-color,box-shadow] duration-150 ease-[var(--ease)] placeholder:text-(--very-faint) focus:border-(--input-focus) focus:shadow-[0_0_0_2px_var(--input-focus-ring)]"
            type="text"
            inputmode="numeric"
            pattern="[0-9]{1,2}:[0-9]{2}"
            placeholder="15:00"
            autocomplete="off"
            @blur="normalizeStopTime"
            @keydown="handleStopKeydown"
          />
        </label>
      </div>

      <label class="flex min-w-0 flex-col gap-1.5">
        <span class="text-[10px] font-bold uppercase tracking-[0.04em] text-(--faint)">Note</span>
        <input
          ref="noteField"
          v-model="note"
          class="min-h-[34px] w-full rounded-md border border-(--border) bg-(--input-bg) px-2.5 py-[7px] text-xs text-(--text) tabular-nums outline-none transition-[border-color,box-shadow] duration-150 ease-[var(--ease)] placeholder:text-(--very-faint) focus:border-(--input-focus) focus:shadow-[0_0_0_2px_var(--input-focus-ring)]"
          placeholder="Focused work"
          autocomplete="off"
          @keydown="handleNoteKeydown"
        />
      </label>

      <p
        class="mt-0.5 mb-0 min-h-[18px] rounded-md border border-(--border) bg-(--surface) px-2.5 py-2 text-[11px] text-(--muted)"
      >
        {{ statusText }}
      </p>
      <p class="m-0 min-h-4 text-[11px] text-(--danger)">
        {{ localError || tasks.error || computedError }}
      </p>
    </div>

    <template #footer>
      <footer
        class="flex min-h-[46px] items-center justify-end gap-2 border-t border-(--border) bg-(--surface-inset) px-3.5 py-2.5 max-[760px]:flex-wrap"
      >
        <span class="mr-auto text-[11px] text-(--faint) max-[760px]:mr-0">
          <kbd class="mr-1 rounded border border-(--accent) bg-(--surface-selected) px-[5px] py-px font-[family-name:var(--font-mono)] text-[10px] text-(--accent)">esc</kbd>
          cancel
        </span>
        <button
          class="min-h-[30px] rounded-md border border-(--border) bg-(--surface) px-3 py-1.5 text-[11px] text-(--muted) transition-[color,background,border-color] duration-150 ease-[var(--ease)] hover:bg-(--surface-strong) hover:text-(--text)"
          type="button"
          :disabled="tasks.loading"
          @click="tasks.closeModal()"
        >Cancel</button>
        <button
          class="min-h-[30px] rounded-md border border-(--accent) bg-(--accent) px-3 py-1.5 text-[11px] font-bold text-(--bg) transition-[color,background,border-color] duration-150 ease-[var(--ease)] disabled:cursor-default disabled:opacity-[0.42]"
          type="submit"
          :disabled="Boolean(computedError) || tasks.loading"
        >{{ tasks.loading ? "Adding..." : "Add" }}</button>
      </footer>
    </template>
  </ModalShell>
</template>
