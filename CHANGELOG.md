# Changelog

## [0.1.1] - Unreleased

- Detect delayed verbs with Harper's Brill part-of-speech tagger, recalibrate the scorecard grade, and add a project dictionary (`.zinsser/dictionary.txt`, `--dictionary`).
- Fix cadence diagnostic locations: columns pointed one character early, and drifted further after any link, emphasis or inline code earlier in the paragraph.
- Find `.zinsser/dictionary.txt` in the checked path's parent directories, so checking a subdirectory uses the project dictionary.
- Split sentences in linear time; very long paragraphs no longer slow `check` quadratically.
- Run Zinsser's style rules with a built-in engine; Vale is no longer needed, and `score` grades the same with or without it. The rules stay Vale-format YAML: project copies from `zinsser init` are used when present, and an installed Vale still runs any other styles named in `.vale.ini`. `--no-styles` is an alias for `--no-vale`.
- The style engine supports `nonword`, `raw` and `MinAlertLevel`, and hands rules it cannot reproduce exactly (other rule types, non-default `scope`) to an installed Vale instead of skipping them silently; without Vale they are named as not checked.
- Add the default `pos-tagger` feature. `--no-default-features` builds without harper-brill and its burn dependency (65 crates instead of 173).
- Flag by-agent passives such as "were confused by" that an adjective exception hid; print scorecard targets from the grading thresholds.
- `zinsser score` exits 1 when an installed Vale fails, as `check` does; a missing Vale is still skipped.
- Keep closing quotes and brackets with the sentence they close.
- Match abstract nouns case-insensitively; uppercase text bypassed the density check.
- Count an ellipsis or stacked punctuation (`...`, `?!`) as one sentence end; `Wait... really?!` was scored as five one-word sentences.

## [0.1.0] - 2026-11-08

- Add `zinsser check` for files and recursively scanned Markdown directories.
- Run Harper grammar and spelling checks with native Rust cadence heuristics.
- Add optional Vale style rules, with `zinsser init` to install project-local rules and configuration.
- Add JSON diagnostics and a prose craft scorecard.
- Publish the crate to crates.io for installation with `cargo install zinsser`.
- Generate prebuilt binaries for macOS, Linux, and Windows; provide a shell installer for Unix.
