<svelte:options runes />

<script lang="ts">
  import AlertCircle from 'lucide-svelte/icons/alert-circle';
  import CheckCircle2 from 'lucide-svelte/icons/check-circle-2';
  import Clock from 'lucide-svelte/icons/clock';
  import Loader2 from 'lucide-svelte/icons/loader-2';
  import X from 'lucide-svelte/icons/x';
  import type { TaskProgress } from '$lib/stores/downloads.svelte';
  import {
    formatInstallPlanSummary,
    getInstallPlanCounts,
    getInstallPlanSafetyNote
  } from '$lib/install/preflight';
  import { getInstallRecoveryGuidance } from '$lib/install/recovery';
  import { summarizeTaskProgress } from '$lib/install/task-summary';

  type Props = {
    task: TaskProgress;
    oncancel?: (_uid: string) => void;
    ondismiss?: (_uid: string) => void;
  };

  const { task, oncancel, ondismiss }: Props = $props();

  const summary = $derived(summarizeTaskProgress(task));
  const installPlan = $derived(task.installPlan ?? []);
  const planCounts = $derived(getInstallPlanCounts(installPlan));
  const planSummary = $derived(formatInstallPlanSummary(installPlan));
  const safetyNote = $derived(getInstallPlanSafetyNote(installPlan));
  const recovery = $derived(getInstallRecoveryGuidance(task));
</script>

<div class="border-border bg-card flex flex-col gap-1.5 rounded-md border px-3 py-2.5">
  <div class="flex items-center gap-2">
    {#if task.state === 'queued'}
      <Clock size={14} class="text-muted-foreground shrink-0" />
    {:else if task.state === 'downloading' || task.state === 'planning' || task.state === 'extracting'}
      <Loader2 size={14} class="text-primary shrink-0 animate-spin" />
    {:else if task.state === 'complete'}
      <CheckCircle2 size={14} class="shrink-0 text-green-500" />
    {:else if task.state === 'failed'}
      <AlertCircle size={14} class="text-destructive shrink-0" />
    {:else}
      <X size={14} class="text-muted-foreground shrink-0" />
    {/if}

    <div class="min-w-0 flex-1">
      <p class="truncate text-sm font-medium">{task.name || task.uid}</p>
      <div class="text-muted-foreground flex items-center gap-2 text-xs">
        <span>{summary.stateLabel}</span>
        {#if summary.sizeLabel}
          <span>{summary.sizeLabel}</span>
        {/if}
        {#if summary.speedLabel}
          <span>{summary.speedLabel}</span>
        {/if}
        {#if summary.extractionLabel}
          <span>{summary.extractionLabel}</span>
        {/if}
      </div>
    </div>

    {#if summary.isActive && oncancel}
      <button
        onclick={() => oncancel(task.uid)}
        class="hover:bg-accent flex h-6 w-6 shrink-0 cursor-pointer items-center justify-center rounded"
        aria-label="Cancel download"
      >
        <X size={12} />
      </button>
    {:else if summary.isTerminal && ondismiss}
      <button
        onclick={() => ondismiss(task.uid)}
        class="hover:bg-accent flex h-6 w-6 shrink-0 cursor-pointer items-center justify-center rounded"
        aria-label="Dismiss"
      >
        <X size={12} />
      </button>
    {/if}
  </div>

  {#if task.state === 'downloading' || task.state === 'extracting'}
    <div class="bg-secondary h-1.5 w-full overflow-hidden rounded-full">
      <div
        class="bg-primary h-full rounded-full transition-all duration-150 ease-out"
        style="width: {summary.progressPercent}%"
      ></div>
    </div>
  {/if}

  {#if installPlan.length > 0}
    <div class="border-border/70 bg-muted/30 rounded-md border px-2 py-1.5">
      <div class="mb-1.5 flex items-start justify-between gap-2">
        <div class="min-w-0">
          <p class="text-foreground text-[11px] font-semibold">Preflight passed</p>
          <p class="text-muted-foreground text-[11px]">
            {planSummary}
            {#if summary.expectedSizeLabel}
              · {summary.expectedSizeLabel}
            {/if}
          </p>
        </div>
        <div class="flex shrink-0 gap-1">
          {#if planCounts.add > 0}
            <span class="rounded border px-1.5 py-0.5 text-[10px]">+{planCounts.add}</span>
          {/if}
          {#if planCounts.replace > 0}
            <span class="border-warning/40 bg-warning/10 rounded border px-1.5 py-0.5 text-[10px]"
              >R{planCounts.replace}</span
            >
          {/if}
        </div>
      </div>
      <div class="space-y-1">
        {#each installPlan as item (item.folderName)}
          <p class="text-muted-foreground flex items-center justify-between gap-2 text-[11px]">
            <span class="min-w-0 truncate">
              <span class="text-foreground font-mono">{item.folderName}</span>
              {#if item.reason}
                <span class="opacity-80"> · {item.reason}</span>
              {/if}
            </span>
            <span class="shrink-0 font-medium">{item.action === 'replace' ? 'Replace' : 'Add'}</span>
          </p>
        {/each}
      </div>
      <p class="text-muted-foreground mt-1.5 text-[11px]">{safetyNote}</p>
    </div>
  {/if}

  {#if task.error}
    <p class="text-destructive text-xs">{task.error}</p>
  {/if}

  {#if recovery}
    <div class="border-border/70 bg-muted/30 rounded-md border px-2 py-1.5">
      <p class="text-foreground text-[11px] font-semibold">{recovery.title}</p>
      <p class="text-muted-foreground mt-0.5 text-[11px]">{recovery.action}</p>
      <details class="mt-1">
        <summary class="text-muted-foreground cursor-pointer text-[11px]">Diagnostics</summary>
        <pre class="mt-1 max-h-24 overflow-auto rounded bg-background p-2 text-[10px] whitespace-pre-wrap">{recovery.diagnostics}</pre>
      </details>
    </div>
  {/if}
</div>
