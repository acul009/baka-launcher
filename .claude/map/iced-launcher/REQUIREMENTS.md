# Launcher requirements

## Confirmed

- Build a Rust iced application matching the supplied launcher image.
- Show the launcher window immediately on execution.
- Focus search immediately; typing searches installed applications.
- Enter launches the first result.
- No background service. Index caching is optional.
- Distribute one executable with required application resources embedded.
- Follow AGENTS.md and installed airstrike guides; use MAP planning and review.
- Verify layout and input behavior headlessly where possible.

## Reference

`/home/acul/.codex/attachments/b8311c3c-1b9c-4995-b093-02bb361e9d6d/image-1.png`

The image shows a dark rounded window, full-width search, a narrow left
sidebar, and a main panel containing pinned applications above an application
grid. App icons have centered labels; the main panel leaves generous space.

## Initial evidence

- Cargo.toml contains only the package declaration and an empty dependencies section.
- src/main.rs contains the default Hello World program.
- Local iced and map skills and airstrike guides are available under
  /home/acul/.codex. No installation is needed.
- No existing implementation or tests are present.

## Resolved scope and later instructions

- User authorized iced master and all MAP stages after reviewing the Linux-first
  plan and iced_test request. No session/power operations were added.
- Hardware rendering requested; user explicitly permits software fallback.
- Floating startup requested and confirmed by user on native COSMIC Wayland.
- No additional prior art, exclusions or timeline constraints were provided.

Implementation and verification evidence is recorded in SESSION_HANDOFF.md.
