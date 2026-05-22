<script setup lang="ts">
import { computed, nextTick, onMounted, ref } from "vue";
import {
  connectionTypeLabels,
  useConnectionsStore,
} from "../../stores/connections";
import { ConnectionType } from "../../types/types";
import ModalShell from "./ModalShell.vue";

const emit = defineEmits<{
  close: [];
}>();

const connections = useConnectionsStore();
const hostnameField = ref<HTMLInputElement | null>(null);
const usernameField = ref<HTMLInputElement | null>(null);

const connectionTypes = computed(() =>
  (Object.keys(connectionTypeLabels) as ConnectionType[]).map((value) => ({
    value,
    label: connectionTypeLabels[value],
  })),
);

const detailsError = computed(() => {
  if (!connections.draftName.trim()) return "Connection name is required.";
  if (connections.normalizedDraftHostname) return "";
  return "Hostname is required.";
});

const credentialsError = computed(() => {
  if (!connections.draftUsername.trim()) return `${connections.identityLabel} is required.`;
  if (!connections.draftToken.trim()) return "Token is required.";
  return "";
});

const closeModal = () => {
  connections.resetSetup();
  emit("close");
};

const goToCredentials = async () => {
  connections.nextSetupStep();

  if (connections.setupStep === "credentials") {
    await nextTick();
    usernameField.value?.focus();
  }
};

const saveConnection = async () => {
  if (await connections.saveConnection()) {
    emit("close");
  }
};

const submit = () => {
  if (connections.setupStep === "details") {
    void goToCredentials();
    return;
  }

  void saveConnection();
};

onMounted(async () => {
  await nextTick();
  hostnameField.value?.focus();
});
</script>

