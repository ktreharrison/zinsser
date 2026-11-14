//! Runs the bundled Vale rules with a real `vale` binary. Skips (with a note)
//! when Vale is not installed, because Vale is an optional pass.

use std::process::Command;

fn passive_hits(text: &str) -> Option<Vec<String>> {
    if Command::new("vale").arg("--version").output().is_err() {
        eprintln!("vale not installed; skipping Vale rule test");
        return None;
    }
    let dir = std::env::temp_dir().join(format!(
        "zinsser-vale-{}-{}",
        std::process::id(),
        text.len()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let root = std::env::current_dir().unwrap();
    std::fs::write(
        dir.join(".vale.ini"),
        format!(
            "StylesPath = {}\nMinAlertLevel = suggestion\n\n[*.md]\nBasedOnStyles = Zinsser\n",
            root.join("styles").display()
        ),
    )
    .unwrap();
    std::fs::write(dir.join("t.md"), text).unwrap();
    let out = Command::new("vale")
        .args(["--output=JSON", "t.md"])
        .current_dir(&dir)
        .output()
        .unwrap();
    std::fs::remove_dir_all(&dir).ok();
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    Some(
        json.get("t.md")
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
            .filter(|m| m["Check"] == "Zinsser.PassiveVoice")
            .map(|m| m["Match"].as_str().unwrap_or("").to_lowercase())
            .collect(),
    )
}

#[test]
fn passive_voice_keeps_by_agent_passives_and_skips_linking_verbs() {
    let Some(hits) = passive_hits(
        "The users were confused by the unclear instructions.\n\n\
         She was tired after the run.\n\n\
         He is excited about the launch.\n\n\
         The report was written by the team.\n\n\
         The file was sent yesterday.\n",
    ) else {
        return;
    };
    assert_eq!(hits, ["were confused by", "was written by", "was sent"]);
}

#[test]
fn builtin_passive_voice_matches_the_vale_expectation() {
    // Runs with or without Vale installed.
    let text = "The users were confused by the unclear instructions.\n\n\
                She was tired after the run.\n\n\
                He is excited about the launch.\n\n\
                The report was written by the team.\n\n\
                The file was sent yesterday.\n";
    let hits: Vec<String> = zinsser::style_pass::StylePass::bundled()
        .check(text, "t.md")
        .into_iter()
        .filter(|d| d.rule_id == "Zinsser.PassiveVoice")
        .map(|d| d.message)
        .collect();
    assert_eq!(hits.len(), 3, "{hits:#?}");
    for phrase in ["were confused by", "was written by", "was sent"] {
        assert!(
            hits.iter().any(|m| m.contains(&format!("('{phrase}')"))),
            "{phrase}: {hits:#?}"
        );
    }
}
