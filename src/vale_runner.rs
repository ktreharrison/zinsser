use crate::types::{Diagnostic, Severity};
use anyhow::Result;
use colored::Colorize;
use serde_json::Value;
use std::process::Command;
use std::sync::Once;

static VALE_MISSING_NOTICE: Once = Once::new();

pub struct ValeRunner;

impl ValeRunner {
    pub fn run(file_path: &str) -> Result<Vec<Diagnostic>> {
        let output = match Command::new("vale")
            .arg("--output=JSON")
            .arg(file_path)
            .output()
        {
            Ok(output) => output,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                VALE_MISSING_NOTICE.call_once(|| {
                    eprintln!(
                        "{}",
                        "Note: 'vale' binary not found in PATH; skipping Vale style pass.".yellow()
                    );
                });
                return Ok(Vec::new());
            }
            Err(error) => return Err(error.into()),
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !output.status.success() {
            // Vale exit code 1 means issues found. Exit code 2 means a real error.
            if output.status.code() == Some(2) {
                return Err(anyhow::anyhow!("Vale Error: {}", stderr));
            }

            if stdout.trim().is_empty() {
                return Err(anyhow::anyhow!("Vale failed to produce output: {}", stderr));
            }
        }

        if stdout.trim().is_empty() {
            return Ok(Vec::new());
        }

        let v: Value = serde_json::from_str(&stdout)?;

        let mut diagnostics = Vec::new();
        if let Some(files_map) = v.as_object() {
            if let Some(messages) = files_map.get(file_path).and_then(|m| m.as_array()) {
                for msg in messages {
                    if let Some(m) = msg.as_object() {
                        let rule_id = m.get("Check").and_then(|v| v.as_str()).unwrap_or("");
                        if rule_id.eq_ignore_ascii_case("Zinsser.CadenceMonotony") {
                            continue;
                        }
                        diagnostics.push(Self::parse_msg(m, file_path));
                    }
                }
            } else {
                for (_, val) in files_map {
                    if let Some(messages) = val.as_array() {
                        for msg in messages {
                            if let Some(m) = msg.as_object() {
                                let rule_id = m.get("Check").and_then(|v| v.as_str()).unwrap_or("");
                                if rule_id.eq_ignore_ascii_case("Zinsser.CadenceMonotony") {
                                    continue;
                                }
                                diagnostics.push(Self::parse_msg(m, file_path));
                            }
                        }
                    }
                }
            }
        }

        Ok(diagnostics)
    }

    fn parse_msg(m: &serde_json::Map<String, Value>, file_path: &str) -> Diagnostic {
        let rule_id = m.get("Check").and_then(|v| v.as_str()).unwrap_or("Unknown");
        let message = m.get("Message").and_then(|v| v.as_str()).unwrap_or("");
        let line = m.get("Line").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let span = m.get("Span").and_then(|v| v.as_array());
        let col = span
            .and_then(|s| s.first().and_then(|v| v.as_u64()))
            .unwrap_or(0) as usize;

        let severity = match m.get("Severity").and_then(|v| v.as_str()) {
            Some("error") => Severity::Error,
            Some("suggestion") => Severity::Suggestion,
            _ => Severity::Warning,
        };

        Diagnostic {
            file: file_path.to_string(),
            line,
            column: col,
            severity,
            rule_id: rule_id.to_string(),
            message: message.to_string(),
            suggested_fix: None,
        }
    }
}
