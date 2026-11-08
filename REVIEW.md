# Technical Review Packet: `zinsser`

## 1. Project Intent
`zinsser` is a "Prose Compiler" designed to strip corporate bloat, AI-generated "slop," and robotic cadence from technical documentation. It moves beyond simple grammar checks to enforce **structural craft** based on the classical canon (Zinsser, Williams, Pinker).

## 2. Architecture
- **Low-Level (Syntax/Grammar):** Embedded `harper-core` (Rust) for token-level linting and POS tagging.
- **Mid-Level (Style/Vocab):** `Vale` (Go) using declarative YAML for "banned" words and substitutions.
- **High-Level (Structure/Cadence):** Custom Rust passes calculating inter-sentence dynamics.

## 3. Implementation Audit Guide

### A. The 4 Statistical Craft Algorithms (`src/cadence_pass.rs`)
Reviewers should check these for mathematical soundness and false-positive risk:
1. **Sentence Length Monotony:** Flags $\ge 4$ consecutive sentences within a 5-word length band.
2. **Delayed Verb (Left-Branching Lag):** Flags sentences where the first finite verb appears after token index 14 in a long sentence.
3. **Abstract Noun Density:** Flags paragraphs where $>8\%$ of words match nominalization suffixes (`-tion`, `-ment`, etc.).
4. **Old-to-New Cohesion:** Checks for "Cold Starts" by analyzing stem overlap and cohesion bridges between Sentence $N$ and Sentence $N+1$.

### B. The Style Rules (`styles/Zinsser/`)
Audit these for "over-zealousness":
- **AI_Tells.yml:** AI/LLM voice markers.
- **CadenceMonotony.yml:** Rhythmic repetition.
- **Clutter.yml:** Ruthless economy (Zinsser).
- **Hedging.yml:** Removing qualifiers (Pinker).
- **Nominalizations.yml:** Action-in-the-verb (Williams).
- **PassiveVoice.yml:** Passive constructions.
- **Puffery.yml:** AI-slop detection (Corporate buzzwords).
- **WeaselWords.yml:** Vague quantifiers.
- **Greybeard.yml:** Selected writing tics.

### C. The Integration
The `zinsser` CLI orchestrates these as a pipeline:
`Markdown AST` $\rightarrow$ `Harper POS` $\rightarrow$ `Cadence Stats` $\rightarrow$ `Vale JSON` $\rightarrow$ `Unified Report`.

## 4. Evaluation Criteria for Reviewers
1. **False Positive Rate:** Will these rules annoy a professional writer, or do they actually improve clarity?
2. **Algorithm Accuracy:** Does the "Delayed Verb" heuristic correctly identify the main action?
3. **Completeness:** Are there major "AI-isms" missing from the `Puffery.yml` list?
4. **Performance:** Is the Rust $\rightarrow$ Vale subprocess bridge efficient?
