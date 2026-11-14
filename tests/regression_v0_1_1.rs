use zinsser::cadence_pass::CadencePass;
use zinsser::harper_pass::HarperPass;
use zinsser::markdown::ProseBlock;
use zinsser::scoring::compute_grade;
use zinsser::user_dictionary::load_user_dictionary;

fn block(text: &str) -> ProseBlock {
    ProseBlock::new(text.to_string(), 0)
}

/// tkmd README sentence that v0.1.0 flagged with "Reader waits 19 words"
/// even though the main verb ("prevents") arrives early. The hardcoded verb
/// list missed both "pair" and "prevents".
#[test]
fn delayed_verb_not_flagged_when_verb_arrives_early() {
    let pass = CadencePass::new();
    let text = "When you pair with AI coding agents (Claude Code, Codex, Gemini, OpenCode), tkmd prevents agents from falsely claiming work is finished.";
    let diags = pass.check_block(&block(text), "test.md", "");
    assert!(
        diags.iter().all(|d| d.rule_id != "Cadence.DelayedVerb"),
        "early main verb must not be flagged: {:?}",
        diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    assert!(
        diags.iter().all(|d| d.rule_id != "Cadence.NoVerbFound"),
        "the early verb must actually be detected, not just unflagged"
    );
}

/// "manages" was missing from the hardcoded verb list entirely.
#[test]
fn manages_is_recognized_as_a_verb() {
    let pass = CadencePass::new();
    let text = "tkmd manages every task as a plain Markdown card with YAML frontmatter and a durable audit trail for agents and humans alike.";
    let diags = pass.check_block(&block(text), "test.md", "");
    assert!(
        diags.iter().all(|d| d.rule_id != "Cadence.DelayedVerb"),
        "'manages' at word two must not be flagged: {:?}",
        diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    assert!(
        diags.iter().all(|d| d.rule_id != "Cadence.NoVerbFound"),
        "'manages' must actually be detected as a verb, not just unflagged"
    );
}

/// The genuinely delayed verb must still be caught after the Harper switch.
#[test]
fn genuinely_delayed_verb_still_flagged() {
    let pass = CadencePass::new();
    let text = "After a long and incredibly complicated series of cross-functional architecture review meetings in three different global regional office locations, we found the bug.";
    let diags = pass.check_block(&block(text), "test.md", "");
    assert!(
        diags.iter().any(|d| d.rule_id == "Cadence.DelayedVerb"),
        "a verb 20+ words deep must still be flagged"
    );
}

/// Score contract: the deliberately bad fixture must grade clearly worse
/// than the clean fixture, and lint findings must move the grade.
#[test]
fn score_separates_bad_prose_from_good_prose() {
    // Metric profiles measured on fixtures/slop.md and fixtures/clean_craft.md.
    let slop = compute_grade(30.2, 3.6, 4.0, 7.6, 0, 0);
    let clean = compute_grade(6.2, 2.8, 4.0, 1.4, 0, 0);
    assert_eq!(clean, 'A');
    assert!(
        slop != 'A' && slop != 'B',
        "slop fixture must not grade A or B, got {}",
        slop
    );
}

/// Project dictionary: words in `.zinsser/dictionary.txt` or `--dictionary`
/// files must not be spelling-flagged by the Harper pass.
#[test]
fn project_dictionary_suppresses_known_terms() {
    let dir = std::env::temp_dir().join(format!(
        "zinsser-dict-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let dict_path = dir.join("words.txt");
    std::fs::write(&dict_path, "DuckDB\nPOSIX\nKanban\n").unwrap();

    let dictionary = load_user_dictionary(std::slice::from_ref(&dict_path), &dir)
        .expect("dictionary load should succeed")
        .expect("dictionary with words should load");

    // Mixed case and sentence-initial variants are covered automatically.
    let content = "The events log is queryable with duckdb, follows POSIX conventions, and renders a kanban board.";
    let diags = HarperPass::lint_with_dictionary(content, "test.md", dictionary).unwrap();
    for term in ["duckdb", "POSIX", "kanban"] {
        assert!(
            !diags.iter().any(|d| d.message.contains(term)),
            "{} should not be flagged once allowlisted",
            term
        );
    }

    // Without the project words, at least one of them is flagged.
    let baseline = HarperPass::lint(content, "test.md").unwrap();
    assert!(
        baseline.iter().any(|d| d.message.contains("duckdb")
            || d.message.contains("DuckDB")
            || d.message.contains("POSIX")),
        "baseline should flag unknown technical terms"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

/// Vale rule sanity: the passive-voice regex and its adjective exceptions,
/// and the puffery list, must compile and behave as intended. Vale itself
/// applies these YAML rules; here we verify the regex semantics directly.
#[test]
fn vale_passive_voice_regex_semantics() {
    let yaml = include_str!("../styles/Zinsser/PassiveVoice.yml");
    let token_res: Vec<regex::Regex> = yaml
        .lines()
        .take_while(|l| !l.starts_with("exceptions:"))
        .filter(|l| l.trim_start().starts_with("- '"))
        .map(|l| {
            regex::Regex::new(l.trim().trim_start_matches("- '").trim_end_matches('\'')).unwrap()
        })
        .collect();
    assert!(!token_res.is_empty(), "passive token regexes present");
    let is_flagged = |text: &str| token_res.iter().any(|re| re.is_match(text));

    // True passives are still caught.
    assert!(is_flagged("The report was written by the team."));
    assert!(is_flagged("It is being held tomorrow."));
    // Known false positive from the review.
    assert!(is_flagged("She was tired.")); // regex matches; Vale exceptions suppress it

    let exceptions: Vec<regex::Regex> = yaml
        .lines()
        .skip_while(|l| !l.starts_with("exceptions:"))
        .skip(1)
        .filter(|l| l.trim_start().starts_with("- '"))
        .map(|l| {
            regex::Regex::new(l.trim().trim_start_matches("- '").trim_end_matches('\'')).unwrap()
        })
        .collect();
    assert!(!exceptions.is_empty(), "exceptions list must exist");
    // Vale tests exceptions against the matched token text, not the whole
    // sentence. Tokens form one alternation, so the first that matches wins.
    let alternation = regex::Regex::new(
        &token_res
            .iter()
            .map(|re| format!("(?:{})", re.as_str()))
            .collect::<Vec<_>>()
            .join("|"),
    )
    .unwrap();
    let alert = |text: &str| -> Option<String> {
        let found = alternation.find(text)?.as_str().to_string();
        (!exceptions.iter().any(|e| e.is_match(&found))).then_some(found)
    };
    assert_eq!(
        alert("She was tired."),
        None,
        "adjective exception must cover 'was tired'"
    );
    assert_eq!(
        alert("The report was written by the team.").as_deref(),
        Some("was written by"),
        "exceptions must not swallow true passives"
    );
    assert_eq!(
        alert("The users were confused by the instructions.").as_deref(),
        Some("were confused by"),
        "a by-agent passive must survive the adjective exception"
    );
}

/// Tables and YAML frontmatter are not prose: their cell text must not be
/// sentence-analyzed (v0.1.0 flagged table rows as delayed-verb prose).
#[test]
fn tables_and_frontmatter_are_not_prose() {
    let content = "---\ntitle: Demo\n---\n\nReal prose here.\n\n| Command | Shortcut | Action |\n| ------- | -------- | ------ |\n| check   | c        | Uses the lint engine to render diagnostics for each file |\n";
    let blocks = zinsser::markdown::extract_prose_blocks(content);
    assert_eq!(blocks.len(), 1, "only the real prose paragraph remains");
    assert!(blocks[0].text.contains("Real prose here."));
    assert!(!blocks[0].text.contains("Command"));
    assert!(!blocks[0].text.contains("title"));
}

/// Vale rule sanity: bare "landscape" must no longer be puffery; the
/// "ever-evolving landscape" phrase stays.
#[test]
fn vale_puffery_no_longer_flags_plain_landscape() {
    let yaml = include_str!("../styles/Zinsser/Puffery.yml");
    let tokens: Vec<&str> = yaml
        .lines()
        .skip_while(|l| !l.starts_with("tokens:"))
        .skip(1)
        .take_while(|l| l.trim_start().starts_with("- "))
        .map(|l| l.trim().trim_start_matches("- "))
        .collect();
    assert!(
        !tokens.contains(&"landscape"),
        "bare 'landscape' must be removed"
    );
    assert!(
        tokens.contains(&"ever-evolving landscape"),
        "the evergreen-landscape phrase stays flagged"
    );
}

/// Scoring policy: spelling/vocabulary findings are dictionary coverage,
/// not prose quality. Prose with clean metrics and only unknown technical
/// words must keep its base grade.
#[test]
fn spelling_suggestions_do_not_sink_grade() {
    use zinsser::scoring::{compute_grade, suggestion_penalty};

    let content =
        "DuckDB keeps the kanban log queryable. POSIX tooling renders each webhook payload.";
    let diags = HarperPass::lint(content, "test.md").unwrap();
    let spelling = diags
        .iter()
        .filter(|d| d.rule_id == "Harper.Spelling")
        .count();
    assert!(
        spelling >= 2,
        "test setup needs real spelling findings, got {} diagnostics: {:?}",
        spelling,
        diags.iter().map(|d| &d.rule_id).collect::<Vec<_>>()
    );

    // Vocabulary findings contribute zero penalty, so the grade stays at base.
    assert_eq!(suggestion_penalty(&diags), 0);
    assert_eq!(
        compute_grade(6.2, 2.8, 4.0, 1.4, suggestion_penalty(&diags), 0),
        'A'
    );
}

/// Scoring policy: non-spelling suggestions still count, but a suggestion
/// flood costs at most one letter.
#[test]
fn non_spelling_suggestions_capped_at_one_letter() {
    use zinsser::scoring::{compute_grade, suggestion_penalty};
    use zinsser::types::{Diagnostic, Severity};

    let diag = |rule_id: &str| Diagnostic {
        file: "test.md".to_string(),
        line: 0,
        column: 0,
        severity: Severity::Suggestion,
        rule_id: rule_id.to_string(),
        message: "synthetic".to_string(),
        suggested_fix: None,
    };

    let flood: Vec<Diagnostic> = (0..20).map(|_| diag("Harper.Grammar")).collect();
    assert_eq!(suggestion_penalty(&flood), 1);
    assert_eq!(
        compute_grade(6.2, 2.8, 4.0, 1.4, suggestion_penalty(&flood), 0),
        'B',
        "a suggestion flood costs one letter, not a freefall"
    );

    let mixed: Vec<Diagnostic> = (0..20)
        .map(|i| {
            diag(if i % 2 == 0 {
                "Harper.Spelling"
            } else {
                "Harper.Grammar"
            })
        })
        .collect();
    assert_eq!(suggestion_penalty(&mixed), 1);
}
