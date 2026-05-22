<script setup lang="ts">
import { ref } from "vue";
import AddConnectionModal from "../components/modals/AddConnectionModal.vue";
import {
  connectionTypeLabels,
  useConnectionsStore,
} from "../stores/connections";
import { themeOptions, useSettingsStore } from "../stores/settings";

defineEmits<{
  close: [];
}>();

const settings = useSettingsStore();
const connections = useConnectionsStore();
const showAddConnectionModal = ref(false);

const openAddConnectionModal = () => {
  connections.resetSetup();
  showAddConnectionModal.value = true;
};

const closeAddConnectionModal = () => {
  showAddConnectionModal.value = false;
  connections.resetSetup();
};

const handleFontScaleInput = (event: Event) => {
  const input = event.target as HTMLInputElement;
  settings.fontScale = Number(input.value);
  settings.applyCurrentAppearance();
};

const handleFontScaleChange = (event: Event) => {
  const input = event.target as HTMLInputElement;
  settings.setFontScale(Number(input.value));
};

const handleModalKeydown = (event: KeyboardEvent) => {
  if (event.key === "Escape" || event.key === "Esc") {
    event.preventDefault();
    event.stopPropagation();
    closeAddConnectionModal();
  }
};
</script>

<template>
  <section
    class="grid min-h-0 grid-rows-[auto_minmax(0,1fr)] overflow-hidden bg-(--panel)"
  >
    <header
      class="flex min-h-[46px] items-center justify-between border-b border-(--border) bg-[rgba(255,255,255,0.012)] px-[18px] max-[760px]:min-h-[auto] max-[760px]:gap-2 max-[760px]:px-3.5 max-[760px]:py-2.5"
    >
      <div class="min-w-0">
        <h2 class="m-0 text-xs leading-none font-bold text-(--text)">
          Settings
        </h2>
        <p class="mt-1 mb-0 text-[10px] leading-none text-(--faint)">
          Preferences are saved to the app config file.
        </p>
      </div>

      <button
        class="min-h-[30px] rounded-md border border-(--border) bg-(--surface) px-3 py-1.5 text-[11px] text-(--muted) transition-[color,background,border-color] duration-150 ease-[var(--ease)] hover:bg-(--surface-strong) hover:text-(--text)"
        type="button"
        @click="$emit('close')"
      >
        Back
      </button>
    </header>

    <div class="min-h-0 overflow-auto px-[18px] py-4 max-[760px]:px-3.5">
      <div class="mx-auto flex w-full max-w-[760px] flex-col gap-3">
        <section
          class="rounded-md border border-(--border) bg-(--surface) p-4"
        >
          <div class="mb-4 grid gap-4">
            <div class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-4 max-[760px]:grid-cols-1">
              <div class="min-w-0">
                <h3 class="m-0 text-[12px] font-bold text-(--text)">
                  Font size
                </h3>
                <p class="mt-1.5 mb-0 max-w-[560px] text-[11px] leading-5 text-(--faint)">
                  {{ settings.fontScalePercent }}%
                </p>
              </div>

              <input
                class="h-7 w-[220px] accent-(--accent) max-[760px]:w-full"
                type="range"
                min="0.9"
                max="1.25"
                step="0.05"
                :value="settings.fontScale"
                :disabled="settings.loading || settings.saving"
                aria-label="Font size"
                @input="handleFontScaleInput"
                @change="handleFontScaleChange"
              />
            </div>

            <div class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-4 max-[760px]:grid-cols-1">
              <div class="min-w-0">
                <h3 class="m-0 text-[12px] font-bold text-(--text)">
                  Theme
                </h3>
                <p class="mt-1.5 mb-0 max-w-[560px] text-[11px] leading-5 text-(--faint)">
                  {{ settings.activeTheme.name }}
                </p>
              </div>

              <div
                class="flex flex-wrap gap-0.5 rounded-md border border-(--border) bg-(--surface) p-[3px]"
                aria-label="Theme"
              >
                <button
                  v-for="theme in themeOptions"
                  :key="theme.id"
                  class="rounded px-2.5 py-[5px] text-[10.5px] leading-none transition-[color,background] duration-150 ease-[var(--ease)]"
                  :class="
                    settings.theme === theme.id
                      ? 'bg-(--surface-selected) font-semibold text-(--accent)'
                      : 'text-(--faint) hover:bg-(--surface-hover) hover:text-(--muted)'
                  "
                  type="button"
                  :aria-pressed="settings.theme === theme.id"
                  :disabled="settings.loading || settings.saving"
                  @click="settings.setTheme(theme.id)"
                >
                  {{ theme.name }}
                </button>
              </div>
            </div>
          </div>
        </section>

        <section
          class="rounded-md border border-(--border) bg-(--surface) p-4"
        >
          <div
            class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-4 max-[760px]:grid-cols-1"
          >
            <div class="min-w-0">
              <h3 class="m-0 text-[12px] font-bold text-(--text)">
                Squash publish worklogs
              </h3>
              <p class="mt-1.5 mb-0 max-w-[560px] text-[11px] leading-5 text-(--faint)">
                Multiple local slots for the same ticket publish as one Jira
                worklog, placed sequentially with the rest of the day.
              </p>
            </div>

            <button
              class="relative h-[28px] w-[48px] rounded-full border transition-[background,border-color,opacity] duration-150 ease-[var(--ease)] disabled:cursor-default disabled:opacity-50"
              :class="
                settings.publishSquashedWorklogs
                  ? 'border-(--accent) bg-(--surface-selected)'
                  : 'border-(--border) bg-(--surface-inset)'
              "
              type="button"
              role="switch"
              :aria-checked="settings.publishSquashedWorklogs"
              :disabled="settings.loading || settings.saving"
              @click="
                settings.setPublishSquashedWorklogs(
                  !settings.publishSquashedWorklogs,
                )
              "
            >
              <span
                class="absolute top-1/2 h-5 w-5 -translate-y-1/2 rounded-full shadow-[0_2px_8px_rgba(0,0,0,0.28)] transition-[left,background] duration-150 ease-[var(--ease)]"
                :class="
                  settings.publishSquashedWorklogs
                    ? 'left-[22px] bg-(--accent)'
                    : 'left-1 bg-(--muted)'
                "
                aria-hidden="true"
              ></span>
            </button>
          </div>
        </section>

        <section
          class="rounded-md border border-(--border) bg-(--surface) p-4"
        >
          <div class="mb-3 flex items-start justify-between gap-3">
            <div class="min-w-0">
              <h3 class="m-0 text-[12px] font-bold text-(--text)">
                Connections
              </h3>
              <p class="mt-1.5 mb-0 max-w-[560px] text-[11px] leading-5 text-(--faint)">
                Add Jira workspaces, switch the active connection, or remove
                unused connections.
              </p>
            </div>

            <button
              class="min-h-[30px] shrink-0 rounded-md border border-(--accent) bg-(--surface-selected) px-3 py-1.5 text-[11px] font-semibold text-(--accent) transition-[color,background,border-color] duration-150 ease-[var(--ease)] hover:bg-(--surface-hover) hover:text-(--text)"
              type="button"
              :disabled="connections.loading"
              @click="openAddConnectionModal"
            >
              Add connection
            </button>
          </div>

          <div
            v-if="connections.connections.length > 0"
            class="overflow-hidden rounded-md border border-(--border) bg-(--surface-inset)"
          >
            <div
              v-for="connection in connections.connections"
              :key="connection.id"
              class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-3 border-t border-(--border) px-3 py-2 first:border-t-0 max-[760px]:grid-cols-1"
            >
              <div class="min-w-0">
                <div class="flex min-w-0 items-center gap-2">
                  <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-[12px] font-semibold text-(--text)">
                    {{ connection.name }}
                  </span>
                  <span
                    v-if="connections.activeConnection?.id === connection.id"
                    class="shrink-0 rounded-[3px] bg-(--surface-selected) px-1.5 py-px text-[8px] font-bold uppercase tracking-[0.04em] text-(--accent)"
                  >
                    active
                  </span>
                </div>
                <div class="mt-1 flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1 text-[10.5px] text-(--faint)">
                  <span>{{ connectionTypeLabels[connection.type] }}</span>
                  <span class="text-(--very-faint)">·</span>
                  <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap">{{ connection.hostname }}</span>
                  <span class="text-(--very-faint)">·</span>
                  <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap">{{ connection.username }}</span>
                </div>
              </div>

              <div class="flex shrink-0 items-center justify-end gap-2 max-[760px]:justify-start">
                <button
                  class="min-h-[28px] rounded-md border border-(--border) bg-(--surface) px-2.5 py-1 text-[10.5px] text-(--muted) transition-[color,background,border-color] duration-150 ease-[var(--ease)] hover:bg-(--surface-strong) hover:text-(--text) disabled:cursor-default disabled:opacity-40"
                  type="button"
                  :disabled="
                    connections.loading ||
                    connections.activeConnection?.id === connection.id
                  "
                  @click="connections.setActiveConnection(connection.id)"
                >
                  Switch
                </button>
                <button
                  class="min-h-[28px] rounded-md border border-(--danger-border) bg-(--danger-surface) px-2.5 py-1 text-[10.5px] text-(--danger) transition-[color,background,border-color] duration-150 ease-[var(--ease)] hover:bg-(--surface-hover) disabled:cursor-default disabled:opacity-40"
                  type="button"
                  :disabled="connections.loading"
                  @click="connections.deleteConnection(connection.id)"
                >
                  Delete
                </button>
              </div>
            </div>
          </div>

          <p
            v-else
            class="m-0 rounded-md border border-(--border) bg-(--surface-inset) px-3 py-3 text-center text-[11px] text-(--faint)"
          >
            No connections configured.
          </p>
        </section>

        <p
          class="m-0 min-h-4 text-[11px]"
          :class="
            settings.error || (!showAddConnectionModal && connections.error)
              ? 'text-(--danger)'
              : 'text-(--faint)'
          "
        >
          {{
            (!showAddConnectionModal && connections.error) ||
            settings.error ||
            (settings.saving
              ? "Saving settings..."
              : settings.loading || connections.loading
                ? "Loading settings..."
                : "")
          }}
        </p>
      </div>
    </div>
  </section>

  <Transition name="modal">
    <div
      v-if="showAddConnectionModal"
      class="fixed inset-0 z-30 grid place-items-center bg-(--modal-overlay) p-[18px] backdrop-blur-[5px]"
      tabindex="-1"
      @click.self="closeAddConnectionModal"
      @keydown="handleModalKeydown"
    >
      <AddConnectionModal @close="closeAddConnectionModal" />
    </div>
  </Transition>
</template>
