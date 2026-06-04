<script setup lang="ts">
import { computed } from "vue";
import { useIdleStore } from "../../stores/idle";
import { useAllTasksStore } from "../../stores/allTasks";
import { formatClock, formatDuration } from "../../tasks/time";
import ModalShell from "./ModalShell.vue";

const idle = useIdleStore();
const tasks = useAllTasksStore();

const period = computed(() => idle.currentPeriod);
const runningTask = computed(() => tasks.runningTask);

const durationLabel = computed(() => {
    const p = period.value;
    if (!p) return "";
    return formatDuration((p.to - p.from) * 1000);
});

const fmtTime = (ts: number) => formatClock(new Date(ts * 1000));

const idleStart = computed(() =>
    period.value ? fmtTime(period.value.from) : "",
);
const idleEnd = computed(() => (period.value ? fmtTime(period.value.to) : ""));

const ticketKey = computed(() => runningTask.value?.key ?? "");

const canApply = computed(
    () => idle.wasWorking !== null && idle.continueTracking !== null,
);

const resultSummary = computed(() => {
    if (idle.wasWorking === null || idle.continueTracking === null) return "";
    const key = ticketKey.value;

    if (idle.wasWorking && idle.continueTracking) {
        return `Keep the ${key} slot running – no changes.`;
    }
    if (idle.wasWorking && !idle.continueTracking) {
        return `Stop ${key} now.`;
    }
    if (!idle.wasWorking && idle.continueTracking) {
        return `Close ${key} at ${idleStart.value}, leave the ${durationLabel.value} gap untracked, start a fresh ${key} slot at ${idleEnd.value}.`;
    }
    return `Close ${key} at ${idleStart.value}, leave the ${durationLabel.value} gap untracked, stop tracking.`;
});

const handleKeydown = (event: KeyboardEvent) => {
    if (event.key === "Escape") {
        event.preventDefault();
        idle.dismiss();
    }
    if (event.key === "Enter" && canApply.value) {
        event.preventDefault();
        void idle.resolve();
    }
};
</script>

