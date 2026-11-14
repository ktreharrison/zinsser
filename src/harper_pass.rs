use crate::types::{Diagnostic, Severity};
use anyhow::Result;
use harper_core::linting::{LintGroup, LintGroupConfig, LintKind, Linter};
use harper_core::Dictionary;
use harper_core::{Document, FullDictionary};
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

/// Remove an ordered-list marker (`1.`, `2)`, `10.`) from the text before a
/// link, so numbered navigation lists count as link-only lines too. As in
/// CommonMark, the marker must be followed by whitespace: `3.[x]` starts a
/// paragraph, not a list item, and keeps its spelling checks.
fn strip_ordered_marker(before: &str) -> &str {
    let digits = before.chars().take_while(char::is_ascii_digit).count();
    let mut rest = before[digits..].chars();
    match (rest.next(), rest.next()) {
        (Some('.' | ')'), Some(c)) if digits > 0 && c.is_whitespace() => &before[digits + 1..],
        _ => before,
    }
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
        let is_marker = |c: char| c.is_whitespace() || bullets.contains(&c);
        // Strip the marker before trimming trailing whitespace: the space
        // after `1.` is what makes it a list marker.
        let before =
            strip_ordered_marker(content[line_start..range.start].trim_start_matches(is_marker))
                .trim_matches(is_marker);
        let after = content[range.end..line_end]
            .trim_matches(|c: char| c.is_whitespace() || bullets.contains(&c) || c == ',');
        if before.is_empty() && after.is_empty() {
            out.push(range);
        }
    }
    out
}

/// Whether `pos` falls inside one of `ranges`, which must be sorted by start
/// and non-overlapping (links do not nest, and the parser reports them in
/// source order).
fn in_sorted_ranges(ranges: &[Range<usize>], pos: usize) -> bool {
    let i = ranges.partition_point(|r| r.start <= pos);
    i > 0 && ranges[i - 1].contains(&pos)
}

/// Spelling lints on tokens that are not words. harper-core 0.16 reads a
/// possessive right after a link as one token with the link target
/// (`FastAPI](https://fastapi.tiangolo.com)'s`); other versions flag only the
/// `s`. (harper-core 0.16 does not flag file extensions such as the `py` of
/// `table_movie.py`, so no filter is needed for those here.)
fn is_not_a_word(chars: &[char], start: usize, end: usize) -> bool {
    let word: String = chars
        .get(start..end.min(chars.len()))
        .unwrap_or(&[])
        .iter()
        .collect();
    let prev = |n: usize| start.checked_sub(n).and_then(|i| chars.get(i)).copied();
    let spans_link_target = word.contains("](");
    let possessive_after_link = matches!(prev(1), Some('\'' | '’')) && prev(2) == Some(')');
    spans_link_target || possessive_after_link
}

/// Decides which Harper spelling lints are not real misspellings, with all
/// per-document work done once so each lint costs O(log n): Harper spans are
/// char indices, mapped to byte offsets through a table, and link-only ranges
/// are searched by binary search.
pub(crate) struct SpellingFilter {
    chars: Vec<char>,
    byte_of: Vec<usize>,
    link_only: Vec<Range<usize>>,
    len: usize,
}

impl SpellingFilter {
    pub(crate) fn new(content: &str) -> Self {
        let mut link_only = link_only_line_ranges(content);
        link_only.sort_by_key(|r| r.start);
        Self {
            chars: content.chars().collect(),
            byte_of: content.char_indices().map(|(b, _)| b).collect(),
            link_only,
            len: content.len(),
        }
    }

    pub(crate) fn byte_offset(&self, char_index: usize) -> usize {
        self.byte_of.get(char_index).copied().unwrap_or(self.len)
    }

    /// Whether a spelling lint over chars `start..end` should be dropped.
    pub(crate) fn skips(&self, start: usize, end: usize) -> bool {
        is_non_english_word(&self.chars, start, end)
            || is_not_a_word(&self.chars, start, end)
            || in_sorted_ranges(&self.link_only, self.byte_offset(start))
    }
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

        let filter = SpellingFilter::new(content);
        let mut diagnostics = Vec::new();
        for lint in lints {
            if matches!(lint.lint_kind, LintKind::Spelling)
                && filter.skips(lint.span.start, lint.span.end)
            {
                continue;
            }
            diagnostics.push(Diagnostic {
                file: file_path.to_string(),
                line: 0,
                // A per-lint `char_indices().nth()` here was quadratic in the
                // number of lints; the precomputed table is O(1).
                column: filter.byte_offset(lint.span.start),
                severity: Severity::Suggestion,
                rule_id: format!("Harper.{:?}", lint.lint_kind),
                message: lint.message.to_string(),
                suggested_fix: lint.suggestions.first().map(|s| format!("{}", s)),
            });
        }

        Ok(diagnostics)
    }
}

#[cfg(test)]
mod tests {
    use super::{in_sorted_ranges, link_only_line_ranges, SpellingFilter};
    use std::time::{Duration, Instant};

    #[test]
    fn sorted_range_lookup_matches_a_linear_scan() {
        let ranges = vec![2..5, 10..11, 20..30];
        for pos in 0..35 {
            let linear = ranges.iter().any(|r| r.contains(&pos));
            assert_eq!(in_sorted_ranges(&ranges, pos), linear, "pos {pos}");
        }
        assert!(!in_sorted_ranges(&[], 3));
    }

    #[test]
    fn ordered_marker_needs_whitespace_after_it() {
        // CommonMark: `3. [x]` is a list item, `3.[x]` is a paragraph whose
        // link text keeps its spelling checks. (Checked on the ranges directly:
        // harper-core 0.16 itself panics on `3.[...` in debug builds.)
        assert_eq!(
            link_only_line_ranges("3. [Deutsche](README.de.md)\n").len(),
            1
        );
        assert_eq!(
            link_only_line_ranges("10) [Polskie](README.pl.md)\n").len(),
            1
        );
        assert!(link_only_line_ranges("3.[teh guide](guide.md)\n").is_empty());
        assert!(link_only_line_ranges("3)[teh guide](guide.md)\n").is_empty());
    }

    #[test]
    fn spelling_filter_scales_to_large_link_lists() {
        // Goes through the production filter on a real document: 100k
        // link-only lines, then a lookup at every link label. A linear scan
        // of the link ranges per lint would be ~10^10 comparisons (minutes).
        let content: String = (0..100_000)
            .map(|i| format!("- [Wrdz{i}](page{i}.md)\n"))
            .collect();
        let filter = SpellingFilter::new(&content);
        // ASCII content, so a byte offset is also the char index.
        assert!(content.is_ascii());
        let label_starts: Vec<usize> = content.match_indices('[').map(|(b, _)| b + 1).collect();
        let start = Instant::now();
        let skipped = label_starts
            .iter()
            .filter(|&&i| filter.skips(i, i + 4))
            .count();
        assert_eq!(skipped, 100_000, "every link-only label is skipped");
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "took {:?}",
            start.elapsed()
        );
    }
}
