# Phase 1 — dependency baseline and pure model

Prerequisites: approved MAP, reviewer and expert approval recorded in MAP_PLAN.
User authorizes iced master and iced_test. Read AGENTS.md and installed iced and
Rust guides. Do not add other unapproved crates or unsafe code.

1. Replace the empty dependency table with iced from git branch master, disable
   defaults and use wgpu primary plus tiny-skia fallback, x11, wayland,
   wayland-dlopen, tokio, image, svg,
   fira-sans, advanced. Use matching master source for iced_test. Cargo.lock pins
   a single upstream revision; baseline master is cd04b74ca7073766f654535d6e72c452ef749d07.
2. Add model workspace member, no iced or I/O in it; thiserror is allowed.
   Keep root main minimal and compile an iced window. Inspect its linkage before
   introducing required helpers/libraries/sidecars. Normal OS graphics/display
   infrastructure is the baseline; stop for other runtime exceptions.
3. Add model/src/entry.rs with Entry, related parse error and metadata. Suggested
   public boundary: `Entry::parse(id: String, path: PathBuf, text: &str,
   locale: &str) -> Result<Option<Self>, Error>`. Hidden/nonapplication entries
   can return None; catalog must still mark their ID seen before parsing.
4. Parse only Desktop Entry section, localization fallback, escaped metadata,
   Exec quoting and field codes. Command is explicit program + argv, never shell.
   Parse Terminal/Path/TryExec and desktop visibility constraints. Invalid Exec
   yields a typed error. Support empty quoted arguments and percent literals.
5. Add model/src/search.rs: deterministic ranked index list; empty query orders
   installed preferred pins then others, without duplicates; nonempty search is
   independent of pin status. Return exact indexes UI will render and activate.

Files: Cargo.toml, Cargo.lock, src/main.rs baseline, model/Cargo.toml,
model/src/lib.rs, model/src/entry.rs, model/src/search.rs.

Exit: minimal iced binary compiles, linkage checked, pure parsing/search tests
pass. Verify cargo fmt --all --check, cargo check --workspace, cargo test -p model.

| Input | Expected |
|---|---|
| quoted executable/arg with spaces | preserved argv boundaries |
| %c %k %i %% and file placeholders | expanded metadata; no empty file args |
| unknown code/unmatched quote | typed error |
| action-section Exec | ignored |
| pinned weak match vs exact unpinned match | exact match first |

Do not implement GUI polish, cache, persistence or system integration here.
