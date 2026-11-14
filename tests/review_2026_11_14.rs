//! Regression tests for defects found in the 14 Nov 2026 review.

use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};
use zinsser::cadence_pass::CadencePass;
use zinsser::markdown::{extract_prose_blocks, get_line_column};
use zinsser::scoring::compute_grade;
use zinsser::user_dictionary::find_project_dictionary;

const DELAYED: &str = "After a long and incredibly complicated series of cross-functional architecture review meetings in three different global regional office locations, we found the bug.";

/// Run the cadence pass the way `zinsser check` does and return the source
/// (line, column) of the first diagnostic with `rule_id`.
fn locate(content: &str, rule_id: &str) -> Option<(usize, usize)> {
    let pass = CadencePass::new();
    for block in extract_prose_blocks(content) {
        for d in pass.check_block(&block, "t.md", content) {
            if d.rule_id == rule_id {
                return Some(get_line_column(content, block.source_offset(d.column)));
            }
        }
    }
    None
}

fn column_of(content: &str, needle: &str) -> (usize, usize) {
    let byte = content.find(needle).expect("needle present");
    get_line_column(content, byte)
}

#[test]
fn delayed_verb_points_at_sentence_not_preceding_space() {
    let content = format!("Plain intro. {DELAYED}\n");
    assert_eq!(
        locate(&content, "Cadence.DelayedVerb"),
        Some(column_of(&content, "After"))
    );
}

#[test]
fn delayed_verb_location_survives_inline_markup() {
    // The link target and inline code are dropped from the prose text; the
    // reported column must still land on the sentence in the source.
    let content = format!(
        "See [the docs](https://example.com/a/very/long/path/that/is/not/prose) and `cargo test` here. {DELAYED}\n"
    );
    assert_eq!(
        locate(&content, "Cadence.DelayedVerb"),
        Some(column_of(&content, "After"))
    );
}

#[test]
fn delayed_verb_location_on_a_wrapped_line() {
    let content = format!("Intro with **bold** words.\n{DELAYED}\n");
    assert_eq!(
        locate(&content, "Cadence.DelayedVerb"),
        Some(column_of(&content, "After"))
    );
}

#[test]
fn split_sentences_handles_multibyte_text_and_terminators() {
    let text = "Café is open. Naïve plans fail! Why? Done";
    let sentences: Vec<&str> = CadencePass::split_sentences(text)
        .into_iter()
        .map(str::trim)
        .collect();
    assert_eq!(
        sentences,
        ["Café is open.", "Naïve plans fail!", "Why?", "Done"]
    );
}

#[test]
fn split_sentences_is_linear_enough_for_long_paragraphs() {
    // 40k sentences in one paragraph. The old splitter was quadratic and took
    // tens of seconds at this size; the linear one takes milliseconds.
    let text = "The team shipped it. ".repeat(40_000);
    let start = std::time::Instant::now();
    let sentences = CadencePass::split_sentences(&text);
    assert_eq!(
        sentences.iter().filter(|s| !s.trim().is_empty()).count(),
        40_000
    );
    assert!(
        start.elapsed() < std::time::Duration::from_secs(5),
        "split_sentences took {:?}",
        start.elapsed()
    );
}

