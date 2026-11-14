use crate::markdown::ProseBlock;
use crate::types::{Diagnostic, Severity};
use harper_brill::{brill_tagger, BrillTagger, FreqDict, Tagger, UPOS};
use harper_core::{Dictionary, FullDictionary, WordMetadata};
use regex::Regex;
use std::collections::HashSet;
use std::sync::Arc;

pub struct CadencePass {
    abstract_noun_regex: Regex,
    stop_words: HashSet<String>,
    abstract_allowlist: HashSet<String>,
    dictionary: Arc<dyn Dictionary>,
    tagger: Arc<BrillTagger<FreqDict>>,
}

impl CadencePass {
    pub fn new() -> Self {
        Self::with_dictionary(FullDictionary::curated())
    }

    pub fn with_dictionary(dictionary: Arc<dyn Dictionary>) -> Self {
        Self {
            dictionary,
            tagger: brill_tagger(),
            abstract_noun_regex: Regex::new(r"\b[a-zA-Z]{4,}(?:tion|sion|ment|ence|ance|ibility|ability|ization|isation|ality|osity|itude)(?:s)?\b").unwrap(),
            stop_words: ["the", "and", "a", "an", "of", "to", "in", "is", "it", "that", "as", "for", "was", "with", "on", "by"].iter().map(|s| s.to_string()).collect(),
            abstract_allowlist: [
                "function", "functions", "document", "documents", "environment", "environments",
                "element", "elements", "argument", "arguments", "statement", "statements",
                "section", "sections", "version", "versions", "question", "questions",
                "station", "stations", "mention", "mentions", "moment", "moments",
                "position", "positions", "condition", "conditions", "connection", "connections",
                "location", "locations", "direction", "directions", "component", "components",
                "segment", "segments", "instrument", "instruments", "comment", "comments",
                "requirement", "requirements", "deployment", "deployments"
            ].iter().map(|s| s.to_string()).collect(),
        }
    }

    fn normalize_token(token: &str) -> String {
        token
            .trim_matches(|c: char| !c.is_alphanumeric())
            .to_lowercase()
    }

    pub fn split_sentences(text: &str) -> Vec<&str> {
        // One pass over (byte index, char) pairs. The previous version called
        // `char_indices().nth(i)` per terminator, which is quadratic on long
        // paragraphs.
        let mut sentences = Vec::new();
        let mut start = 0;
        let indexed: Vec<(usize, char)> = text.char_indices().collect();
        let chars: Vec<char> = indexed.iter().map(|&(_, c)| c).collect();

        for (i, &(byte_idx, c)) in indexed.iter().enumerate() {
            if c == '.' || c == '!' || c == '?' {
                if Self::is_abbreviation_at(&chars, i) {
                    continue;
                }
                let end = byte_idx + c.len_utf8();
                sentences.push(&text[start..end]);
                start = end;
            }
        }
        if start < text.len() {
            sentences.push(&text[start..]);
        }
        sentences
    }

    /// Byte offset of the first non-whitespace character of `sentence`
    /// within `text`. `sentence` must be a slice of `text`. Sentences after
    /// the first start with the space that followed the previous terminator;
    /// pointing at that space put every diagnostic one column early.
    fn sentence_offset(text: &str, sentence: &str) -> usize {
        let leading = sentence.len() - sentence.trim_start().len();
        sentence.as_ptr() as usize - text.as_ptr() as usize + leading
    }

    fn is_abbreviation_at(chars: &[char], i: usize) -> bool {
        if chars[i] != '.' {
            return false;
        }

        if i > 0
            && chars[i - 1].is_ascii_digit()
            && i + 1 < chars.len()
            && chars[i + 1].is_ascii_digit()
        {
            return true;
        }

        if i + 2 < chars.len() && chars[i + 1].is_ascii_alphabetic() && chars[i + 2] == '.' {
            return true;
        }

        // Last whitespace-delimited token ending at `i`. Scanning back from
        // `i` replaces collecting the whole prefix on every period.
        let token_start = chars[..=i]
            .iter()
            .rposition(|c| c.is_whitespace())
            .map_or(0, |p| p + 1);
        let last_token: String = chars[token_start..=i].iter().collect();
        let clean_token = last_token
            .trim_start_matches(|c: char| !c.is_alphanumeric())
            .to_lowercase();

        let abbrevs = [
            "e.g.", "i.e.", "dr.", "mr.", "mrs.", "ms.", "prof.", "vs.", "etc.", "fig.", "inc.",
            "ltd.", "jr.", "sr.", "st.",
        ];

        abbrevs.contains(&clean_token.as_str())
    }

