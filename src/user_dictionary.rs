use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{bail, Result};
use harper_core::WordMetadata;
use harper_core::{Dictionary, FullDictionary, MergedDictionary};

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
/// still find it. Explicit `--dictionary` paths are used as given; a missing,
/// unreadable, oversized or empty explicit file is an error, not a silent
/// fallback. The default file is optional: problems with it only warn.
///
/// Returns `Ok(None)` when no project words were found, so callers can fall
/// back to the curated dictionary alone.
pub fn load_user_dictionary(
    extra_paths: &[PathBuf],
    anchor: &Path,
) -> Result<Option<Arc<dyn Dictionary>>> {
    let mut words: Vec<String> = Vec::new();

    if let Some(default_path) = find_project_dictionary(anchor) {
        match read_words(&default_path, &mut words) {
            ReadOutcome::Read { usable: 0 } => eprintln!(
                "Dictionary file {} has no usable words; using the curated dictionary only",
                default_path.display()
            ),
            ReadOutcome::Read { .. } => {}
            ReadOutcome::TooLarge(len) => eprintln!(
                "Dictionary file {} is too large ({} bytes, max {}); skipping",
                default_path.display(),
                len,
                MAX_DICTIONARY_BYTES
            ),
            ReadOutcome::Unreadable(error) => {
                eprintln!("Could not read {}: {}", default_path.display(), error)
            }
        }
    }
    for path in extra_paths {
        if !path.is_file() {
            bail!("Dictionary file not found: {}", path.display());
        }
        match read_words(path, &mut words) {
            ReadOutcome::Read { usable: 0 } => {
                bail!("Dictionary file {} has no usable words", path.display())
            }
            ReadOutcome::Read { .. } => {}
            ReadOutcome::TooLarge(len) => bail!(
                "Dictionary file {} is too large ({} bytes, max {})",
                path.display(),
                len,
                MAX_DICTIONARY_BYTES
            ),
            ReadOutcome::Unreadable(error) => {
                bail!(
                    "Could not read dictionary file {}: {}",
                    path.display(),
                    error
                )
            }
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

    let mut user_dict = FullDictionary::new();
    user_dict.extend_words(
        variants
            .iter()
            .map(|w| (w.chars().collect::<Vec<char>>(), WordMetadata::default())),
    );

    let mut merged = MergedDictionary::new();
    merged.add_dictionary(FullDictionary::curated());
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

/// What reading one dictionary file produced. The caller decides whether a
/// problem is a warning (default file) or an error (explicit file).
enum ReadOutcome {
    /// `usable` counts the file's valid entries, whether or not the global
    /// word cap let them all in, so "empty file" does not depend on what
    /// other dictionaries loaded first.
    Read {
        usable: usize,
    },
    TooLarge(u64),
    Unreadable(String),
}

/// Read at most `MAX_DICTIONARY_BYTES + 1` bytes through one open handle, so
/// the size check and the read see the same file even if it is replaced or
/// grows meanwhile.
fn read_capped(path: &Path) -> Result<String, ReadOutcome> {
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(|e| ReadOutcome::Unreadable(e.to_string()))?;
    let mut bytes = Vec::new();
    file.take(MAX_DICTIONARY_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| ReadOutcome::Unreadable(e.to_string()))?;
    if bytes.len() as u64 > MAX_DICTIONARY_BYTES {
        let len = std::fs::metadata(path).map_or(bytes.len() as u64, |m| m.len());
        return Err(ReadOutcome::TooLarge(len.max(bytes.len() as u64)));
    }
    String::from_utf8(bytes).map_err(|e| ReadOutcome::Unreadable(e.to_string()))
}

fn read_words(path: &Path, words: &mut Vec<String>) -> ReadOutcome {
    let mut usable = 0;
    let mut cap_reported = false;
    match read_capped(path) {
        Err(outcome) => outcome,
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
                usable += 1;
                if words.len() >= MAX_DICTIONARY_WORDS {
                    if !cap_reported {
                        eprintln!(
                            "Dictionary word cap ({}) reached while reading {}; remaining entries ignored",
                            MAX_DICTIONARY_WORDS,
                            path.display()
                        );
                        cap_reported = true;
                    }
                    continue;
                }
                words.push(word.to_string());
            }
            ReadOutcome::Read { usable }
        }
    }
}