<template>
  <ModalShell
    labelled-by="add-connection-modal-title"
    width-class="w-[min(560px,100%)]"
    @close="closeModal"
    @submit="submit"
  >
    <template #title>
      <h2 id="add-connection-modal-title" class="m-0 text-[13px] font-bold text-(--text)">
        Add connection
      </h2>
    </template>

    <div
      v-if="connections.setupStep === 'details'"
      class="flex flex-col gap-3 p-4"
    >
      <fieldset class="m-0 flex min-w-0 flex-col gap-1.5 border-0 p-0">
        <legend class="text-[10px] font-bold uppercase tracking-[0.04em] text-(--faint)">
          Connection type
        </legend>
        <div class="grid grid-cols-2 gap-1 rounded-md border border-(--border) bg-(--surface) p-[3px]">
          <button
            v-for="type in connectionTypes"
            :key="type.value"
            class="rounded px-2.5 py-2 text-[11px] leading-none transition-[color,background] duration-150 ease-[var(--ease)]"
            :class="
              connections.draftType === type.value
                ? 'bg-(--surface-selected) font-semibold text-(--accent)'
                : 'text-(--faint) hover:bg-(--surface-hover) hover:text-(--muted)'
            "
            type="button"
            @click="connections.setConnectionType(type.value)"
          >
            {{ type.label }}
          </button>
        </div>
      </fieldset>

      <div class="grid grid-cols-2 gap-3 max-[760px]:grid-cols-1">
        <label class="flex min-w-0 flex-col gap-1.5">
          <span class="text-[10px] font-bold uppercase tracking-[0.04em] text-(--faint)">Name</span>
          <input
            v-model="connections.draftName"
            class="min-h-[34px] w-full rounded-md border border-(--border) bg-(--input-bg) px-2.5 py-[7px] text-xs text-(--text) outline-none transition-[border-color,box-shadow] duration-150 ease-[var(--ease)] placeholder:text-(--very-faint) focus:border-(--input-focus) focus:shadow-[0_0_0_2px_var(--input-focus-ring)]"
            placeholder="work"
            autocomplete="organization-title"
          />
        </label>

        <label class="flex min-w-0 flex-col gap-1.5">
          <span class="text-[10px] font-bold uppercase tracking-[0.04em] text-(--faint)">Hostname</span>
          <input
            ref="hostnameField"
            v-model="connections.draftHostname"
            class="min-h-[34px] w-full rounded-md border border-(--border) bg-(--input-bg) px-2.5 py-[7px] text-xs text-(--text) outline-none transition-[border-color,box-shadow] duration-150 ease-[var(--ease)] placeholder:text-(--very-faint) focus:border-(--input-focus) focus:shadow-[0_0_0_2px_var(--input-focus-ring)]"
            placeholder="acme.atlassian.net"
            autocomplete="url"
          />
        </label>
      </div>

      <p class="m-0 min-h-4 text-[11px] text-(--danger)">
        {{ connections.error || detailsError }}
      </p>
    </div>

    <div
      v-else
      class="flex flex-col gap-3 p-4"
    >
      <div class="text-[11px] text-(--faint)">
        <span class="font-semibold text-(--text)">{{ connections.draftName }}</span>
        <span class="text-(--very-faint)"> · </span>
        <span class="font-semibold text-(--accent)">{{ connections.connectionTypeLabel }}</span>
        <span class="text-(--very-faint)"> · </span>
        <span>{{ connections.normalizedDraftHostname }}</span>
      </div>

      <div class="grid grid-cols-2 gap-3 max-[760px]:grid-cols-1">
        <label class="flex min-w-0 flex-col gap-1.5">
          <span class="text-[10px] font-bold uppercase tracking-[0.04em] text-(--faint)">{{ connections.identityLabel }}</span>
          <input
            ref="usernameField"
            v-model="connections.draftUsername"
            class="min-h-[34px] w-full rounded-md border border-(--border) bg-(--input-bg) px-2.5 py-[7px] text-xs text-(--text) outline-none transition-[border-color,box-shadow] duration-150 ease-[var(--ease)] placeholder:text-(--very-faint) focus:border-(--input-focus) focus:shadow-[0_0_0_2px_var(--input-focus-ring)]"
            :placeholder="connections.draftType === 'jira-cloud' ? 'you@company.com' : 'you'"
            autocomplete="username"
          />
        </label>

        <label class="flex min-w-0 flex-col gap-1.5">
          <span class="text-[10px] font-bold uppercase tracking-[0.04em] text-(--faint)">Token</span>
          <input
            v-model="connections.draftToken"
            class="min-h-[34px] w-full rounded-md border border-(--border) bg-(--input-bg) px-2.5 py-[7px] text-xs text-(--text) outline-none transition-[border-color,box-shadow] duration-150 ease-[var(--ease)] placeholder:text-(--very-faint) focus:border-(--input-focus) focus:shadow-[0_0_0_2px_var(--input-focus-ring)]"
            type="password"
            placeholder="Paste access token"
            autocomplete="current-password"
          />
        </label>
      </div>

      <p class="m-0 min-h-4 text-[11px] text-(--danger)">
        {{ connections.error || credentialsError }}
      </p>
    </div>

    <template #footer>
      <footer class="flex min-h-[46px] items-center justify-end gap-2 border-t border-(--border) bg-(--surface-inset) px-3.5 py-2.5 max-[760px]:flex-wrap">
        <button
          v-if="connections.setupStep === 'credentials'"
          class="mr-auto min-h-[30px] rounded-md border border-(--border) bg-(--surface) px-3 py-1.5 text-[11px] text-(--muted) transition-[color,background,border-color] duration-150 ease-[var(--ease)] hover:bg-(--surface-strong) hover:text-(--text)"
          type="button"
          :disabled="connections.loading"
          @click="connections.previousSetupStep()"
        >
          Back
        </button>
        <button
          class="min-h-[30px] rounded-md border border-(--border) bg-(--surface) px-3 py-1.5 text-[11px] text-(--muted) transition-[color,background,border-color] duration-150 ease-[var(--ease)] hover:bg-(--surface-strong) hover:text-(--text)"
          type="button"
          :disabled="connections.loading"
          @click="closeModal"
        >
          Cancel
        </button>
        <button
          v-if="connections.setupStep === 'details'"
          class="min-h-[30px] rounded-md border border-(--accent) bg-(--accent) px-3 py-1.5 text-[11px] font-bold text-(--bg) transition-[color,background,border-color] duration-150 ease-[var(--ease)] disabled:cursor-default disabled:opacity-[0.42]"
          type="submit"
          :disabled="!connections.isDetailsValid || connections.loading"
        >
          Next
        </button>
        <button
          v-else
          class="min-h-[30px] rounded-md border border-(--accent) bg-(--accent) px-3 py-1.5 text-[11px] font-bold text-(--bg) transition-[color,background,border-color] duration-150 ease-[var(--ease)] disabled:cursor-default disabled:opacity-[0.42]"
          type="submit"
          :disabled="!connections.isCredentialsValid || connections.loading"
        >
          {{ connections.loading ? "Checking..." : "Connect" }}
        </button>
      </footer>
    </template>
  </ModalShell>
</template>
