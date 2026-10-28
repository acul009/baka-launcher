# Phase 3 — launcher window

Prerequisites: phases 1–2 verified. Read AGENTS, iced skill, MAP_PLAN, actual model
and catalog/launch APIs. Keep one screen cell together in src/launcher.rs.

1. Replace starter with thin main invoking application boot/update/view. Embed
   default Fira Sans through iced feature; centered undecorated window. Per later
   user instruction, use fixed 1200x760 min/max limits and disable resizing so
   COSMIC starts it floating. Set a stable Linux application ID.
2. State owns query, loaded apps, exact ordered visible indexes, loading/error
   and launching guard. Boot returns State + initial focus task and async scan
   (blocking I/O off UI thread). Preserve query when scan completes. Focus again
   on window-open event if needed. Keep initial task testable separately from I/O.
3. Build top search with embedded search SVG, dark rounded border, narrow left
   rail and main content panel. Empty query shows PINNED then divider and
   APPLICATIONS; query shows one ranked grid. No fake entries for missing apps.
4. Responsive columns and scrolling, centered icons/labels and wrapping long
   names. Use captured icon data, embedded fallback for missing assets. Respect
   reference colors/spacing, avoid arbitrary extra panels or user-facing internals.
5. Search update recomputes indexes; Enter and click resolve same displayed entry
   identity and launch once. Successful spawn exits; failure retains search/error.
   No-match Enter no-op. Escape closes. Sidebar offers only truthful scoped actions.

Files: src/main.rs, src/lib.rs, src/launcher.rs, assets/*.svg if used (include_bytes).
Pattern: `fn new() -> (State, Task<Message>)`, `fn update(&mut self, Message)
-> Task<Message>`, `fn view(&self) -> Element<'_, Message>`.

Exit: launcher works with fixture data and real catalog; compile/clippy pass.
Verify search while loading, empty catalog, no-match submission, spawn failures,
and reload preserving usable focus. No persistence or session/power behavior.
