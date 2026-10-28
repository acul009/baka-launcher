# Phase 2 — platform discovery and launch

Prerequisite: phase 1 model types and dependency/linkage baseline pass. Read
AGENTS, MAP_PLAN and model source. No external launcher helper requirement.

1. Implement src/catalog.rs. Discover XDG data roots with defaults. Traverse
   applications recursively in deterministic order; avoid directory symlinks.
   Derive desktop IDs, mark IDs seen before parsing/visibility to preserve user
   Hidden overrides. Skip unreadable/malformed entries without aborting scan.
2. Use model Entry parser; filter NoDisplay, OnlyShowIn/NotShowIn against current
   desktops, TryExec using executable permission/PATH. Select locale from LC_ALL,
   LC_MESSAGES, LANG. Keep OS I/O isolated. Unsupported platforms report clearly.
3. Resolve SVG/PNG icons from absolute paths and common theme locations, with
   bounded file count/bytes; capture bytes before returning. Missing/broken icons
   use embedded fallback in UI; no path reads in view. Do not copy FOSS artwork.
4. Implement src/launch.rs using Command plus argv and Path current directory.
   Terminal entries use an installed terminal and known exec flag; error if none.
   Successful spawn is acceptance; failure is recoverable and visible. Do not
   wait for target termination or create a permanent reaper/indexing process.
5. Fixture tests: user Hidden masks system ID, nested IDs, desktop filters,
   TryExec, unreadable/disappearing icons; harmless subprocess proves argv/cwd
   and literal shell-looking args. Tests must not start installed GUI apps.

Files: src/catalog.rs, src/launch.rs, root module declarations, tests as needed.
Example boundary: `catalog::scan() -> Result<Vec<catalog::App>, catalog::Error>`;
`launch::spawn(entry: &model::entry::Entry) -> Result<(), launch::Error>`.
App owns Entry plus immutable icon bytes/handle; no filesystem work in view.

Exit: relevant fixture tests and cargo check pass. Run fmt, cargo test --workspace,
cargo clippy --workspace --all-targets -- -D warnings.

| Fixture | Expected |
|---|---|
| user Hidden + system visible same ID | absent |
| recursive symlink cycle | no loop |
| missing icon after index | captured icon/fallback, no panic |
| nonexistent executable | error; UI can remain open |

Do not add cache, daemon, service, registry or launcher-helper dependencies.
