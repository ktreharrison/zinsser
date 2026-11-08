use serde::{Deserialize, Serialize};
use colored::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Severity {
    Error,
    Warning,
    Suggestion,
}

impl Severity {
    pub fn to_colored_string(&self) -> String {
        match self {
            Severity::Error => "error".red().bold().to_string(),
            Severity::Warning => "warning".yellow().to_string(),
            Severity::Suggestion => "suggestion".blue().to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub severity: Severity,
    pub rule_id: String,
    pub message: String,
    pub suggested_fix: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DocumentScore {
    pub file: String,
    pub word_count: usize,
    pub sentence_count: usize,
    pub avg_sentence_length: f64,
    pub sentence_length_stddev: f64,
    pub abstract_noun_pct: f64,
    pub avg_sv_distance: f64,
    pub grade: char,
}
