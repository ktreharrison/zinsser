use crate::types::{Diagnostic, Severity};
use crate::markdown::ProseBlock;
use regex::Regex;
use std::collections::HashSet;

pub struct CadencePass {
    abstract_noun_regex: Regex,
    stop_words: HashSet<String>,
    abstract_allowlist: HashSet<String>,
}

impl CadencePass {
    pub fn new() -> Self {
        Self {
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
        token.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase()
    }

    pub fn split_sentences(text: &str) -> Vec<&str> {
        let mut sentences = Vec::new();
        let mut start = 0;
        let chars: Vec<char> = text.chars().collect();

        for i in 0..chars.len() {
            let c = chars[i];
            if c == '.' || c == '!' || c == '?' {
                if Self::is_abbreviation_at(text, &chars, i) {
                    continue;
                }

                let byte_idx = text.char_indices().nth(i).map(|(idx, _)| idx).unwrap_or(text.len());
                sentences.push(&text[start..byte_idx + 1]);
                start = byte_idx + 1;
            }
        }
        if start < text.len() {
            sentences.push(&text[start..]);
        }
        sentences
    }

    fn is_abbreviation_at(_text: &str, chars: &[char], i: usize) -> bool {
        if chars[i] != '.' {
            return false;
        }

        if i > 0 && chars[i - 1].is_ascii_digit() && i + 1 < chars.len() && chars[i + 1].is_ascii_digit() {
            return true;
        }

        if i + 2 < chars.len() && chars[i + 1].is_ascii_alphabetic() && chars[i + 2] == '.' {
            return true;
        }

        let prefix: String = chars[..=i].iter().collect();
        let last_token = prefix.split_whitespace().last().unwrap_or("");
        let clean_token = last_token
            .trim_start_matches(|c: char| !c.is_alphanumeric())
            .to_lowercase();

        let abbrevs = [
            "e.g.", "i.e.", "dr.", "mr.", "mrs.", "ms.", "prof.", "vs.", "etc.",
            "fig.", "inc.", "ltd.", "jr.", "sr.", "st.",
        ];

        abbrevs.contains(&clean_token.as_str())
    }

    fn find_first_verb_index(&self, words: &[&str]) -> Option<usize> {
        for (idx, &w) in words.iter().enumerate() {
            if (idx == 0 || idx == 1) && w.to_lowercase().ends_with("ing") {
                continue;
            }

            let w_norm = Self::normalize_token(w);

            if ["is", "are", "was", "were", "has", "have", "had", "can", "could", "will", "would", "shall", "should", "may", "might", "must", "do", "does", "did", "provide", "makes", "perform", "achieve", "build", "run", "use", "create", "set", "start", "connect", "improve", "reduce", "found", "saw", "went", "made", "became", "gave", "took", "felt", "kept", "held"].contains(&w_norm.as_str()) {
                return Some(idx);
            }

            if w_norm.ends_with("ed") {
                let is_adj = if idx > 0 {
                    let prev = Self::normalize_token(words[idx-1]);
                    ["a", "an", "the", "very", "incredibly", "extremely", "highly", "quite"].contains(&prev.as_str())
                } else {
                    false
                };
                if !is_adj {
                    return Some(idx);
                }
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
                if trimmed.is_empty() { continue; }

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

        let avg_len = if total_sentences > 0 { total_words as f64 / total_sentences as f64 } else { 0.0 };
        let variance = if all_lengths.len() > 1 {
            let mean = avg_len;
            all_lengths.iter().map(|&l| (l as f64 - mean).powi(2)).sum::<f64>() / (all_lengths.len() - 1) as f64
        } else { 0.0 };
        let stddev = variance.sqrt();
        let abstract_pct = if total_words > 0 { (abstract_nouns as f64 / total_words as f64) * 100.0 } else { 0.0 };
        let avg_sv = if sv_count > 0 { total_sv_dist as f64 / sv_count as f64 } else { 0.0 };

        (total_words, total_sentences, avg_len, stddev, abstract_pct, avg_sv)
    }

    pub fn check_block(&self, block: &ProseBlock, file_path: &str, _content: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let sentences = Self::split_sentences(&block.text);

        let mut valid_sentences = Vec::new();
        for s in &sentences {
            if !s.trim().is_empty() {
                valid_sentences.push(*s);
            }
        }

        if valid_sentences.is_empty() { return diagnostics; }

        if valid_sentences.len() >= 4 {
            let lengths: Vec<usize> = valid_sentences.iter()
                .map(|s| s.split_whitespace().count())
                .collect();
            for i in 0..=lengths.len() - 4 {
                let window = &lengths[i..i+4];
                let min_len = *window.iter().min().unwrap();
                let max_len = *window.iter().max().unwrap();
                if min_len >= 8 && (max_len - min_len) <= 5 {
                    let relative_byte_offset = sentences[i].as_ptr() as usize - block.text.as_ptr() as usize;
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
                let relative_byte_offset = sent.as_ptr() as usize - block.text.as_ptr() as usize;
                if let Some(idx) = self.find_first_verb_index(&words) {
                    if idx >= 14 {
                        diagnostics.push(Diagnostic {
                            file: file_path.to_string(),
                            line: 0,
                            column: relative_byte_offset,
                            severity: Severity::Warning,
                            rule_id: "Cadence.DelayedVerb".to_string(),
                            message: format!("Delayed verb: Reader waits {} words before reaching the main verb.", idx + 1),
                            suggested_fix: None,
                        });
                    }
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
                let s_next = valid_sentences[i+1];
                let current_stems: HashSet<String> = s_current.split_whitespace()
                    .filter(|w| w.len() >= 4)
                    .map(Self::normalize_token)
                    .filter(|w| !self.stop_words.contains(w))
                    .collect();
                let next_opening: Vec<String> = s_next.split_whitespace()
                    .take(7)
                    .map(Self::normalize_token)
                    .collect();
                let has_bridge = next_opening.iter().any(|w| {
                    ["this", "these", "that", "those", "such", "its", "their", "he", "she", "they", "it", "however", "therefore", "instead", "because", "while", "when", "if"].contains(&w.as_str())
                });
                let has_overlap = next_opening.iter().any(|w| current_stems.contains(w));
                if !has_bridge && !has_overlap {
                    cold_starts += 1;
                } else {
                    cold_starts = 0;
                }
                if cold_starts >= 3 {
                    let relative_byte_offset = sentences[i+1].as_ptr() as usize - block.text.as_ptr() as usize;
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
