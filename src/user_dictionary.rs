use std::path::{Path, PathBuf};
use std::sync::Arc;

use harper_core::WordMetadata;
use harper_core::{Dictionary, FullDictionary, MergedDictionary};

/// Default project dictionary location, relative to the working directory.
pub const DEFAULT_DICTIONARY_PATH: &str = ".zinsser/dictionary.txt";

/// Load project words from `.zinsser/dictionary.txt` (when present) and any
/// extra paths passed on the command line. One word per line; blank lines and
/// lines starting with `#` are ignored.
///
/// Returns `None` when no project words were found, so callers can fall back
/// to the curated dictionary alone.
pub fn load_user_dictionary(extra_paths: &[PathBuf]) -> Option<Arc<dyn Dictionary>> {
    let mut words: Vec<String> = Vec::new();

    let default_path = Path::new(DEFAULT_DICTIONARY_PATH);
    if default_path.is_file() {
        read_words(default_path, &mut words);
    }
    for path in extra_paths {
        if path.is_file() {
            read_words(path, &mut words);
        } else {
            eprintln!("Dictionary file not found: {}", path.display());
        }
    }

    if words.is_empty() {
        return None;
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

    let mut user_dict = FullDictionary::new();
    user_dict.extend_words(
        variants
            .iter()
            .map(|w| (w.chars().collect::<Vec<char>>(), WordMetadata::default())),
    );

    let mut merged = MergedDictionary::new();
    merged.add_dictionary(FullDictionary::curated());
    merged.add_dictionary(Arc::new(user_dict));
    Some(Arc::new(merged))
}

fn read_words(path: &Path, words: &mut Vec<String>) {
    match std::fs::read_to_string(path) {
        Ok(contents) => {
            for line in contents.lines() {
                let word = line.trim();
                if !word.is_empty() && !word.starts_with('#') {
                    words.push(word.to_string());
                }
            }
        }
        Err(error) => eprintln!("Could not read {}: {}", path.display(), error),
    }
}
