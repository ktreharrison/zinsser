mod cadence_pass;
mod harper_pass;
mod markdown;
mod reporter;
mod scoring;
mod types;
mod user_dictionary;
mod vale_runner;

use anyhow::Result;
use cadence_pass::CadencePass;
use clap::{Parser, Subcommand};
use harper_core::Dictionary;
use harper_core::FullDictionary;
use harper_pass::HarperPass;
use markdown::{extract_prose_blocks, get_line_column};
use reporter::Reporter;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use types::{Diagnostic, DocumentScore, Severity};
use user_dictionary::load_user_dictionary;
use vale_runner::ValeRunner;

#[derive(Parser)]
#[command(name = "zinsser")]
#[command(about = "Local-first prose linter and cadence engine", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check a file or recursively check a directory, skipping .git, target, and node_modules
    Check {
        path: PathBuf,
        #[arg(long)]
        format: Option<String>,
        #[arg(long)]
        no_vale: bool,
        #[arg(long)]
        no_harper: bool,
        /// Extra dictionary file (one word per line). `.zinsser/dictionary.txt` is always loaded when present.
        #[arg(long)]
        dictionary: Vec<PathBuf>,
    },
    /// Generate a craft scorecard for a file
    Score {
        path: PathBuf,
        /// Extra dictionary file (one word per line). `.zinsser/dictionary.txt` is always loaded when present.
        #[arg(long)]
        dictionary: Vec<PathBuf>,
    },
    /// Install missing Vale rules and initialize .vale.ini in the current directory
    Init,
}

const VALE_ASSETS: [(&str, &str); 10] = [
    (
        "styles/Zinsser/AI_Tells.yml",
        include_str!("../styles/Zinsser/AI_Tells.yml"),
    ),
    (
        "styles/Zinsser/CadenceMonotony.yml",
        include_str!("../styles/Zinsser/CadenceMonotony.yml"),
    ),
    (
        "styles/Zinsser/Clutter.yml",
        include_str!("../styles/Zinsser/Clutter.yml"),
    ),
    (
        "styles/Zinsser/Greybeard.yml",
        include_str!("../styles/Zinsser/Greybeard.yml"),
    ),
    (
        "styles/Zinsser/Hedging.yml",
        include_str!("../styles/Zinsser/Hedging.yml"),
    ),
    (
        "styles/Zinsser/Nominalizations.yml",
        include_str!("../styles/Zinsser/Nominalizations.yml"),
    ),
    (
        "styles/Zinsser/PassiveVoice.yml",
        include_str!("../styles/Zinsser/PassiveVoice.yml"),
    ),
    (
        "styles/Zinsser/Puffery.yml",
        include_str!("../styles/Zinsser/Puffery.yml"),
    ),
    (
        "styles/Zinsser/WeaselWords.yml",
        include_str!("../styles/Zinsser/WeaselWords.yml"),
    ),
    (
        "styles/config/scripts/CadenceMonotony.tengo",
        include_str!("../styles/config/scripts/CadenceMonotony.tengo"),
    ),
];

fn write_if_missing(path: &Path, contents: &str) -> Result<bool> {
    use std::io::Write;

    let mut file = match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists && path.is_file() => {
            return Ok(false);
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            anyhow::bail!("{} exists and is not a file", path.display());
        }
        Err(error) => return Err(error.into()),
    };

    if let Err(error) = file.write_all(contents.as_bytes()) {
        drop(file);
        let _ = std::fs::remove_file(path);
        return Err(error.into());
    }
    Ok(true)
}

