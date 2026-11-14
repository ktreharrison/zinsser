use zinsser::cadence_pass::CadencePass;
use zinsser::markdown::ProseBlock;

#[test]
fn test_mutation_monotony_breaker() {
    let pass = CadencePass::new();
    // BEFORE: 4 consecutive sentences between 9 and 10 words (triggers Monotony)
    let bad = "The engineering team deployed the new caching service on Tuesday. \
               The database cluster handled the heavy traffic load without issue. \
               Our monitoring dashboard showed a steady decrease in tail latency. \
               The product manager approved the rollout for all regional users.";
    let diags_before = pass.check_block(&ProseBlock { text: bad.to_string(), start_byte: 0, segments: Vec::new() }, "test.md", "");
    assert!(diags_before.iter().any(|d| d.rule_id == "Cadence.Monotony"), "Should flag monotonous 4-sentence block");

    // AFTER MUTATION: Sentence 3 becomes a 3-word punch (clears Monotony)
    let mutated = "The engineering team deployed the new caching service on Tuesday. \
                   The database cluster handled the heavy traffic load without issue. \
                   Latency dropped fast. \
                   The product manager approved the rollout for all regional users.";
    let diags_after = pass.check_block(&ProseBlock { text: mutated.to_string(), start_byte: 0, segments: Vec::new() }, "test.md", "");
    assert!(diags_after.iter().all(|d| d.rule_id != "Cadence.Monotony"), "Short punch sentence must break monotony");
}

#[test]
fn test_mutation_delayed_verb_frontloading() {
    let pass = CadencePass::new();
    // BEFORE: 20 words before the finite verb "found"
    let bad = "After a long and incredibly complicated series of cross-functional architecture review meetings in three different global regional office locations, we found the bug.";
    let diags_before = pass.check_block(&ProseBlock { text: bad.to_string(), start_byte: 0, segments: Vec::new() }, "test.md", "");
    assert!(diags_before.iter().any(|d| d.rule_id == "Cadence.DelayedVerb"), "Should flag delayed verb");

    // AFTER MUTATION: Front-load subject and verb ("We found the bug...")
    let mutated = "We found the bug after a long and incredibly complicated series of cross-functional architecture review meetings in three different global regional office locations.";
    let diags_after = pass.check_block(&ProseBlock { text: mutated.to_string(), start_byte: 0, segments: Vec::new() }, "test.md", "");
    assert!(diags_after.iter().all(|d| d.rule_id != "Cadence.DelayedVerb"), "Front-loading verb must clear warning");
}

#[test]
fn test_mutation_nominalization_to_concrete_verb() {
    let pass = CadencePass::new();
    // BEFORE: >8% abstract nominalizations in a block > 35 words
    let bad = "The implementation of the optimization and the administration of the organization required authorization and coordination across the entire engineering division during the transition period for our global infrastructure in order to maintain high performance and reliability.";
    let diags_before = pass.check_block(&ProseBlock { text: bad.to_string(), start_byte: 0, segments: Vec::new() }, "test.md", "");
    assert!(diags_before.iter().any(|d| d.rule_id == "Cadence.AbstractDensity"));

    // AFTER MUTATION: Concrete actors and active verbs
    let mutated = "The team built the fast cache and led the company while managers approved and coordinated work across every engineering group as we moved our global servers to the new cluster to keep the system fast and stable.";
    let diags_after = pass.check_block(&ProseBlock { text: mutated.to_string(), start_byte: 0, segments: Vec::new() }, "test.md", "");
    assert!(diags_after.iter().all(|d| d.rule_id != "Cadence.AbstractDensity"));
}

