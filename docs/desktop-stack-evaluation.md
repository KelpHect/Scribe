# Desktop Stack Evaluation

Last updated: 2026-05-18

## Decision

Keep Scribe on the current Wails + Svelte 5 stack until profiling proves either Wails or Svelte is the bottleneck. The highest-value work remains startup scanning, ESOUI cache behavior, bridge event volume, catalog indexing, virtualized list rendering, install safety, and clearer recovery flows.

Do not start a rewrite from this document alone. Any shell or framework migration needs a spike with the same fixture workflows, measurements, and rollback plan.

## Current Baseline

- Backend/domain: Go, Wails-bound methods, SQLite cache/settings.
- Frontend: Svelte 5 runes, Vite, TanStack Query, virtualized lists.
- Linux shell: system WebKitGTK through Wails.
- Current measured hot-path work: indexed Find More filter/sort is below the frame budget in the 7k-addon fixture benchmark.

## Evaluation Matrix

| Option | Startup | Memory | Package size | Linux/Fedora dependency story | Windows behavior | Runtime ownership | Native API access | Bridge replacement cost | Regression risk | Fit for Scribe |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Wails + Svelte 5 | Good when startup scan and remote refresh are backgrounded. | Lowest-risk current path because Go runtime plus system webview is already measured locally. | Small relative to Chromium-bundled shells. | Depends on GTK/WebKitGTK packages; Fedora needs `gtk3-devel` and `webkit2gtk4.1-devel` with current build tags. | Already wired in CI/release flow. | Uses OS webview; Linux behavior depends on distro WebKitGTK. | Existing Go filesystem, SQLite, archive, and Wails runtime calls stay intact. | None. | Lowest. Existing app behavior, tests, and packaging stay valid. | Best default. Keep optimizing. |
| Wails + SolidJS | Similar shell startup; frontend may reduce reactive churn in some views. | Potentially lower UI update overhead, but only if Svelte hot paths remain a measured bottleneck. | Likely comparable or smaller frontend bundle, but app package still Wails/system webview. | Same Wails/Linux WebKitGTK constraints. | Same Wails behavior. | Same OS webview. | Existing Go API can remain, but frontend stores/routes/components are rewritten. | Medium. Frontend service shape can stay, UI state must be ported. | Medium-high. Rebuilds UI without solving backend/cache/install issues. | Spike only after Svelte hot paths exceed budget. |
| Tauri + Svelte/Solid | Potentially fast shell startup, but Rust side and IPC must be rebuilt. | Usually attractive because it uses platform webviews, but Linux still depends on WebKitGTK. | Usually small because it does not bundle Chromium. | Tauri v2 Linux prerequisites still include WebKitGTK development packages. Fedora/KDE behavior still needs validation. | New Windows packaging/runtime path. | Uses system webview; Rust/Tauri owns shell. | Requires porting Wails runtime calls and Go-native filesystem/archive/cache bridge to Rust commands or sidecar Go. | High. Wails bindings, events, build, release scripts, and native integrations change. | High. Many working app paths are replaced at once. | Only if Wails itself is proven to block stability or packaging. |
| Electron + Svelte/Solid | Predictable Chromium startup, but heavier than system-webview shells. | Higher baseline memory is expected because Electron uses Chromium multi-process architecture plus Node runtime. | Larger package because Chromium/runtime are bundled. | Avoids distro WebKitGTK mismatches, which may improve Linux support predictability. | Strong desktop tooling and predictable Chromium behavior. | App owns Chromium and Node versions through Electron. | Native APIs move to Node/Electron main process or a Go sidecar. | High. Wails bridge and release flow must be replaced. | High, but runtime predictability may help if WebKitGTK is the true source of issues. | Consider only if WebKitGTK instability outweighs memory/package cost. |
| Avalonia/C# native | Removes webview dependency entirely. | Could be good, but unknown without a real implementation. | Native .NET app artifacts; different deployment profile. | New .NET/Avalonia Linux validation burden. | New Windows native stack. | App owns native UI stack. | Requires rewriting backend/domain logic or embedding Go. | Very high. This was already attempted and rolled back. | Very high. Full parity is expensive. | Not active. Do not revive without a new accepted plan. |

## Required Spike Measurements

Every framework or shell spike must use the same scenarios:

- Cold launch to first usable window.
- Warm launch with existing catalog/cache/settings.
- Memory at idle, after Find More browse, after detail browsing, and during concurrent downloads.
- Find More search/filter/sort latency with the fixture catalog.
- Installed and Find More scroll smoothness with images enabled.
- Task center responsiveness during concurrent downloads and extraction.
- Install/update preflight, rollback, failure, retry, and cancellation behavior.
- Package size and produced artifacts.
- Fedora KDE, Debian/Ubuntu, and Windows smoke behavior.

## Decision Rules

- SolidJS is justified only if Svelte route/store hot paths exceed frame budget after indexed helpers, batching, and virtualization fixes.
- Tauri is justified only if Wails bridge or shell behavior is proven to be the bottleneck and the Rust/IPC replacement cost is accepted.
- Electron is justified only if owning Chromium materially improves stability or Linux support enough to outweigh memory and package-size cost.
- Native rewrite work is out of scope unless there is a new accepted plan and a strict parity-first milestone.

## SolidJS Spike Decision

Status: not run, not kept.

Reason: the current Svelte hot paths now have baselines, and the optimized Find More indexed filter/sort benchmark is under frame budget for the 7k-addon fixture catalog. A SolidJS port would still need to rebuild route state, stores, component behavior, and Wails service wrappers while not addressing backend install/cache/download bottlenecks. That migration cost is not justified until Svelte-specific rendering remains over budget after the current helper extraction, event batching, and virtual-list work.

Trigger to revisit: a reproducible profile shows Svelte route/store rendering, not backend work, bridge events, images, or filtering helpers, as the dominant cause of search, scroll, or task-center jank.

## Alternate Shell Spike Decision

Status: not run, not kept.

Reason: no current baseline proves Wails itself is the bottleneck. The measured and recently fixed problems are in Scribe-owned paths: startup scan timing, catalog indexing, bridge progress-event volume, image/list behavior, install preflight clarity, stale artifact cleanup, dependency resolution, and recovery messaging. A Tauri, Electron, or custom-shell spike would replace bindings, native runtime calls, release scripts, and Linux/Windows packaging without first showing that the shell is what blocks responsiveness or stability.

Trigger to revisit: a reproducible profile or support case isolates Wails/WebKitGTK bridge or shell behavior as the dominant problem after Scribe-owned startup, cache, filtering, progress, image, and install paths are within budget.

## Sources

- Wails installation and Linux dependency notes: https://wails.io/docs/gettingstarted/installation/
- Wails Linux guide: https://wails.io/docs/v2.10/guides/linux/
- Tauri v2 prerequisites: https://v2.tauri.app/start/prerequisites/
- Electron process model: https://www.electronjs.org/docs/latest/tutorial/process-model
- SolidJS fine-grained reactivity: https://docs.solidjs.com/advanced-concepts/fine-grained-reactivity