    /// Words Harper marks only as verbs (never noun/adjective/adverb/
    /// conjunction/article/preposition) are safe to treat as verbs without
    /// sentence context.
    fn is_strict_verb_metadata(meta: &WordMetadata) -> bool {
        meta.verb.is_some()
            && meta.noun.is_none()
            && meta.adjective.is_none()
            && meta.adverb.is_none()
            && meta.conjunction.is_none()
            && !meta.article
            && !meta.preposition
    }

    /// Inflection candidates for a word, so "manages" resolves through
    /// "manage" and "configured" through "configure". Harper tags
    /// inflections like "manages" as plural nouns, so the base form must
    /// be checked too.
    fn stem_candidates(word: &str) -> Vec<String> {
        let mut out = Vec::new();
        let len = word.chars().count();
        let strip = |n: usize| -> String { word.chars().take(len - n).collect() };

        if word.ends_with("ies") && len > 4 {
            out.push(format!("{}y", strip(3))); // carries -> carry
        }
        if word.ends_with("es") && len > 3 {
            out.push(strip(1)); // manages -> manage
            out.push(strip(2)); // fixes -> fix
        } else if word.ends_with('s') && !word.ends_with("ss") && len > 3 {
            out.push(strip(1)); // walks -> walk
        }
        if word.ends_with("ed") && len > 3 {
            out.push(strip(1)); // configured -> configure
            out.push(strip(2)); // walked -> walk
            let base = strip(2);
            let mut cs = base.chars().rev();
            if let (Some(a), Some(b)) = (cs.next(), cs.next()) {
                if a == b && !"aeiou".contains(a) {
                    out.push(strip(3)); // stopped -> stop
                }
            }
        }
        if word.ends_with("ing") && len > 4 {
            out.push(strip(3)); // walking -> walk
            out.push(format!("{}e", strip(3))); // making -> make
            let base = strip(3);
            let mut cs = base.chars().rev();
            if let (Some(a), Some(b)) = (cs.next(), cs.next()) {
                if a == b && !"aeiou".contains(a) {
                    out.push(strip(4)); // running -> run
                }
            }
        }
        out
    }

    fn is_verb_word(&self, normalized: &str) -> bool {
        if normalized.is_empty() {
            return false;
        }
        let meta = self.dictionary.get_word_metadata_str(normalized);
        if Self::is_strict_verb_metadata(&meta) || VERB_WORDLIST.contains(&normalized) {
            return true;
        }
        // Words already known as adjectives ("complicated", "finished") are
        // participles acting as adjectives; do not resolve them through
        // their verbal base form.
        if meta.adjective.is_some() {
            return false;
        }
        for candidate in Self::stem_candidates(normalized) {
            let stem_meta = self.dictionary.get_word_metadata_str(&candidate);
            if Self::is_strict_verb_metadata(&stem_meta)
                || VERB_WORDLIST.contains(&candidate.as_str())
            {
                return true;
            }
        }
        false
    }

