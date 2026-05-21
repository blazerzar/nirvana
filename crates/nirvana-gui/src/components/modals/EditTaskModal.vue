<script setup lang="ts">
import { nextTick, onMounted } from "vue";
import EditIcon from "../../assets/icons/edit.svg?raw";
import { useEditTaskModal } from "../../composables/useEditTaskModal";
import { useAllTasksStore } from "../../stores/allTasks";
import ModalShell from "./ModalShell.vue";

const tasks = useAllTasksStore();
const editModal = useEditTaskModal();
const {
  computedError,
  deleteSession,
  durationInput,
  error,
  firstField,
  handleDurationKeydown,
  handleStartKeydown,
  handleStopKeydown,
  knownTask,
  note,
  applyDurationToStop,
  normalizeDuration,
  normalizeStartTime,
  normalizeStopTime,
  readOnly,
  reset,
  start,
  stop,
  submit,
  ticketKey,
} = editModal;

onMounted(async () => {
  reset();
  await nextTick();
  firstField.value?.focus();
  firstField.value?.select();
});
</script>

<template>
  <ModalShell
    labelled-by="edit-modal-title"
    width-class="w-[min(660px,100%)]"
    @close="tasks.closeModal()"
    @submit="submit"
  >
    <template #title>
      <h2 id="edit-modal-title" class="m-0 text-[13px] font-bold text-(--text)">
        {{ readOnly ? "Cannot edit session" : "Edit work session" }}
      </h2>
    </template>

    <div
      v-if="readOnly"
      class="flex min-h-[260px] flex-col items-center justify-center px-8 py-10 text-center"
    >
      <div
        class="mb-4 flex h-16 w-16 items-center justify-center rounded-full border border-[rgba(255,154,134,0.22)] bg-[rgba(255,154,134,0.08)] text-[#ff9a86] [&>svg]:h-8 [&>svg]:w-8"
        aria-hidden="true"
        v-html="EditIcon"
      ></div>
      <h3 class="m-0 text-[13px] font-bold text-(--text)">
        Published slots cannot be edited
      </h3>
      <p
        class="mt-2 mb-0 max-w-[330px] text-[11px] leading-relaxed text-(--faint)"
      >
        This work session has already been published. Make changes in the
        destination system instead.
      </p>
    </div>

    <div v-else class="flex flex-col gap-3 p-4">
      <div class="grid grid-cols-[minmax(0,1fr)_minmax(0,1fr)] gap-3 max-[760px]:grid-cols-1">
        <label class="flex min-w-0 flex-col gap-1.5">
          <span class="text-[10px] font-bold uppercase tracking-[0.04em] text-(--faint)">Ticket</span>
          <input
            v-model="ticketKey"
            readonly
            title="Ticket changes are not supported yet."
            class="min-h-[34px] w-full rounded-md border border-(--border) bg-[rgba(0,0,0,0.24)] px-2.5 py-[7px] text-xs text-(--text) tabular-nums outline-none transition-[border-color,box-shadow] duration-150 ease-[var(--ease)] placeholder:text-(--very-faint) read-only:cursor-default read-only:text-(--muted) focus:border-[rgba(149,222,200,0.7)] focus:shadow-[0_0_0_2px_rgba(149,222,200,0.1)]"
            autocomplete="off"
          />
        </label>

        <label class="flex min-w-0 flex-col gap-1.5">
          <span class="text-[10px] font-bold uppercase tracking-[0.04em] text-(--faint)">Duration</span>
          <input
            v-model="durationInput"
            class="min-h-[34px] w-full rounded-md border border-(--border) bg-[rgba(0,0,0,0.24)] px-2.5 py-[7px] text-xs text-(--text) tabular-nums outline-none transition-[border-color,box-shadow] duration-150 ease-[var(--ease)] placeholder:text-(--very-faint) focus:border-[rgba(149,222,200,0.7)] focus:shadow-[0_0_0_2px_rgba(149,222,200,0.1)]"
            type="text"
            inputmode="text"
            placeholder="30m"
            autocomplete="off"
            @input="applyDurationToStop"
            @blur="normalizeDuration"
            @keydown="handleDurationKeydown"
          />
        </label>

        <label class="flex min-w-0 flex-col gap-1.5">
          <span class="text-[10px] font-bold uppercase tracking-[0.04em] text-(--faint)">Start time</span>
          <input
            ref="firstField"
            v-model="start"
            class="min-h-[34px] w-full rounded-md border border-(--border) bg-[rgba(0,0,0,0.24)] px-2.5 py-[7px] text-xs text-(--text) tabular-nums outline-none transition-[border-color,box-shadow] duration-150 ease-[var(--ease)] placeholder:text-(--very-faint) read-only:cursor-default read-only:text-(--muted) focus:border-[rgba(149,222,200,0.7)] focus:shadow-[0_0_0_2px_rgba(149,222,200,0.1)]"
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
            class="min-h-[34px] w-full rounded-md border border-(--border) bg-[rgba(0,0,0,0.24)] px-2.5 py-[7px] text-xs text-(--text) tabular-nums outline-none transition-[border-color,box-shadow] duration-150 ease-[var(--ease)] placeholder:text-(--very-faint) read-only:cursor-default read-only:text-(--muted) focus:border-[rgba(149,222,200,0.7)] focus:shadow-[0_0_0_2px_rgba(149,222,200,0.1)]"
            type="text"
            inputmode="numeric"
            pattern="[0-9]{1,2}:[0-9]{2}"
            placeholder="Leave empty for running"
            autocomplete="off"
            @blur="normalizeStopTime"
            @keydown="handleStopKeydown"
          />
        </label>
      </div>

      <label class="flex min-w-0 flex-col gap-1.5">
        <span class="text-[10px] font-bold uppercase tracking-[0.04em] text-(--faint)">Note</span>
        <input
          v-model="note"
          class="min-h-[34px] w-full rounded-md border border-(--border) bg-[rgba(0,0,0,0.24)] px-2.5 py-[7px] text-xs text-(--text) tabular-nums outline-none transition-[border-color,box-shadow] duration-150 ease-[var(--ease)] placeholder:text-(--very-faint) read-only:cursor-default read-only:text-(--muted) focus:border-[rgba(149,222,200,0.7)] focus:shadow-[0_0_0_2px_rgba(149,222,200,0.1)]"
          placeholder="Focused work"
          autocomplete="off"
        />
      </label>

      <p
        class="mt-0.5 mb-0 min-h-[18px] rounded-md border border-(--border) bg-(--surface) px-2.5 py-2 text-[11px] text-(--muted)"
      >
        {{ knownTask ? `Saving ${knownTask.key}` : "Ticket changes are not supported yet." }}
      </p>
      <p class="m-0 min-h-4 text-[11px] text-[#ff9a86]">{{ error || computedError }}</p>
    </div>

    <template v-if="!readOnly" #footer>
      <footer
        class="flex min-h-[46px] items-center justify-end gap-2 border-t border-(--border) bg-[rgba(255,255,255,0.012)] px-3.5 py-2.5 max-[760px]:flex-wrap"
      >
        <button
          class="mr-auto min-h-[30px] rounded-md border border-[rgba(255,154,134,0.22)] bg-[rgba(255,154,134,0.06)] px-3 py-1.5 text-[11px] text-[#ff9a86] transition-[color,background,border-color] duration-150 ease-[var(--ease)] disabled:cursor-default disabled:opacity-[0.42] max-[760px]:mr-0"
          type="button"
          :disabled="tasks.loading"
          @click="deleteSession"
        >
          Delete
        </button>
        <span class="mr-auto text-[11px] text-(--faint) max-[760px]:mr-0">
          <kbd class="mr-1 rounded border border-[rgba(149,222,200,0.22)] bg-[rgba(149,222,200,0.1)] px-[5px] py-px font-[family-name:var(--font-mono)] text-[10px] text-(--accent)">esc</kbd>
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
        >{{ tasks.loading ? "Saving..." : "Save" }}</button>
      </footer>
    </template>
  </ModalShell>
</template>
