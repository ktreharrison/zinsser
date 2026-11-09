use crate::types::{Diagnostic, Severity};

/// Penalty contribution (in warning-equivalents) from suggestion-severity
/// lint findings.
///
/// Spelling/vocabulary findings (`Harper.Spelling`) never affect the grade:
/// unknown project words are a dictionary-coverage issue, not a
/// prose-quality issue, and a project the curated dictionary does not know
/// should not see its score sink on vocabulary alone. Other suggestions
/// (grammar, style) pull the grade down at half weight, capped at one
/// letter, so a suggestion flood cannot sink otherwise clean prose.
pub fn suggestion_penalty(diagnostics: &[Diagnostic]) -> usize {
    let non_spelling = diagnostics
        .iter()
        .filter(|d| matches!(d.severity, Severity::Suggestion))
        .filter(|d| d.rule_id != "Harper.Spelling")
        .count();
    (non_spelling / 2).min(1)
}

/// Grade computation for `zinsser score`.
///
/// The base grade comes from the rhythm metrics (sentence-length variation,
/// abstraction density, subject-verb distance, average length). Lint findings
/// then pull the grade down: every warning costs one letter, every error two.
/// This keeps the grade honest about problems the checker itself reports,
/// instead of scoring prose the linter simultaneously complains about.
pub fn compute_grade(
    avg_len: f64,
    stddev: f64,
    abs_pct: f64,
    avg_sv: f64,
    warnings: usize,
    errors: usize,
) -> char {
    let base = if (stddev >= 5.0 || (avg_len < 10.0 && stddev >= 1.0))
        && abs_pct <= 4.0
        && avg_sv <= 6.0
        && avg_len <= 25.0
    {
        'A'
    } else if (stddev >= 3.0 || (avg_len < 15.0 && stddev >= 0.5))
        && abs_pct <= 7.0
        && avg_sv <= 8.0
        && avg_len <= 30.0
    {
        'B'
    } else if (stddev >= 1.0 || avg_len < 20.0) && abs_pct <= 12.0 && avg_len <= 40.0 {
        'C'
    } else {
        'F'
    };

    downgrade(base, warnings + 2 * errors)
}

fn downgrade(grade: char, steps: usize) -> char {
    let mut grade = grade;
    for _ in 0..steps {
        grade = match grade {
            'A' => 'B',
            'B' => 'C',
            'C' => 'F',
            other => other,
        };
    }
    grade
}

#[cfg(test)]
mod tests {
    use super::compute_grade;

    #[test]
    fn clean_short_prose_grades_a() {
        // Mirrors fixtures/clean_craft.md
        assert_eq!(compute_grade(6.2, 2.8, 4.0, 1.4, 0, 0), 'A');
    }

    #[test]
    fn bloated_prose_cannot_grade_b() {
        // Mirrors fixtures/slop.md: long sentences, high S-V distance.
        assert_eq!(compute_grade(30.2, 3.6, 4.0, 7.6, 0, 0), 'C');
    }

    #[test]
    fn lint_warnings_pull_grade_down() {
        assert_eq!(compute_grade(6.2, 2.8, 4.0, 1.4, 1, 0), 'B');
        assert_eq!(compute_grade(6.2, 2.8, 4.0, 1.4, 2, 0), 'C');
        assert_eq!(compute_grade(6.2, 2.8, 4.0, 1.4, 3, 0), 'F');
    }

    #[test]
    fn lint_errors_cost_double() {
        assert_eq!(compute_grade(6.2, 2.8, 4.0, 1.4, 0, 1), 'C');
    }

    #[test]
    fn broken_metrics_fail() {
        assert_eq!(compute_grade(50.0, 0.5, 15.0, 20.0, 0, 0), 'F');
    }
}
