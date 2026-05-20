<script setup lang="ts">
import { openUrl } from "@tauri-apps/plugin-opener";
import { nextTick, ref, watch } from "vue";
import ExternalLinkIcon from "../assets/icons/external-link.svg?raw";
import TicketIcon from "../assets/icons/ticket.svg?raw";
import EmptyState from "../components/EmptyState.vue";
import {
    formatClock,
    formatDuration,
    useAllTasksStore,
} from "../stores/allTasks";
import { TaskSession, TaskSummary, TaskTimelineSession } from "../types/types";

const tasks = useAllTasksStore();
const ticketList = ref<HTMLElement | null>(null);
const timelineList = ref<HTMLElement | null>(null);

const sessionDuration = (session: TaskSession) =>
    formatDuration(
        (session.end ?? tasks.now).getTime() - session.start.getTime(),
    );

const sessionRange = (session: TaskSession) =>
    `${formatClock(session.start)} – ${session.end ? formatClock(session.end) : "now"}`;

const slotLabel = (count: number) =>
    `${count} ${count === 1 ? "slot" : "slots"}`;

const openTaskUrl = async (url?: string) => {
    if (!url) return;

    try {
        await openUrl(url);
    } catch {
        window.open(url, "_blank", "noopener,noreferrer");
    }
};

const rowClass = (summary: TaskSummary) => ({
    "flex items-start gap-2.5 border-t border-[rgba(255,255,255,0.04)] px-[18px] py-[9px] transition-colors duration-150 ease-[var(--ease)] first:border-t-0 hover:bg-[rgba(255,255,255,0.025)] max-[760px]:px-3.5": true,
    "bg-[rgba(149,222,200,0.06)]": tasks.selectedTaskId === summary.task.id,
    "bg-[rgba(149,222,200,0.05)]":
        summary.isActive && tasks.selectedTaskId !== summary.task.id,
});

const timelineRowClass = (entry: TaskTimelineSession) => ({
    "grid min-h-10 grid-cols-[minmax(0,1fr)_auto] items-center gap-3 border-t border-[rgba(255,255,255,0.04)] px-[18px] py-1.5 transition-colors duration-150 ease-[var(--ease)] first:border-t-0 hover:bg-[rgba(255,255,255,0.025)] max-[760px]:gap-2 max-[760px]:px-3.5": true,
    "bg-[rgba(149,222,200,0.06)]": tasks.selectedSessionId === entry.session.id,
    "bg-[rgba(149,222,200,0.05)]":
        entry.isActive && tasks.selectedSessionId !== entry.session.id,
});

const contentKey = () =>
    `${tasks.viewMode}-${tasks.selectedDate.toISOString().slice(0, 10)}`;

watch(
    () => [tasks.viewMode, tasks.selectedTaskId, tasks.selectedSessionId],
    async () => {
        await nextTick();

        const list =
            tasks.viewMode === "ticket" ? ticketList.value : timelineList.value;
        list?.querySelector<HTMLElement>("[data-selected='true']")?.scrollIntoView({
            block: "nearest",
        });
    },
);
</script>