fn collect_markdown_files(path: PathBuf) -> Result<Vec<PathBuf>> {
    if !path.is_dir() {
        return Ok(vec![path]);
    }

    let mut pending = vec![path];
    let mut files = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(directory)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let entry_path = entry.path();
            if file_type.is_dir() {
                if ![".git", "target", "node_modules"]
                    .contains(&entry.file_name().to_str().unwrap_or(""))
                {
                    pending.push(entry_path);
                }
            } else if file_type.is_file() && entry_path.extension().is_some_and(|ext| ext == "md") {
                files.push(entry_path);
            }
        }
    }
    files.sort();
    Ok(files)
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Check {
            path,
            format,
            no_vale,
            no_harper,
            dictionary,
        } => {
            let dictionary: Arc<dyn Dictionary> = load_user_dictionary(&dictionary, &path)?
                .unwrap_or_else(|| FullDictionary::curated());
            let files = collect_markdown_files(path)?;
            let mut pass_failed = false;
            let mut all_diagnostics = Vec::new();

            for file_path in files {
                let content = std::fs::read_to_string(&file_path)?;
                let path_str = file_path.to_string_lossy();

                // 1. Harper Pass
                if !no_harper {
                    match HarperPass::lint_with_dictionary(&content, &path_str, dictionary.clone())
                    {
                        Ok(mut diags) => {
                            for d in &mut diags {
                                (d.line, d.column) = get_line_column(&content, d.column);
                            }
                            all_diagnostics.extend(diags);
                        }
                        Err(e) => {
                            pass_failed = true;
                            eprintln!("Harper pass failed for {}: {}", path_str, e);
                        }
                    }
                }

                // 2. Cadence Pass
                let blocks = extract_prose_blocks(&content);
                let cadence = CadencePass::with_dictionary(dictionary.clone());
                for block in blocks {
                    let mut diags = cadence.check_block(&block, &path_str, &content);
                    for d in &mut diags {
                        (d.line, d.column) =
                            get_line_column(&content, block.start_byte.saturating_add(d.column));
                    }
                    all_diagnostics.extend(diags);
                }

                // 3. Vale Pass
                if !no_vale {
                    match ValeRunner::run(&path_str) {
                        Ok(diags) => all_diagnostics.extend(diags),
                        Err(e) => {
                            pass_failed = true;
                            eprintln!("Vale pass failed for {}: {}", path_str, e);
                        }
                    }
                }
            }

            all_diagnostics.sort_by(|a, b| {
                a.file
                    .cmp(&b.file)
                    .then(a.line.cmp(&b.line))
                    .then(a.column.cmp(&b.column))
            });

            if format.as_deref() == Some("json") {
                println!("{}", serde_json::to_string_pretty(&all_diagnostics)?);
            } else {
                Reporter::print_diagnostics(&all_diagnostics);
            }

            if pass_failed
                || all_diagnostics
                    .iter()
                    .any(|d| matches!(d.severity, Severity::Error | Severity::Warning))
            {
                std::process::exit(1);
            }
        }
        Commands::Score { path, dictionary } => {
            let dictionary: Arc<dyn Dictionary> = load_user_dictionary(&dictionary, &path)?
                .unwrap_or_else(|| FullDictionary::curated());
            let content = std::fs::read_to_string(&path)?;
            let path_str = path.to_string_lossy().to_string();
            let blocks = extract_prose_blocks(&content);
            let cadence = CadencePass::with_dictionary(dictionary.clone());
            let (words, sents, avg_len, stddev, abs_pct, avg_sv) = cadence.calculate_score(&blocks);

            // Factor the lint findings into the grade: prose the checker
            // itself complains about should not score an A. Vale findings
            // count when Vale is installed (an optional pass). Spelling
            // suggestions never affect the grade - unknown project words are
            // dictionary coverage, not prose quality; other suggestions pull
            // the grade down at half weight, capped at one letter.
            let mut lint_diagnostics: Vec<Diagnostic> =
                HarperPass::lint_with_dictionary(&content, &path_str, dictionary.clone())?;
            for block in &blocks {
                lint_diagnostics.extend(cadence.check_block(block, &path_str, &content));
            }
            match ValeRunner::run(&path_str) {
                Ok(diags) => lint_diagnostics.extend(diags),
                Err(e) => eprintln!("Vale pass failed for {}: {}", path_str, e),
            }
            let lint_warnings = lint_diagnostics
                .iter()
                .filter(|d| matches!(d.severity, Severity::Warning))
                .count();
            let lint_errors = lint_diagnostics
                .iter()
                .filter(|d| matches!(d.severity, Severity::Error))
                .count();
            let lint_suggestions = lint_diagnostics
                .iter()
                .filter(|d| matches!(d.severity, Severity::Suggestion))
                .count();

            let grade = scoring::compute_grade(
                avg_len,
                stddev,
                abs_pct,
                avg_sv,
                lint_warnings + scoring::suggestion_penalty(&lint_diagnostics),
                lint_errors,
            );

            let score = DocumentScore {
                file: path_str,
                word_count: words,
                sentence_count: sents,
                avg_sentence_length: avg_len,
                sentence_length_stddev: stddev,
                abstract_noun_pct: abs_pct,
                avg_sv_distance: avg_sv,
                lint_warnings,
                lint_errors,
                lint_suggestions,
                grade,
            };
            Reporter::print_scorecard(&score);
        }
        Commands::Init => {
            for (relative_path, contents) in VALE_ASSETS {
                let path = Path::new(relative_path);
                std::fs::create_dir_all(path.parent().unwrap())?;
                if write_if_missing(path, contents)? {
                    println!("Installed {}", relative_path);
                } else {
                    println!("Kept existing {}", relative_path);
                }
            }

            if write_if_missing(Path::new(".vale.ini"), "StylesPath = styles\nMinAlertLevel = suggestion\n\n[*.{md,txt}]\nBasedOnStyles = Zinsser\n")? {
                println!("Initialized .vale.ini.");
            } else {
                println!("Kept existing .vale.ini unchanged.");
            }

            std::fs::create_dir_all(".zinsser")?;
            if write_if_missing(
                Path::new(".zinsser/dictionary.txt"),
                "# Project words zinsser should not spelling-flag, one per line.\n# Plural and inflected forms need their own entries.\n",
            )? {
                println!("Initialized .zinsser/dictionary.txt.");
            } else {
                println!("Kept existing .zinsser/dictionary.txt unchanged.");
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::collect_markdown_files;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn check_recurses_into_directories_and_skips_build_trees() {
        let root = std::env::temp_dir().join(format!(
            "zinsser-files-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("nested")).unwrap();
        fs::create_dir_all(root.join("target/nested")).unwrap();
        fs::write(root.join("top.md"), "top").unwrap();
        fs::write(root.join("nested/deep.md"), "nested").unwrap();
        fs::write(root.join("target/nested/generated.md"), "ignored").unwrap();
        fs::write(root.join("notes.txt"), "ignored").unwrap();

        let files = collect_markdown_files(root.clone()).unwrap();
        assert_eq!(
            files,
            vec![root.join("nested/deep.md"), root.join("top.md")]
        );
        fs::remove_dir_all(root).unwrap();
    }
}
