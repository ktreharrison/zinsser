# Zinsser

**Zinsser** is a local-first prose checker for professional writing. It combines grammar checks from Harper, configurable style rules from Vale, and its own sentence-cadence heuristics.

## Features

- **Anti-Slop Engine:** Detects and flags AI-generated "corporate speak" and "LLM-isms" using a curated vocabulary of modern AI markers.
- **Cadence checks:** Zinsser's Rust pass estimates sentence-length monotony, delayed verbs, abstract-noun density, and sentence-to-sentence cohesion.
- **Harper:** Runs `harper-core` grammar and spelling checks. The cadence pass uses Harper's Brill part-of-speech tagger (`harper-brill`, bundled and offline) to find the first verb, with a dictionary fallback.
- **Vale:** Runs the project's YAML wording and phrasing rules when Vale is installed. Without Vale, this pass is skipped with a note.

## Installation

Install Zinsser from crates.io with Cargo:

```bash
cargo install zinsser
```

For a smaller, faster build, leave out the part-of-speech tagger:

```bash
cargo install zinsser --no-default-features
```

The tagger comes from `harper-brill`, which always pulls in the `burn` machine-learning framework. Without it, Zinsser builds about 65 crates instead of 173 and finds verbs with its dictionary and word list only, so delayed-verb findings and subject-verb distance are less accurate.

Cargo installs binaries to `$HOME/.cargo/bin`. Add that directory to `PATH` if your shell cannot find `zinsser`.

For a prebuilt Unix binary from GitHub Releases, run this after the release installer is published:

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/ktreharrison/zinsser/releases/latest/download/zinsser-installer.sh | sh
```

The script downloads a prebuilt binary from the latest GitHub release. It becomes available after the first release is published; use Cargo until then.

In each project you want to check, initialize the local Vale rules and configuration:

```bash
cd path/to/your/project
zinsser init
```

Vale is optional. Install [Vale](https://vale.sh/) to enable Zinsser's YAML style rules. Without it, Zinsser skips that pass and prints a note; Harper and cadence checks still run.

## Usage

### Check a file or directory
```bash
zinsser check fixtures/bad_corporate.md
```

First six findings:

```bash
fixtures/bad_corporate.md:3:1: warning [Zinsser.Clutter] Use 'to' instead of 'In order to'.
fixtures/bad_corporate.md:3:26: warning [Zinsser.Puffery] Avoid empty corporate/AI puffery: 'synergy'. Use concrete specifics.
fixtures/bad_corporate.md:3:43: warning [Zinsser.Clutter] Use 'use' instead of 'utilize'.
fixtures/bad_corporate.md:3:74: warning [Zinsser.Clutter] Use 'because' instead of 'Due to the fact that'.
fixtures/bad_corporate.md:3:115: warning [Zinsser.Puffery] Avoid empty corporate/AI puffery: 'ever-evolving landscape'. Use concrete specifics.
fixtures/bad_corporate.md:3:140: suggestion [Zinsser.Greybeard] Reflexive slop detected ('it is important to note that'). Cut it for a tighter, more professional tone.
```

Pass a directory, such as `docs/`, to recursively check its Markdown files. Zinsser skips `.git`, `target`, and `node_modules`.

Optional flags:

- `--format json`: Emit JSON diagnostics for scripts and editors.
- `--no-vale`: Run Harper and cadence checks without Vale.
- `--no-harper`: Skip grammar and spelling checks.
- `--dictionary <path>`: Extra word list (one word per line) for project terms. The nearest `.zinsser/dictionary.txt` at or above the checked path is always loaded, so `zinsser check docs/` from the project root finds the file `zinsser init` created. Case variants are handled; plurals and other inflected forms need their own entries.

Checks exit with status 1 when they find a warning or error.

### Get a Craft Scorecard
```bash
zinsser score fixtures/clean_craft.md
```

```bash
--- Craft Scorecard: fixtures/clean_craft.md ---
Words: 75 | Sentences: 12
Avg Sentence Length: 6.2 (A needs <= 25.0)
Sentence Length StdDev: 2.8 (A needs >= 5.0, or >= 1.0 when avg length < 10.0)
Abstract Noun Density: 4.0% (A needs <= 4.0%)
Avg Subject-Verb Distance: 1.4 (A needs <= 6.0)
Lint Findings: 0 warnings, 0 errors, 0 suggestions (warnings cost one letter, errors two; spelling suggestions do not affect the grade)
Overall Grade: A
--------------------------------------------
```

The grade combines the rhythm metrics with the linter's own findings: prose the checker complains about cannot score an A. Warnings cost one letter grade each, errors two. Spelling findings (unknown words) never affect the grade: they mark dictionary coverage, not prose quality, and project terms belong in `.zinsser/dictionary.txt` or `--dictionary`. Other suggestion-level findings (grammar, style) cost at most one letter in total.

The score reflects the checks that actually ran. The Vale style pass is optional: when a `vale` binary is installed its findings count toward the grade; without Vale, the grade reflects Harper and cadence findings only, and the run prints a note that the Vale pass was skipped. Run `zinsser init` and install Vale to make scores comparable across machines.

The scorecard measures the properties above, not whether the writing is good.

### Initialize project
```bash
zinsser init
```

`init` installs the Vale rules and Tengo script under `styles/`, creates `.vale.ini`, and seeds `.zinsser/dictionary.txt` for project vocabulary. Existing files are preserved.

## Architecture

`zinsser check` combines three passes:
1. **Harper** checks grammar and spelling.
2. **Zinsser's Rust pass** applies cadence and density heuristics.
3. **Vale** applies the project's declarative wording rules via 9 rule files:
   - `AI_Tells.yml`: Detects LLM-specific phrasing.
   - `CadenceMonotony.yml`: Flags rhythmic repetition.
   - `Clutter.yml`: Enforces Ruthless Economy.
   - `Hedging.yml`: Eliminates unnecessary qualifiers.
   - `Nominalizations.yml`: Targets "Zombie Nouns".
   - `PassiveVoice.yml`: Flags passive constructions.
   - `Puffery.yml`: Targets corporate bloat.
   - `WeaselWords.yml`: Flags vague quantifiers.
   - `Greybeard.yml`: Flags selected writing tics.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.
