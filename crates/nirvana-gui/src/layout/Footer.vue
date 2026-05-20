<script setup lang="ts">
import { computed } from "vue";
import { formatDuration, useAllTasksStore } from "../stores/allTasks";
import Button from "../components/Button.vue";

const tasks = useAllTasksStore();

const actions = computed(() => [
    {
        key: "s",
        label: "Start",
        fn: () => tasks.openStartModal(),
        disabled: tasks.loading,
    },
    {
        key: "x",
        label: "Stop",
        fn: () => tasks.stopActiveTask(),
        disabled: tasks.loading || tasks.activeTask === null,
    },
    {
        key: "w",
        label: "Switch",
        fn: () => tasks.switchToPreviousTask(),
        disabled:
            tasks.loading ||
            tasks.activeTask === null ||
            tasks.previousTask === null,
    },
    {
        key: "e",
        label: "Edit",
        fn: () => tasks.openEditModal(),
        disabled: true,
    },
    {
        key: "p",
        label: "Publish",
        fn: () => tasks.openPublishModal(),
        disabled: tasks.loading || tasks.publishableSessions.length === 0,
    },
]);
</script>

<template>
    <footer
        class="flex min-h-11 items-center justify-between border-t border-(--border) bg-[rgba(255,255,255,0.012)] px-4.5 max-[760px]:min-h-auto max-[760px]:flex-wrap max-[760px]:gap-2 max-[760px]:px-3.5 max-[760px]:py-2.5"
    >
        <!-- Left: Tracking status -->
        <div
            class="flex min-w-0 items-center gap-[7px] overflow-hidden whitespace-nowrap text-[11px] text-(--muted)"
        >
            <template v-if="tasks.activeTask && tasks.activeSession">
                <div
                    class="h-1.5 w-1.5 shrink-0 animate-[dot-pulse_2s_var(--ease)_infinite] rounded-full bg-(--success) shadow-[0_0_6px_rgba(131,210,158,0.45)]"
                ></div>
                <span class="shrink-0 font-semibold text-(--accent)">{{
                    tasks.activeTask.key
                }}</span>
                <span class="shrink-0 text-(--faint)">
                    +{{
                        formatDuration(
                            tasks.now.getTime() -
                                tasks.activeSession.start.getTime(),
                        )
                    }}
                </span>
            </template>
            <template v-else>
                <div
                    class="h-1.5 w-1.5 shrink-0 rounded-full bg-(--very-faint)"
                ></div>
                <span class="shrink-0 text-(--faint) max-[520px]:hidden"
                    >Idle</span
                >
                <span
                    v-if="tasks.selectedTask"
                    class="shrink-0 font-semibold text-(--muted)"
                >
                    {{ tasks.selectedTask.key }}
                </span>
                <span
                    v-if="tasks.selectedTask"
                    class="shrink-0 text-(--faint) max-[900px]:hidden"
                    >selected</span
                >
            </template>
        </div>

        <!-- Right: Action buttons + totals -->
        <div class="flex items-center gap-2 max-[760px]:flex-wrap">
            <Button
                v-for="action in actions"
                :key="action.key"
                :shortcut="action.key"
                :disabled="action.disabled"
                @click="action.fn()"
                >{{ action.label }}</Button
            >

            <div class="mx-1 h-3.5 w-px bg-(--border)"></div>

            <div
                class="flex items-center gap-[5px] whitespace-nowrap text-[11px] text-(--faint)"
            >
                <span>Day</span>
                <strong class="font-semibold text-(--muted)">{{
                    formatDuration(tasks.selectedDateTotalMs)
                }}</strong>
                <span class="text-(--very-faint)">·</span>
                <span>Unpublished</span>
                <strong class="font-semibold text-(--muted)">{{
                    formatDuration(tasks.selectedDateUnpublishedTotalMs)
                }}</strong>
            </div>
        </div>
    </footer>
</template>
