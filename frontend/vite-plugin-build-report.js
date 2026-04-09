import { writeFileSync } from 'node:fs';
import { join, basename } from 'node:path';

const BUDGETS = {
  'index.js': 20_000,
  'index.css': 55_000,
  'route-find-more.js': 250_000,
  'route-settings.js': 80_000,
  'route-installed.js': 20_000,
  'route-updates.js': 10_000,
  'vendor-ui.js': 30_000,
  'vendor-ui.css': 20_000,
  'vendor-icons.js': 5_000,
  'vendor-tanstack-data.js': 5_000,
  'vendor-tanstack-ui.js': 10_000,
  'vendor-svelte.js': 15_000,
};

const TOTAL_GZIP_BUDGET = 500_000;

function matchBudget(fileName) {
  if (fileName.includes('index-') && fileName.endsWith('.js')) return 'index.js';
  if (fileName.includes('index-') && fileName.endsWith('.css')) return 'index.css';
  if (fileName.includes('route-find-more')) return 'route-find-more.js';
  if (fileName.includes('route-settings')) return 'route-settings.js';
  if (fileName.includes('route-installed')) return 'route-installed.js';
  if (fileName.includes('route-updates')) return 'route-updates.js';
  if (fileName.includes('vendor-ui') && fileName.endsWith('.js')) return 'vendor-ui.js';
  if (fileName.includes('vendor-ui') && fileName.endsWith('.css')) return 'vendor-ui.css';
  if (fileName.includes('vendor-icons')) return 'vendor-icons.js';
  if (fileName.includes('vendor-tanstack-data')) return 'vendor-tanstack-data.js';
  if (fileName.includes('vendor-tanstack-ui')) return 'vendor-tanstack-ui.js';
  if (fileName.includes('vendor-svelte')) return 'vendor-svelte.js';
  return null;
}

export default function buildReportPlugin() {
  return {
    name: 'build-report',
    writeBundle(options, bundle) {
      const chunks = [];
      let totalJs = 0;
      let totalCss = 0;
      let totalGzipJs = 0;
      let totalGzipCss = 0;
      const violations = [];

      for (const [fileName, output] of Object.entries(bundle)) {
        if (output.type === 'chunk' || output.type === 'asset') {
          const size = output.type === 'chunk' ? output.code.length : output.source.length;
          const gzipSize = output.type === 'chunk' ? output.code.length : output.source.length;
          const isJs = fileName.endsWith('.js');
          const isCss = fileName.endsWith('.css');

          if (isJs) {
            totalJs += size;
          } else if (isCss) {
            totalCss += size;
          }

          const budgetKey = matchBudget(fileName);
          const budget = budgetKey ? BUDGETS[budgetKey] : null;

          if (budget && size > budget) {
            violations.push({
              file: fileName,
              size,
              budget,
              overBy: size - budget,
            });
          }

          chunks.push({
            file: fileName,
            size,
            type: isJs ? 'js' : isCss ? 'css' : 'other',
            budget: budget ?? null,
            withinBudget: budget ? size <= budget : null,
          });
        }
      }

      const report = {
        timestamp: new Date().toISOString(),
        totalJsBytes: totalJs,
        totalCssBytes: totalCss,
        totalBytes: totalJs + totalCss,
        totalGzipBudgetBytes: TOTAL_GZIP_BUDGET,
        chunks: chunks.sort((a, b) => b.size - a.size),
        violations,
        budgets: BUDGETS,
      };

      const reportPath = join(options.dir ?? 'dist', 'build-report.json');
      writeFileSync(reportPath, JSON.stringify(report, null, 2));

      if (violations.length > 0) {
        console.warn('\n[budget] Bundle size violations:');
        for (const v of violations) {
          console.warn(`  ${v.file}: ${(v.size / 1024).toFixed(1)}KB (budget: ${(v.budget / 1024).toFixed(1)}KB, +${(v.overBy / 1024).toFixed(1)}KB over)`);
        }
      } else {
        console.log('[budget] All chunks within size budgets ✓');
      }
    },
  };
}
