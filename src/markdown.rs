use pulldown_cmark::{Parser, Event, Tag, TagEnd};

#[derive(Debug)]
pub struct ProseBlock {
    pub text: String,
    pub start_byte: usize,
}

pub fn extract_prose_blocks(content: &str) -> Vec<ProseBlock> {
    let parser = Parser::new(content);
    let mut blocks = Vec::new();
    let mut current_text = String::new();
    let mut block_depth = 0;
    let mut start_byte = 0;

    for (event, range) in parser.into_offset_iter() {
        match event {
            Event::Start(tag) => {
                match tag {
                    Tag::CodeBlock(_) | Tag::Heading { .. } | Tag::List(_) | Tag::BlockQuote(_) | Tag::Table(_) | Tag::HtmlBlock | Tag::MetadataBlock(_) => {
                        block_depth += 1;
                        if !current_text.is_empty() {
                            blocks.push(ProseBlock {
                                text: current_text.clone(),
                                start_byte,
                            });
                            current_text.clear();
                        }
                    }
                    Tag::Paragraph => {
                        if !current_text.is_empty() {
                            blocks.push(ProseBlock {
                                text: current_text.clone(),
                                start_byte,
                            });
                            current_text.clear();
                        }
                        start_byte = range.start;
                    }
                    _ => {}
                }
            }
            Event::End(tag_end) => {
                match tag_end {
                    TagEnd::CodeBlock | TagEnd::Heading(_) | TagEnd::List(_) | TagEnd::BlockQuote(_) | TagEnd::Table | TagEnd::HtmlBlock | TagEnd::MetadataBlock(_) => {
                        block_depth -= 1;
                        if block_depth == 0 {
                            start_byte = range.end;
                        }
                    }
                    TagEnd::Paragraph => {
                        if !current_text.is_empty() {
                            blocks.push(ProseBlock {
                                text: current_text.clone(),
                                start_byte,
                            });
                            current_text.clear();
                        }
                        start_byte = range.end;
                    }
                    _ => {}
                }
            }
            Event::Text(text) if block_depth == 0 => {
                if current_text.is_empty() {
                    start_byte = range.start;
                }
                current_text.push_str(&text);
            }
            Event::SoftBreak | Event::HardBreak if block_depth == 0 => {
                current_text.push(' ');
            }
            _ => {}
        }
    }

    if !current_text.is_empty() {
        blocks.push(ProseBlock {
            text: current_text,
            start_byte,
        });
    }

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