    /// First verb in the sentence. Primary detector is Harper's Brill
    /// tagger, which assigns part-of-speech tags with sentence context, so
    /// "review meetings" tags as a noun while "found" in "we found the
    /// bug" tags as the verb. When the tagger finds no verb at all, fall
    /// back to dictionary metadata plus a curated wordlist. Errors fail
    /// safe: an unrecognized verb means no warning, never a confidently
    /// wrong word count.
    fn find_first_verb_index(&self, words: &[&str]) -> Option<usize> {
        let cleaned: Vec<String> = words.iter().map(|w| Self::normalize_token(w)).collect();
        let tags = self.tagger.tag_sentence(&cleaned);
        for (idx, tag) in tags.iter().enumerate() {
            // Skip participial openers ("Walking slowly..., the man found ...").
            if idx <= 1 && cleaned.get(idx).is_some_and(|w| w.ends_with("ing")) {
                continue;
            }
            if matches!(tag, Some(UPOS::VERB) | Some(UPOS::AUX)) {
                return Some(idx);
            }
        }
        self.find_first_verb_index_fallback(words)
    }

    /// Dictionary-metadata fallback for sentences the tagger cannot tag.
    fn find_first_verb_index_fallback(&self, words: &[&str]) -> Option<usize> {
        for (idx, &w) in words.iter().enumerate() {
            // Skip participial openers ("Walking slowly..., the man found ...").
            if idx <= 1 && w.to_lowercase().ends_with("ing") {
                continue;
            }
            let w_norm = Self::normalize_token(w);
            if self.is_verb_word(&w_norm) {
                return Some(idx);
            }
        }
        None
    }

    pub fn calculate_score(&self, blocks: &[ProseBlock]) -> (usize, usize, f64, f64, f64, f64) {
        let mut total_words = 0;
        let mut total_sentences = 0;
        let mut all_lengths = Vec::new();
        let mut abstract_nouns = 0;
        let mut total_sv_dist = 0;
        let mut sv_count = 0;

        for block in blocks {
            let sentences = Self::split_sentences(&block.text);
            for sent in sentences {
                let trimmed = sent.trim();
                if trimmed.is_empty() {
                    continue;
                }

                total_sentences += 1;
                let words: Vec<&str> = trimmed.split_whitespace().collect();
                let len = words.len();
                total_words += len;
                all_lengths.push(len);

                for mat in self.abstract_noun_regex.find_iter(sent) {
                    let word = mat.as_str().to_lowercase();
                    if !self.abstract_allowlist.contains(&word) {
                        abstract_nouns += 1;
                    }
                }

                if let Some(idx) = self.find_first_verb_index(&words) {
                    total_sv_dist += idx;
                    sv_count += 1;
                }
            }
        }

        let avg_len = if total_sentences > 0 {
            total_words as f64 / total_sentences as f64
        } else {
            0.0
        };
        let variance = if all_lengths.len() > 1 {
            let mean = avg_len;
            all_lengths
                .iter()
                .map(|&l| (l as f64 - mean).powi(2))
                .sum::<f64>()
                / (all_lengths.len() - 1) as f64
        } else {
            0.0
        };
        let stddev = variance.sqrt();
        let abstract_pct = if total_words > 0 {
            (abstract_nouns as f64 / total_words as f64) * 100.0
        } else {
            0.0
        };
        let avg_sv = if sv_count > 0 {
            total_sv_dist as f64 / sv_count as f64
        } else {
            0.0
        };

        (
            total_words,
            total_sentences,
            avg_len,
            stddev,
            abstract_pct,
            avg_sv,
        )
    }

    pub fn check_block(
        &self,
        block: &ProseBlock,
        file_path: &str,
        _content: &str,
    ) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let sentences = Self::split_sentences(&block.text);

        let mut valid_sentences = Vec::new();
        for s in &sentences {
            if !s.trim().is_empty() {
                valid_sentences.push(*s);
            }
        }

        if valid_sentences.is_empty() {
            return diagnostics;
        }

