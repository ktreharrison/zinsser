use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

#[derive(Debug, Default, Clone)]
pub struct ProseBlock {
    pub text: String,
    pub start_byte: usize,
    /// `(offset in text, offset in source)` at the start of each run of
    /// source text. Inline markup (link targets, emphasis markers, inline
    /// code) is dropped from `text`, so `start_byte + offset` drifts after
    /// the first piece of markup. Empty for hand-built blocks, which fall
    /// back to `start_byte + offset`.
    pub segments: Vec<(usize, usize)>,
}

impl ProseBlock {
    pub fn new(text: impl Into<String>, start_byte: usize) -> Self {
        Self {
            text: text.into(),
            start_byte,
            segments: Vec::new(),
        }
    }

    /// Map a byte offset in `text` to a byte offset in the source document.
    pub fn source_offset(&self, text_offset: usize) -> usize {
        match self.segments.partition_point(|&(t, _)| t <= text_offset) {
            0 => self.start_byte.saturating_add(text_offset),
            i => {
                let (t, s) = self.segments[i - 1];
                s + (text_offset - t)
            }
        }
    }
}

fn flush(
    blocks: &mut Vec<ProseBlock>,
    text: &mut String,
    segments: &mut Vec<(usize, usize)>,
    start_byte: usize,
) {
    if !text.is_empty() {
        blocks.push(ProseBlock {
            text: std::mem::take(text),
            start_byte,
            segments: std::mem::take(segments),
        });
    }
    segments.clear();
}

pub fn extract_prose_blocks(content: &str) -> Vec<ProseBlock> {
    // Enable tables and frontmatter so their contents are excluded from
    // prose analysis instead of being read as sentences.
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS;
    let parser = Parser::new_ext(content, options);
    let mut blocks = Vec::new();
    let mut current_text = String::new();
    let mut segments: Vec<(usize, usize)> = Vec::new();
    let mut block_depth = 0;
    let mut start_byte = 0;

    for (event, range) in parser.into_offset_iter() {
        match event {
            Event::Start(tag) => match tag {
                Tag::CodeBlock(_)
                | Tag::Heading { .. }
                | Tag::List(_)
                | Tag::BlockQuote(_)
                | Tag::Table(_)
                | Tag::HtmlBlock
                | Tag::MetadataBlock(_) => {
                    block_depth += 1;
                    flush(&mut blocks, &mut current_text, &mut segments, start_byte);
                }
                Tag::Paragraph => {
                    flush(&mut blocks, &mut current_text, &mut segments, start_byte);
                    start_byte = range.start;
                }
                _ => {}
            },
            Event::End(tag_end) => match tag_end {
                TagEnd::CodeBlock
                | TagEnd::Heading(_)
                | TagEnd::List(_)
                | TagEnd::BlockQuote(_)
                | TagEnd::Table
                | TagEnd::HtmlBlock
                | TagEnd::MetadataBlock(_) => {
                    block_depth -= 1;
                    if block_depth == 0 {
                        start_byte = range.end;
                    }
                }
                TagEnd::Paragraph => {
                    flush(&mut blocks, &mut current_text, &mut segments, start_byte);
                    start_byte = range.end;
                }
                _ => {}
            },
            Event::Text(text) if block_depth == 0 => {
                if current_text.is_empty() {
                    start_byte = range.start;
                }
                segments.push((current_text.len(), range.start));
                current_text.push_str(&text);
            }
            Event::SoftBreak | Event::HardBreak if block_depth == 0 => {
                segments.push((current_text.len(), range.start));
                current_text.push(' ');
            }
            _ => {}
        }
    }

    flush(&mut blocks, &mut current_text, &mut segments, start_byte);
    blocks
}

/// Text blocks in the scope Vale lints: paragraphs, headings, list items,
/// blockquotes and table cells, joined across inline markup and soft breaks.
/// Code blocks, inline code, raw HTML, front matter and link destinations are
/// skipped. Each block keeps a source map, like prose blocks.
pub fn extract_style_blocks(content: &str) -> Vec<ProseBlock> {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS;
    let mut blocks = Vec::new();
    let mut text = String::new();
    let mut segments: Vec<(usize, usize)> = Vec::new();
    let mut start_byte = 0;
    // Depth inside code blocks, HTML blocks and front matter.
    let mut skip_depth = 0usize;

    for (event, range) in Parser::new_ext(content, options).into_offset_iter() {
        match event {
            Event::Start(Tag::CodeBlock(_) | Tag::HtmlBlock | Tag::MetadataBlock(_)) => {
                flush(&mut blocks, &mut text, &mut segments, start_byte);
                skip_depth += 1;
            }
            Event::End(TagEnd::CodeBlock | TagEnd::HtmlBlock | TagEnd::MetadataBlock(_)) => {
                skip_depth = skip_depth.saturating_sub(1);
            }
            Event::Start(
                Tag::Paragraph
                | Tag::Heading { .. }
                | Tag::Item
                | Tag::TableCell
                | Tag::FootnoteDefinition(_),
            )
            | Event::End(
                TagEnd::Paragraph
                | TagEnd::Heading(_)
                | TagEnd::Item
                | TagEnd::TableCell
                | TagEnd::FootnoteDefinition,
            ) => flush(&mut blocks, &mut text, &mut segments, start_byte),
            Event::Text(t) if skip_depth == 0 => {
                if text.is_empty() {
                    start_byte = range.start;
                }
                segments.push((text.len(), range.start));
                text.push_str(&t);
            }
            Event::SoftBreak | Event::HardBreak if skip_depth == 0 && !text.is_empty() => {
                segments.push((text.len(), range.start));
                text.push(' ');
            }
            // Inline code, inline HTML and everything else are not prose.
            Event::Code(_) | Event::InlineHtml(_) | Event::Html(_) if !text.is_empty() => {
                // Keep words on either side apart, as Vale does.
                segments.push((text.len(), range.start));
                text.push(' ');
            }
            _ => {}
        }
    }
    flush(&mut blocks, &mut text, &mut segments, start_byte);
    blocks
}

pub fn get_line_column(content: &str, byte_offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut column = 1;
    for (idx, c) in content.char_indices() {
        if idx >= byte_offset {
            break;
        }
        if c == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}
