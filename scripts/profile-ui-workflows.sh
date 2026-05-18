#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

OUT_DIR="${OUT_DIR:-build/reports/ui-profile}"
mkdir -p "$OUT_DIR"

STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
REPORT="$OUT_DIR/ui-profile-$STAMP.md"
LATEST="$OUT_DIR/latest.md"
TEST_LOG="$OUT_DIR/vitest-$STAMP.log"
BENCH_LOG="$OUT_DIR/frontend-bench-$STAMP.log"
BUILD_LOG="$OUT_DIR/frontend-build-$STAMP.log"

echo "== Frontend workflow smoke tests =="
npm --prefix frontend run test -- --run | tee "$TEST_LOG"

echo
echo "== Frontend catalog/profile benchmarks =="
npm --prefix frontend run bench -- --run | tee "$BENCH_LOG"

echo
echo "== Frontend build and bundle report =="
npm --prefix frontend run build | tee "$BUILD_LOG"

node - "$REPORT" "$TEST_LOG" "$BENCH_LOG" "$BUILD_LOG" <<'NODE'
const fs = require('node:fs');
const [reportPath, testLog, benchLog, buildLog] = process.argv.slice(2);
const buildReportPath = 'frontend/dist/build-report.json';
const buildReport = fs.existsSync(buildReportPath)
  ? JSON.parse(fs.readFileSync(buildReportPath, 'utf8'))
  : null;

const workflowRows = [
  ['Installed', 'addon health, update decisions, install queue guards', 'frontend/src/lib/addons/*.test.ts; frontend/src/lib/stores/install-queue.test.ts'],
  ['Find More', 'catalog status, search ranking, filter metadata benchmark, interaction timing probes', 'frontend/src/lib/catalog/status.test.ts; frontend/src/lib/perf/remote-list.*; frontend/src/lib/diagnostics/frontend-perf.test.ts'],
  ['Updates', 'matched update-state normalization and update decision helpers', 'frontend/src/lib/services/esoui-service.test.ts; frontend/src/lib/addons/decision.test.ts'],
  ['Settings', 'diagnostics export shape, path redaction, frontend performance snapshot export', 'frontend/src/lib/diagnostics/*.test.ts'],
  ['Addon details', 'local health/update/dependency data used by detail surfaces', 'frontend/src/lib/addons/health.test.ts; frontend/src/lib/addons/decision.test.ts'],
  ['Remote addon details', 'remote catalog status, service normalization, search ranking metadata', 'frontend/src/lib/services/esoui-service.test.ts; frontend/src/lib/perf/remote-list.test.ts'],
  ['Dependency banners', 'missing dependency plan normalization and install queue dedupe', 'frontend/src/lib/services/esoui-service.test.ts; frontend/src/lib/stores/install-queue.test.ts'],
  ['Task center', 'retry filtering, active-task dedupe, progress event diagnostics', 'frontend/src/lib/stores/install-queue.test.ts; frontend/src/lib/diagnostics/frontend-perf.test.ts'],
  ['Failure/retry states', 'recoverable route errors, stale cache states, retryable install tasks', 'frontend/src/lib/routes/recovery.test.ts; frontend/src/lib/catalog/status.test.ts; frontend/src/lib/stores/install-queue.test.ts']
];

const lines = [];
lines.push('# UI Workflow Smoke/Profile Report');
lines.push('');
lines.push(`Generated at: ${new Date().toISOString()}`);
lines.push('');
lines.push('## Commands');
lines.push('');
lines.push('```bash');
lines.push('npm --prefix frontend run test -- --run');
lines.push('npm --prefix frontend run bench -- --run');
lines.push('npm --prefix frontend run build');
lines.push('```');
lines.push('');
lines.push('## Workflow Coverage Map');
lines.push('');
lines.push('| Workflow | Fixture/mock coverage | Test or benchmark path |');
lines.push('| --- | --- | --- |');
for (const row of workflowRows) {
  lines.push(`| ${row[0]} | ${row[1]} | \`${row[2]}\` |`);
}
lines.push('');
lines.push('## Generated Artifacts');
lines.push('');
lines.push(`- Test log: \`${testLog}\``);
lines.push(`- Benchmark log: \`${benchLog}\``);
lines.push(`- Build log: \`${buildLog}\``);
if (buildReport) {
  lines.push(`- Build report timestamp: \`${buildReport.timestamp}\``);
  lines.push(`- Total JS bytes: \`${buildReport.totalJsBytes}\``);
  lines.push(`- Total CSS bytes: \`${buildReport.totalCssBytes}\``);
  lines.push(`- Total bytes: \`${buildReport.totalBytes}\``);
  lines.push(`- Budget violations: \`${buildReport.violations?.length ?? 0}\``);
}
lines.push('');
lines.push('## Manual Desktop Follow-Up');
lines.push('');
lines.push('This script exercises mocked and fixture-backed workflow seams. It does not replace a real Wails desktop smoke pass.');
lines.push('');
lines.push('Before release, still launch the app and manually click through Installed, Find More, Updates, Settings, addon detail dialogs, dependency banners, task center retry/cancel, and diagnostics export.');
lines.push('');

fs.writeFileSync(reportPath, `${lines.join('\n')}\n`);
fs.copyFileSync(reportPath, 'build/reports/ui-profile/latest.md');
NODE

echo
echo "wrote $REPORT"
echo "updated $LATEST"
