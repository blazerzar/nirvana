<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { AppInfo, AppView, ViewMode } from "./types/types";
import TaskModals from "./components/TaskModals.vue";
import Footer from "./layout/Footer.vue";
import ConnectionSetup from "./page/ConnectionSetup.vue";
import Main from "./page/Main.vue";
import Settings from "./page/Settings.vue";
import { useAllTasksStore } from "./stores/allTasks";
import { useConnectionsStore } from "./stores/connections";
import { useSettingsStore } from "./stores/settings";
import SettingsIcon from "./assets/icons/settings.svg?raw";

const tasks = useAllTasksStore();
const connections = useConnectionsStore();
const settings = useSettingsStore();
const appInfo = ref<AppInfo>({ name: "nirvana", version: "0.2.1-pre-alpha" });
const currentView = ref<AppView>("tracker");

const viewModes: { label: string; value: ViewMode }[] = [
    { label: "Ticket", value: "ticket" },
    { label: "Day", value: "day" },
];

const dateTransitionName = () => {
    if (tasks.dayTransitionDirection === "previous") {
        return "date-slide-previous";
    }

    if (tasks.dayTransitionDirection === "next") {
        return "date-slide-next";
    }

    return "date-swap";
};

const getAppInfo = async () => {
    try {
        const response = await invoke<AppInfo>("get_app_info");
        appInfo.value = {
            name: response.name || "nirvana",
            version: response.version || "0.2.1-pre-alpha",
        };
    } catch {
        appInfo.value = { name: "nirvana", version: "0.2.1-pre-alpha" };
    }
};

const handleKeydown = (event: KeyboardEvent) => {
    const target = event.target as HTMLElement | null;
    const isTyping =
        target?.tagName === "INPUT" ||
        target?.tagName === "TEXTAREA" ||
        target?.isContentEditable;

    const key = event.key.toLowerCase();

    if (tasks.activeModal) {
        if (key === "escape" || key === "esc") {
            event.preventDefault();
            tasks.closeModal();
        }

        return;
    }

    if (currentView.value === "settings") {
        if (key === "escape" || key === "esc") {
            event.preventDefault();
            currentView.value = "tracker";
        }
        return;
    }

    if (isTyping || event.metaKey || event.ctrlKey || event.altKey) return;
    if (!connections.activeConnection) return;

    if (key === "arrowdown" || key === "arrowup") {
        event.preventDefault();
        tasks.navigateSelection(key === "arrowdown" ? 1 : -1);
        return;
    }

    if (key === "arrowleft" || key === "arrowright") {
        event.preventDefault();
        if (key === "arrowleft") {
            tasks.previousDay();
        } else {
            tasks.nextDay();
        }
        return;
    }

    if (["s", "a", "x", "w", "p", "e"].includes(key)) {
        event.preventDefault();
        tasks.runShortcut(key);
    }
};

let ticker: number | undefined;

onMounted(() => {
    getAppInfo();
    connections.initialize();
    settings.initialize();
    ticker = window.setInterval(() => tasks.tick(), 1000);
    window.addEventListener("keydown", handleKeydown);
});

onUnmounted(() => {
    if (ticker) window.clearInterval(ticker);
    window.removeEventListener("keydown", handleKeydown);
});

watch(
    () => connections.activeConnection,
    (connection) => {
        if (connection) {
            void tasks.loadSelectedDate();
        } else {
            tasks.setRunningSlot(null);
        }
    },
);
</script>

