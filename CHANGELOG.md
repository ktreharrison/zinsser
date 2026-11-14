# Changelog

## [0.2.1] - 2026-11-14

- Add `zinsser --version` (and `-V`); it was rejected as an unexpected argument.
- Treat a link that is the whole item of a numbered list (`1. [Deutsche](README.de.md)`) as a link-only line, like bulleted ones, so its label is not spell-checked. Numbered-list prose is still checked. (Sourcery review of #4.)
- Look up link-only lines by binary search instead of scanning them for every spelling finding, which was quadratic on large navigation lists. (Sourcery review of #4.)
- An explicit `--dictionary` that is over the 1 MB cap or has no usable words is now an error instead of being skipped or silently replaced by the curated dictionary. The optional default `.zinsser/dictionary.txt` still only warns. (Sourcery review of #3.)
- Only treat `1.` or `2)` as a list marker when whitespace follows it, as in CommonMark; `3.[link](x)` starts a paragraph and its link text is still spell-checked. (Sourcery review of #5.)
- Judge "no usable words" per dictionary file, so an empty explicit file is an error even after earlier dictionaries filled the 100,000-word cap; warn when the default `.zinsser/dictionary.txt` exists but is empty; read dictionaries through one bounded read so the 1 MB cap holds even if the file changes while being read. (Sourcery review of #5.)
- Compute Harper finding columns from a precomputed table; a per-finding scan from the start of the file was quadratic in the number of findings.
- Security scanners flag `zip` 7.4.0 in `Cargo.lock` (CVE-2026-115905 and others). It is not compiled into zinsser: it belongs to `candle-core`, reached only through burn's optional `burn-candle` backend, which zinsser does not enable (`cargo tree -i zip -e all --target all` prints nothing). Cargo locks optional dependencies anyway, and `candle-core` 0.9.2 requires zip 7.x while the fixes are in 9.x, so the entry stays until upstream moves.

## [0.2.0] - 2026-11-14

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
