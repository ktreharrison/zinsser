# Changelog

## [0.1.1] - Unreleased

- Detect delayed verbs with Harper's Brill part-of-speech tagger, recalibrate the scorecard grade, and add a project dictionary (`.zinsser/dictionary.txt`, `--dictionary`).
- Fix cadence diagnostic locations: columns pointed one character early, and drifted further after any link, emphasis or inline code earlier in the paragraph.
- Find `.zinsser/dictionary.txt` in the checked path's parent directories, so checking a subdirectory uses the project dictionary.
- Split sentences in linear time; very long paragraphs no longer slow `check` quadratically.

## [0.1.0] - 2026-11-08

- Add `zinsser check` for files and recursively scanned Markdown directories.
- Run Harper grammar and spelling checks with native Rust cadence heuristics.
- Add optional Vale style rules, with `zinsser init` to install project-local rules and configuration.
- Add JSON diagnostics and a prose craft scorecard.
- Publish the crate to crates.io for installation with `cargo install zinsser`.
- Generate prebuilt binaries for macOS, Linux, and Windows; provide a shell installer for Unix.
