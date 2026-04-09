<svelte:options runes />

<script lang="ts">
  interface Props {
    percent: number;
    class?: string;
  }

  const { percent, class: className = '' }: Props = $props();
</script>

<div class="bg-secondary h-1.5 w-full overflow-hidden rounded-full {className}">
  <div
    class="bg-primary h-full rounded-full transition-all duration-150 ease-out"
    style="width: {Math.min(100, Math.max(0, percent))}%"
  ></div>
</div>
