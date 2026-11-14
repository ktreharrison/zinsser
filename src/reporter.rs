use crate::scoring::{
    A_MAX_ABSTRACT_PCT, A_MAX_AVG_LEN, A_MAX_SV_DISTANCE, A_MIN_STDDEV, A_MIN_STDDEV_SHORT,
    A_SHORT_AVG_LEN,
};
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
        println!(
            "Avg Sentence Length: {:.1} (A needs <= {:.1})",
            score.avg_sentence_length, A_MAX_AVG_LEN
        );
        println!(
            "Sentence Length StdDev: {:.1} (A needs >= {:.1}, or >= {:.1} when avg length < {:.1})",
            score.sentence_length_stddev, A_MIN_STDDEV, A_MIN_STDDEV_SHORT, A_SHORT_AVG_LEN
        );
        println!(
            "Abstract Noun Density: {:.1}% (A needs <= {:.1}%)",
            score.abstract_noun_pct, A_MAX_ABSTRACT_PCT
        );
        println!(
            "Avg Subject-Verb Distance: {:.1} (A needs <= {:.1})",
            score.avg_sv_distance, A_MAX_SV_DISTANCE
        );
        println!("Lint Findings: {} warnings, {} errors, {} suggestions (warnings cost one letter, errors two; spelling suggestions do not affect the grade)", score.lint_warnings, score.lint_errors, score.lint_suggestions);
        println!("Overall Grade: {}", score.grade.to_string().bold().green());
        println!("--------------------------------------------\n");
    }
}
