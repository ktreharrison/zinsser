use crate::types::{Diagnostic, Severity};
use anyhow::Result;
use harper_core::linting::{LintGroup, LintKind, Linter};
use harper_core::spell::{Dictionary, FstDictionary, MergedDictionary};
use harper_core::{Dialect, Document};
use pulldown_cmark::{Event, Options, Parser, Tag};
use std::ops::Range;
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

/// Byte ranges of Markdown links that are the only content of their line,
/// apart from a list marker or bullet: translation and navigation lists
/// (`- [Deutsche](README.de.md)`, rich's `• [Polskie readme](...)`). Their
/// text is a language or file name, not prose, so spelling is not checked
/// there (issue #2). A link inside a sentence is still checked.
fn link_only_line_ranges(content: &str) -> Vec<Range<usize>> {
    let bullets: &[char] = &['-', '*', '+', '•', '·', '|'];
    let mut out = Vec::new();
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    for (event, range) in Parser::new_ext(content, options).into_offset_iter() {
        if !matches!(event, Event::Start(Tag::Link { .. })) {
            continue;
        }
        let line_start = content[..range.start].rfind('\n').map_or(0, |i| i + 1);
        let line_end = content[range.end..]
            .find('\n')
            .map_or(content.len(), |i| range.end + i);
        let before = content[line_start..range.start]
            .trim_matches(|c: char| c.is_whitespace() || bullets.contains(&c));
        let after = content[range.end..line_end]
            .trim_matches(|c: char| c.is_whitespace() || bullets.contains(&c) || c == ',');
        if before.is_empty() && after.is_empty() {
            out.push(range);
        }
    }
    out
}

/// Spelling lints on tokens that are not words: the `s` of a possessive right
/// after a link (`[FastAPI](...)'s`), and a file extension split off a name in
/// link text or prose (`table_movie.py` -> `py`).
fn is_not_a_word(chars: &[char], start: usize) -> bool {
    let prev = |n: usize| start.checked_sub(n).and_then(|i| chars.get(i)).copied();
    let possessive_after_link = matches!(prev(1), Some('\'' | '’')) && prev(2) == Some(')');
    let file_extension =
        prev(1) == Some('.') && prev(2).is_some_and(|c| c.is_alphanumeric() || c == '_');
    possessive_after_link || file_extension
}

/// Upstream rules that write debug output to stderr; see `lint_with_dictionary`.
const NOISY_RULES: [&str; 2] = ["SeemsLikeAdjective", "ExpandIme"];

pub struct HarperPass;

impl HarperPass {
    /// Lint with the curated dictionary only. The binary always goes
    /// through `lint_with_dictionary`.
    pub fn lint(content: &str, file_path: &str) -> Result<Vec<Diagnostic>> {
        Self::lint_with_dictionary(content, file_path, FstDictionary::curated())
    }

    pub fn lint_with_dictionary(
        content: &str,
        file_path: &str,
        dictionary: Arc<dyn Dictionary>,
    ) -> Result<Vec<Diagnostic>> {
        // Parse with the same dictionary so project words get real metadata
        // instead of being treated as unknown words.
        // harper-core 3 takes a sized dictionary type, so wrap the trait object.
        let mut merged = MergedDictionary::new();
        merged.add_dictionary(dictionary);
        let dictionary = Arc::new(merged);
        let doc = Document::new_markdown_default(content, dictionary.as_ref());
        let mut linter = LintGroup::new_curated(dictionary, Dialect::American);
        // harper-core 3.2.0 ships two rules that print a debug line to stderr
        // on every match (`eprintln!("🚨 ...")` in seems_like_adjective.rs and
        // expand_ime.rs). Disable them so zinsser's output stays clean.
        for rule in NOISY_RULES {
            linter.config.set_rule_enabled(rule, false);
        }
        let lints = linter.lint(&doc);

        let chars: Vec<char> = content.chars().collect();
        // Harper spans are char indices; map them to byte offsets once.
        let byte_of: Vec<usize> = content.char_indices().map(|(b, _)| b).collect();
        let to_byte = |char_index: usize| byte_of.get(char_index).copied().unwrap_or(content.len());
        let link_only = link_only_line_ranges(content);
        let mut diagnostics = Vec::new();
        for lint in lints {
            if matches!(lint.lint_kind, LintKind::Spelling) {
                let start_byte = to_byte(lint.span.start);
                if is_non_english_word(&chars, lint.span.start, lint.span.end)
                    || is_not_a_word(&chars, lint.span.start)
                    || link_only.iter().any(|r| r.contains(&start_byte))
                {
                    continue;
                }
            }
            diagnostics.push(Diagnostic {
                file: file_path.to_string(),
                line: 0,
                column: to_byte(lint.span.start),
                severity: Severity::Suggestion,
                rule_id: format!("Harper.{:?}", lint.lint_kind),
                message: lint.message.to_string(),
                suggested_fix: lint.suggestions.first().map(|s| format!("{}", s)),
            });
        }

        Ok(diagnostics)
    }
}
