<svelte:options runes />

<script lang="ts">
  import AlertTriangle from 'lucide-svelte/icons/alert-triangle';
  import CheckCircle from 'lucide-svelte/icons/check-circle';
  import Info from 'lucide-svelte/icons/info';
  import ShieldCheck from 'lucide-svelte/icons/shield-check';
  import { Badge, Button } from '$lib/components/ui';
  import type { AddonHealthIssue, AddonHealthSummary } from '$lib/addons/health';

  interface Props {
    summary: AddonHealthSummary;
    busy?: boolean;
    oninstallrequired?: () => void;
    onqueueupdates?: () => void;
  }

  const { summary, busy = false, oninstallrequired, onqueueupdates }: Props = $props();

  function preview(values: string[]): string {
    if (values.length === 0) return '';
    const shown = values.slice(0, 4).join(', ');
    return values.length > 4 ? `${shown}, +${values.length - 4} more` : shown;
  }

  function runAction(issue: AddonHealthIssue) {
    if (issue.key === 'missing-required') {
      oninstallrequired?.();
      return;
    }
    if (issue.key === 'outdated') {
      onqueueupdates?.();
    }
  }
</script>

<section class="bg-muted/20 border-border rounded-lg border p-3">
  <div class="flex flex-wrap items-center justify-between gap-2">
    <div class="flex min-w-0 items-center gap-2">
      <div class="bg-background flex h-8 w-8 shrink-0 items-center justify-center rounded-md border">
        <ShieldCheck size={16} class="text-primary" />
      </div>
      <div class="min-w-0">
        <p class="text-foreground text-sm font-semibold">Local addon health</p>
        <p class="text-muted-foreground text-xs">
          {#if summary.issueCount === 0}
            No local dependency, update, unknown-folder, or manifest issues detected.
          {:else}
            {summary.issueCount} local issue{summary.issueCount === 1 ? '' : 's'} detected from installed files and cached metadata.
          {/if}
        </p>
      </div>
    </div>
    <Badge variant={summary.issueCount === 0 ? 'success' : 'warning'} class="text-xs">
      {summary.issueCount === 0 ? 'Healthy' : `${summary.issueCount} issue${summary.issueCount === 1 ? '' : 's'}`}
    </Badge>
  </div>

  {#if summary.issues.length > 0}
    <div class="mt-3 grid gap-2 lg:grid-cols-2">
      {#each summary.issues as issue (issue.key)}
        <div class="bg-background/70 border-border rounded-md border p-3">
          <div class="flex items-start justify-between gap-2">
            <div class="min-w-0">
              <p class="text-foreground flex items-center gap-1.5 text-xs font-semibold">
                {#if issue.severity === 'warning'}
                  <AlertTriangle size={13} class="text-warning" />
                {:else}
                  <Info size={13} class="text-primary" />
                {/if}
                {issue.title}
              </p>
              <p class="text-muted-foreground mt-1 text-xs">{issue.description}</p>
            </div>
            <Badge variant={issue.severity === 'warning' ? 'warning' : 'outline'} class="shrink-0 text-xs">
              {issue.count}
            </Badge>
          </div>

          {#if issue.dependencyFolderNames.length > 0}
            <p class="text-muted-foreground mt-2 truncate text-[11px]">
              Dependencies: <span class="font-mono">{preview(issue.dependencyFolderNames)}</span>
            </p>
          {/if}
          {#if issue.addonFolderNames.length > 0}
            <p class="text-muted-foreground mt-1 truncate text-[11px]">
              Addons: <span class="font-mono">{preview(issue.addonFolderNames)}</span>
            </p>
          {/if}

          {#if issue.actionLabel}
            <Button
              type="button"
              variant="outline"
              size="sm"
              class="mt-3"
              onclick={() => runAction(issue)}
              disabled={busy}
            >
              {issue.actionLabel}
            </Button>
          {/if}
        </div>
      {/each}
    </div>
  {:else}
    <div class="mt-3 flex items-center gap-2 text-xs text-muted-foreground">
      <CheckCircle size={13} class="text-success" />
      Installed addons and dependency declarations look consistent.
    </div>
  {/if}
</section>
