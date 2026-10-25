# Contributing to Vellum Grid

Thank you for your interest in Vellum Grid. Bug reports, feature ideas and pull requests are all welcome.

## Reporting bugs and suggesting features

Please [open an issue](https://github.com/mfurkan46/vellum-grid/issues/new/choose) and pick the matching template. For bugs, include:

- the Vellum Grid version (Help > About) and your operating system,
- the steps to reproduce the problem and what you expected to happen,
- a sample file if the problem depends on one (remove any private data first).

Issues may be written in English or Turkish.

## Development setup

You need [Rust](https://rustup.rs) 1.95 or later. On Linux, install the system libraries listed in the [README](README.md#building-from-source).

```bash
cargo build
cargo run -p vellum-grid
```

## Before opening a pull request

All of the following must pass. The same checks run in CI.

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --target wasm32-unknown-unknown -- -D warnings
cargo test
```

## Guidelines

- **Engine first.** Spreadsheet logic belongs in `crates/core` and must not depend on the user interface, the file system or the platform. Every rule in the engine is covered by unit tests.
- **Canonical storage, localized display.** Cells, files and the CLI always use the English, language-independent form (`=SUM(A1,B1)`, `1234.5`, `TRUE`). Text shown to or typed by the user goes through `Locale`.
- **Numbers are `f64`** and are shown with up to 15 significant digits through `format_number`.
- **No hard-coded colors, sizes or strings in screen code.** Colors and sizes come from the theme tokens (`theme.rs`), texts come from the language tables (`text.rs`) and are always added in both Turkish and English.
- **Shared look.** Components used by more than one Vellum application live in `crates/ui` (`vellum-ui`).
- **Keep it small.** Avoid new dependencies unless they are really needed; add them through `[workspace.dependencies]`.
- **Commits** have a short subject line in English written in the imperative mood ("Fix column width tooltip in dark theme").

## License

By contributing, you agree that your contributions will be dual licensed under the MIT and Apache-2.0 licenses, as described in the [README](README.md#license).
