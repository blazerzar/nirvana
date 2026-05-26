<script setup lang="ts">
import { nextTick, onMounted } from "vue";
import { useCreateTaskModal } from "../../composables/useCreateTaskModal";
import { useAllTasksStore } from "../../stores/allTasks";
import TicketKeyCombobox from "../TicketKeyCombobox.vue";
import ModalShell from "./ModalShell.vue";

const tasks = useAllTasksStore();
const createModal = useCreateTaskModal();
const {
  advancedStopDateVisible,
  computedError,
  durationInput,
  firstField,
  handleDurationInput,
  handleDurationKeydown,
  handleNoteKeydown,
  handleStartDateInput,
  handleStartKeydown,
  handleStartTimeInput,
  handleStopDateInput,
  handleStopKeydown,
  handleStopTimeInput,
  handleTicketKeydown,
  highlightedResultIndex,
  knownTask,
  localError,
  note,
  noteField,
  normalizeStartDate,
  normalizeStopDate,
  normalizeStartTime,
  normalizeStopTime,
  normalizeDuration,
  reset,
  rangePreview,
  searchOpen,
  searchResults,
  selectSearchResult,
  slotCountLabel,
  startDateInput,
  start,
  stopDayLabel,
  stopDayOffset,
  stopDateInput,
  stop,
  submit,
  ticketKey,
  toggleAdvancedStopDate,
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
          <TicketKeyCombobox
            ref="firstField"
            v-model="ticketKey"
            v-model:search-open="searchOpen"
            v-model:highlighted-result-index="highlightedResultIndex"
            results-id="create-ticket-results"
            :search-results="searchResults"
            :handle-ticket-keydown="handleTicketKeydown"
            :select-search-result="selectSearchResult"
            :slot-count-label="slotCountLabel"
          />
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
            @input="handleDurationInput"
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
            v-model="startDateInput"
            class="min-h-[34px] w-full rounded-md border border-(--border) bg-(--input-bg) px-2.5 py-[7px] text-xs text-(--text) tabular-nums outline-none transition-[border-color,box-shadow] duration-150 ease-[var(--ease)] placeholder:text-(--very-faint) focus:border-(--input-focus) focus:shadow-[0_0_0_2px_var(--input-focus-ring)]"
            type="date"
            autocomplete="off"
            :disabled="tasks.loading"
            @input="handleStartDateInput"
            @change="normalizeStartDate"
            @blur="normalizeStartDate"
          />
        </label>

        <label class="flex min-w-0 flex-col gap-1.5">
          <span class="text-[10px] font-bold uppercase tracking-[0.04em] text-(--faint)">Start</span>
          <input
            v-model="start"
            class="min-h-[34px] w-full rounded-md border border-(--border) bg-(--input-bg) px-2.5 py-[7px] text-xs text-(--text) tabular-nums outline-none transition-[border-color,box-shadow] duration-150 ease-[var(--ease)] placeholder:text-(--very-faint) focus:border-(--input-focus) focus:shadow-[0_0_0_2px_var(--input-focus-ring)]"
            type="text"
            inputmode="numeric"
            pattern="[0-9]{1,2}:[0-9]{2}"
            placeholder="14:30"
            autocomplete="off"
            @input="handleStartTimeInput"
            @blur="normalizeStartTime"
            @keydown="handleStartKeydown"
          />
        </label>

        <label class="flex min-w-0 flex-col gap-1.5">
          <span class="flex items-center justify-between gap-2 text-[10px] font-bold uppercase tracking-[0.04em] text-(--faint)">
            <span>Stop</span>
            <button
              type="button"
              class="rounded border px-1.5 py-px text-[9px] leading-none transition-[color,background,border-color] duration-150 ease-[var(--ease)]"
              :class="stopDayOffset === 0 && !advancedStopDateVisible ? 'border-(--border) bg-(--surface) text-(--very-faint) hover:text-(--muted)' : 'border-(--accent) bg-(--surface-selected) text-(--accent)'"
              title="Edit stop date"
              @click="toggleAdvancedStopDate"
            >
              {{ stopDayLabel }}
            </button>
          </span>
          <div class="grid grid-cols-[minmax(0,1fr)] gap-2" :class="advancedStopDateVisible ? 'grid-cols-[minmax(0,1fr)_minmax(0,0.9fr)]' : ''">
            <input
              v-model="stop"
              class="min-h-[34px] w-full rounded-md border border-(--border) bg-(--input-bg) px-2.5 py-[7px] text-xs text-(--text) tabular-nums outline-none transition-[border-color,box-shadow] duration-150 ease-[var(--ease)] placeholder:text-(--very-faint) focus:border-(--input-focus) focus:shadow-[0_0_0_2px_var(--input-focus-ring)]"
              type="text"
              inputmode="numeric"
              pattern="[0-9]{1,2}:[0-9]{2}"
              placeholder="15:00"
              autocomplete="off"
              @input="handleStopTimeInput"
              @blur="normalizeStopTime"
              @keydown="handleStopKeydown"
            />
            <input
              v-if="advancedStopDateVisible"
              v-model="stopDateInput"
              class="min-h-[34px] w-full rounded-md border border-(--border) bg-(--input-bg) px-2.5 py-[7px] text-xs text-(--text) tabular-nums outline-none transition-[border-color,box-shadow] duration-150 ease-[var(--ease)] placeholder:text-(--very-faint) focus:border-(--input-focus) focus:shadow-[0_0_0_2px_var(--input-focus-ring)]"
              type="date"
              aria-label="Stop date"
              autocomplete="off"
              :disabled="tasks.loading"
              @input="handleStopDateInput"
              @change="normalizeStopDate"
              @blur="normalizeStopDate"
            />
          </div>
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
        {{ rangePreview }}
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
