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
    /// Parse one Vale rule file. Returns `Ok(None)` for rule types this engine
    /// does not implement.
    pub fn parse(style: &str, rule: &str, yaml: &str) -> Result<Option<Self>> {
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

        let kind = match doc["extends"].as_str() {
            Some("existence") => {
                let tokens = strings(&doc["tokens"]);
                if tokens.is_empty() {
                    return Err(anyhow!("{name}: existence rule has no tokens"));
                }
                let source = format!(r"{flag}\b(?:{})\b", alternation(&tokens));
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
                let source = format!(r"{flag}\b(?:{})\b", alternation(&keys));
                Kind::Substitution {
                    pattern: Regex::new(&source)
                        .with_context(|| format!("{name}: invalid swap regex"))?,
                    exceptions: exceptions_regex(doc, ignorecase, &name)?,
                    swaps,
                }
            }
            _ => return Ok(None),
        };
        Ok(Some(Self {
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
}

impl StylePass {
    pub fn bundled() -> Self {
        let rules = BUNDLED_RULES
            .iter()
            .filter_map(|(rule, yaml)| {
                StyleRule::parse(STYLE, rule, yaml).expect("bundled rules are valid")
            })
            .collect();
        Self {
            rules,
            source: RuleSource::Bundled,
        }
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
            return Ok(Self::bundled());
        }
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&dir)
            .with_context(|| format!("cannot read {}", dir.display()))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "yml" || x == "yaml"))
            .collect();
        entries.sort();
        let mut rules = Vec::new();
        for path in entries {
            let rule = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            let yaml = std::fs::read_to_string(&path)
                .with_context(|| format!("cannot read {}", path.display()))?;
            if let Some(parsed) = StyleRule::parse(STYLE, &rule, &yaml)
                .with_context(|| format!("in {}", path.display()))?
            {
                rules.push(parsed);
            }
        }
        Ok(Self {
            rules,
            source: RuleSource::Project(dir),
        })
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
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            "StylesPath" => styles_path = dir.join(value.trim()),
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
    })
}
