//! Built-in engine for Zinsser's Vale-format style rules.
//!
//! The rules stay in Vale's YAML format (`styles/Zinsser/*.yml`), so the same
//! files work with Vale itself. This engine implements the two rule types the
//! bundled rules use, `existence` and `substitution`, with Vale's matching
//! semantics:
//!
//! - tokens (or swap keys) form one alternation wrapped in `\b(?:...)\b`, and
//!   the first alternative that matches at a position wins;
//! - `ignorecase: true` adds `(?i)`;
//! - `exceptions` are tested against the matched text, not the sentence;
//! - matches may span inline markup and soft line breaks, and are reported at
//!   the line and character column where they start;
//! - code blocks, inline code, HTML and link destinations are not checked.
//!
//! Other rule types (`script`, `sequence`, ...) are skipped here. Zinsser
//! implements its one script rule (CadenceMonotony) natively, and projects
//! that need other rule types can still run Vale; see `vale_config`.

use crate::markdown::{extract_style_blocks, get_line_column};
use crate::types::{Diagnostic, Severity};
use anyhow::{anyhow, Context, Result};
use regex::Regex;
use std::path::{Path, PathBuf};
use yaml_rust2::{Yaml, YamlLoader};

/// Name of the style the bundled rules belong to.
pub const STYLE: &str = "Zinsser";

/// Bundled rule files: the same bytes `zinsser init` installs.
pub const BUNDLED_RULES: [(&str, &str); 9] = [
    ("AI_Tells", include_str!("../styles/Zinsser/AI_Tells.yml")),
    (
        "CadenceMonotony",
        include_str!("../styles/Zinsser/CadenceMonotony.yml"),
    ),
    ("Clutter", include_str!("../styles/Zinsser/Clutter.yml")),
    ("Greybeard", include_str!("../styles/Zinsser/Greybeard.yml")),
    ("Hedging", include_str!("../styles/Zinsser/Hedging.yml")),
    (
        "Nominalizations",
        include_str!("../styles/Zinsser/Nominalizations.yml"),
    ),
    (
        "PassiveVoice",
        include_str!("../styles/Zinsser/PassiveVoice.yml"),
    ),
    ("Puffery", include_str!("../styles/Zinsser/Puffery.yml")),
    (
        "WeaselWords",
        include_str!("../styles/Zinsser/WeaselWords.yml"),
    ),
];

enum Kind {
    Existence {
        pattern: Regex,
        exceptions: Option<Regex>,
    },
    Substitution {
        pattern: Regex,
        exceptions: Option<Regex>,
        /// Each swap key anchored to the whole match, with its replacement.
        swaps: Vec<(Regex, String)>,
    },
}

/// Rules Zinsser implements natively elsewhere (the cadence pass), so neither
/// this engine nor Vale needs to run them.
pub const NATIVE_RULES: [&str; 1] = ["CadenceMonotony"];

/// Result of reading one rule file.
pub enum ParseOutcome {
    /// The engine runs this rule exactly as Vale would.
    Rule(StyleRule),
    /// Only Vale can run this rule faithfully; the string says why.
    NeedsVale(String),
}

pub struct StyleRule {
    /// `Style.Rule`, matching Vale's `Check` field.
    pub name: String,
    severity: Severity,
    message: String,
    kind: Kind,
}

fn severity_of(level: Option<&str>) -> Severity {
    match level {
        Some("error") => Severity::Error,
        Some("suggestion") => Severity::Suggestion,
        // Vale's default level is warning.
        _ => Severity::Warning,
    }
}

fn case_flag(ignorecase: bool) -> &'static str {
    if ignorecase {
        "(?i)"
    } else {
        ""
    }
}