#[test]
fn project_dictionary_is_found_from_subdirectories() {
    let root = std::env::temp_dir().join(format!(
        "zinsser-dict-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(root.join(".zinsser")).unwrap();
    fs::create_dir_all(root.join("docs/deep")).unwrap();
    fs::write(root.join(".zinsser/dictionary.txt"), "Zorblaxify\n").unwrap();
    fs::write(root.join("docs/deep/page.md"), "We Zorblaxify.\n").unwrap();

    let expected = root.join(".zinsser/dictionary.txt").canonicalize().unwrap();
    for anchor in [
        root.clone(),
        root.join("docs"),
        root.join("docs/deep/page.md"),
    ] {
        assert_eq!(
            find_project_dictionary(&anchor).map(|p| p.canonicalize().unwrap()),
            Some(expected.clone()),
            "anchor {}",
            anchor.display()
        );
    }
    fs::remove_dir_all(root).unwrap();
}

/// Score metrics computed from the real fixtures, not hand-copied numbers.
/// The previous "mirrors fixtures/slop.md" test hard-coded an S-V distance of
/// 7.6 that had already drifted to 5.0 when the Brill tagger landed.
fn fixture_metrics(name: &str) -> (usize, usize, f64, f64, f64, f64) {
    let content = fs::read_to_string(format!("fixtures/{name}.md")).unwrap();
    CadencePass::new().calculate_score(&extract_prose_blocks(&content))
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.05
}

/// Subject-verb distance depends on verb detection: the Brill tagger with the
/// default `pos-tagger` feature, the dictionary fallback without it.
#[cfg(feature = "pos-tagger")]
const SLOP_SV: f64 = 5.0;
#[cfg(not(feature = "pos-tagger"))]
const SLOP_SV: f64 = 3.8; // 3.2 with harper-core 0.16 metadata
#[cfg(feature = "pos-tagger")]
const CLEAN_SV: f64 = 1.4;
#[cfg(not(feature = "pos-tagger"))]
const CLEAN_SV: f64 = 1.75;

#[test]
fn calculate_score_matches_known_fixture_metrics() {
    let (words, sents, avg, sd, abs, sv) = fixture_metrics("slop");
    assert_eq!((words, sents), (151, 5));
    assert!(
        close(avg, 30.2) && close(sd, 3.6) && close(abs, 4.0) && close(sv, SLOP_SV),
        "slop metrics drifted: avg={avg} sd={sd} abs={abs} sv={sv}"
    );

    let (words, sents, avg, sd, abs, sv) = fixture_metrics("clean_craft");
    assert_eq!((words, sents), (75, 12));
    assert!(
        close(avg, 6.2) && close(sd, 2.8) && close(abs, 4.0) && close(sv, CLEAN_SV),
        "clean_craft metrics drifted: avg={avg} sd={sd} abs={abs} sv={sv}"
    );
}

#[test]
fn computed_fixture_grades_order_slop_below_clean() {
    let grade = |name| {
        let (_, _, avg, sd, abs, sv) = fixture_metrics(name);
        compute_grade(avg, sd, abs, sv, 0, 0)
    };
    assert_eq!(grade("clean_craft"), 'A');
    assert_eq!(grade("slop"), 'C');
    assert!(grade("bad_corporate") > grade("clean_craft"));
}

#[test]
fn stacked_terminators_end_one_sentence() {
    let sentences: Vec<&str> = CadencePass::split_sentences("Wait... really?! We shipped it.")
        .into_iter()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    assert_eq!(sentences, ["Wait...", "really?!", "We shipped it."]);
    // Abbreviations and decimals still do not split.
    assert_eq!(
        CadencePass::split_sentences("Use e.g. version 2.5 now.")
            .into_iter()
            .filter(|s| !s.trim().is_empty())
            .count(),
        1
    );
}

#[test]
fn one_non_spelling_suggestion_costs_nothing_two_cost_one_letter() {
    use zinsser::scoring::suggestion_penalty;
    use zinsser::types::{Diagnostic, Severity};
    let suggestion = |rule: &str| Diagnostic {
        file: "t.md".into(),
        line: 1,
        column: 1,
        severity: Severity::Suggestion,
        rule_id: rule.into(),
        message: String::new(),
        suggested_fix: None,
    };
    assert_eq!(suggestion_penalty(&[suggestion("Harper.Style")]), 0);
    assert_eq!(
        suggestion_penalty(&[suggestion("Harper.Style"), suggestion("Cadence.ColdStart")]),
        1
    );
    assert_eq!(
        suggestion_penalty(&[suggestion("Harper.Spelling"), suggestion("Harper.Spelling")]),
        0
    );
}

#[test]
fn closing_quotes_and_brackets_stay_with_their_sentence() {
    let sentences: Vec<&str> =
        CadencePass::split_sentences("He said \"Go.\" She left. (It rained.) Done.")
            .into_iter()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
    assert_eq!(
        sentences,
        ["He said \"Go.\"", "She left.", "(It rained.)", "Done."]
    );
}

#[test]
fn abstract_density_ignores_case() {
    let lower = "the implementation of the organization prioritization methodology requires facilitation of optimization and standardization of documentation plus rationalization of allocation and communication for transformation across departments while the team waits and we all hope for a result soon.";
    let pass = CadencePass::new();
    let density = |text: &str| {
        pass.check_block(&zinsser::markdown::ProseBlock::new(text, 0), "t.md", "")
            .into_iter()
            .any(|d| d.rule_id == "Cadence.AbstractDensity")
    };
    assert!(lower.split_whitespace().count() >= 35);
    assert!(
        density(lower),
        "lowercase paragraph must trigger the density warning"
    );
    assert!(
        density(&lower.to_uppercase()),
        "uppercase text must not bypass it"
    );
}

#[test]
fn scorecard_targets_match_the_grading_thresholds() {
    use zinsser::scoring::{A_MAX_ABSTRACT_PCT, A_MAX_SV_DISTANCE};
    // Exactly at the printed limit must still grade A; just over must not.
    assert_eq!(compute_grade(6.2, 2.8, A_MAX_ABSTRACT_PCT, 1.4, 0, 0), 'A');
    assert_ne!(
        compute_grade(6.2, 2.8, A_MAX_ABSTRACT_PCT + 0.1, 1.4, 0, 0),
        'A'
    );
    assert_eq!(compute_grade(6.2, 2.8, 4.0, A_MAX_SV_DISTANCE, 0, 0), 'A');
    assert_ne!(
        compute_grade(6.2, 2.8, 4.0, A_MAX_SV_DISTANCE + 0.1, 0, 0),
        'A'
    );
}

/// Issue #1: click README (pallets/click 8abb77a, line 10). v0.1.0 reported
/// "Reader waits 20 words before reaching the main verb"; the verb is "aims",
/// the second word. Holds with and without the pos-tagger feature.
#[test]
fn issue_1_early_verb_in_wrapped_sentence_is_not_delayed() {
    let content = "It aims to make the process of writing command line tools quick and fun\n\
                   while also preventing any frustration caused by the inability to\n\
                   implement an intended CLI API.\n";
    assert!(
        content.split_whitespace().count() > 18,
        "must reach the delayed-verb check"
    );
    assert_eq!(locate(content, "Cadence.DelayedVerb"), None);
}

/// Issue #2: Harper's spelling check is English-only, so words with letters
/// outside ASCII (translation lists: Français, Türkçe, दी) are not reported,
/// while English typos still are.
#[test]
fn spelling_skips_non_english_words_but_keeps_english_typos() {
    let spelled = |text: &str| -> Vec<String> {
        zinsser::harper_pass::HarperPass::lint(text, "t.md")
            .unwrap()
            .into_iter()
            .filter(|d| d.rule_id == "Harper.Spelling")
            .map(|d| d.message)
            .collect()
    };
    let list = "- [Français](README.fr.md)\n- [Türkçe](README.tr.md)\n- [हिन्दी](README.hi.md)\n";
    assert_eq!(
        spelled(list),
        Vec::<String>::new(),
        "non-English words must not be flagged"
    );
    let typo = spelled("We fixed teh bug.");
    assert_eq!(
        typo.len(),
        1,
        "English typo must still be flagged: {typo:?}"
    );
}

/// harper-core 3.2.0 rules SeemsLikeAdjective and ExpandIme print a debug
/// line to stderr on every match; zinsser disables them.
#[test]
fn harper_debug_output_does_not_reach_stderr() {
    let dir = std::env::temp_dir().join(format!("zinsser-stderr-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // From rich 3a109cc README line 419; triggered SeemsLikeAdjective.
    std::fs::write(
        dir.join("t.md"),
        "Here's what it looks like on OSX (similar on Linux):\n",
    )
    .unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_zinsser"))
        .args(["check", "t.md", "--no-vale", "--format", "json"])
        .current_dir(&dir)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!stderr.contains('🚨'), "stderr: {stderr}");
    std::fs::remove_dir_all(dir).ok();
}

fn spelling_words(text: &str) -> Vec<String> {
    zinsser::harper_pass::HarperPass::lint(text, "t.md")
        .unwrap()
        .into_iter()
        .filter(|d| d.rule_id == "Harper.Spelling")
        .map(|d| {
            let start = d.column;
            text[start..]
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .next()
                .unwrap_or("")
                .to_string()
        })
        .collect()
}

/// Issue #2 remainder: links that are the whole line (translation lists) are
/// not spell-checked, as a Markdown list or as `•` lines in a paragraph.
#[test]
fn link_only_lines_are_not_spell_checked() {
    let lists = "- [Deutsche](README.de.md)\n- [Polskie readme](README.pl.md)\n\nIntro:\n • [svenska](README.sv.md)\n • [brasileiro readme](README.pt.md)\n";
    assert_eq!(spelling_words(lists), Vec::<String>::new());
}

#[test]
fn typos_in_list_prose_and_sentence_links_are_still_flagged() {
    assert_eq!(spelling_words("- We fixed teh bug.\n"), ["teh"]);
    assert_eq!(
        spelling_words("See the [teh guide](guide.md) for details.\n"),
        ["teh"]
    );
}

#[test]
fn possessive_after_link_and_file_extensions_are_not_words() {
    // typer 00381d6 README: `[FastAPI](...)'s` flagged `s`.
    assert_eq!(
        spelling_words("It is [Python](https://www.python.org)'s little sibling.\n"),
        Vec::<String>::new()
    );
    // rich 3a109cc README: `table_movie.py` flagged `py`.
    assert_eq!(
        spelling_words("Run [table_movie.py](https://example.com/t.py) now.\n"),
        Vec::<String>::new()
    );
}