        if valid_sentences.len() >= 4 {
            let lengths: Vec<usize> = valid_sentences
                .iter()
                .map(|s| s.split_whitespace().count())
                .collect();
            for i in 0..=lengths.len() - 4 {
                let window = &lengths[i..i + 4];
                let min_len = *window.iter().min().unwrap();
                let max_len = *window.iter().max().unwrap();
                if min_len >= 8 && (max_len - min_len) <= 5 {
                    let relative_byte_offset =
                        Self::sentence_offset(&block.text, valid_sentences[i]);
                    diagnostics.push(Diagnostic {
                        file: file_path.to_string(),
                        line: 0,
                        column: relative_byte_offset,
                        severity: Severity::Suggestion,
                        rule_id: "Cadence.Monotony".to_string(),
                        message: format!("Monotonous rhythm: 4 consecutive sentences have nearly identical length ({:?}). Vary your rhythm.", window),
                        suggested_fix: None,
                    });
                    break;
                }
            }
        }

        for sent in &valid_sentences {
            let words: Vec<&str> = sent.split_whitespace().collect();
            if words.len() > 18 {
                let relative_byte_offset = Self::sentence_offset(&block.text, sent);
                if let Some(idx) = self.find_first_verb_index(&words) {
                    if idx >= 14 {
                        let verb = Self::normalize_token(words[idx]);
                        diagnostics.push(Diagnostic {
                            file: file_path.to_string(),
                            line: 0,
                            column: relative_byte_offset,
                            severity: Severity::Warning,
                            rule_id: "Cadence.DelayedVerb".to_string(),
                            message: format!(
                                "Delayed verb: reader waits {} words before the first verb ('{}').",
                                idx + 1,
                                verb
                            ),
                            suggested_fix: None,
                        });
                    }
                } else {
                    diagnostics.push(Diagnostic {
                        file: file_path.to_string(),
                        line: 0,
                        column: relative_byte_offset,
                        severity: Severity::Suggestion,
                        rule_id: "Cadence.NoVerbFound".to_string(),
                        message: format!("No verb detected in this {}-word sentence. If it is a fragment, complete it; if the verb is buried late, split the sentence.", words.len()),
                        suggested_fix: None,
                    });
                }
            }
        }

        if block.text.split_whitespace().count() >= 35 {
            let total_words = block.text.split_whitespace().count();
            let mut abstract_count = 0;
            let mut examples = Vec::new();
            for mat in self.abstract_noun_regex.find_iter(&block.text) {
                let word = mat.as_str().to_lowercase();
                if !self.abstract_allowlist.contains(&word) {
                    abstract_count += 1;
                    if examples.len() < 3 {
                        examples.push(word);
                    }
                }
            }
            let pct = (abstract_count as f64 / total_words as f64) * 100.0;

            if pct > 8.0 {
                diagnostics.push(Diagnostic {
                    file: file_path.to_string(),
                    line: 0,
                    column: 0,
                    severity: Severity::Warning,
                    rule_id: "Cadence.AbstractDensity".to_string(),
                    message: format!("High abstraction density ({:.1}%): Found {} abstract nouns ({}). Replace with concrete subjects.", pct, abstract_count, examples.join(", ")),
                    suggested_fix: None,
                });
            }
        }

        if valid_sentences.len() >= 4 {
            let mut cold_starts = 0;
            for i in 0..valid_sentences.len() - 1 {
                let s_current = valid_sentences[i];
                let s_next = valid_sentences[i + 1];
                let current_stems: HashSet<String> = s_current
                    .split_whitespace()
                    .filter(|w| w.len() >= 4)
                    .map(Self::normalize_token)
                    .filter(|w| !self.stop_words.contains(w))
                    .collect();
                let next_opening: Vec<String> = s_next
                    .split_whitespace()
                    .take(7)
                    .map(Self::normalize_token)
                    .collect();
                let has_bridge = next_opening.iter().any(|w| {
                    [
                        "this",
                        "these",
                        "that",
                        "those",
                        "such",
                        "its",
                        "their",
                        "he",
                        "she",
                        "they",
                        "it",
                        "however",
                        "therefore",
                        "instead",
                        "because",
                        "while",
                        "when",
                        "if",
                    ]
                    .contains(&w.as_str())
                });
                let has_overlap = next_opening.iter().any(|w| current_stems.contains(w));
                if !has_bridge && !has_overlap {
                    cold_starts += 1;
                } else {
                    cold_starts = 0;
                }
                if cold_starts >= 3 {
                    let relative_byte_offset =
                        Self::sentence_offset(&block.text, valid_sentences[i + 1]);
                    diagnostics.push(Diagnostic {
                        file: file_path.to_string(),
                        line: 0,
                        column: relative_byte_offset,
                        severity: Severity::Suggestion,
                        rule_id: "Cadence.ColdStart".to_string(),
                        message: "Disconnected flow (Old-to-New rule): Consecutive sentences introduce new subjects without linking back.".to_string(),
                        suggested_fix: None,
                    });
                    break;
                }
            }
        }
        diagnostics
    }
}

