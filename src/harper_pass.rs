use crate::types::{Diagnostic, Severity};
use anyhow::Result;
use harper_core::linting::{LintGroup, LintGroupConfig, Linter};
use harper_core::{Document, FullDictionary};

pub struct HarperPass;

impl HarperPass {
    pub fn lint(content: &str, file_path: &str) -> Result<Vec<Diagnostic>> {
        let dictionary = FullDictionary::curated();
        let doc = Document::new_markdown_default_curated(content);
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