#[test]
fn test_mutation_cold_start_to_cohesive_bridge() {
    let pass = CadencePass::new();
    // BEFORE: 4 sentences with zero shared stems and zero bridge tokens
    let bad = "Database shards require careful memory tuning. \
               Network switches drop packets under heavy load. \
               Frontend bundles contain too many third-party scripts. \
               Security audits happen every quarter in Brussels.";
    let diags_before = pass.check_block(&ProseBlock { text: bad.to_string(), start_byte: 0, segments: Vec::new() }, "test.md", "");
    assert!(diags_before.iter().any(|d| d.rule_id == "Cadence.ColdStart"), "Should flag 3 consecutive cold starts");

    // AFTER MUTATION: Link each sentence to the previous one (Old-to-New contract)
    let mutated = "Database shards require careful memory tuning. \
                   This tuning prevents network switches from dropping packets. \
                   When packets drop, frontend bundles fail to load. \
                   However, quarterly security audits catch these failures early.";
    let diags_after = pass.check_block(&ProseBlock { text: mutated.to_string(), start_byte: 0, segments: Vec::new() }, "test.md", "");
    assert!(diags_after.iter().all(|d| d.rule_id != "Cadence.ColdStart"), "Adding cohesion bridges must clear ColdStart");
}

#[test]
fn test_edge_case_abbreviations_and_decimals() {
    let sentences = CadencePass::split_sentences("Visit e.g. the site. Use v1.2.0. Dr. Smith is here.");
    println!("Sentences: {:?}", sentences);
    assert_eq!(sentences.len(), 3);
    assert!(sentences[0].contains("e.g."));
    assert!(sentences[1].contains("v1.2.0"));
}

#[test]
fn test_edge_case_technical_allowlist_no_false_positive() {
    let pass = CadencePass::new();
    let text = "The deployment of the function in the environment is a requirement for the system.";
    let blocks = [ProseBlock::new(text.to_string(), 0)];
    let diags = pass.check_block(&blocks[0], "test.md", "");
    // "deployment", "function", "environment", "requirement" are allowlisted.
    assert!(diags.iter().all(|d| d.rule_id != "Cadence.AbstractDensity"));
}

#[test]
fn test_edge_case_participial_opener_still_catches_delayed_verb() {
    let pass = CadencePass::new();
    let text = "Walking slowly through the garden in the middle of the night, with a lantern in his left hand, the old man found a gold coin.";
    let blocks = [ProseBlock::new(text.to_string(), 0)];
    let diags = pass.check_block(&blocks[0], "test.md", "");
    assert!(diags.iter().any(|d| d.rule_id == "Cadence.DelayedVerb"));
}

#[test]
fn test_edge_case_unicode_and_emoji_line_numbers() {
    let content = "Line 1\n🦀 Emoji\nLine 3";
    assert_eq!(zinsser::markdown::get_line_column(content, 10), (2, 2));
}

#[test]
fn harper_column_accounts_for_multibyte_characters() {
    let content = "🦀 teh";
    let diagnostic = zinsser::harper_pass::HarperPass::lint(content, "test.md")
        .unwrap()
        .into_iter()
        .find(|d| d.message.contains("teh"))
        .unwrap();
    assert_eq!(
        zinsser::markdown::get_line_column(content, diagnostic.column),
        (1, 3)
    );
}

#[test]
fn test_fixture_contracts() {
    // Simple check that essential files exist
    assert!(std::path::Path::new("fixtures/slop.md").exists());
    assert!(std::path::Path::new("fixtures/craft.md").exists());
}

#[test]
fn test_edge_case_markdown_code_and_tables_ignored() {
    // This would typically test extract_prose_blocks
    let content = "Prose\n\n```rust\nfn main() {}\n```\n\nMore prose";
    let blocks = zinsser::markdown::extract_prose_blocks(content);
    assert_eq!(blocks.len(), 2);
    assert!(!blocks[0].text.contains("fn main"));
}

#[test]
fn test_edge_case_words_ending_in_st_split_properly() {
    let text = "The cache is fast. We ran the test. Customers come first. St. Louis is far away.";
    let sents = zinsser::cadence_pass::CadencePass::split_sentences(text);
    let non_empty: Vec<_> = sents.into_iter().filter(|s| !s.trim().is_empty()).collect();
    assert_eq!(non_empty.len(), 4, "Words ending in 'st.' like 'fast.' and 'test.' must split sentences properly while preserving 'St. Louis'");
}