fn strings(value: &Yaml) -> Vec<String> {
    value
        .as_vec()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| match item {
                    Yaml::String(s) => Some(s.clone()),
                    Yaml::Integer(i) => Some(i.to_string()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

fn alternation(items: &[String]) -> String {
    items
        .iter()
        .map(|item| format!("(?:{item})"))
        .collect::<Vec<_>>()
        .join("|")
}

fn exceptions_regex(doc: &Yaml, ignorecase: bool, rule: &str) -> Result<Option<Regex>> {
    let items = strings(&doc["exceptions"]);
    if items.is_empty() {
        return Ok(None);
    }
    let source = format!("{}(?:{})", case_flag(ignorecase), alternation(&items));
    Regex::new(&source)
        .map(Some)
        .with_context(|| format!("{rule}: invalid exceptions regex"))
}

impl StyleRule {
    /// Parse one Vale rule file. Rules the engine cannot reproduce exactly are
    /// returned as `NeedsVale` instead of being approximated.
    pub fn parse(style: &str, rule: &str, yaml: &str) -> Result<ParseOutcome> {
        let name = format!("{style}.{rule}");
        let docs =
            YamlLoader::load_from_str(yaml).with_context(|| format!("{name}: invalid YAML"))?;
        let doc = docs
            .first()
            .ok_or_else(|| anyhow!("{name}: empty rule file"))?;
        let ignorecase = doc["ignorecase"].as_bool().unwrap_or(false);
        let message = doc["message"].as_str().unwrap_or("").to_string();
        let severity = severity_of(doc["level"].as_str());
        let flag = case_flag(ignorecase);
        // `nonword: true` drops Vale's \b...\b wrapping around the tokens.
        let nonword = doc["nonword"].as_bool().unwrap_or(false);
        // Only the default scope (`text`, the whole prose) is implemented.
        if let Some(scope) = doc["scope"]
            .as_str()
            .or_else(|| doc["scope"].as_vec().map(|_| "a scope list"))
        {
            if scope != "text" {
                return Ok(ParseOutcome::NeedsVale(format!("scope `{scope}`")));
            }
        }

        let kind = match doc["extends"].as_str() {
            Some("existence") => {
                let tokens = strings(&doc["tokens"]);
                let raw = strings(&doc["raw"]);
                if tokens.is_empty() && raw.is_empty() {
                    return Err(anyhow!("{name}: existence rule has no tokens or raw"));
                }
                // Vale's construction: the token group (word-bounded unless
                // nonword, and empty when there are no tokens) followed directly
                // by the raw pieces joined with nothing between them.
                let token_group = if tokens.is_empty() {
                    String::from("(?:)")
                } else if nonword {
                    format!("(?:{})", alternation(&tokens))
                } else {
                    format!(r"(?:\b(?:{})\b)", alternation(&tokens))
                };
                let source = format!("{flag}{token_group}{}", raw.concat());
                Kind::Existence {
                    pattern: Regex::new(&source)
                        .with_context(|| format!("{name}: invalid token regex"))?,
                    exceptions: exceptions_regex(doc, ignorecase, &name)?,
                }
            }
            Some("substitution") => {
                let swap = doc["swap"]
                    .as_hash()
                    .ok_or_else(|| anyhow!("{name}: substitution rule has no swap map"))?;
                let mut keys = Vec::new();
                let mut swaps = Vec::new();
                for (key, value) in swap {
                    let key = key
                        .as_str()
                        .ok_or_else(|| anyhow!("{name}: non-string swap key"))?;
                    let replacement = match value {
                        Yaml::String(s) => s.clone(),
                        other => other.as_i64().map(|i| i.to_string()).unwrap_or_default(),
                    };
                    let anchored = Regex::new(&format!("{flag}^(?:{key})$"))
                        .with_context(|| format!("{name}: invalid swap key {key:?}"))?;
                    keys.push(key.to_string());
                    swaps.push((anchored, replacement));
                }
                let source = if nonword {
                    format!("{flag}(?:{})", alternation(&keys))
                } else {
                    format!(r"{flag}\b(?:{})\b", alternation(&keys))
                };
                Kind::Substitution {
                    pattern: Regex::new(&source)
                        .with_context(|| format!("{name}: invalid swap regex"))?,
                    exceptions: exceptions_regex(doc, ignorecase, &name)?,
                    swaps,
                }
            }
            Some(other) => return Ok(ParseOutcome::NeedsVale(format!("rule type `{other}`"))),
            None => return Err(anyhow!("{name}: missing `extends`")),
        };
        Ok(ParseOutcome::Rule(Self {
            name,
            severity,
            message,
            kind,
        }))
    }

    /// Matches in one block of text: (start byte, matched text, message, fix).
    fn matches(&self, text: &str) -> Vec<(usize, String, String, Option<String>)> {
        let (pattern, exceptions) = match &self.kind {
            Kind::Existence {
                pattern,
                exceptions,
            } => (pattern, exceptions),
            Kind::Substitution {
                pattern,
                exceptions,
                ..
            } => (pattern, exceptions),
        };
        let mut out = Vec::new();
        for found in pattern.find_iter(text) {
            let matched = found.as_str();
            if exceptions.as_ref().is_some_and(|e| e.is_match(matched)) {
                continue;
            }
            match &self.kind {
                Kind::Existence { .. } => {
                    out.push((
                        found.start(),
                        matched.to_string(),
                        self.message.replacen("%s", matched, 1),
                        None,
                    ));
                }
                Kind::Substitution { swaps, .. } => {
                    let replacement = swaps
                        .iter()
                        .find(|(key, _)| key.is_match(matched))
                        .map(|(_, r)| r.clone())
                        .unwrap_or_default();
                    // Vale fills the first %s with the replacement and the
                    // second with the matched text.
                    let message = self
                        .message
                        .replacen("%s", &replacement, 1)
                        .replacen("%s", matched, 1);
                    out.push((
                        found.start(),
                        matched.to_string(),
                        message,
                        Some(replacement),
                    ));
                }
            }
        }
        out
    }
}

/// Where the active rules came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleSource {
    Bundled,
    Project(PathBuf),
}

pub struct StylePass {
    rules: Vec<StyleRule>,
    pub source: RuleSource,
    /// Rules in the style that only Vale can run faithfully, with the reason.
    /// `check` hands these to an installed Vale; `score` leaves them out.
    pub needs_vale: Vec<HandOff>,
    /// Alerts below this level are dropped, like Vale's `MinAlertLevel`.
    min_level: Severity,
    /// Configuration this engine does not apply (for example `Vocab`), to be
    /// reported rather than ignored.
    pub unsupported_config: Vec<String>,
}

fn level_rank(severity: &Severity) -> u8 {
    match severity {
        Severity::Suggestion => 0,
        Severity::Warning => 1,
        Severity::Error => 2,
    }
}

/// A rule handed to Vale: `(Style.Rule, reason)`.
pub type HandOff = (String, String);

fn collect(
    outcomes: impl IntoIterator<Item = (String, Result<ParseOutcome>)>,
) -> Result<(Vec<StyleRule>, Vec<HandOff>)> {
    let mut rules = Vec::new();
    let mut needs_vale = Vec::new();
    for (rule, outcome) in outcomes {
        if NATIVE_RULES.contains(&rule.as_str()) {
            continue;
        }
        match outcome? {
            ParseOutcome::Rule(parsed) => rules.push(parsed),
            ParseOutcome::NeedsVale(reason) => needs_vale.push((format!("{STYLE}.{rule}"), reason)),
        }
    }
    Ok((rules, needs_vale))
}

impl StylePass {
    pub fn bundled() -> Self {
        let (rules, needs_vale) = collect(
            BUNDLED_RULES
                .iter()
                .map(|(rule, yaml)| (rule.to_string(), StyleRule::parse(STYLE, rule, yaml))),
        )
        .expect("bundled rules are valid");
        assert!(
            needs_vale.is_empty(),
            "bundled rules must all run built in: {needs_vale:?}"
        );
        Self {
            rules,
            source: RuleSource::Bundled,
            needs_vale,
            min_level: Severity::Suggestion,
            unsupported_config: Vec::new(),
        }
    }

    fn with_config(mut self, config: &ValeConfig) -> Self {
        if let Some(level) = &config.min_alert_level {
            self.min_level = severity_of(Some(level));
        }
        if !config.vocab.is_empty() {
            self.unsupported_config.push(format!(
                "Vocab = {} (accepted and rejected terms are applied by Vale only)",
                config.vocab.join(", ")
            ));
        }
        self
    }

    /// Rules for the project containing `anchor`: the project's own
    /// `<StylesPath>/Zinsser/*.yml` when its `.vale.ini` points at one,
    /// otherwise the bundled copies.
    pub fn for_project(anchor: &Path) -> Result<Self> {
        let Some(config) = vale_config(anchor) else {
            return Ok(Self::bundled());
        };
        let dir = config.styles_path.join(STYLE);
        if !dir.is_dir() {
            return Ok(Self::bundled().with_config(&config));
        }
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&dir)
            .with_context(|| format!("cannot read {}", dir.display()))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "yml" || x == "yaml"))
            .collect();
        entries.sort();
        let mut outcomes = Vec::new();
        for path in entries {
            let rule = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            let outcome = std::fs::read_to_string(&path)
                .with_context(|| format!("cannot read {}", path.display()))
                .and_then(|yaml| {
                    StyleRule::parse(STYLE, &rule, &yaml)
                        .with_context(|| format!("in {}", path.display()))
                });
            outcomes.push((rule, outcome));
        }
        let (rules, needs_vale) = collect(outcomes)?;
        Ok(Self {
            rules,
            source: RuleSource::Project(dir),
            needs_vale,
            min_level: Severity::Suggestion,
            unsupported_config: Vec::new(),
        }
        .with_config(&config))
    }

    pub fn rule_names(&self) -> Vec<&str> {
        self.rules.iter().map(|r| r.name.as_str()).collect()
    }

    /// Lint Markdown `content`, reporting 1-based lines and character columns
    /// like Vale.
    pub fn check(&self, content: &str, file_path: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        for block in extract_style_blocks(content) {
            for rule in &self.rules {
                if level_rank(&rule.severity) < level_rank(&self.min_level) {
                    continue;
                }
                for (start, _matched, message, fix) in rule.matches(&block.text) {
                    let (line, column) = get_line_column(content, block.source_offset(start));
                    diagnostics.push(Diagnostic {
                        file: file_path.to_string(),
                        line,
                        column,
                        severity: rule.severity.clone(),
                        rule_id: rule.name.clone(),
                        message,
                        suggested_fix: fix,
                    });
                }
            }
        }
        diagnostics
            .sort_by(|a, b| (a.line, a.column, &a.rule_id).cmp(&(b.line, b.column, &b.rule_id)));
        diagnostics
    }
}

