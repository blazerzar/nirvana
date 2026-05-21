<script setup lang="ts">
import { computed } from "vue";
import {
  formatClock,
  formatDayLabel,
  formatDuration,
  useAllTasksStore,
} from "../../stores/allTasks";
import { useSettingsStore } from "../../stores/settings";
import { BackendSlot, TaskTimelineSession } from "../../types/types";
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

type PublishPreviewSource = {
  id: string;
  ticketKey: string;
  title: string;
  note?: string;
  startedAt: Date;
  durationMs: number;
};

const sourcesFromSessions = (sessions: TaskTimelineSession[]) =>
  sessions.map((entry) => ({
    id: `slot-${entry.session.id}`,
    ticketKey: entry.task.key,
    title: entry.task.title,
    note: entry.session.note,
    startedAt: entry.session.start,
    durationMs: entry.durationMs,
  }));

const sourcesFromSlots = (slots: BackendSlot[]) =>
  slots
    .map((slot) => {
      const startedAt = new Date(slot.started_at * 1000);
      const stoppedAt = slot.stopped_at ? new Date(slot.stopped_at * 1000) : null;

      return {
        id: `all-slot-${slot.id}`,
        ticketKey: slot.ticket_key,
        title: slot.summary || slot.ticket_key,
        note: slot.note ?? undefined,
        startedAt,
        durationMs: stoppedAt ? stoppedAt.getTime() - startedAt.getTime() : 0,
      };
    })
    .filter((source) => source.durationMs > 0);

const individualPreviewWorklogs = (sources: PublishPreviewSource[]) =>
  sources.map((source) => ({
    ...source,
    sourceSlotCount: 1,
  }));

const localDayKey = (date: Date) =>
  `${date.getFullYear()}-${(date.getMonth() + 1)
    .toString()
    .padStart(2, "0")}-${date.getDate().toString().padStart(2, "0")}`;

const squashedPreviewWorklogs = (sources: PublishPreviewSource[]) => {
  const dayGroups: {
    day: string;
    firstStartMs: number;
    worklogs: PublishPreviewWorklog[];
  }[] = [];

  [...sources]
    .sort((left, right) => left.startedAt.getTime() - right.startedAt.getTime())
    .forEach((source) => {
      const day = localDayKey(source.startedAt);
      let dayGroup = dayGroups.find((group) => group.day === day);
      if (!dayGroup) {
        dayGroup = {
          day,
          firstStartMs: source.startedAt.getTime(),
          worklogs: [],
        };
        dayGroups.push(dayGroup);
      }

      const existing = dayGroup.worklogs.find(
        (worklog) => worklog.ticketKey === source.ticketKey,
      );

      if (existing) {
        existing.durationMs += source.durationMs;
        existing.sourceSlotCount += 1;
        return;
      }

      dayGroup.worklogs.push({
        id: `ticket-${day}-${source.ticketKey}`,
        ticketKey: source.ticketKey,
        title: source.title,
        startedAt: new Date(dayGroup.firstStartMs),
        durationMs: source.durationMs,
        sourceSlotCount: 1,
      });
    });

  return dayGroups.flatMap((group) => {
    let cursorMs = group.firstStartMs;
    group.worklogs.forEach((worklog) => {
      worklog.startedAt = new Date(cursorMs);
      cursorMs += worklog.durationMs;
    });
    return group.worklogs;
  });
};

const previewSources = computed(() => sourcesFromSessions(publishableSessions.value));
const allPreviewSources = computed(() => sourcesFromSlots(tasks.allPublishableSlots));

const previewWorklogs = computed(() =>
  settings.publishSquashedWorklogs
    ? squashedPreviewWorklogs(previewSources.value)
    : individualPreviewWorklogs(previewSources.value),
);

const allPreviewWorklogs = computed(() =>
  settings.publishSquashedWorklogs
    ? squashedPreviewWorklogs(allPreviewSources.value)
    : individualPreviewWorklogs(allPreviewSources.value),
);

const totalDurationMs = computed(() =>
  previewWorklogs.value.reduce((sum, worklog) => sum + worklog.durationMs, 0),
);

const allDurationMs = computed(() =>
  allPreviewWorklogs.value.reduce((sum, worklog) => sum + worklog.durationMs, 0),
);

