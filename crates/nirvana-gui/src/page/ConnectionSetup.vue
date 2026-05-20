<script setup lang="ts">
import { computed, nextTick, onMounted, ref } from "vue";
import {
  connectionTypeLabels,
  useConnectionsStore,
} from "../stores/connections";
import { ConnectionType } from "../types/types";

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
  if (connections.normalizedDraftHostname) return "";
  return "Hostname is required.";
});

const credentialsError = computed(() => {
  if (!connections.draftUsername.trim()) return `${connections.identityLabel} is required.`;
  if (!connections.draftToken.trim()) return "Token is required.";
  return "";
});

const goToCredentials = async () => {
  connections.nextSetupStep();

  if (connections.setupStep === "credentials") {
    await nextTick();
    usernameField.value?.focus();
  }
};

const saveConnection = () => {
  connections.saveConnection();
};

onMounted(() => {
  hostnameField.value?.focus();
});
</script>

<template>
  <main class="grid h-screen place-items-center overflow-hidden bg-(--bg) px-4 py-6 text-(--text)">
    <div class="w-[min(440px,100%)]">
      <div class="mb-3 flex items-center justify-between px-1">
        <div class="flex items-center gap-2.5">
          <span
            class="h-1.5 w-1.5 rounded-full"
            :class="connections.setupStep === 'details' ? 'bg-(--accent)' : 'bg-(--very-faint)'"
          ></span>
          <span
            class="text-[10px] font-semibold uppercase tracking-[0.04em]"
            :class="connections.setupStep === 'details' ? 'text-(--accent)' : 'text-(--faint)'"
          >Details</span>
        </div>
        <div class="h-px flex-1 bg-(--border) mx-2"></div>
        <div class="flex items-center gap-2.5">
          <span
            class="h-1.5 w-1.5 rounded-full"
            :class="connections.setupStep === 'credentials' ? 'bg-(--accent)' : 'bg-(--very-faint)'"
          ></span>
          <span
            class="text-[10px] font-semibold uppercase tracking-[0.04em]"
            :class="connections.setupStep === 'credentials' ? 'text-(--accent)' : 'text-(--faint)'"
          >Credentials</span>
        </div>
      </div>

      <section
        class="overflow-hidden rounded-lg border border-(--border) bg-(--panel) shadow-[0_18px_44px_rgba(0,0,0,0.32)]"
        aria-labelledby="connection-setup-title"
      >
      <header class="px-4 pt-4 pb-1">
        <div class="flex items-center gap-2.5">
          <div class="h-2 w-2 shrink-0 rounded-full bg-(--accent) shadow-[0_0_8px_rgba(149,222,200,0.5)]"></div>
          <h1 id="connection-setup-title" class="m-0 text-[13px] font-bold text-(--text)">
            Connect nirvana
          </h1>
        </div>
      </header>

      <form
        v-if="connections.setupStep === 'details'"
        class="flex flex-col gap-3 px-4 pt-3 pb-4"
        @submit.prevent="goToCredentials"
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
                  ? 'bg-[rgba(149,222,200,0.12)] font-semibold text-(--accent)'
                  : 'text-(--faint) hover:bg-[rgba(255,255,255,0.035)] hover:text-(--muted)'
              "
              type="button"
              @click="connections.setConnectionType(type.value)"
            >
              {{ type.label }}
            </button>
          </div>
        </fieldset>

        <label class="flex min-w-0 flex-col gap-1.5">
          <span class="text-[10px] font-bold uppercase tracking-[0.04em] text-(--faint)">Hostname</span>
          <input
            ref="hostnameField"
            v-model="connections.draftHostname"
            class="min-h-[34px] w-full rounded-md border border-(--border) bg-[rgba(0,0,0,0.24)] px-2.5 py-[7px] text-xs text-(--text) outline-none transition-[border-color,box-shadow] duration-150 ease-[var(--ease)] placeholder:text-(--very-faint) focus:border-[rgba(149,222,200,0.7)] focus:shadow-[0_0_0_2px_rgba(149,222,200,0.1)]"
            placeholder="acme.atlassian.net"
            autocomplete="url"
          />
        </label>

        <p class="m-0 min-h-4 text-[11px] text-[#ff9a86]">
          {{ detailsError }}
        </p>

        <footer class="flex items-center justify-end gap-2 pt-1">
          <button
            class="min-h-[30px] rounded-md border border-(--accent) bg-(--accent) px-3 py-1.5 text-[11px] font-bold text-(--bg) transition-[color,background,border-color] duration-150 ease-[var(--ease)] disabled:cursor-default disabled:opacity-[0.42]"
            type="submit"
            :disabled="!connections.isDetailsValid"
          >
            Next
          </button>
        </footer>
      </form>

      <form
        v-else
        class="flex flex-col gap-3 px-4 pt-3 pb-4"
        @submit.prevent="saveConnection"
      >
        <div class="px-0.5 text-[11px] text-(--faint)">
          <span class="font-semibold text-(--accent)">{{ connections.connectionTypeLabel }}</span>
          <span class="text-(--very-faint)"> · </span>
          <span>{{ connections.normalizedDraftHostname }}</span>
        </div>

        <label class="flex min-w-0 flex-col gap-1.5">
          <span class="text-[10px] font-bold uppercase tracking-[0.04em] text-(--faint)">{{ connections.identityLabel }}</span>
          <input
            ref="usernameField"
            v-model="connections.draftUsername"
            class="min-h-[34px] w-full rounded-md border border-(--border) bg-[rgba(0,0,0,0.24)] px-2.5 py-[7px] text-xs text-(--text) outline-none transition-[border-color,box-shadow] duration-150 ease-[var(--ease)] placeholder:text-(--very-faint) focus:border-[rgba(149,222,200,0.7)] focus:shadow-[0_0_0_2px_rgba(149,222,200,0.1)]"
            :placeholder="connections.draftType === 'jira-cloud' ? 'you@company.com' : 'you'"
            autocomplete="username"
          />
        </label>

        <label class="flex min-w-0 flex-col gap-1.5">
          <span class="text-[10px] font-bold uppercase tracking-[0.04em] text-(--faint)">Token</span>
          <input
            v-model="connections.draftToken"
            class="min-h-[34px] w-full rounded-md border border-(--border) bg-[rgba(0,0,0,0.24)] px-2.5 py-[7px] text-xs text-(--text) outline-none transition-[border-color,box-shadow] duration-150 ease-[var(--ease)] placeholder:text-(--very-faint) focus:border-[rgba(149,222,200,0.7)] focus:shadow-[0_0_0_2px_rgba(149,222,200,0.1)]"
            type="password"
            placeholder="Paste access token"
            autocomplete="current-password"
          />
        </label>

        <p class="m-0 min-h-4 text-[11px] text-[#ff9a86]">
          {{ credentialsError }}
        </p>

        <footer class="flex items-center justify-end gap-2 pt-1">
          <button
            class="mr-auto min-h-[30px] rounded-md border border-(--border) bg-(--surface) px-3 py-1.5 text-[11px] text-(--muted) transition-[color,background,border-color] duration-150 ease-[var(--ease)] hover:bg-(--surface-strong) hover:text-(--text)"
            type="button"
            @click="connections.previousSetupStep()"
          >
            Back
          </button>
          <button
            class="min-h-[30px] rounded-md border border-(--accent) bg-(--accent) px-3 py-1.5 text-[11px] font-bold text-(--bg) transition-[color,background,border-color] duration-150 ease-[var(--ease)] disabled:cursor-default disabled:opacity-[0.42]"
            type="submit"
            :disabled="!connections.isCredentialsValid"
          >
            Connect
          </button>
        </footer>
      </form>
      </section>
    </div>
  </main>
</template>
