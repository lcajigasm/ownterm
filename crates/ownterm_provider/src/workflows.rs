//! Local command workflows stored as YAML under `~/.ownterm/workflows`.
//!
//! The desktop command search and the `/` menu already load that directory.
//! This module writes the same document shape, including `{{argument}}`
//! placeholders, without a Warp Drive catalog.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};

/// One command workflow. Argument names come from `{{name}}` placeholders.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalWorkflow {
    pub name: String,
    pub command: String,
    pub description: String,
    pub arguments: Vec<String>,
}

impl LocalWorkflow {
    pub fn new(
        name: impl Into<String>,
        command: impl Into<String>,
        description: impl Into<String>,
    ) -> anyhow::Result<Self> {
        let name = name.into();
        let command = command.into();
        let description = description.into();
        validate_field("name", &name)?;
        validate_field("command", &command)?;
        validate_field("description", &description)?;
        if name.contains('/') || name.contains('\\') || name.contains("..") {
            bail!("name must be a title, not a path");
        }
        let name = name.trim().to_owned();
        file_stem(&name)?;
        Ok(Self {
            arguments: argument_names(command.trim()),
            name,
            command: command.trim().to_owned(),
            description: description.trim().to_owned(),
        })
    }
}

/// `~/.ownterm/workflows`.
pub fn default_workflows_dir() -> anyhow::Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
        .context("HOME is not set")?;
    Ok(home.join(".ownterm").join("workflows"))
}

/// Writes or updates one workflow file. Other workflows already in that file stay.
pub fn write_local_workflow(dir: &Path, workflow: &LocalWorkflow) -> anyhow::Result<PathBuf> {
    fs::create_dir_all(dir).with_context(|| format!("failed to create {}", dir.display()))?;
    let path = destination(dir, &workflow.name)?;
    let mut docs = if path.exists() {
        parse_workflow_file(&fs::read_to_string(&path)?)?
    } else {
        Vec::new()
    };
    if let Some(existing) = docs.iter_mut().find(|doc| doc.name == workflow.name) {
        *existing = workflow.clone();
    } else {
        docs.push(workflow.clone());
    }
    let text = format_workflow_file(&docs)?;
    fs::write(&path, &text).with_context(|| format!("failed to write {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644))
            .with_context(|| format!("failed to set permissions on {}", path.display()))?;
    }
    Ok(path)
}

/// Reads every `*.yaml` and `*.yml` file in `dir`. A missing directory is empty.
/// A file that is not a workflow document is skipped.
pub fn list_local_workflows(dir: &Path) -> anyhow::Result<Vec<LocalWorkflow>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut workflows = Vec::new();
    let mut names = fs::read_dir(dir)
        .with_context(|| format!("failed to read {}", dir.display()))?
        .collect::<Result<Vec<_>, _>>()
        .with_context(|| format!("failed to read {}", dir.display()))?;
    names.sort_by_key(|entry| entry.file_name());
    for entry in names {
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !is_yaml_file(file_name) {
            continue;
        }
        let text = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        if let Ok(docs) = parse_workflow_file(&text) {
            workflows.extend(docs);
        }
    }
    workflows.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(workflows)
}

pub fn format_workflow_file(workflows: &[LocalWorkflow]) -> anyhow::Result<String> {
    if workflows.is_empty() {
        bail!("a workflow file needs at least one command");
    }
    let mut text = String::new();
    for workflow in workflows {
        text.push_str(&format_one(workflow)?);
    }
    Ok(text)
}

pub fn parse_workflow_file(text: &str) -> anyhow::Result<Vec<LocalWorkflow>> {
    let mut workflows = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    let flush = |lines: &[&str], workflows: &mut Vec<LocalWorkflow>| -> anyhow::Result<()> {
        if lines.iter().all(|line| line.trim().is_empty()) {
            return Ok(());
        }
        if let Some(workflow) = parse_document(lines)? {
            workflows.push(workflow);
        }
        Ok(())
    };
    for line in text.lines() {
        if line.trim() == "---" {
            flush(&current, &mut workflows)?;
            current.clear();
        } else {
            current.push(line);
        }
    }
    flush(&current, &mut workflows)?;
    Ok(workflows)
}

