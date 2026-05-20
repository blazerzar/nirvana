<script setup lang="ts">
import { computed } from "vue";
import {
  formatClock,
  formatDuration,
  useAllTasksStore,
} from "../../stores/allTasks";
import ModalShell from "./ModalShell.vue";

const tasks = useAllTasksStore();

const publishableSessions = computed(() => tasks.publishableSessions);

const totalDurationMs = computed(() =>
  publishableSessions.value.reduce(
    (sum, entry) => sum + entry.durationMs,
    0,
  ),
);

const slotLabel = computed(
  () =>
    `${publishableSessions.value.length} ${
      publishableSessions.value.length === 1 ? "slot" : "slots"
    }`,
);

const sessionRange = (start: Date, end: Date | null) =>
  `${formatClock(start)} - ${end ? formatClock(end) : "now"}`;
</script>

<template>
  <ModalShell
    labelled-by="publish-modal-title"
    width-class="w-[min(620px,100%)]"
    @close="tasks.closeModal()"
    @submit="tasks.confirmPublishUnpublished()"
  >
    <template #title>
      <h2 id="publish-modal-title" class="m-0 text-[13px] font-bold text-(--text)">
        Publish day
      </h2>
    </template>

    <div class="flex flex-col gap-3 p-4">
      <div
        class="rounded-md border border-(--border) bg-(--surface) px-3 py-2"
      >
        <div class="flex flex-wrap items-center gap-x-2 gap-y-1 text-[11px]">
          <span class="font-semibold text-(--text)">{{ tasks.selectedDateLabel }}</span>
          <span class="text-(--very-faint)">·</span>
          <span class="text-(--muted)">{{ slotLabel }}</span>
          <span class="text-(--very-faint)">·</span>
          <span class="font-semibold text-(--accent)">{{ formatDuration(totalDurationMs) }}</span>
        </div>
      </div>

      <div
        v-if="publishableSessions.length > 0"
        class="max-h-[min(320px,46vh)] overflow-auto rounded-md border border-(--border) bg-[rgba(0,0,0,0.16)]"
      >
        <div
          v-for="entry in publishableSessions"
          :key="entry.session.id"
          class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-3 border-t border-[rgba(255,255,255,0.04)] px-3 py-2 text-[11px] first:border-t-0 max-[760px]:grid-cols-1 max-[760px]:gap-1"
        >
          <div class="min-w-0">
            <div class="flex min-w-0 items-center gap-2">
              <span class="shrink-0 font-mono font-semibold text-(--accent)">{{ entry.task.key }}</span>
              <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap font-medium text-(--muted)">{{ entry.task.title }}</span>
            </div>
            <div
              v-if="entry.session.note"
              class="mt-1 overflow-hidden text-ellipsis whitespace-nowrap text-[10px] italic text-(--faint)"
            >
              {{ entry.session.note }}
            </div>
          </div>

          <div class="flex shrink-0 items-center justify-end gap-2 tabular-nums max-[760px]:justify-start">
            <span class="text-[10px] text-(--faint)">{{ sessionRange(entry.session.start, entry.session.end) }}</span>
            <span class="font-semibold text-(--muted)">{{ formatDuration(entry.durationMs) }}</span>
          </div>
        </div>
      </div>

      <p
        v-else
        class="m-0 rounded-md border border-(--border) bg-(--surface) px-3 py-3 text-center text-[11px] text-(--faint)"
      >
        There are no stopped unpublished slots to publish for this day.
      </p>

      <p class="m-0 min-h-4 text-[11px] text-[#ff9a86]">{{ tasks.error }}</p>
    </div>

    <template #footer>
      <footer
        class="flex min-h-[46px] items-center justify-end gap-2 border-t border-(--border) bg-[rgba(255,255,255,0.012)] px-3.5 py-2.5 max-[760px]:flex-wrap"
      >
        <span class="mr-auto text-[11px] text-(--faint) max-[760px]:mr-0">
          <kbd class="mr-1 rounded border border-[rgba(149,222,200,0.22)] bg-[rgba(149,222,200,0.1)] px-[5px] py-px font-[family-name:var(--font-mono)] text-[10px] text-(--accent)">esc</kbd>
          cancel
        </span>
        <button
          class="min-h-[30px] rounded-md border border-(--border) bg-(--surface) px-3 py-1.5 text-[11px] text-(--muted) transition-[color,background,border-color] duration-150 ease-[var(--ease)] hover:bg-(--surface-strong) hover:text-(--text)"
          type="button"
          :disabled="tasks.loading"
          @click="tasks.closeModal()"
        >
          Cancel
        </button>
        <button
          class="min-h-[30px] rounded-md border border-(--accent) bg-(--accent) px-3 py-1.5 text-[11px] font-bold text-(--bg) transition-[color,background,border-color] duration-150 ease-[var(--ease)] disabled:cursor-default disabled:opacity-[0.42]"
          type="submit"
          :disabled="tasks.loading || publishableSessions.length === 0"
        >
          {{ tasks.loading ? "Publishing..." : "Publish" }}
        </button>
      </footer>
    </template>
  </ModalShell>
</template>