/// The parts of a `.vale.ini` that Zinsser reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValeConfig {
    pub path: PathBuf,
    pub styles_path: PathBuf,
    /// Every style named in a `BasedOnStyles` line.
    pub based_on_styles: Vec<String>,
    pub min_alert_level: Option<String>,
    pub vocab: Vec<String>,
}

impl ValeConfig {
    /// Styles other than Zinsser's own, which only an installed Vale can run.
    pub fn external_styles(&self) -> Vec<&str> {
        self.based_on_styles
            .iter()
            .map(String::as_str)
            .filter(|s| *s != STYLE)
            .collect()
    }
}

/// Nearest `.vale.ini` at or above `anchor`, as Vale itself finds it.
pub fn vale_config(anchor: &Path) -> Option<ValeConfig> {
    let start = if anchor.is_dir() {
        anchor.to_path_buf()
    } else {
        anchor.parent().map(Path::to_path_buf).unwrap_or_default()
    };
    let start = if start.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        start
    };
    let start = start.canonicalize().unwrap_or(start);
    let path = start
        .ancestors()
        .map(|d| d.join(".vale.ini"))
        .find(|p| p.is_file())?;
    let text = std::fs::read_to_string(&path).ok()?;
    let dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
    let mut styles_path = dir.join("styles");
    let mut based_on_styles = Vec::new();
    let mut min_alert_level = None;
    let mut vocab = Vec::new();
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            "StylesPath" => styles_path = dir.join(value.trim()),
            "MinAlertLevel" => min_alert_level = Some(value.trim().to_lowercase()),
            "Vocab" => vocab.extend(
                value
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty()),
            ),
            "BasedOnStyles" => based_on_styles.extend(
                value
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty()),
            ),
            _ => {}
        }
    }
    based_on_styles.sort();
    based_on_styles.dedup();
    Some(ValeConfig {
        path,
        styles_path,
        based_on_styles,
        min_alert_level,
        vocab,
    })
}
