# Iced launcher

## Current state

The repository is an unimplemented Rust 2024 binary. Cargo.toml has no
dependencies and src/main.rs prints Hello World. There are no tests or
pre-existing architecture to preserve. The installed airstrike iced skill and
guides are available locally.

## Target state

Running one executable opens a centered, undecorated dark launcher matching
the supplied image: search across the top, narrow left rail, pinned row,
divider, application grid. Search receives focus immediately. Typing filters
real installed applications; Enter launches the first visible result. Clicking
an application launches that application. Successful launch closes the launcher;
failure stays visible with a concise error. Escape closes it.

Initial platform assumption: Linux, matching this workspace and reference.
Keep OS integration isolated for later portability; do not claim Windows or
macOS application indexing. Session/power operations are outside the explicit
request and are not executed. Sidebar treatment must be honest about available
actions rather than offering misleading working-looking controls.

## Constraints

- Rust only, no unsafe application code, no service or resident process.
- User requests hardware rendering; enable wgpu primary with tiny-skia software
  fallback (explicitly approved). Verify adapters and final executable linkage.
- One executable, embedded default font and fallback assets; no installer or
  required app-owned sidecar resources. Host application metadata and icons are
  discovered resources, not bundled runtime requirements.
- Use iced and standard-library functionality; thiserror and tokio are approved
  stack crates if needed. No new third-party direct crates without permission.
- User approved installing iced master and running all MAP stages on 2026-10-28,
  in response to the plan and iced_test permission request. iced_test is approved.
- Pin iced and its test crate to one identical revision, record Cargo.lock.
- Prefer a fresh scan per invocation; indexing cache is unnecessary initially.
- No shell interpretation of desktop Exec. Launch using Command plus arguments.
- Do not install desktop integration files, services, global hotkeys, or modify
  system preferences. No registry access or session/power actions.

## Design

Use the iced skill's two-crate arrangement: root application and a small model
crate containing pure desktop-entry parsing, command expansion and search.
The root owns filesystem discovery, subprocess launch, and one launcher TEA
cell. Avoid speculative screen routing or generic component frameworks.

Open the window and focus search before asynchronous discovery completes.
Show loading, empty, no-match, and failure states. Preserve typed query when
indexing completes. Compute filtering in update, not repeatedly in view.

Scan XDG_DATA_HOME/applications and XDG_DATA_DIRS/applications recursively,
with documented defaults; skip directory symlinks to avoid cycles. Bound icon
lookup and file sizes. Resolve desktop IDs and user-over-system precedence
before visibility filtering, so Hidden=true user entries mask system entries.
Respect Type, Hidden, NoDisplay, OnlyShowIn, NotShowIn, TryExec, and localized
Name. Keep ordering deterministic; search name, generic name and keywords.
Use exact/prefix/substring ordering with stable name/ID tie breaks.

Parse Desktop Entry escaping and Exec quoting in pure Rust. Expand supported
field codes (%c, %k, %i, %%), omit file/URL arguments for an ordinary launch,
reject unknown or malformed codes rather than guessing. Honor Path. Handle
Terminal=true using an existing terminal with an explicit argument convention;
report an actionable error if none is available. Do not silently launch terminal
applications without a terminal. Entries requiring D-Bus activation need a valid
Exec fallback; document unsupported activation-only entries.

Display installed application icons when resolvable from absolute paths or
common theme directories. Missing or unreadable icons use an embedded vector
fallback. Do not require an icon theme, external font or asset folder. Keep icon
lookup bounded and outside view. Resolve exact formats/features during phase 1.
Capture icon bytes during discovery so disappearing files cannot break rendering;
test missing/unreadable icons and disappearance after indexing.

Default pins prefer installed Chrome, Brave, VS Code, Files, and Obsidian by
desktop ID; never display fake launchable entries for missing applications.
For an empty query, pins appear first and remaining applications below without
duplicates. A nonempty query shows one ranked result grid independent of pin
status. Enter always uses index zero of the exact list rendered. Test a stronger
non-pinned match competing with a pinned app. Pin persistence is not required.

Use proportional spacing from the image, scaling it to a practical desktop
window. User subsequently requested floating startup: set matching minimum and
maximum window size, with resizing disabled, using COSMIC's dialog heuristic.
Grid columns adapt to available width; content scrolls without horizontal
overflow and names wrap. Embed the default font for consistent text metrics.

## Phases and files

1. **Model and dependency baseline:** Cargo.toml, Cargo.lock, model/Cargo.toml,
   model/src/lib.rs and focused model modules. Select a verified iced revision,
   settle test-crate permission, add parser/search/Exec tests. First build a
   minimal iced binary and inspect linkage: normal OS display infrastructure is
   the platform baseline, but any additional required library/helper or sidecar
   must be resolved or explicitly approved before introduction. Root remains a
   compilable starter while model tests pass.
2. **Discovery and launch:** src/catalog.rs, src/launch.rs, model types as needed.
   Add XDG precedence, visibility, executable checks, bounded icon resolution,
   and launch integration tests with temporary desktop files and a harmless
   test subprocess. No installed application needs to launch during tests.
3. **Window and interaction:** src/main.rs, src/lib.rs, src/launcher.rs, embedded
   assets if needed. Add async scan, startup focus task, responsive reference
   layout, search, Enter/click launch, Escape and visible errors.
