# Scribe product design: The Tamrielic Ledger

> **Status note (2026 UI rework):** the presentation direction in this document
> is **superseded** by [`docs/ui-rework-design.md`](docs/ui-rework-design.md)
> ("Scribe Glass"). All backend, persistence, networking, install-safety, and
> performance constraints recorded here remain in force; colors, shell layout,
> and component styling are now governed by the Scribe Glass brief and the
> tokens in `crates/scribe-app/src/theme.rs`. The historical content below is
> preserved for reference.

Status: accepted implementation direction for the ground-up 2026 UI rebuild.

This document was written before implementation. It records the evidence, alternatives, design system, interaction rules, responsive behavior, and build sequence for the new Scribe experience. The Rust core, GPUI, `gpui-component`, MMOUI integration, and filesystem safety contract remain in place; the previous presentation shell does not.

## Product promise

Scribe is a fast, trustworthy Windows desktop steward for an ESO addon library. It should make three jobs feel effortless:

1. Understand what is installed and whether it is healthy.
2. Find an addon and judge it without leaving the flow.
3. Apply an install, update, repair, or uninstall with the consequence made explicit.

The product personality is archival rather than medieval-fantasy cosplay: precise, calm, tactile, and recognizably connected to Tamriel through materials and cartographic detail. Safety must be visible without turning the app into an operations console.

## Evidence gathered

### Current Rust application inventory

- Library: category-grouped installed addons, collapsible chapter headers, category art, search, details, update and uninstall, bulk selection, required and optional dependency notices, and context menus.
- Discover: virtual catalog, search, searchable category picker, compatibility selector, sort and direction, hide-installed, refresh, detail inspection, install, and ESOUI links.
- Maintenance: available-update filtering, update all, refresh, and clear current-state messaging.
- Dossiers: local and remote metadata, authorship, version/API/folder/category/statistics/dates, dependency state, bounded screenshots, full-window lightbox, description, changelog, website links, and uninstall confirmation.
- Activity: immediate task transitions, throttled progress, cancellation, retry, dismiss, and failure text.
- Workbench: AddOns folder selection, rescan/open/copy, appearance, catalog/storage/scan health, explicit cache rebuilding, diagnostics, performance counters, and project/provider information.
- Cross-cutting states: first-run/no-folder, empty, filtered-empty, loading, stale cache, offline, degraded storage, scan failure, install/update/uninstall progress, success, failure, and cancellation.
- Input: pointer, Enter/Space activation, Escape dismissal, lightbox arrows, context-menu keyboard access, page shortcuts, search focus, and settings shortcut.

### Previous Svelte application inventory

The retired UI also exposed keyboard-shortcut help, richer diagnostics/about copy, task-queue history, dependency-health panels, optional-dependency handling, and recovery guidance. Those are functional precedents, not visual templates. The new app preserves or improves their useful outcomes without restoring the old web shell.

### Real-window findings

The current release build was inspected at approximately 1024 x 640, 1120 x 770, and 2560 x 1392, with populated, clean-profile, offline, storage-degraded, install-success, dependency, and image-failure states. Test profiles and test AddOns folders were isolated under `target`; real addon data was not modified.

Material problems found:

- The dark sidebar, title bar, page heading, and contextual toolbar all compete to announce hierarchy.
- The discovery sort surface can open upward into or clip against the title region.
- A modal visually covers the application while underlying page controls remain represented in the accessibility tree.
- At ultrawide sizes, row metadata and row actions can be separated by more than a thousand pixels.
- The persistent category filter is easy to forget and makes a later catalog visit feel unexpectedly incomplete.
- Settings is a long stack of visually identical bordered cards, so importance and recovery risk are difficult to scan.
- The title-bar status can say that the archive is connected while local storage is degraded; offline empty-cache text can claim cached data remains available when none exists.
- Fast tasks can disappear before a person sees durable success feedback.
- Row-level detail activation and child install affordances compete; an apparent install target can lead into detail loading/failure instead of a clear install result.
- The first-run experience is an empty state plus a settings detour rather than a focused setup path.
- Warm paper surfaces, black chrome, repeated rounded panels, and tiny uppercase metadata make the product feel themed but not structurally distinctive.

## External references

The design uses primary or official guidance:

