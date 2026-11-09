use crate::types::{Diagnostic, DocumentScore};
use colored::*;

pub struct Reporter;

impl Reporter {
    pub fn print_diagnostics(diagnostics: &[Diagnostic]) {
        for diag in diagnostics {
            println!(
                "{}:{}:{}: {} [{}] {}",
                diag.file.green(),
                diag.line.to_string().yellow(),
                diag.column.to_string().yellow(),
                diag.severity.to_colored_string(),
                diag.rule_id.blue(),
                diag.message
            );
        }
    }

    pub fn print_scorecard(score: &DocumentScore) {
        println!("\n--- Craft Scorecard: {} ---", score.file.bold());
        println!("Words: {} | Sentences: {}", score.word_count, score.sentence_count);
        println!("Avg Sentence Length: {:.1}", score.avg_sentence_length);
        println!("Sentence Length StdDev: {:.1} (Target >= 5.0)", score.sentence_length_stddev);
        println!("Abstract Noun Density: {:.1}% (Target < 4.0%)", score.abstract_noun_pct);
        println!("Avg Subject-Verb Distance: {:.1} (Target < 6.0)", score.avg_sv_distance);
        println!("Lint Findings: {} warnings, {} errors, {} suggestions (each warning costs one letter, each error two, two suggestions one)", score.lint_warnings, score.lint_errors, score.lint_suggestions);
        println!("Overall Grade: {}", score.grade.to_string().bold().green());
        println!("--------------------------------------------\n");
    }
}
