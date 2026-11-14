# Changelog

## [0.2.0] - Unreleased

### Breaking changes (library API)

The `zinsser` command line is compatible; these affect code that uses the crate as a library (reported by `cargo semver-checks` against 0.1.0):

- `ValeRunner::run` takes a second argument, the list of Zinsser rules handed to Vale (`&[]` for none).
- `ProseBlock` has a new public field, `segments`; build blocks with `ProseBlock::new(text, start_byte)`.
- `DocumentScore` has new public fields: `lint_warnings`, `lint_errors`, `lint_suggestions`.
- `CadencePass` is no longer `UnwindSafe` / `RefUnwindSafe`.

### Changes

- Do not spell-check a link that is the whole line, apart from a list marker or bullet (translation and navigation lists, #2), or the token harper-core reads across a link and a possessive (`[FastAPI](...)'s`). Links inside sentences are still checked. On five public READMEs this removes 23 of 115 spelling hits.
- Strip symbol tables from release binaries (5.9 MB to 5.0 MB on macOS).
- Do not report spelling for words containing non-ASCII letters (`Français`, `Türkçe`, `दी`): Harper checks English only, and translation lists were the main source of spelling noise (#2). English typos are still reported.
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
