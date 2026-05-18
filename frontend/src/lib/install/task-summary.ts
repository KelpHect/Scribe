import type { TaskProgress } from '$lib/stores/downloads.svelte';
import { formatBytes } from '$lib/utils';

export type TaskDisplaySummary = {
  stateLabel: string;
  speedLabel: string;
  sizeLabel: string;
  extractionLabel: string;
  isActive: boolean;
  isTerminal: boolean;
  progressPercent: number;
  expectedSizeLabel: string;
};

export function summarizeTaskProgress(task: TaskProgress): TaskDisplaySummary {
  return {
    stateLabel: getTaskStateLabel(task.state),
    speedLabel:
      task.state === 'downloading' && task.speed > 0 ? `${formatBytes(task.speed)}/s` : '',
    sizeLabel: getTaskSizeLabel(task),
    extractionLabel:
      task.state === 'extracting' && task.totalFiles > 0
        ? `${task.filesExtracted}/${task.totalFiles} files`
        : '',
    isActive:
      task.state === 'queued' ||
      task.state === 'planning' ||
      task.state === 'downloading' ||
      task.state === 'extracting',
    isTerminal: task.state === 'complete' || task.state === 'failed' || task.state === 'cancelled',
    progressPercent: Math.min(100, Math.max(0, task.percent)),
    expectedSizeLabel: task.totalBytes > 0 ? formatBytes(task.totalBytes) : ''
  };
}

function getTaskStateLabel(state: TaskProgress['state']): string {
  switch (state) {
    case 'queued':
      return 'Queued';
    case 'planning':
      return 'Planning install';
    case 'downloading':
      return 'Downloading';
    case 'extracting':
      return 'Extracting';
    case 'complete':
      return 'Complete';
    case 'failed':
      return 'Failed';
    case 'cancelled':
      return 'Cancelled';
    default:
      return '';
  }
}

function getTaskSizeLabel(task: TaskProgress): string {
  if (task.state !== 'downloading') return '';
  if (task.totalBytes > 0) {
    return `${formatBytes(task.bytesDownloaded)} / ${formatBytes(task.totalBytes)}`;
  }
  if (task.bytesDownloaded > 0) {
    return formatBytes(task.bytesDownloaded);
  }
  return '';
}