fn format_one(workflow: &LocalWorkflow) -> anyhow::Result<String> {
    let mut text = format!(
        "---\nname: {}\ncommand: {}\ndescription: {}\n",
        quote(&workflow.name),
        quote(&workflow.command),
        quote(&workflow.description),
    );
    if workflow.arguments.is_empty() {
        text.push_str("arguments: []\n---\n");
    } else {
        text.push_str("arguments:\n");
        for argument in &workflow.arguments {
            text.push_str(&format!(
                "  - name: {argument}\n    arg_type: Text\n    description: {}\n",
                quote(&format!("Value for {argument}")),
            ));
        }
        text.push_str("---\n");
    }
    Ok(text)
}

fn parse_document(lines: &[&str]) -> anyhow::Result<Option<LocalWorkflow>> {
    let mut name = None;
    let mut command = None;
    let mut description = None;
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || line.starts_with([' ', '\t', '-']) {
            continue;
        }
        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() || value == "[]" || value == "|" || value == ">" {
            continue;
        }
        let parsed = parse_scalar(value)?;
        match key.trim() {
            "name" => name = Some(parsed),
            "command" => command = Some(parsed),
            "description" => description = Some(parsed),
            _ => {}
        }
    }
    match (name, command) {
        (Some(name), Some(command)) => Ok(Some(LocalWorkflow::new(
            name,
            command,
            description.unwrap_or_default(),
        )?)),
        (None, None) => Ok(None),
        _ => Ok(None),
    }
}

fn parse_scalar(value: &str) -> anyhow::Result<String> {
    let mut chars = value.chars();
    match chars.next() {
        Some('"') => {
            let mut out = String::new();
            let mut escaped = false;
            let mut closed = false;
            for ch in chars {
                if escaped {
                    let decoded = match ch {
                        '\\' => '\\',
                        '"' => '"',
                        'n' => '\n',
                        'r' => '\r',
                        't' => '\t',
                        other => bail!("unknown escape \\{other} in workflow YAML"),
                    };
                    if decoded.is_control() {
                        bail!("workflow fields must be a single line");
                    }
                    out.push(decoded);
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    closed = true;
                    break;
                } else {
                    out.push(ch);
                }
            }
            if !closed || escaped {
                bail!("workflow string is missing a closing quote");
            }
            Ok(out)
        }
        Some('\'') => {
            let rest = chars.as_str();
            let Some(end) = rest.find('\'') else {
                bail!("workflow string is missing a closing quote");
            };
            Ok(rest[..end].replace("''", "'"))
        }
        _ => Ok(value.to_owned()),
    }
}

fn destination(dir: &Path, name: &str) -> anyhow::Result<PathBuf> {
    let stem = file_stem(name)?;
    for index in 0..50 {
        let file_name = if index == 0 {
            format!("{stem}.yaml")
        } else {
            format!("{stem}-{index}.yaml")
        };
        let path = dir.join(file_name);
        if !path.exists() || file_holds_name(&path, name) {
            return Ok(path);
        }
    }
    bail!("could not find a free file name for {name}");
}

fn file_holds_name(path: &Path, name: &str) -> bool {
    let Ok(text) = fs::read_to_string(path) else {
        return false;
    };
    parse_workflow_file(&text)
        .ok()
        .is_some_and(|docs| docs.iter().any(|doc| doc.name == name))
}

fn file_stem(name: &str) -> anyhow::Result<String> {
    let mut stem = String::new();
    let mut pending_dash = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            if pending_dash && !stem.is_empty() {
                stem.push('-');
            }
            pending_dash = false;
            stem.push(ch.to_ascii_lowercase());
        } else {
            pending_dash = true;
        }
    }
    if stem.is_empty() {
        bail!("name needs a letter or digit");
    }
    Ok(stem)
}

fn argument_names(command: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut rest = command;
    while let Some(start) = rest.find("{{") {
        rest = &rest[start + 2..];
        let Some(end) = rest.find("}}") else {
            break;
        };
        let name = &rest[..end];
        if is_argument_name(name) && !names.iter().any(|existing| existing == name) {
            names.push(name.to_owned());
        }
        rest = &rest[end + 2..];
    }
    names
}

fn is_argument_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => {}
        _ => return false,
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn validate_field(key: &str, value: &str) -> anyhow::Result<()> {
    if key != "description" && value.trim().is_empty() {
        bail!("{key} must not be empty");
    }
    if value.chars().any(|ch| ch.is_control()) {
        bail!("{key} must be a single line");
    }
    Ok(())
}

fn quote(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn is_yaml_file(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    lower.ends_with(".yaml") || lower.ends_with(".yml")
}

#[cfg(test)]
#[path = "workflows_tests.rs"]
mod tests;
