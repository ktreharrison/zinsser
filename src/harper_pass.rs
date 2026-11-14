use crate::types::{Diagnostic, Severity};
use anyhow::Result;
use harper_core::linting::{LintGroup, LintGroupConfig, LintKind, Linter};
use harper_core::Dictionary;
use harper_core::{Document, FullDictionary};
use std::sync::Arc;

/// Harper's spelling check is English-only, so a word containing a letter
/// outside ASCII (`Français`, `दी`, `Türkçe`) is not English and cannot be
/// misspelled English. Translation lists in READMEs were the main source of
/// spelling noise (issue #2: 16 of rich's 20 remaining hits). The cost is that
/// typos in such words (`naïve`) go unreported, acceptable for an English
/// prose checker.
fn is_non_english_word(chars: &[char], start: usize, end: usize) -> bool {
    chars
        .get(start..end.min(chars.len()))
        .is_some_and(|word| word.iter().any(|c| c.is_alphabetic() && !c.is_ascii()))
}

pub struct HarperPass;

impl HarperPass {
    /// Lint with the curated dictionary only. The binary always goes
    /// through `lint_with_dictionary`.
    pub fn lint(content: &str, file_path: &str) -> Result<Vec<Diagnostic>> {
        Self::lint_with_dictionary(content, file_path, FullDictionary::curated())
    }

    pub fn lint_with_dictionary(
        content: &str,
        file_path: &str,
        dictionary: Arc<dyn Dictionary>,
    ) -> Result<Vec<Diagnostic>> {
        // Parse with the same dictionary so project words get real metadata
        // instead of being treated as unknown words.
        let doc = Document::new_markdown_default(content, &dictionary);
        let mut linter = LintGroup::new(LintGroupConfig::default(), dictionary);
        let lints = linter.lint(&doc);

        let chars: Vec<char> = content.chars().collect();
        let mut diagnostics = Vec::new();
        for lint in lints {
            if matches!(lint.lint_kind, LintKind::Spelling)
                && is_non_english_word(&chars, lint.span.start, lint.span.end)
            {
                continue;
            }
            diagnostics.push(Diagnostic {
                file: file_path.to_string(),
                line: 0,
                column: content
                    .char_indices()
                    .nth(lint.span.start)
                    .map_or(content.len(), |(byte_offset, _)| byte_offset),
                severity: Severity::Suggestion,
                rule_id: format!("Harper.{:?}", lint.lint_kind),
                message: lint.message.to_string(),
                suggested_fix: lint.suggestions.first().map(|s| format!("{}", s)),
            });
        }

        Ok(diagnostics)
    }
}
