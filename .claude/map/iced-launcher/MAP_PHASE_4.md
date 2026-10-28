# Phase 4 — headless proof and executable delivery

Prerequisites: phases 1–3 compile and pass focused tests. Read AGENTS, MAP_PLAN,
actual startup/update/view and iced_test APIs from the locked master revision.

1. Headless tests use Simulator::with_size and the real launcher view. Consume
   actual boot focus task through iced_test::runtime::task::into_stream, apply
   Action::Widget with Simulator::operate; type without clicking. Do not replace
   this with a test-only focus call. Feed produced messages through real update.
2. Test ranked first-result Enter, empty/no-match Enter, correct tile click,
   stronger unpinned match, loading query preservation, and recoverable error.
   Harmless fixture launches verify actual subprocess output and no double spawn.
3. Inspect widget bounds at reference ratio, default size and small minimum.
   Test long labels/many entries, scroll reachability. Capture a PNG via headless
   snapshot to target/ for visual comparison; inspect it against reference.
4. Run fmt/check/test/clippy, release build. Inspect file/ldd and launch from an
   unrelated directory with isolated XDG fixture; own font/assets must be embedded.
   Native window smoke test if display access is available, via approved GUI run.
5. README documents build/run, Linux scope, key behavior, scan/no-daemon model,
   desktop-entry and terminal limits, platform baseline and executable location.
   Update MAP checklists, goon.yaml, SESSION_HANDOFF with actual evidence/results.

Files: src/launcher tests or tests/, README.md, MAP docs. No broad cleanup.
Exit: all acceptance evidence in MAP_PLAN recorded, screenshot inspected, one
release executable delivered. Report limitations without claiming unrun checks.

| Scenario | Expected |
|---|---|
| boot then type without click | query changes |
| filtered result then Enter | first rendered app spawns |
| spawn failure | error + usable search; no exit |
| close/launch | launcher process ends; no background service |
