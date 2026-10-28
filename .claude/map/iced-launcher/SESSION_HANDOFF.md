# Iced launcher session

Build a Linux-first Rust iced launcher matching the supplied image, immediately
focused search, Enter first result, no background service, one executable.

Planning: user authorized all MAP stages and iced master/iced_test. Agent
exploration, two critique rounds, MIT Onagre comparison and expert review are
complete. Reviewers approve. MAP_PLAN.md records findings. Four phase docs are
self-contained and cross-reference the same 1 -> 2 -> 3 -> 4 ordering.

Current implementation: phases 1–4 complete. Release executable:
`target/release/baka-launcher` (approximately 20 MB). iced and iced_test master
are locked at cd04b74ca7073766f654535d6e72c452ef749d07. Hardware wgpu renderer
enabled with user-approved tiny-skia fallback. No remote/tracking configured.

Validation passed:

- cargo fmt --all --check
- cargo check --workspace --all-targets --locked
- ICED_TEST_BACKEND=tiny-skia cargo test --workspace --locked: 28 tests
- cargo clippy --workspace --all-targets --locked -- -D warnings
- cargo build --release --locked
- ICED_TEST_BACKEND=wgpu cargo test --lib launcher::tests::render_reference_preview
- Visual inspection of both headless previews in target/.
- Native Wayland launch from /tmp with isolated XDG fixtures; user confirms it
  floats on COSMIC. Trace proves matching min/max size constraints. Native test
  window closed. Actual native keyboard automation did not prove activation;
  the real-widget headless test proves typing, first-result subprocess and exit.
- Final linkage only libc/libm/libgcc_s/loader; own resources compiled in.
- Dependency license tree has no GPL-only requirement; self_cell offers Apache-2.0.

Later user requirement: floating startup. Main sets fixed min/max 1200x760,
resizable=false and Linux application ID baka-launcher. This follows COSMIC's
native Wayland dialog sizing heuristic; no compositor configuration changed.

Key implementation files: src/main.rs (window), src/launcher.rs (TEA/view),
src/launcher/tests.rs (widget proof), src/catalog.rs (XDG/icon I/O), src/launch.rs
(Command spawn), model/src/entry.rs and search.rs (pure parsing/ranking).
README.md documents running and Linux limitations. Known minor robustness limit:
header-valid but corrupt icon files can render blank; missing/unreadable icons
use embedded fallback. No other unresolved required work found by final review.

Read AGENTS.md, MAP_PLAN.md, relevant phase, actual Cargo/source and installed
iced/Rust guides. Use cargo fmt/check/test/clippy and build --release as recorded
in goon.yaml. Final review needs real startup widget focus and rendered screenshot.

Keep changes local, no commits/push required. Do not add unapproved crates,
external launcher helpers, runtime sidecars, unsafe, services or system actions.
Use /goon iced-launcher to resume and /handoff to record future session state.