<template>
    <main
        v-if="!connections.initialized"
        class="grid h-screen place-items-center bg-(--bg) text-(--faint)"
    >
        <div class="flex items-center gap-2 text-[11px]">
            <span
                class="h-1.5 w-1.5 animate-[dot-pulse_2s_var(--ease)_infinite] rounded-full bg-(--accent)"
            ></span>
            <span>Loading connection</span>
        </div>
    </main>

    <ConnectionSetup v-else-if="!connections.activeConnection" />

    <main v-else class="h-screen overflow-hidden bg-(--bg) text-(--text)">
        <section
            class="grid h-full grid-rows-[auto_auto_minmax(0,1fr)_auto] overflow-hidden bg-(--panel)"
        >
            <!-- Row 1: Brand + Workspace -->
            <header
                class="flex min-h-[46px] items-center justify-between border-b border-(--border) bg-[rgba(255,255,255,0.012)] px-[18px] max-[760px]:min-h-[auto] max-[760px]:flex-wrap max-[760px]:gap-2 max-[760px]:px-3.5 max-[760px]:py-2.5"
            >
                <div class="flex items-center gap-2.5">
                    <div
                        class="h-2 w-2 shrink-0 rounded-full bg-(--accent) shadow-[0_0_8px_rgba(149,222,200,0.5)]"
                    ></div>
                    <h1
                        class="m-0 text-xs leading-none font-bold uppercase tracking-wide text-(--text)"
                    >
                        {{ appInfo.name }}
                    </h1>
                    <span
                        class="text-xs leading-none uppercase font-medium text-(--very-faint)"
                        >v{{ appInfo.version }}</span
                    >
                </div>

                <div
                    class="flex items-center gap-1.5 rounded-full border border-(--border) bg-(--surface) px-3 py-1.25 text-[11px] leading-none"
                    aria-label="Current workspace"
                >
                    <div
                        class="h-1.25 w-1.25 shrink-0 rounded-full bg-(--success) shadow-[0_0_5px_rgba(131,210,158,0.4)]"
                    ></div>
                    <span class="text-(--muted)">{{
                        connections.activeConnection.name
                    }}</span>
                    <span class="text-(--very-faint)">·</span>
                    <span class="text-(--faint)">{{
                        connections.activeConnection.hostname
                    }}</span>
                </div>
            </header>

            <!-- Row 2: View title + Filter + Settings -->
            <div
                class="grid min-h-9 grid-cols-[1fr_auto_1fr] items-center gap-2 border-b border-(--border) px-[18px] max-[760px]:min-h-[auto] max-[760px]:grid-cols-1 max-[760px]:px-3.5 max-[760px]:py-2.5"
            >
                <div class="flex items-center gap-1.5 text-xs leading-none">
                    <span class="font-semibold text-(--text)">
                        {{ tasks.viewMode === "ticket" ? "Ticket" : "Day" }}
                    </span>
                    <span class="text-(--very-faint)">·</span>
                    <span class="text-(--faint)">
                        {{
                            tasks.viewMode === "ticket" ? "Grouped" : "Timeline"
                        }}
                    </span>
                </div>

                <div
                    class="flex items-center gap-0.5 justify-self-center rounded-md border border-(--border) bg-(--surface) p-[3px]"
                    aria-label="Date navigator"
                >
                    <button
                        class="inline-flex h-4 min-w-4 items-center justify-center rounded px-1.5 text-[12px] leading-none text-(--faint) transition-[color,background] duration-150 ease-(--ease) hover:bg-[rgba(255,255,255,0.04)] hover:text-(--muted)"
                        type="button"
                        aria-label="Previous day"
                        @click="tasks.previousDay()"
                    >
                        ‹
                    </button>
                    <button
                        class="min-w-[92px] rounded px-2 py-[3px] text-center text-[10px] leading-none font-semibold text-(--text) transition-[color,background] duration-150 ease-(--ease) hover:bg-[rgba(255,255,255,0.025)] hover:text-(--accent)"
                        type="button"
                        aria-label="Go to today"
                        @click="tasks.goToToday()"
                    >
                        <span class="relative inline-grid min-h-3 w-[92px] overflow-hidden">
                            <Transition :name="dateTransitionName()">
                                <span
                                    :key="tasks.selectedDateLabel"
                                    class="inline-block whitespace-nowrap"
                                >
                                    {{ tasks.selectedDateLabel }}
                                </span>
                            </Transition>
                        </span>
                    </button>
                    <button
                        class="inline-flex h-4 min-w-4 items-center justify-center rounded px-1.5 text-[12px] leading-none text-(--faint) transition-[color,background] duration-150 ease-(--ease) hover:bg-[rgba(255,255,255,0.04)] hover:text-(--muted) disabled:cursor-default disabled:text-(--very-faint) disabled:hover:bg-transparent disabled:hover:text-(--very-faint)"
                        type="button"
                        aria-label="Next day"
                        :disabled="!tasks.canNavigateNextDay"
                        @click="tasks.nextDay()"
                    >
                        ›
                    </button>
                </div>

                <div class="flex items-center justify-end gap-2">
                    <div
                        class="flex gap-0.5 rounded-md border border-(--border) bg-(--surface) p-[3px]"
                        aria-label="View mode"
                    >
                        <button
                            v-for="viewMode in viewModes"
                            :key="viewMode.value"
                            class="inline-flex items-center gap-[5px] rounded px-2.5 py-[3px] text-[10px] leading-none transition-[color,background] duration-150 ease-[var(--ease)]"
                            :class="
                                tasks.viewMode === viewMode.value
                                    ? 'bg-[rgba(149,222,200,0.12)] font-semibold text-(--accent)'
                                    : 'text-(--faint)'
                            "
                            type="button"
                            @click="tasks.setViewMode(viewMode.value)"
                        >
                            {{ viewMode.label }}
                        </button>
                    </div>

                    <button
                        class="flex h-7 w-7 items-center justify-center rounded-[7px] border border-(--border) bg-(--surface) text-(--faint) transition-[color,background] duration-150 ease-[var(--ease)] hover:bg-(--surface-strong) hover:text-(--muted)"
                        :class="
                            currentView === 'settings'
                                ? 'border-(--accent) bg-[rgba(149,222,200,0.12)] text-(--accent)'
                                : ''
                        "
                        type="button"
                        aria-label="Settings"
                        :aria-pressed="currentView === 'settings'"
                        @click="
                            currentView =
                                currentView === 'settings'
                                    ? 'tracker'
                                    : 'settings'
                        "
                    >
                        <span
                            class="h-[13px] w-[13px] [&>svg]:h-full [&>svg]:w-full"
                            aria-hidden="true"
                            v-html="SettingsIcon"
                        ></span>
                    </button>
                </div>
            </div>

            <Settings
                v-if="currentView === 'settings'"
                @close="currentView = 'tracker'"
            />
            <Main v-else />
            <Footer v-if="currentView === 'tracker'" />
            <TaskModals v-if="currentView === 'tracker'" />
        </section>
    </main>
</template>