<template>
    <Transition name="modal">
        <div
            v-if="period && runningTask"
            class="fixed inset-0 z-20 grid place-items-center bg-(--modal-overlay) p-[18px] backdrop-blur-[5px]"
            @keydown="handleKeydown"
            @click.self="idle.dismiss()"
        >
            <ModalShell
                labelled-by="idle-modal-title"
                width-class="w-[min(480px,100%)]"
                @close="idle.dismiss()"
                @submit="canApply && idle.resolve()"
            >
                <template #title>
                    <div class="flex w-full items-center justify-between">
                        <span
                            id="idle-modal-title"
                            class="text-[11px] font-semibold uppercase tracking-wide text-(--muted)"
                        >
                            welcome back · idle detected
                        </span>
                        <span class="text-[10px] text-(--very-faint)">
                            <kbd
                                class="rounded border border-(--border) bg-(--surface) px-1 py-px font-[family-name:var(--font-mono)] text-[10px]"
                                >esc</kbd
                            >
                            to close
                        </span>
                    </div>
                </template>

                <div class="flex flex-col gap-4 p-4">
                    <!-- Idle alert -->
                    <div
                        class="rounded-md border border-(--border) bg-(--surface) px-3 py-2.5"
                    >
                        <p class="m-0 text-xs font-semibold text-(--text)">
                            you were away for {{ durationLabel }}
                        </p>
                        <p
                            class="m-0 mt-1 text-[11px] leading-relaxed text-(--faint)"
                        >
                            no keyboard or mouse input between
                            {{ idleStart }} and {{ idleEnd }}. the timer kept
                            running on the current slot.
                        </p>
                    </div>

                    <!-- Two-column radio grid -->
                    <div class="grid grid-cols-2 gap-3">
                        <!-- Column 1: what were they doing -->
                        <div class="flex flex-col gap-1.5">
                            <span
                                class="text-[11px] font-semibold text-(--text)"
                                >those {{ durationLabel }}?</span
                            >
                            <label
                                class="flex cursor-pointer items-center gap-2 rounded-md border px-3 py-2 transition-colors duration-100"
                                :class="
                                    idle.wasWorking === false
                                        ? 'border-(--accent) bg-[rgba(149,222,200,0.06)]'
                                        : 'border-(--border) bg-(--surface) hover:border-(--border-strong)'
                                "
                            >
                                <input
                                    type="radio"
                                    name="idle-what"
                                    class="cursor-pointer accent-(--accent)"
                                    :checked="idle.wasWorking === false"
                                    @change="idle.wasWorking = false"
                                />
                                <span
                                    class="text-[11px] font-medium text-(--text)"
                                    >idle time</span
                                >
                            </label>
                            <label
                                class="flex cursor-pointer items-center gap-2 rounded-md border px-3 py-2 transition-colors duration-100"
                                :class="
                                    idle.wasWorking === true
                                        ? 'border-(--accent) bg-[rgba(149,222,200,0.06)]'
                                        : 'border-(--border) bg-(--surface) hover:border-(--border-strong)'
                                "
                            >
                                <input
                                    type="radio"
                                    name="idle-what"
                                    class="cursor-pointer accent-(--accent)"
                                    :checked="idle.wasWorking === true"
                                    @change="idle.wasWorking = true"
                                />
                                <span
                                    class="text-[11px] font-medium text-(--text)"
                                    >working</span
                                >
                            </label>
                        </div>

                        <!-- Column 2: and now -->
                        <div class="flex flex-col gap-1.5">
                            <span
                                class="text-[11px] font-semibold text-(--text)"
                                >and now?</span
                            >
                            <label
                                class="flex cursor-pointer items-center gap-2 rounded-md border px-3 py-2 transition-colors duration-100"
                                :class="
                                    idle.continueTracking === true
                                        ? 'border-(--accent) bg-[rgba(149,222,200,0.06)]'
                                        : 'border-(--border) bg-(--surface) hover:border-(--border-strong)'
                                "
                            >
                                <input
                                    type="radio"
                                    name="idle-now"
                                    class="cursor-pointer accent-(--accent)"
                                    :checked="idle.continueTracking === true"
                                    @change="idle.continueTracking = true"
                                />
                                <span
                                    class="text-[11px] font-medium text-(--text)"
                                    >continue {{ ticketKey }}</span
                                >
                            </label>
                            <label
                                class="flex cursor-pointer items-center gap-2 rounded-md border px-3 py-2 transition-colors duration-100"
                                :class="
                                    idle.continueTracking === false
                                        ? 'border-(--accent) bg-[rgba(149,222,200,0.06)]'
                                        : 'border-(--border) bg-(--surface) hover:border-(--border-strong)'
                                "
                            >
                                <input
                                    type="radio"
                                    name="idle-now"
                                    class="cursor-pointer accent-(--accent)"
                                    :checked="idle.continueTracking === false"
                                    @change="idle.continueTracking = false"
                                />
                                <span
                                    class="text-[11px] font-medium text-(--text)"
                                    >stop tracking</span
                                >
                            </label>
                        </div>
                    </div>

                    <!-- Result summary -->
                    <div
                        v-if="resultSummary"
                        class="rounded-md border border-(--border) bg-(--surface-inset) px-3 py-2"
                    >
                        <div class="flex items-center justify-between">
                            <span
                                class="text-[9px] font-bold uppercase tracking-[0.06em] text-(--very-faint)"
                                >result</span
                            >
                            <span
                                class="text-[9px] font-bold uppercase tracking-[0.06em] text-(--faint)"
                                >to apply</span
                            >
                        </div>
                        <p
                            class="m-0 mt-1 text-[11px] leading-relaxed text-(--muted)"
                        >
                            {{ resultSummary }}
                        </p>
                    </div>
                </div>

                <template #footer>
                    <footer
                        class="flex min-h-[46px] items-center justify-end gap-2 border-t border-(--border) bg-(--surface-inset) px-3.5 py-2.5"
                    >
                        <button
                            class="min-h-[30px] rounded-md border border-(--border) bg-(--surface) px-3 py-1.5 text-[11px] text-(--muted) transition-[color,background,border-color] duration-150 ease-[var(--ease)] hover:bg-(--surface-strong) hover:text-(--text)"
                            type="button"
                            @click="idle.dismiss()"
                        >
                            Dismiss
                        </button>
                        <button
                            class="min-h-[30px] rounded-md border border-(--accent) bg-(--accent) px-3 py-1.5 text-[11px] font-bold text-(--bg) transition-[color,background,border-color] duration-150 ease-[var(--ease)] disabled:cursor-default disabled:opacity-[0.42]"
                            type="submit"
                            :disabled="!canApply"
                        >
                            Apply
                        </button>
                    </footer>
                </template>
            </ModalShell>
        </div>
    </Transition>
</template>
