use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{bail, Result};
use harper_core::spell::{Dictionary, FstDictionary, MergedDictionary, MutableDictionary};
use harper_core::DictWordMetadata;

/// Default project dictionary location, relative to the checked project.
pub const DEFAULT_DICTIONARY_PATH: &str = ".zinsser/dictionary.txt";

/// Largest dictionary file accepted, in bytes. Bigger inputs are almost
/// certainly a mistake and would exhaust memory when merged.
const MAX_DICTIONARY_BYTES: u64 = 1024 * 1024;
/// Caps on merged word count and single-word length.
const MAX_DICTIONARY_WORDS: usize = 100_000;
const MAX_WORD_CHARS: usize = 100;

/// Load project words from `.zinsser/dictionary.txt` (when present) and any
/// extra paths passed on the command line. One word per line; blank lines and
/// lines starting with `#` are ignored.
///
/// The default dictionary is found the way git finds `.git`: starting at
/// `anchor` (the directory itself, or a file's parent) and walking up, the
/// nearest `.zinsser/dictionary.txt` wins. `zinsser init` writes it at the
/// project root, so checking `docs/` or `docs/page.md` from that root must
/// still find it. Explicit `--dictionary` paths are used as given; a missing
/// or unreadable explicit path is an error, not a silent fallback.
///
/// Returns `Ok(None)` when no project words were found, so callers can fall
/// back to the curated dictionary alone.
pub fn load_user_dictionary(
    extra_paths: &[PathBuf],
    anchor: &Path,
) -> Result<Option<Arc<dyn Dictionary>>> {
    let mut words: Vec<String> = Vec::new();

    if let Some(default_path) = find_project_dictionary(anchor) {
        read_words(&default_path, &mut words);
    }
    for path in extra_paths {
        if !path.is_file() {
            bail!("Dictionary file not found: {}", path.display());
        }
        if !read_words(path, &mut words) {
            bail!("Could not read dictionary file: {}", path.display());
        }
    }

    if words.is_empty() {
        return Ok(None);
    }

    // Add lowercase/capitalized variants so a listed word works at the
    // start of a sentence too. Plural and other inflected forms still
    // need their own entries.
    let mut variants: Vec<String> = words.clone();
    for word in &words {
        let lower = word.to_lowercase();
        if lower != *word {
            variants.push(lower);
        }
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            let capitalized: String = first.to_uppercase().chain(chars.as_str().chars()).collect();
            if capitalized != *word {
                variants.push(capitalized);
            }
        }
    }

    let mut user_dict = MutableDictionary::new();
    user_dict.extend_words(variants.iter().map(|w| {
        (
            w.chars().collect::<Vec<char>>(),
            DictWordMetadata::default(),
        )
    }));

    let mut merged = MergedDictionary::new();
    merged.add_dictionary(FstDictionary::curated());
    merged.add_dictionary(Arc::new(user_dict));
    Ok(Some(Arc::new(merged)))
}

/// Nearest `.zinsser/dictionary.txt` at or above `anchor`.
pub fn find_project_dictionary(anchor: &Path) -> Option<PathBuf> {
    let start = if anchor.is_dir() {
        anchor.to_path_buf()
    } else {
        anchor.parent().map(Path::to_path_buf).unwrap_or_default()
    };
    // `Path::new("README.md").parent()` is "", which has no ancestors to walk.
    let start = if start.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        start
    };
    let start = start.canonicalize().unwrap_or(start);
    start
        .ancestors()
        .map(|dir| dir.join(DEFAULT_DICTIONARY_PATH))
        .find(|candidate| candidate.is_file())
}

/// Returns false when the file could not be read at all.
fn read_words(path: &Path, words: &mut Vec<String>) -> bool {
    match std::fs::metadata(path) {
        Ok(meta) if meta.len() > MAX_DICTIONARY_BYTES => {
            eprintln!(
                "Dictionary file {} is too large ({} bytes, max {}); skipping",
                path.display(),
                meta.len(),
                MAX_DICTIONARY_BYTES
            );
            return true;
        }
        Err(error) => {
            eprintln!("Could not read {}: {}", path.display(), error);
            return false;
        }
        _ => {}
    }
    match std::fs::read_to_string(path) {
        Ok(contents) => {
            for line in contents.lines() {
                let word = line.trim();
                if word.is_empty() || word.starts_with('#') {
                    continue;
                }
                if word.chars().count() > MAX_WORD_CHARS {
                    eprintln!(
                        "Skipping overlong dictionary entry in {} ({} chars, max {})",
                        path.display(),
                        word.chars().count(),
                        MAX_WORD_CHARS
                    );
                    continue;
                }
                if words.len() >= MAX_DICTIONARY_WORDS {
                    eprintln!(
                        "Dictionary word cap ({}) reached while reading {}; remaining entries ignored",
                        MAX_DICTIONARY_WORDS,
                        path.display()
                    );
                    break;
                }
                words.push(word.to_string());
            }
            true
        }
        Err(error) => {
            eprintln!("Could not read {}: {}", path.display(), error);
            false
        }
    }
}
