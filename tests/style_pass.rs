//! The built-in style engine must reproduce Vale's output on the bundled
//! rules. `tests/style_corpus/vale_baseline.json` was captured from Vale 3.27
//! with this command (run from the repo root):
//!
//!   vale --config .vale.ini --output=JSON <file>
//!
//! for every file listed in `CORPUS`. Regenerate it whenever a rule changes,
//! and check the diff: it is the contract for Vale compatibility.

use std::collections::BTreeSet;
use zinsser::style_pass::StylePass;

const CORPUS: [&str; 5] = [
    "tests/style_corpus/mixed.md",
    "fixtures/bad_corporate.md",
    "fixtures/clean_craft.md",
    "fixtures/craft.md",
    "fixtures/slop.md",
];

type Alert = (String, usize, usize, String, String, String, String);

fn baseline() -> BTreeSet<Alert> {
    let raw = std::fs::read_to_string("tests/style_corpus/vale_baseline.json").unwrap();
    let items: Vec<serde_json::Value> = serde_json::from_str(&raw).unwrap();
    items
        .iter()
        .map(|a| {
            (
                a["file"].as_str().unwrap().to_string(),
                a["line"].as_u64().unwrap() as usize,
                a["column"].as_u64().unwrap() as usize,
                a["check"].as_str().unwrap().to_string(),
                a["match"].as_str().unwrap().to_string(),
                a["severity"].as_str().unwrap().to_string(),
                a["message"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

fn severity_name(s: &zinsser::types::Severity) -> String {
    format!("{s:?}").to_lowercase()
}

fn builtin() -> BTreeSet<Alert> {
    let pass = StylePass::bundled();
    let mut out = BTreeSet::new();
    for file in CORPUS {
        let content = std::fs::read_to_string(file).unwrap();
        for d in pass.check(&content, file) {
            // Recover the matched text from the source position for comparison.
            let line = content.lines().nth(d.line - 1).unwrap_or("");
            let start: String = line.chars().skip(d.column - 1).collect();
            out.insert((
                d.file.clone(),
                d.line,
                d.column,
                d.rule_id.clone(),
                start,
                severity_name(&d.severity),
                d.message.clone(),
            ));
        }
    }
    out
}

#[test]
fn builtin_engine_matches_pinned_vale_output() {
    let expected = baseline();
    let actual = builtin();
    // Compare on everything except the match text, which the built-in side
    // only knows as "the rest of the line from the column"; check that the
    // Vale match is a prefix of it.
    let key = |a: &Alert| (a.0.clone(), a.1, a.2, a.3.clone(), a.5.clone(), a.6.clone());
    let exp_keys: BTreeSet<_> = expected.iter().map(key).collect();
    let act_keys: BTreeSet<_> = actual.iter().map(key).collect();
    let missing: Vec<_> = exp_keys.difference(&act_keys).collect();
    let extra: Vec<_> = act_keys.difference(&exp_keys).collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "built-in engine differs from Vale.\nmissing ({}):\n{:#?}\nextra ({}):\n{:#?}",
        missing.len(),
        missing,
        extra.len(),
        extra
    );
    for e in &expected {
        let a = actual.iter().find(|a| key(a) == key(e)).unwrap();
        assert!(
            a.4.starts_with(&e.4) || a.4.to_lowercase().starts_with(&e.4.to_lowercase()),
            "{}:{}:{} {}: source at column does not start with Vale match {:?}: {:?}",
            e.0,
            e.1,
            e.2,
            e.3,
            e.4,
            a.4
        );
    }
}

fn temp_project(tag: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("zinsser-style-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("styles/Zinsser")).unwrap();
    std::fs::create_dir_all(root.join("docs")).unwrap();
    root
}

#[test]
fn vale_config_is_found_upward_and_names_external_styles() {
    let root = temp_project("config");
    std::fs::write(
        root.join(".vale.ini"),
        "StylesPath = styles\n\n[*.md]\nBasedOnStyles = Zinsser, Team ,Microsoft\n",
    )
    .unwrap();
    let config = zinsser::style_pass::vale_config(&root.join("docs")).unwrap();
    assert_eq!(
        config.styles_path,
        root.canonicalize().unwrap().join("styles")
    );
    assert_eq!(config.external_styles(), ["Microsoft", "Team"]);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn project_rule_files_replace_the_bundled_ones() {
    let root = temp_project("override");
    std::fs::write(root.join(".vale.ini"), "StylesPath = styles\n").unwrap();
    std::fs::write(
        root.join("styles/Zinsser/Words.yml"),
        "extends: existence\nmessage: \"Avoid '%s'.\"\nlevel: error\nignorecase: true\ntokens:\n  - widget\n",
    )
    .unwrap();
    // A rule type the engine does not run is skipped, not an error.
    std::fs::write(
        root.join("styles/Zinsser/Seq.yml"),
        "extends: sequence\nmessage: x\ntokens:\n  - tag: NN\n",
    )
    .unwrap();
    let pass = StylePass::for_project(&root.join("docs")).unwrap();
    assert_eq!(pass.rule_names(), ["Zinsser.Words"]);
    let diags = pass.check("The Widget works.\n", "a.md");
    assert_eq!(diags.len(), 1);
    assert_eq!((diags[0].line, diags[0].column), (1, 5));
    assert_eq!(diags[0].message, "Avoid 'Widget'.");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn invalid_project_rule_is_an_error() {
    let root = temp_project("invalid");
    std::fs::write(root.join(".vale.ini"), "StylesPath = styles\n").unwrap();
    std::fs::write(
        root.join("styles/Zinsser/Bad.yml"),
        "extends: existence\nmessage: x\ntokens:\n  - '(unclosed'\n",
    )
    .unwrap();
    let error = StylePass::for_project(&root)
        .err()
        .expect("invalid regex must fail");
    assert!(format!("{error:#}").contains("Bad.yml"), "{error:#}");
    std::fs::remove_dir_all(root).unwrap();
}

/// When Vale is installed, compare against it live, so the pinned baseline
/// cannot drift from the rule files unnoticed.
#[test]
fn builtin_engine_matches_live_vale_when_installed() {
    if std::process::Command::new("vale")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("vale not installed; skipping live comparison");
        return;
    }
    let mut live = BTreeSet::new();
    for file in CORPUS {
        let out = std::process::Command::new("vale")
            .args(["--config", ".vale.ini", "--output=JSON", file])
            .output()
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or_default();
        for (_, alerts) in json.as_object().into_iter().flatten() {
            for a in alerts.as_array().into_iter().flatten() {
                live.insert((
                    file.to_string(),
                    a["Line"].as_u64().unwrap() as usize,
                    a["Span"][0].as_u64().unwrap() as usize,
                    a["Check"].as_str().unwrap().to_string(),
                    a["Message"].as_str().unwrap().to_string(),
                ));
            }
        }
    }
    let ours: BTreeSet<_> = builtin()
        .into_iter()
        .map(|a| (a.0, a.1, a.2, a.3, a.6))
        .collect();
    assert_eq!(ours, live, "built-in engine and live Vale disagree");
}