- [GPUI architecture and testing](https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md): retained application state with immediate-mode rendering, custom elements where warranted, actions, async integration, and testability.
- [GPUI API documentation](https://docs.rs/gpui/latest/gpui/): focus, window state, accessibility, and virtual-list capabilities.
- [`gpui-component` component catalogue](https://longbridge.github.io/gpui-component/docs/components/): current title bar, dialog, focus trap, list, popover, resizable, sheet, tabs, settings, and virtual-list patterns.
- [`gpui-component` release history](https://github.com/longbridge/gpui-component/releases): recent modal/background-interaction, popover, list, select, and title-bar fixes are reasons to prefer direct current APIs over preserving local abstractions.
- [Windows accessibility overview](https://learn.microsoft.com/en-us/windows/apps/design/accessibility/accessibility-overview) and [keyboard interaction guidance](https://learn.microsoft.com/en-us/windows/apps/develop/input/keyboard-interactions): logical focus order, clear focus, and standard Enter/Space/Escape behavior.
- [Windows responsive layout guidance](https://learn.microsoft.com/en-us/windows/apps/design/layout/) and [desktop app best practices](https://learn.microsoft.com/en-us/windows/apps/get-started/best-practices): resize continuously and validate across snap-sized windows.
- [Windows settings guidance](https://learn.microsoft.com/en-us/windows/apps/design/app-settings/guidelines-for-app-settings): settings is supporting utility, not a peer to the product's primary jobs.
- [WCAG 2.2 target size](https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum), [focus appearance](https://www.w3.org/WAI/WCAG22/Understanding/focus-appearance.html), and [contrast](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html): minimum targets, visible perimeter, and contrast requirements.
- [ESO AddOn support](https://help.elderscrollsonline.com/app/answers/detail/a_id/10321/~/how-do-i-install-/-uninstall-add-ons) and [Minion's official workflow](https://www.minion.gg/): users expect install/update/manage workflows, but file removal deserves explicit care.
- Official ESO [concept art](https://www.elderscrollsonline.com/en-us/media/category/concept-art/), [wallpapers](https://www.elderscrollsonline.com/en-us/media/category/wallpapers/), [One Tamriel](https://www.elderscrollsonline.com/en-us/updates/update/onetamriel), and [Murkmire](https://www.elderscrollsonline.com/en-us/updates/dlc/murkmire) references informed material and color choices without copying the game's interface.

## Directions considered

### A. The Tamrielic Ledger — selected

A shallow horizontal command deck contains the three primary jobs: Library, Discover, and Maintenance. Beneath it, one contextual instrument strip serves the active job. The main workspace is a centered, ruled ledger with compact chapter bands and rows. Details open as a dossier sheet; Activity remains a floating dispatch tray; Settings opens as a utility workbench.

Why it wins: it removes the sidebar, keeps high-frequency navigation visible, gives 1024-pixel windows their horizontal space back, constrains ultrawide reading distance, supports virtual rows, and maps naturally to Scribe's archive/stewardship identity.

### B. The Cartographer's Atlas — not selected

A visual category map, spatial tiles, and map-like zoom would make discovery memorable. It was rejected as the product shell because it privileges browsing over the daily installed/update jobs, makes keyboard traversal less predictable, and risks turning large-catalog performance into a layout problem. Cartographic motifs remain as quiet texture and wayfinding, not navigation geometry.

### C. The Addon Workshop — not selected

A permanent master list plus resizable inspector would be efficient for power users. It was rejected as the universal shell because it compresses both panes at snap widths, keeps an inspector visible when no decision is being made, and encourages dense control panels. Its useful idea survives as an adaptive wide-window dossier, never as a mandatory split view.

## Chosen information architecture

### Global shell

- Component `TitleBar`, 34 px high, with the Scribe mark and truthful aggregate health language. The existing 46 x 34 native Windows-control overlay remains above it.
- Command deck, 66 px high, with product context at left, three horizontal destination tabs in the center/left, and Workbench plus Activity utilities at right.
- Context strip, normally 52 px and allowed to become two rows in compact mode. It contains search and only controls relevant to the active destination.
- Ledger workspace fills the remaining height. There is no global sidebar, duplicated wordmark, docked task center, or generic page-title card.

### Destinations

- **Library** replaces Installed. Its chapters are MMOUI categories; dependency calls-to-action sit immediately above the affected chapters. Normal mode prioritizes details and one-addon actions. Selection is an explicit temporary mode.
- **Discover** replaces Find More. Search, category, compatibility, sort, and installed visibility form one instrument strip. Active non-default filters appear as removable tokens so state cannot hide silently.
- **Maintenance** replaces Updates. It is an operational summary: available updates first, dependency repair second, then recent local activity. When there is nothing to do, it explains that the library is current and offers catalog refresh/browse actions.
- **Workbench** replaces Settings as a utility surface. It is not a fourth peer tab. At wide widths it uses a 240 px section index and a bounded reading pane; at compact widths the index becomes a horizontal section switcher. Sections are Library, Health & recovery, Preferences, and About & diagnostics.

### Dossiers and confirmations

- At widths of 1320 px or more, addon details use a trailing dossier sheet up to 720 px wide with a restrained backdrop. At smaller widths, the dossier becomes a full-workspace sheet below the title bar.
- The screenshot rail precedes description/changelog. Images remain bounded, lazy, and failure-safe. Lightboxes remain final root children.
- Destructive confirmations use a compact centered decision dialog over the dossier/workspace, with the addon folders and rollback implications named.
- While a modal, dossier, or lightbox owns focus, the underlying surface is removed from pointer and keyboard traversal as well as visually obscured.

### Activity and feedback

- Activity is a bottom-right dispatch tray, no wider than 420 px. Its collapsed seal reports the number and strongest state: working, attention, or complete.
- Task state changes render immediately; repeated progress counters stay throttled.
- Success remains visible for at least a short acknowledgement interval and can be dismissed. Failure persists until retry/dismiss. Fast operations therefore do not vanish without confirmation.
- Global notices state actual capability: online refresh availability, cached catalog availability, storage availability, and scan configuration are separate facts.

## Visual system

### Color tokens

The palette is based on ink, aged bone, tarnished brass, lapis, verdigris, and sealing wax. Decorative use is restrained; semantic use is consistent.

| Token | Value | Use |
| --- | --- | --- |
| `ink-950` | `#101519` | title bar, command deck, primary text on light material |
| `ink-850` | `#182026` | raised dark surfaces |
| `iron-700` | `#263039` | dark hover and selected wells |
| `bone-100` | `#F4ECD9` | high-emphasis text on dark surfaces |
| `bone-200` | `#E8DDC7` | ledger background |
| `stone-300` | `#D8CDB8` | muted dark-surface text and dividers |
| `brass-500` | `#C9974F` | focus, selection, primary accent |
| `verdigris-600` | `#2E746E` | connected/healthy state with light text |
| `lapis-600` | `#315B7D` | information state with light text |
| `wax-600` | `#90464A` | destructive/error state with light text |

Verified WCAG contrast pairs include bone on ink 15.61:1, muted stone on ink 11.67:1, ink on ledger 13.64:1, ink on brass 7.02:1, bone on verdigris 4.65:1, bone on lapis 6.11:1, and bone on wax 5.65:1. Verdigris on ink is decorative only because it is 3.36:1.

### Type

- Windows system UI face (`Segoe UI Variable` with `Segoe UI` fallback) for all interface text; `Consolas` only for paths and copied diagnostics.
- Scale: 11 px metadata, 12 px secondary/control, 13 px row/body, 15 px emphasized row/title, 20 px section title, 28 px empty/onboarding title.
- Uppercase is limited to short eyebrow labels. Body, controls, statuses, and navigation use sentence case.
- Line-height and wrapping take priority over squeezing text into a single line. Action labels may not be truncated without an accessible full label.

### Space, geometry, and elevation

- Base spacing unit: 4 px. Core rhythm: 4, 8, 12, 16, 24, 32, and 40 px.
- Pointer targets: 32 px minimum for dense desktop utilities, 40 px for primary actions and destination tabs, 46 x 34 px for Windows controls. No icon-only action relies on a sub-24 px hit target.
- Radii: 2 px for rules/tags, 6 px for controls, 10 px only for sheets/dialogs. Rows are ruled, not individually carded.
- Elevation: border and surface shift first; one restrained shadow for sheets/dialogs. The UI must not become a stack of floating rounded cards.
- Icons: direct `gpui-component`/Lucide icons at 16, 18, or 20 px. MMOUI category artwork remains the preferred content image; category-aware semantic art remains the fallback.
- Decorative motif: one-pixel etched rules, sparse compass ticks, and category color notches. No faux parchment noise behind dense text.

### Motion

- 90 ms press/hover response, 140 ms popover, 180 ms dossier transition, and no gratuitous page translation.
- Reduced-motion mode removes nonessential interpolation and keeps only immediate state change.
- Loading uses stable geometry and restrained progress; no layout-jumping skeletons.

## Responsive rules

The minimum supported window remains 1024 x 640.

- **Compact, 1024–1199 px:** command deck labels remain visible; contextual controls may form two rows; Workbench sections become a horizontal switcher; dossiers occupy the workspace; optional metadata columns collapse before actions do.
- **Standard, 1200–1679 px:** one-row instrument strip where content permits; ledger max width about 1280 px; dossier uses width-aware overlay behavior.
- **Wide, 1680 px and above:** ledger max width 1560 px; row content uses explicit columns so the primary action stays within about 220 px of the decision metadata; details use the trailing dossier. Extra width becomes intentional breathing room, never an unbounded gap inside a row.
- Popovers calculate from trigger and viewport, prefer opening below, and clamp inside the content viewport without crossing the title bar.
- Every responsive breakpoint is tested through continuous resize, Windows snap-like widths, maximized ultrawide, and keyboard traversal.

## Accessibility and interaction contract

- Focus order follows visual order: title utility, destinations, context controls, workspace, Activity, then active overlay.
- A 2 px brass focus perimeter with at least 3:1 adjacent contrast is used on every interactive element; focus never depends on color fill alone.
- The focus brass is darkened to `#9C6D32` against light ledger surfaces, providing 3.36:1 against ledger and 4.06:1 against ink.
- Rows expose one clear row-details action. Child buttons stop pointer and keyboard propagation.
- Enter and Space activate focused buttons/rows; Escape dismisses the topmost temporary layer; arrows move within menus, category choices, tabs where applicable, and the lightbox.
- Destination tabs expose selected state. Filters expose names and current values. Icon-only actions always have accessible labels and tooltips.
- Empty, loading, stale, offline, degraded, progress, success, failure, and cancellation states use role/status semantics without repeated noisy announcements.
- Modal/dossier/lightbox focus is trapped and restored to its invoker. Covered controls are unavailable to accessibility traversal.
- Color is always paired with icon and text. Text and essential controls satisfy WCAG AA contrast; critical focus and state pairs are regression-tested.

## State language

- **Online, cached:** “Catalog current” with last refresh time.
- **Offline, cached:** “Offline · showing saved catalog” with age and retry.
- **Offline, no cache:** “Catalog unavailable offline” with a concrete retry explanation; never claim saved data exists.
- **Storage degraded:** “Local archive unavailable” independent of catalog network state; offer the explicit rebuild workflow without overwriting retained data.
- **No folder:** focused three-step setup: choose AddOns folder, scan, review detected addons. Settings remains available but is not required as a detour.
- **Task success:** name the completed operation and affected addon/folders. **Task failure:** name the safe retained state plus retry/details. **Cancellation:** name what was rolled back or left unchanged.

## Implementation sequence

1. Freeze the evidence above and add token/contrast/responsive regression tests.
2. Replace the sidebar/page-title shell with TitleBar + command deck + context strip + ledger workspace.
3. Convert navigation, status language, and active-filter disclosure; verify keyboard focus and 1024/ultrawide layouts.
4. Recompose Library, Discover, and Maintenance around ruled chapter/row primitives while retaining virtualization and category artwork.
5. Replace centered detail cards with adaptive dossiers; enforce focus isolation and pointer propagation.
6. Recompose Workbench, first-run setup, health/recovery, empty/loading/offline/degraded states, and durable Activity feedback.
7. Remove obsolete sidebar/card/toolbar presentation helpers and duplicated constants; keep only direct component APIs or documented Scribe-specific primitives.
8. Re-run real-window inspection after each major shell/page/overlay stage, then complete formatting, lint, tests, release build, dependency policy, diff checks, performance fixtures, and documentation sync.

## Acceptance bar

The rebuild is complete only when the previous sidebar-and-stacked-card product is no longer visible, all existing safety-critical workflows remain available, the minimum and ultrawide layouts are coherent, keyboard/focus behavior survives overlays, every major state has truthful language and an action, virtual catalog behavior remains bounded, release-mode performance evidence is captured, and the final release build is left running for inspection.

## Verified implementation evidence

The implemented release build was inspected in the real Windows window at 1122 x 769 and maximized 2560 x 1392. Compact and wide Workbench navigation, downward-clamped sort placement, virtualized Discover rows, full-workspace and trailing-sheet dossiers, repeated Tab traversal, Escape restoration, and rapid Discover-to-Workbench navigation during an in-flight details request all behaved as specified. The same run covered live, cached, offline, no-cache, storage-degraded, empty-library, and configured-library presentation without modifying a real addon folder.

The warnings-denied workspace gate passes formatting, Clippy, all 72 tests, the locked release build, `cargo deny`, and diff sanity. The release-mode UI fixture passes all 20 interaction/layout tests. The disposable Windows acceptance profile passes embedded assets, first frame, catalog readiness, redb creation, graceful close, restart, settings stability, and cleanup. On this host, first-launch/restart first frames measured 129.9/123.8 ms; working sets measured 78.2/78.3 MiB. These are local acceptance figures rather than general release thresholds.
