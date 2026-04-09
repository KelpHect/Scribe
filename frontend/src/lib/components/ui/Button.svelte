<svelte:options runes />

<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { HTMLButtonAttributes } from 'svelte/elements';
  import { cn } from '$lib/utils';

  type Variant = 'default' | 'secondary' | 'outline' | 'ghost' | 'destructive';
  type Size = 'default' | 'sm' | 'lg' | 'icon';

  type Props = HTMLButtonAttributes & {
    variant?: Variant;
    size?: Size;
    class?: string;
    children: Snippet;
  };

  const {
    variant = 'default',
    size = 'default',
    class: className,
    children,
    ...rest
  }: Props = $props();

  const variants: Record<Variant, string> = {
    default: 'bg-primary text-primary-foreground hover:bg-primary/90',
    secondary: 'bg-secondary text-secondary-foreground hover:bg-secondary/80',
    outline: 'border border-input bg-background hover:bg-accent hover:text-accent-foreground',
    ghost: 'hover:bg-accent hover:text-accent-foreground',
    destructive: 'bg-destructive text-destructive-foreground hover:bg-destructive/90'
  };

  const sizes: Record<Size, string> = {
    default: 'h-9 px-4 py-2',
    sm: 'h-8 px-3 text-xs',
    lg: 'h-10 px-6',
    icon: 'h-9 w-9'
  };
</script>

<button
  class={cn(
    'focus-visible:ring-ring inline-flex cursor-pointer items-center justify-center gap-2 rounded-md text-sm font-medium whitespace-nowrap transition-colors focus-visible:ring-1 focus-visible:outline-none disabled:pointer-events-none disabled:opacity-50',
    variants[variant],
    sizes[size],
    className
  )}
  {...rest}
>
  {@render children()}
</button>
