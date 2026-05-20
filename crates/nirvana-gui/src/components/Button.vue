<script setup lang="ts">
defineProps<{ shortcut?: string; disabled?: boolean }>();
defineEmits(["click"]);
</script>

<template>
    <button
        class="inline-flex items-center gap-1 rounded-md border border-[rgba(149,222,200,0.2)] bg-[rgba(149,222,200,0.07)] px-2 py-[3px] text-[10.5px] font-medium text-(--accent) transition-[color,background,border-color,transform] duration-150 ease-[var(--ease)] hover:border-[rgba(149,222,200,0.34)] hover:bg-[rgba(149,222,200,0.12)] hover:text-(--text) active:translate-y-px active:scale-[0.98] disabled:cursor-default disabled:border-(--border) disabled:bg-(--surface) disabled:text-(--very-faint) disabled:opacity-100 disabled:[&>kbd]:bg-[rgba(255,255,255,0.04)] disabled:[&>kbd]:text-(--very-faint)"
        :disabled="disabled"
        type="button"
        @click.stop.prevent="$emit('click')"
    >
        <span class="max-[760px]:hidden"><slot /></span>
        <kbd
            v-if="shortcut"
            class="rounded-[3px] bg-[rgba(149,222,200,0.12)] px-1 py-px font-[family-name:var(--font-mono)] text-[10px] leading-none text-(--accent) opacity-90"
            aria-hidden="true"
        >
            {{ shortcut }}
        </kbd>
    </button>
</template>
