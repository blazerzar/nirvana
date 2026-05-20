<script setup lang="ts">
defineProps<{
  labelledBy: string;
  widthClass: string;
}>();

defineEmits(["close", "submit"]);
</script>

<template>
  <form
    :class="`overflow-hidden rounded-lg border border-(--border) bg-[#0d1011] text-(--text) shadow-[0_24px_70px_rgba(0,0,0,0.54)] ${widthClass}`"
    :aria-labelledby="labelledBy"
    @submit.prevent="$emit('submit')"
  >
    <header class="flex min-h-11 items-center justify-between border-b border-(--border) bg-[rgba(255,255,255,0.018)] pr-3.5 pl-4">
      <slot name="title"></slot>
      <button
        class="inline-flex h-[26px] w-[26px] items-center justify-center rounded-md text-lg leading-none text-(--faint) transition-[color,background] duration-150 ease-[var(--ease)] hover:bg-(--surface) hover:text-(--text)"
        type="button"
        aria-label="Close modal"
        @click="$emit('close')"
      >×</button>
    </header>

    <slot></slot>

    <slot name="footer"></slot>
  </form>
</template>
