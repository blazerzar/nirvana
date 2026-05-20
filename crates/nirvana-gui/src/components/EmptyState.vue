<script setup lang="ts">
withDefaults(
  defineProps<{
    icon: string;
    title: string;
    body: string;
    iconClass?: string;
  }>(),
  {
    iconClass: "h-18 w-18 text-(--very-faint)",
  },
);
</script>

<template>
  <div class="grid min-h-full place-items-center px-6 py-10 text-center text-xs text-(--muted)">
    <div class="flex max-w-70 flex-col items-center gap-3">
      <span
        :class="[iconClass, '[&>svg]:h-full [&>svg]:w-full']"
        aria-hidden="true"
        v-html="icon"
      ></span>
      <div>
        <span class="font-bold text-(--text)">{{ title }}</span>
        <p class="mt-1.5 mb-0 leading-relaxed text-(--faint)">{{ body }}</p>
      </div>
    </div>
  </div>
</template>
