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
import {
    dayBounds,
    sessionEndsAfterDay,
    sessionOverlapMs,
    sessionStartsBeforeDay,
} from "../tasks/time";
import { TaskSession, TaskSummary, TaskTimelineSession } from "../types/types";

const tasks = useAllTasksStore();
const ticketList = ref<HTMLElement | null>(null);
const timelineList = ref<HTMLElement | null>(null);

const sessionDuration = (session: TaskSession) =>
    formatDuration(sessionOverlapMs(session, tasks.selectedDate, tasks.now));

const clippedClock = (date: Date, selectedDate: Date, position: "start" | "end") => {
    const bounds = dayBounds(selectedDate);

    if (date.getTime() === bounds.start.getTime()) {
        return "00:00";
    }

    if (position === "end" && date.getTime() === bounds.end.getTime()) {
        return "24:00";
    }

    return formatClock(date);
};

const sessionRange = (session: TaskSession) => {
    const bounds = dayBounds(tasks.selectedDate);
    const sessionEnd = session.end ?? tasks.now;
    const startsBefore = sessionStartsBeforeDay(session, tasks.selectedDate);
    const endsAfter = sessionEndsAfterDay(session, tasks.selectedDate, tasks.now);
    const visibleStart = new Date(
        Math.max(session.start.getTime(), bounds.start.getTime()),
    );
    const visibleEnd = new Date(
        Math.min(sessionEnd.getTime(), bounds.end.getTime()),
    );

    return `${startsBefore ? "← " : ""}${clippedClock(
        visibleStart,
        tasks.selectedDate,
        "start",
    )} – ${clippedClock(visibleEnd, tasks.selectedDate, "end")}${
        endsAfter ? " →" : ""
    }`;
};

const spanLabel = (startsBeforeDay: boolean, endsAfterDay: boolean) => {
    if (startsBeforeDay && endsAfterDay) return "multi-day";
    if (startsBeforeDay) return "starts previous day";
    if (endsAfterDay) return "continues next day";
    return "";
};

const sessionSpanLabel = (session: TaskSession) =>
    spanLabel(
        sessionStartsBeforeDay(session, tasks.selectedDate),
        sessionEndsAfterDay(session, tasks.selectedDate, tasks.now),
    );

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
    "border-t border-[rgba(255,255,255,0.04)] transition-colors duration-150 ease-[var(--ease)] first:border-t-0 hover:bg-[rgba(255,255,255,0.025)]": true,
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

const contentTransitionName = () => {
    if (tasks.dayTransitionDirection === "previous") {
        return "day-slide-previous";
    }

    if (tasks.dayTransitionDirection === "next") {
        return "day-slide-next";
    }

    return "content-swap";
};

const contentTransitionMode = () =>
    tasks.dayTransitionDirection === "none" ? "out-in" : undefined;

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
    <section class="flex min-h-0 flex-col overflow-hidden">
        <div
            v-if="tasks.error"
            class="border-b border-[rgba(255,154,134,0.18)] bg-[rgba(255,154,134,0.07)] px-[18px] py-2 text-[11px] text-[#ff9a86] max-[760px]:px-3.5"
        >
            {{ tasks.error }}
        </div>
        <div
            v-else-if="tasks.isInitialLoading"
            class="border-b border-(--border) px-[18px] py-2 text-[11px] text-(--faint) max-[760px]:px-3.5"
        >
            Loading tracked work...
        </div>
        <div
            v-else-if="tasks.hasMultiDaySessions"
            class="border-b border-[rgba(245,191,107,0.2)] bg-[rgba(245,191,107,0.07)] px-[18px] py-2 text-[11px] text-[#f5bf6b] max-[760px]:px-3.5"
        >
            This day includes work sessions that span across midnight. Totals count only the portion within this day.
        </div>

        <div class="relative min-h-0 flex-1 overflow-hidden">
            <Transition
                :name="contentTransitionName()"
                :mode="contentTransitionMode()"
            >
                <div
                    v-if="tasks.viewMode === 'ticket'"
                    :key="contentKey()"
                    ref="ticketList"
                    class="h-full min-h-0 overflow-auto py-1"
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
                        <div
                            class="grid min-h-10 grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-2.5 px-[18px] py-1.5 max-[760px]:px-3.5"
                        >
                            <button
                                class="inline-flex h-6 w-5 shrink-0 items-center justify-center bg-transparent text-sm leading-none text-(--very-faint) transition-[color,transform] duration-150 ease-[var(--ease)] hover:text-(--muted)"
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

                            <div class="flex min-w-0 items-center gap-[7px]">
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
                        </div>

                        <Transition name="expand">
                            <div
                                v-if="tasks.isExpanded(summary.task.id)"
                                class="grid overflow-hidden"
                            >
                                <div class="min-h-0 overflow-hidden border-t border-[rgba(255,255,255,0.035)] bg-[rgba(0,0,0,0.12)]">
                                    <div
                                        v-for="session in summary.sessions"
                                        :key="session.id"
                                        class="grid min-h-[30px] cursor-pointer grid-cols-[112px_minmax(0,1fr)_auto_auto_auto] items-center gap-2 border-t border-[rgba(255,255,255,0.035)] px-[48px] py-1 text-[11px] transition-colors duration-150 ease-(--ease) first:border-t-0 hover:bg-[rgba(255,255,255,0.03)] max-[760px]:grid-cols-[minmax(0,1fr)_auto_auto] max-[760px]:px-8"
                                        :class="
                                            tasks.selectedSessionId === session.id
                                                ? 'bg-[rgba(149,222,200,0.08)]'
                                                : ''
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
                                            v-if="sessionSpanLabel(session)"
                                            class="shrink-0 rounded-[3px] border border-[rgba(245,191,107,0.24)] bg-[rgba(245,191,107,0.08)] px-1.25 py-px text-[8px] font-bold uppercase tracking-[0.04em] text-[#f5bf6b] max-[760px]:hidden"
                                        >
                                            {{ sessionSpanLabel(session) }}
                                        </span>
                                        <span
                                            class="h-[5px] w-[5px] shrink-0 rounded-full"
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
                            </div>
                        </Transition>
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
                    class="h-full min-h-0 overflow-auto pt-1.5"
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
                        <span
                            v-if="entry.isMultiDay"
                            class="shrink-0 rounded-[3px] border border-[rgba(245,191,107,0.24)] bg-[rgba(245,191,107,0.08)] px-1.25 py-px text-[8px] font-bold uppercase tracking-[0.04em] text-[#f5bf6b]"
                        >
                            {{ spanLabel(entry.startsBeforeDay, entry.endsAfterDay) }}
                        </span>
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
        </div>
    </section>
</template>
