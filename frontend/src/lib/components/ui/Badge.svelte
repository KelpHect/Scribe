<svelte:options runes />

<script lang="ts">
  import type { Snippet } from 'svelte';
  import { cn } from '$lib/utils';

  type Variant = 'default' | 'secondary' | 'outline' | 'destructive' | 'success' | 'warning';

  type Props = {
    variant?: Variant;
    class?: string;
    children: Snippet;
  };

  const { variant = 'default', class: className, children }: Props = $props();

  const variants: Record<Variant, string> = {
    default: 'bg-primary text-primary-foreground',
    secondary: 'bg-secondary text-secondary-foreground',
    outline: 'border border-input text-foreground',
    destructive: 'bg-destructive text-destructive-foreground',
    success: 'bg-success/15 text-success',
    warning: 'bg-warning/15 text-warning'
  };
</script>

<div
  class={cn(
    'focus:ring-ring inline-flex items-center rounded-md px-2.5 py-0.5 text-xs font-semibold transition-colors focus:ring-2 focus:ring-offset-2 focus:outline-none',
    variants[variant],
    className
  )}
>
  {@render children()}
</div>