const slotLabel = computed(
  () =>
    `${publishableSessions.value.length} ${
      publishableSessions.value.length === 1 ? "slot" : "slots"
    }`,
);

const allSlotLabel = computed(
  () =>
    `${allPreviewSources.value.length} ${
      allPreviewSources.value.length === 1 ? "slot" : "slots"
    }`,
);

const worklogLabel = computed(
  () =>
    `${previewWorklogs.value.length} Jira ${
      previewWorklogs.value.length === 1 ? "worklog" : "worklogs"
    }`,
);

const allWorklogLabel = computed(
  () =>
    `${allPreviewWorklogs.value.length} Jira ${
      allPreviewWorklogs.value.length === 1 ? "worklog" : "worklogs"
    }`,
);

const modeLabel = computed(() =>
  settings.publishSquashedWorklogs ? "Squashed preview" : "Slot preview",
);

const allDateRangeLabel = computed(() => {
  if (allPreviewSources.value.length === 0) return "No unpublished slots";

  const starts = allPreviewSources.value.map((source) => source.startedAt.getTime());
  const start = new Date(Math.min(...starts));
  const end = new Date(Math.max(...starts));
  const startLabel = formatDayLabel(start);
  const endLabel = formatDayLabel(end);

  return startLabel === endLabel ? startLabel : `${startLabel} - ${endLabel}`;
});

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

      <div
        class="rounded-md border border-[rgba(149,222,200,0.16)] bg-[rgba(149,222,200,0.045)] px-3 py-2"
      >
        <div class="flex flex-wrap items-center gap-x-2 gap-y-1 text-[11px]">
          <span class="font-semibold text-(--text)">All unpublished</span>
          <span class="text-(--very-faint)">·</span>
          <span class="text-(--muted)">{{ allDateRangeLabel }}</span>
          <span class="text-(--very-faint)">·</span>
          <span class="text-(--muted)">{{ allWorklogLabel }}</span>
          <span class="text-(--very-faint)">·</span>
          <span class="text-(--muted)">{{ allSlotLabel }}</span>
          <span class="text-(--very-faint)">·</span>
          <span class="font-semibold text-(--accent)">{{ formatDuration(allDurationMs) }}</span>
        </div>
      </div>

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
          class="inline-flex min-h-[30px] min-w-[102px] items-center justify-center gap-1.5 rounded-md border border-[rgba(149,222,200,0.42)] bg-[rgba(149,222,200,0.08)] px-3 py-1.5 text-[11px] font-bold text-(--accent) transition-[color,background,border-color] duration-150 ease-[var(--ease)] hover:bg-[rgba(149,222,200,0.13)] disabled:cursor-default disabled:opacity-[0.42]"
          type="button"
          :disabled="tasks.loading || allPreviewSources.length === 0"
          :aria-busy="tasks.publishingScope === 'all' ? 'true' : 'false'"
          @click="tasks.confirmPublishAllUnpublished()"
        >
          <span
            v-if="tasks.publishingScope === 'all'"
            class="h-3 w-3 shrink-0 animate-spin rounded-full border-2 border-(--accent) border-t-transparent"
            aria-hidden="true"
          ></span>
          <span>{{ tasks.publishingScope === "all" ? "Publishing all..." : "Publish all" }}</span>
        </button>
        <button
          class="inline-flex min-h-[30px] min-w-[82px] items-center justify-center gap-1.5 rounded-md border border-(--accent) bg-(--accent) px-3 py-1.5 text-[11px] font-bold text-(--bg) transition-[color,background,border-color] duration-150 ease-[var(--ease)] disabled:cursor-default disabled:opacity-[0.42]"
          type="submit"
          :disabled="tasks.loading || publishableSessions.length === 0"
          :aria-busy="tasks.publishingScope === 'day' ? 'true' : 'false'"
        >
          <span
            v-if="tasks.publishingScope === 'day'"
            class="h-3 w-3 shrink-0 animate-spin rounded-full border-2 border-(--bg) border-t-transparent"
            aria-hidden="true"
          ></span>
          <span>{{ tasks.publishingScope === "day" ? "Publishing..." : "Publish" }}</span>
        </button>
      </footer>
    </template>
  </ModalShell>
</template>