impl Default for CadencePass {
    fn default() -> Self {
        Self::new()
    }
}

/// Common verbs that are dual-class (noun/verb) in Harper's dictionary, so
/// metadata alone cannot identify them. Supplements strict-metadata
/// detection and inflection stemming. Missing entries fail safe: the
/// delayed-verb check stays quiet rather than guessing.
const VERB_WORDLIST: &[&str] = &[
    "act",
    "add",
    "agree",
    "allow",
    "answer",
    "argue",
    "arise",
    "arrive",
    "ask",
    "attach",
    "attempt",
    "avoid",
    "became",
    "become",
    "begin",
    "believe",
    "belong",
    "break",
    "bring",
    "build",
    "call",
    "can",
    "carry",
    "catch",
    "change",
    "check",
    "choose",
    "claim",
    "close",
    "code",
    "come",
    "complete",
    "configure",
    "connect",
    "consider",
    "continue",
    "could",
    "create",
    "cut",
    "decide",
    "define",
    "delete",
    "deliver",
    "delve",
    "deploy",
    "describe",
    "design",
    "did",
    "die",
    "discuss",
    "do",
    "does",
    "download",
    "draw",
    "drive",
    "drop",
    "eat",
    "enable",
    "end",
    "enter",
    "escape",
    "explain",
    "fall",
    "feel",
    "felt",
    "fetch",
    "fight",
    "find",
    "finish",
    "fly",
    "follow",
    "forget",
    "found",
    "gave",
    "get",
    "give",
    "go",
    "grow",
    "had",
    "handle",
    "happen",
    "has",
    "have",
    "held",
    "help",
    "hit",
    "hold",
    "hope",
    "implement",
    "install",
    "is",
    "join",
    "jump",
    "keep",
    "kept",
    "kill",
    "know",
    "lead",
    "learn",
    "leave",
    "left",
    "let",
    "leverage",
    "lie",
    "like",
    "link",
    "listen",
    "live",
    "load",
    "look",
    "lose",
    "love",
    "made",
    "make",
    "makes",
    "manage",
    "may",
    "mean",
    "meet",
    "merge",
    "might",
    "miss",
    "move",
    "must",
    "need",
    "note",
    "offer",
    "open",
    "order",
    "pair",
    "parse",
    "pass",
    "pay",
    "perform",
    "pick",
    "plan",
    "play",
    "process",
    "pull",
    "push",
    "put",
    "query",
    "reach",
    "read",
    "receive",
    "release",
    "remember",
    "remove",
    "render",
    "repair",
    "report",
    "require",
    "return",
    "rise",
    "run",
    "save",
    "saw",
    "say",
    "schedule",
    "search",
    "see",
    "seem",
    "sell",
    "send",
    "serve",
    "set",
    "shall",
    "share",
    "ship",
    "should",
    "show",
    "sit",
    "speak",
    "split",
    "stand",
    "start",
    "state",
    "stay",
    "stop",
    "store",
    "suggest",
    "support",
    "sync",
    "take",
    "talk",
    "teach",
    "tell",
    "test",
    "think",
    "throw",
    "took",
    "touch",
    "track",
    "transition",
    "trigger",
    "try",
    "turn",
    "underscore",
    "understand",
    "update",
    "upload",
    "use",
    "validate",
    "wait",
    "walk",
    "want",
    "watch",
    "wear",
    "went",
    "will",
    "win",
    "work",
    "worry",
    "would",
    "write",
];
