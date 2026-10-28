# AGENTS.md

## 1. Mandatory capability checks

Before doing any project work, check that the guides and skills from https://airstrike.fyi are available.

If they are not available, check the site and install them.

## 2. Core language rule

Rust is the default and required implementation language.

Do not introduce Python, Java, Go, TypeScript, C#, or other implementation languages.

The only standing exception is small vanilla JavaScript helper code for web frontends, and only where JavaScript is actually necessary.

Do not introduce another language without explicit user permission.

## 3. Distribution and runtime model

The final deliverable should be a single executable.

All required application resources should be compiled or embedded into that executable where practical, including:

- templates
- static assets
- migrations
- schemas
- default configuration
- other runtime resources required for normal operation

The program should not require an installer.

The expected distribution model is that the executable can be copied to a system and run directly.

Avoid external runtime dependencies and required sidecar files.

If a single self-contained executable cannot reasonably be achieved:

1. Stop before introducing the exception.
2. Explain exactly why the constraint cannot be satisfied.
3. Explain the proposed external dependency, runtime file, installation step, or other exception.
4. Ask for explicit user permission before proceeding.

Convenience, convention, or easier implementation is not sufficient justification.

## 4. Engineering principles

Prefer pragmatic, simple, maintainable implementations.

Prefer:

- straightforward code over clever code
- small, understandable abstractions
- standard library functionality where practical
- well-understood technologies
- minimal dependencies
- clear data flow
- explicit behavior

Avoid:

- unnecessary abstraction layers
- speculative extensibility
- unnecessary wrappers
- broad redesigns
- architecture introduced only for theoretical future use
- unrelated refactors

Keep changes tightly scoped to the user's request.

Do not broaden the task into adjacent cleanup, redesign, refactoring, or feature work without confirmation.

Preserve existing project patterns and structure unless the requested change requires otherwise.

Use the smallest viable fix, especially for UI changes.

## 5. Cross-platform behavior

Programs should be cross-platform where practical.

Prefer portable Rust implementations over platform-specific scripting or APIs.

Platform-specific code should be isolated and justified when unavoidable.

Do not call PowerShell for work that can reasonably be performed directly in Rust.

The Windows Registry is off-limits by default.

Prefer ordinary configuration files in the platform-appropriate configuration or data locations.

Any requirement to use the Windows Registry must be explained and explicitly approved by the user first.

## 6. Standard Rust tooling

Use standard Rust tooling:

- `cargo`
- `rustfmt`
- `clippy`
- Rust's built-in test tooling

Do not use `unsafe` Rust.

Do not add backwards-compatibility code, compatibility shims, legacy migrations, or preservation logic unless explicitly requested.

## 7. Dependency policy

Use the established stack below when its capability is needed.

Do not evaluate or substitute alternative crates by default.

Adding any crate that is not already part of the established stack requires:

1. A brief explanation of what the crate is needed for.
2. Explicit user permission before adding it.

Avoid GPL-licensed dependencies.

If a GPL-licensed dependency appears necessary, explain why and ask the user before using it.

Prefer fewer dependencies when a simple implementation with the standard library or existing stack is reasonable.

## 8. Standard dependency stack

### Serialization

Use `serde` for JSON, TOML, and YAML serialization/deserialization.

Do not use `serde` as the assumed XML solution.

XML support, if required, should be treated as a separate design decision.

### CLI

Use `clap` for command-line argument parsing.

### Error handling

Use `thiserror` for application error types.

Prefer explicit, typed errors over opaque error handling.

### Async

Use `tokio` as the async runtime.

### Database

Use SQLite with `sqlx`.

All SQL queries must use SQLx compile-time checked query mechanisms.

Do not introduce runtime/dynamic SQL unless explicitly approved.

SQLx compile-time query metadata must remain valid and consistent with the code.

Do not work around metadata or compile-time query problems by weakening query checking.

### Web server

Use:

- `axum` for HTTP/web serving
- `minijinja` for HTML templates

Templates and required static resources should be embedded into the executable.

Add frontend JavaScript only where it is actually needed.

Frontend JavaScript must be vanilla JavaScript unless the user explicitly approves another approach.

Do not introduce frontend frameworks by default.

### Desktop GUI

Use `iced` for desktop GUI applications.

Consult the available `iced` skill/guide before designing or modifying significant GUI architecture.

## 9. Configuration

Prefer simple configuration files over registry settings, platform-specific configuration databases, or installation-managed state.

Use platform-appropriate user configuration/data directories when persistent configuration is needed.

Configuration should remain understandable and editable without specialized tooling unless the application requirements clearly demand otherwise.

Where practical, defaults should be embedded into the executable.

## 10. Database and migrations

SQLite is the default database when persistence is required.

Use `sqlx` compile-time checked queries only.

Database migrations required by the application should be embedded into the executable where practical.

Do not require users to manually install migration tooling for normal application operation.

## 11. Scope discipline

Implement only what the user requested.

Before making a broader change, ask if it:

- changes architecture beyond what is necessary
- adds a dependency
- changes public behavior outside the requested scope
- introduces a new runtime requirement
- adds installation steps
- adds compatibility behavior not requested
- introduces platform-specific behavior that could be avoided

Do not turn a focused task into a general cleanup exercise.

## 12. Validation

After Rust code changes, run the relevant standard checks whenever possible.

At minimum, for normal Rust code changes:

- `cargo fmt --check`
- `cargo check`

Run appropriate tests when the changed behavior is covered by tests or when new tests are warranted.

Use `clippy` where appropriate for the project and change scope.

If a relevant check cannot be run, report that fact and explain why.

Do not claim validation that was not actually performed.

## 13. Documentation and comments

Prefer code that is understandable without excessive commentary.

Add comments where they explain:

- non-obvious constraints
- important invariants
- platform-specific behavior
- design decisions that would otherwise be easy to accidentally undo

Do not add comments that merely restate the code.

Update user-facing or developer-facing documentation when the requested change makes existing documentation materially inaccurate.

## 14. AGENTS.md changes

Do not silently modify the rules in this file.

If project work suggests that `AGENTS.md` should change, propose the change to the user first and explain why.

Only modify these rules after user approval.

## 15. Decision priority

When multiple implementation options are available, prefer them in this order:

1. Satisfy the explicit user request.
2. Follow this `AGENTS.md`.
3. Use available Rust and `iced` guidance where applicable.
4. Preserve the single-executable distribution model.
5. Use the established dependency stack.
6. Prefer the simplest cross-platform Rust implementation.
7. Minimize new dependencies and runtime requirements.

If these constraints conflict and cannot all be satisfied, explain the conflict and ask the user before relaxing a constraint.