<template>
    <section class="flex min-h-0 flex-col">
        <div
            v-if="tasks.error"
            class="border-b border-[rgba(255,154,134,0.18)] bg-[rgba(255,154,134,0.07)] px-[18px] py-2 text-[11px] text-[#ff9a86] max-[760px]:px-3.5"
        >
            {{ tasks.error }}
        </div>
        <div
            v-else-if="tasks.loading"
            class="border-b border-(--border) px-[18px] py-2 text-[11px] text-(--faint) max-[760px]:px-3.5"
        >
            Loading tracked work...
        </div>

        <Transition name="content-swap" mode="out-in">
            <div
                v-if="tasks.viewMode === 'ticket'"
                :key="contentKey()"
                ref="ticketList"
                class="min-h-0 flex-1 overflow-auto py-1"
                role="list"
                aria-label="Tracked tasks by ticket"
            >
                <TransitionGroup name="row" tag="div">
                    <article
                        v-for="summary in tasks.summaries"
                        :key="summary.task.id"
                        :class="rowClass(summary)"
                        :data-selected="tasks.selectedTaskId === summary.task.id"
                        role="listitem"
                        @click="tasks.selectTask(summary.task.id)"
                    >
                    <button
                        class="inline-flex w-4 shrink-0 items-center justify-center bg-transparent pt-px text-sm leading-none text-(--very-faint) transition-colors duration-150 ease-[var(--ease)] hover:text-(--muted)"
                        type="button"
                        :aria-expanded="tasks.isExpanded(summary.task.id)"
                        @click.stop="tasks.toggleExpanded(summary.task.id)"
                    >
                        <span
                            class="inline-block transition-transform duration-150 ease-(--ease)"
                            :class="
                                tasks.isExpanded(summary.task.id)
                                    ? 'rotate-90'
                                    : ''
                            "
                            >›</span
                        >
                    </button>

                    <div class="min-w-0 flex-1">
                        <div class="mb-0.5 flex items-center gap-[7px]">
                            <span
                                class="shrink-0 font-mono text-[11px] font-semibold"
                                :class="
                                    summary.isActive
                                        ? 'text-(--accent)'
                                        : 'text-(--faint)'
                                "
                            >
                                {{ summary.task.key }}
                            </span>
                            <span
                                class="overflow-hidden text-ellipsis whitespace-nowrap text-[11.5px] font-medium"
                                :class="
                                    summary.isActive
                                        ? 'text-(--text)'
                                        : 'text-(--muted)'
                                "
                            >
                                {{ summary.task.title }}
                            </span>
                            <span
                                v-if="summary.isActive"
                                class="shrink-0 rounded-[3px] bg-(--accent) px-1.25 py-px text-[8px] font-bold uppercase tracking-[0.04em] text-(--bg)"
                                >live</span
                            >
                            <button
                                v-if="summary.task.url"
                                class="inline-flex h-5 w-5 shrink-0 items-center justify-center rounded text-(--very-faint) transition-[color,background] duration-150 ease-(--ease) hover:bg-[rgba(255,255,255,0.04)] hover:text-(--muted) [&>svg]:h-3 [&>svg]:w-3"
                                type="button"
                                :aria-label="`Open ${summary.task.key}`"
                                :title="`Open ${summary.task.key}`"
                                @click.stop="openTaskUrl(summary.task.url)"
                                v-html="ExternalLinkIcon"
                            ></button>
                        </div>
                        <Transition name="expand">
                            <div
                                v-if="tasks.isExpanded(summary.task.id)"
                                class="mt-1.5 flex flex-col gap-0.5 overflow-hidden pl-6.5 max-[760px]:pl-4"
                            >
                                <div
                                    v-for="session in summary.sessions"
                                    :key="session.id"
                                    class="flex cursor-pointer items-center gap-2 rounded-md border px-2.5 py-1.25 text-[11px] transition-[background,border-color] duration-150 ease-(--ease) hover:bg-[rgba(255,255,255,0.04)]"
                                    :class="
                                        tasks.selectedSessionId === session.id
                                            ? 'border-[rgba(149,222,200,0.18)] bg-[rgba(149,222,200,0.08)]'
                                            : 'border-transparent bg-[rgba(255,255,255,0.02)]'
                                    "
                                    @click.stop="
                                        tasks.selectSession(session.id)
                                    "
                                >
                                    <span class="shrink-0 text-(--faint)">{{
                                        sessionRange(session)
                                    }}</span>
                                    <span
                                        class="min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap italic text-(--muted) max-[760px]:hidden"
                                        >{{
                                            session.note || "Focused work"
                                        }}</span
                                    >
                                    <span
                                        class="h-[5px] w-[5px] shrink-0 rounded-full max-[760px]:hidden"
                                        :class="
                                            session.publishState === 'published'
                                                ? 'bg-(--success)'
                                                : 'bg-(--warning)'
                                        "
                                        :title="session.publishState"
                                    ></span>
                                    <span
                                        class="shrink-0 text-(--muted) tabular-nums"
                                        >{{ sessionDuration(session) }}</span
                                    >
                                </div>
                            </div>
                        </Transition>
                    </div>

                    <div class="flex shrink-0 flex-row items-center gap-1.5">
                        <span
                            class="text-[11.5px] tabular-nums"
                            :class="
                                summary.isActive
                                    ? 'font-semibold text-(--text)'
                                    : 'font-medium text-(--muted)'
                            "
                        >
                            {{ formatDuration(summary.totalMs) }}
                        </span>
                        <span class="text-[10px] text-(--very-faint)">{{
                            slotLabel(summary.slotCount)
                        }}</span>
                    </div>
                    </article>
                </TransitionGroup>

                <EmptyState
                    v-if="!tasks.loading && tasks.summaries.length === 0"
                    :icon="TicketIcon"
                    :title="`No tickets tracked on ${tasks.selectedDateLabel}`"
                    body="Start tracking to add the first work session for this day."
                />
            </div>

            <div
                v-else
                :key="contentKey()"
                ref="timelineList"
                class="min-h-0 flex-1 overflow-auto pt-1.5"
                role="list"
                aria-label="Tracked tasks in day order"
            >
                <TransitionGroup name="row" tag="div">
                    <article
                        v-for="entry in tasks.timelineSessions"
                        :key="entry.session.id"
                        :class="timelineRowClass(entry)"
                        :data-selected="tasks.selectedSessionId === entry.session.id"
                        :title="entry.session.note"
                        role="listitem"
                        @click="tasks.selectSession(entry.session.id)"
                    >
                    <div class="flex min-w-0 items-center gap-[7px]">
                        <span
                            class="shrink-0 font-mono text-[11px] font-semibold"
                            :class="
                                entry.isActive
                                    ? 'text-(--accent)'
                                    : 'text-(--faint)'
                            "
                        >
                            {{ entry.task.key }}
                        </span>
                        <div
                            class="flex min-w-0 items-baseline gap-1.5 overflow-hidden"
                        >
                            <span
                                class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-[11.5px] font-medium"
                                :class="
                                    entry.isActive
                                        ? 'text-(--text)'
                                        : 'text-(--muted)'
                                "
                            >
                                {{ entry.task.title }}
                            </span>
                            <span
                                v-if="entry.session.note"
                                class="min-w-[36px] max-w-[min(32vw,320px)] overflow-hidden text-ellipsis whitespace-nowrap text-[10.5px] text-(--faint) max-[520px]:hidden"
                            >
                                {{ entry.session.note }}
                            </span>
                        </div>
                        <span
                            v-if="entry.isActive"
                            class="shrink-0 rounded-[3px] bg-(--accent) px-1.25 py-px text-[8px] font-bold uppercase tracking-[0.04em] text-(--bg)"
                            >live</span
                        >
                        <button
                            v-if="entry.task.url"
                            class="inline-flex h-5 w-5 shrink-0 items-center justify-center rounded text-(--very-faint) transition-[color,background] duration-150 ease-(--ease) hover:bg-[rgba(255,255,255,0.04)] hover:text-(--muted) [&>svg]:h-3 [&>svg]:w-3"
                            type="button"
                            :aria-label="`Open ${entry.task.key}`"
                            :title="`Open ${entry.task.key}`"
                            @click.stop="openTaskUrl(entry.task.url)"
                            v-html="ExternalLinkIcon"
                        ></button>
                    </div>

                    <div
                        class="flex shrink-0 items-center justify-end gap-2 tabular-nums"
                    >
                        <span
                            class="whitespace-nowrap text-[10px] font-medium text-(--faint) max-[760px]:hidden"
                            >{{ sessionRange(entry.session) }}</span
                        >
                        <span
                            class="whitespace-nowrap text-[11px] font-semibold text-(--muted)"
                            >{{ formatDuration(entry.durationMs) }}</span
                        >
                        <span
                            class="h-1.25 w-1.25 shrink-0 rounded-full"
                            :class="
                                entry.session.publishState === 'published'
                                    ? 'bg-(--success)'
                                    : 'bg-(--warning)'
                            "
                            :title="entry.session.publishState"
                        ></span>
                    </div>
                    </article>
                </TransitionGroup>

                <EmptyState
                    v-if="!tasks.loading && tasks.timelineSessions.length === 0"
                    :icon="TicketIcon"
                    :title="`No tickets tracked on ${tasks.selectedDateLabel}`"
                    body="Start tracking to add the first work session for this day."
                />
            </div>
        </Transition>
    </section>
</template>
