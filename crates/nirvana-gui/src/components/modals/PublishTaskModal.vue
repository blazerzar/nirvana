<script setup lang="ts">
import { computed } from "vue";
import {
  formatClock,
  formatDuration,
  useAllTasksStore,
} from "../../stores/allTasks";
import { useSettingsStore } from "../../stores/settings";
import { TaskTimelineSession } from "../../types/types";
import ModalShell from "./ModalShell.vue";

const tasks = useAllTasksStore();
const settings = useSettingsStore();

const publishableSessions = computed(() => tasks.publishableSessions);

type PublishPreviewWorklog = {
  id: string;
  ticketKey: string;
  title: string;
  note?: string;
  startedAt: Date;
  durationMs: number;
  sourceSlotCount: number;
};

const individualPreviewWorklogs = (sessions: TaskTimelineSession[]) =>
  sessions.map((entry) => ({
    id: `slot-${entry.session.id}`,
    ticketKey: entry.task.key,
    title: entry.task.title,
    note: entry.session.note,
    startedAt: entry.session.start,
    durationMs: entry.durationMs,
    sourceSlotCount: 1,
  }));

const squashedPreviewWorklogs = (sessions: TaskTimelineSession[]) => {
  const firstStartMs = Math.min(
    ...sessions.map((entry) => entry.session.start.getTime()),
  );
  const groups: PublishPreviewWorklog[] = [];

  sessions.forEach((entry) => {
    const existing = groups.find(
      (worklog) => worklog.ticketKey === entry.task.key,
    );

    if (existing) {
      existing.durationMs += entry.durationMs;
      existing.sourceSlotCount += 1;
      return;
    }

    groups.push({
      id: `ticket-${entry.task.key}`,
      ticketKey: entry.task.key,
      title: entry.task.title,
      startedAt: new Date(firstStartMs),
      durationMs: entry.durationMs,
      sourceSlotCount: 1,
    });
  });

  let cursorMs = firstStartMs;
  groups.forEach((worklog) => {
    worklog.startedAt = new Date(cursorMs);
    cursorMs += worklog.durationMs;
  });

  return groups;
};

const previewWorklogs = computed(() =>
  settings.publishSquashedWorklogs
    ? squashedPreviewWorklogs(publishableSessions.value)
    : individualPreviewWorklogs(publishableSessions.value),
);

const totalDurationMs = computed(() =>
  previewWorklogs.value.reduce((sum, worklog) => sum + worklog.durationMs, 0),
);

const slotLabel = computed(
  () =>
    `${publishableSessions.value.length} ${
      publishableSessions.value.length === 1 ? "slot" : "slots"
    }`,
);

const worklogLabel = computed(
  () =>
    `${previewWorklogs.value.length} Jira ${
      previewWorklogs.value.length === 1 ? "worklog" : "worklogs"
    }`,
);

const modeLabel = computed(() =>
  settings.publishSquashedWorklogs ? "Squashed preview" : "Slot preview",
);

const sourceSlotLabel = (count: number) =>
  `${count} local ${count === 1 ? "slot" : "slots"}`;

const worklogRange = (worklog: PublishPreviewWorklog) => {
  const end = new Date(worklog.startedAt.getTime() + worklog.durationMs);
  return `${formatClock(worklog.startedAt)} - ${formatClock(end)}`;
};
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
          <span class="text-(--muted)">{{ worklogLabel }}</span>
          <span class="text-(--very-faint)">·</span>
          <span class="text-(--muted)">{{ slotLabel }}</span>
          <span class="text-(--very-faint)">·</span>
          <span class="font-semibold text-(--accent)">{{ formatDuration(totalDurationMs) }}</span>
          <span class="ml-auto text-[10px] text-(--faint) max-[520px]:ml-0">{{ modeLabel }}</span>
        </div>
      </div>

      <div
        v-if="previewWorklogs.length > 0"
        class="max-h-[min(320px,46vh)] overflow-auto rounded-md border border-(--border) bg-[rgba(0,0,0,0.16)]"
      >
        <div
          v-for="worklog in previewWorklogs"
          :key="worklog.id"
          class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-3 border-t border-[rgba(255,255,255,0.04)] px-3 py-2 text-[11px] first:border-t-0 max-[760px]:grid-cols-1 max-[760px]:gap-1"
        >
          <div class="min-w-0">
            <div class="flex min-w-0 items-center gap-2">
              <span class="shrink-0 font-mono font-semibold text-(--accent)">{{ worklog.ticketKey }}</span>
              <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap font-medium text-(--muted)">{{ worklog.title }}</span>
            </div>
            <div
              v-if="worklog.note"
              class="mt-1 overflow-hidden text-ellipsis whitespace-nowrap text-[10px] italic text-(--faint)"
            >
              {{ worklog.note }}
            </div>
            <div
              v-else-if="settings.publishSquashedWorklogs"
              class="mt-1 text-[10px] text-(--faint)"
            >
              {{ sourceSlotLabel(worklog.sourceSlotCount) }}
            </div>
          </div>

          <div class="flex shrink-0 items-center justify-end gap-2 tabular-nums max-[760px]:justify-start">
            <span class="text-[10px] text-(--faint)">{{ worklogRange(worklog) }}</span>
            <span class="font-semibold text-(--muted)">{{ formatDuration(worklog.durationMs) }}</span>
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
          class="inline-flex min-h-[30px] min-w-[82px] items-center justify-center gap-1.5 rounded-md border border-(--accent) bg-(--accent) px-3 py-1.5 text-[11px] font-bold text-(--bg) transition-[color,background,border-color] duration-150 ease-[var(--ease)] disabled:cursor-default disabled:opacity-[0.42]"
          type="submit"
          :disabled="tasks.loading || publishableSessions.length === 0"
          :aria-busy="tasks.loading ? 'true' : 'false'"
        >
          <span
            v-if="tasks.loading"
            class="h-3 w-3 shrink-0 animate-spin rounded-full border-2 border-(--bg) border-t-transparent"
            aria-hidden="true"
          ></span>
          <span>{{ tasks.loading ? "Publishing..." : "Publish" }}</span>
        </button>
      </footer>
    </template>
  </ModalShell>
</template>
