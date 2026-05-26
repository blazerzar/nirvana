<script setup lang="ts">
import { ref } from "vue";
import type { Task } from "../types/types";

type TicketSearchResult = {
  task: Task;
  score: number;
};

defineProps<{
  modelValue: string;
  searchOpen: boolean;
  searchResults: TicketSearchResult[];
  highlightedResultIndex: number;
  resultsId: string;
  placeholder?: string;
  handleTicketKeydown: (event: KeyboardEvent) => void | Promise<void>;
  selectSearchResult: (task: Task) => void | Promise<void>;
  slotCountLabel: (task: Task) => string;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: string];
  "update:searchOpen": [value: boolean];
  "update:highlightedResultIndex": [value: number];
}>();

const root = ref<HTMLElement | null>(null);
const input = ref<HTMLInputElement | null>(null);

defineExpose({
  focus: () => input.value?.focus(),
  select: () => input.value?.select(),
});

const handleInput = (event: Event) => {
  emit("update:modelValue", (event.target as HTMLInputElement).value);
};

const handleFocusout = (event: FocusEvent) => {
  const nextTarget = event.relatedTarget;

  if (nextTarget instanceof Node && root.value?.contains(nextTarget)) {
    return;
  }

  emit("update:searchOpen", false);
};
</script>

<template>
  <div
    ref="root"
    class="relative"
    @focusout="handleFocusout"
  >
    <input
      ref="input"
      :value="modelValue"
      class="min-h-[34px] w-full rounded-md border border-(--border) bg-(--input-bg) px-2.5 py-[7px] text-xs text-(--text) tabular-nums outline-none transition-[border-color,box-shadow] duration-150 ease-[var(--ease)] placeholder:text-(--very-faint) focus:border-(--input-focus) focus:shadow-[0_0_0_2px_var(--input-focus-ring)]"
      :placeholder="placeholder ?? 'NIR-12'"
      autocomplete="off"
      aria-autocomplete="list"
      :aria-expanded="searchOpen && searchResults.length > 0"
      :aria-controls="resultsId"
      @input="handleInput"
      @focus="emit('update:searchOpen', true)"
      @keydown="handleTicketKeydown"
    />

    <div
      v-if="searchOpen && searchResults.length > 0"
      :id="resultsId"
      class="absolute top-[calc(100%+5px)] right-0 left-0 z-40 max-h-[min(238px,48vh)] overflow-auto rounded-[7px] border border-(--border) bg-(--panel) shadow-[0_16px_36px_rgba(0,0,0,0.36)]"
      role="listbox"
      aria-label="Matching tickets"
    >
      <button
        v-for="(result, index) in searchResults"
        :key="result.task.id"
        class="grid min-h-[34px] w-full grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-[9px] border-b border-(--border) px-[9px] py-[7px] text-left text-[11px] last:border-b-0 hover:bg-(--surface-selected) hover:text-(--text) max-[760px]:grid-cols-[auto_minmax(0,1fr)]"
        :class="index === highlightedResultIndex ? 'bg-(--surface-selected) text-(--text)' : 'text-(--muted)'"
        type="button"
        role="option"
        :aria-selected="index === highlightedResultIndex"
        @mousedown.prevent="selectSearchResult(result.task)"
        @mouseenter="emit('update:highlightedResultIndex', index)"
      >
        <span class="whitespace-nowrap font-[family-name:var(--font-mono)] font-bold text-(--accent)">{{ result.task.key }}</span>
        <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-inherit">{{ result.task.title }}</span>
        <span class="whitespace-nowrap text-[10px] text-(--faint) max-[760px]:hidden">
          {{ slotCountLabel(result.task) }}
          <span v-if="result.task.status === 'running'">· running</span>
        </span>
      </button>
    </div>
  </div>
</template>