4. **Headless and distribution verification:** GUI tests alongside launcher,
   README.md, phase/handoff records. Test startup task focus through actual
   widget operations, typing without a preceding click, first-result submission,
   no-match submission, click target, geometry at reference and small sizes,
   long names, scrolling and errors. Build a release executable and verify its
   resources do not rely on the checkout.

Execution order: 1 -> 2 -> 3 -> 4. Each phase must compile and pass its own tests.

## Validation and acceptance evidence

- cargo fmt --all --check
- cargo check --workspace --all-targets
- cargo test --workspace
- cargo clippy --workspace --all-targets -- -D warnings
- cargo build --release; inspect dynamic runtime requirements and explain any
  platform baseline rather than claiming a universally static executable.
- Inspect a rendered headless screenshot against the supplied image.
- Exercise focus and Enter from the startup task using the real widget tree,
  not merely by asserting a focus ID exists in application code.
- Verify the launched command matches the first displayed result using a
  harmless fixture; failed spawning keeps the window open.
- Inspect lifecycle: no service, polling daemon, or detached indexing worker.

## Out of scope

Global shortcut registration, background indexing, installers, launch history,
general settings UI, plugin systems, session/power controls, network search,
and new database persistence are not necessary for the requested launcher.

## Critique resolution log

Directional checkpoint and all remaining MAP stages authorized by user on
2026-10-28. Agent critique, FOSS comparison and expert pass pending.

Review round 1: moved distribution/linkage verification to phase 1, before
introducing runtime exceptions. Defined empty-query pin ordering and nonempty
query ranking with a competing pin regression fixture.

Review round 2: reviewer states "the plan looks great."

FOSS comparison: Onagre c68c716b086afa3cc8ca02a9a03334493ecb5cb1 (MIT),
src/app/mod.rs:106–149 confirms runtime startup focus and unified activation;
src/icons/mod.rs:80–170 confirms PNG/SVG plus embedded fallback. Unlike Onagre,
we require no pop-launcher helper and recover from spawn failures. Capture icon
bytes before rendering and test file disappearance. No source or artwork copied.

Expert review: "the plan looks great." Carry forward symlink-cycle prevention,
bounded icons, actual startup task operations in tests, and successful spawn as
the launch acceptance boundary (not a guarantee the target later initializes).
Use wgpu primary with tiny-skia fallback per subsequent user instruction and
verify early linkage. No unresolved review findings.

## TODO before implementation dispatch

- [x] Resolve directional checkpoint and any scope corrections.
- [x] Obtain approval for iced_test before adding it.
- [x] Finish agent exploration and inspect source APIs for pinned revision.
- [x] Agent critique until reviewer says the plan looks great.
- [x] FOSS comparison with licensing noted; do not copy GPL implementation.
- [x] Domain-expert review and resolve issues.
- [x] Write self-contained phase docs, cross-reference audit, goon.yaml, handoff.
- [x] Recheck AGENTS.md, iced guidance and actual files before edits.

## Execution evidence

All four phases implemented. Master resolved to
cd04b74ca7073766f654535d6e72c452ef749d07 for iced and iced_test.
Phase 1 minimal executable linkage and final release linkage show libc, libm,
libgcc_s and the loader, with no app-specific dynamically linked libraries.
The required font and own artwork are embedded. No installer/helper introduced.

28 tests pass: 13 model, 7 discovery/launch and 8 real-widget GUI tests. Actual
boot focus operations, typing without clicks, preserved focus during index load,
ranked first-result Enter, real harmless subprocess creation, duplicate-submit
guard, click identity, recoverable errors, small/default bounds and scrolling
are covered. Screenshot inspected in software and wgpu previews.

Native Wayland release executable opened from /tmp with isolated XDG fixtures.
Protocol trace /tmp/baka-launcher-smoke/wayland.log confirms set_min_size and
set_max_size(1200,760); user confirms new native Wayland window floats. Native
test window closed after inspection. Keyboard spawning is proven headlessly;
X11 synthetic input smoke did not establish native keyboard activation and is
not claimed as passing. No background service exists.

Independent final agent audit found no blocking behavior issue. Minor known
limit: icon headers are checked but arbitrary corrupt SVG/PNG content may render
blank rather than the fallback. Missing/unreadable icons are covered and safe.

fmt, workspace/all-target check, all-target clippy with -D warnings and release
build pass. README documents run/platform/entry limitations and no-daemon model.

## Appendix A — implementation dispatch preamble

Read AGENTS.md, installed Rust and iced guides, this plan, and your phase doc.
Verify actual files before relying on plan references. Work only within phase
scope. Use Rust and approved dependencies, no unsafe, shell launch, speculative
abstractions or compatibility shims. Preserve the one-executable model. Run
fmt/check, relevant tests, and clippy; report exact results and unresolved risks.
Do not create commits, publish or push unless instructed. If a genuine design
decision is missing, report it before introducing a new dependency or runtime
requirement. Self-review focus delivery, index precedence, quoting, duplicate
results, error behavior, stale comments and asset embedding.

## Appendix B — resume

Read REQUIREMENTS.md, MAP_PLAN.md, goon.yaml, latest SESSION_HANDOFF.md and the
next incomplete phase doc. Inspect current code and check status before running
or restarting anything. Plans describe intent; command output and code prove
completion. Continue the first incomplete phase with Appendix A's constraints.
