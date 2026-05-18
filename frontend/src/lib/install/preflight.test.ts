import { describe, expect, it } from 'vitest';
import {
  formatInstallPlanSummary,
  getInstallPlanCounts,
  getInstallPlanSafetyNote
} from './preflight';

describe('install preflight presentation helpers', () => {
  it('summarizes add and replace folder actions', () => {
    const plan = [
      { folderName: 'LibFoo', action: 'add', reason: 'folder is not installed' },
      { folderName: 'SkyShards', action: 'replace', reason: 'folder already exists' }
    ] as const;

    expect(getInstallPlanCounts(plan)).toEqual({ add: 1, replace: 1, total: 2 });
    expect(formatInstallPlanSummary(plan)).toBe('1 add · 1 replace');
  });

  it('explains rollback behavior when replacements are planned', () => {
    expect(
      getInstallPlanSafetyNote([
        { folderName: 'SkyShards', action: 'replace', reason: 'folder already exists' }
      ])
    ).toContain('restored if the install fails');
  });

  it('explains staging behavior before a plan exists', () => {
    expect(formatInstallPlanSummary()).toBe('No folder changes planned yet');
    expect(getInstallPlanSafetyNote()).toContain('before touching AddOns');
  });
});
