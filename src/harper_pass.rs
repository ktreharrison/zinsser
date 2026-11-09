use crate::types::{Diagnostic, Severity};
use anyhow::Result;
use harper_core::linting::{LintGroup, LintGroupConfig, Linter};
use harper_core::Dictionary;
use harper_core::{Document, FullDictionary};
use std::sync::Arc;

pub struct HarperPass;

impl HarperPass {
    /// Lint with the curated dictionary only. Used by tests; the binary
    /// always goes through `lint_with_dictionary`.
    #[allow(dead_code)]
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

        let mut diagnostics = Vec::new();
        for lint in lints {
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
