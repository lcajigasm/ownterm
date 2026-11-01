//! Local agent tools OwnTerm offers to an OpenAI-compatible model.
//!
//! The names match tools the terminal already runs on this machine. Parsing
//! stays here so a test can lock the arguments without a model call.

use serde_json::Value;

/// One tool definition sent on the chat-completions request.
#[derive(Clone, Debug, PartialEq)]
pub struct LocalAgentToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: Value,
}

/// A tool call after the model's JSON arguments have been read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalAgentTool {
    Shell {
        command: String,
    },
    ReadFiles {
        files: Vec<String>,
    },
    Grep {
        queries: Vec<String>,
        path: String,
    },
    Glob {
        patterns: Vec<String>,
        search_dir: String,
    },
    Edit {
        summary: String,
        replacements: Vec<SearchReplace>,
        new_files: Vec<NewFile>,
        deleted_files: Vec<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchReplace {
    pub file_path: String,
    pub search: String,
    pub replace: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewFile {
    pub file_path: String,
    pub content: String,
}

/// Tools advertised to the model. Order is stable.
pub fn local_agent_tool_specs() -> Vec<LocalAgentToolSpec> {
    vec![
        LocalAgentToolSpec {
            name: "run_shell_command",
            description: "Execute a shell command in the user's terminal.",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "The exact command line." }
                },
                "required": ["command"]
            }),
        },
        LocalAgentToolSpec {
            name: "read_files",
            description: "Read one or more workspace files.",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "files": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "File paths to read."
                    }
                },
                "required": ["files"]
            }),
        },
        LocalAgentToolSpec {
            name: "grep",
            description: "Search file contents on this machine. Does not use a cloud index.",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "queries": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Patterns to search for."
                    },
                    "path": { "type": "string", "description": "Directory or file to search. Defaults to the working directory." }
                },
                "required": ["queries"]
            }),
        },
        LocalAgentToolSpec {
            name: "file_glob",
            description: "List files matching one or more glob patterns.",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "patterns": {
                        "type": "array",
                        "items": { "type": "string" }
                    },
                    "search_dir": { "type": "string", "description": "Directory to search." }
                },
                "required": ["patterns"]
            }),
        },
        LocalAgentToolSpec {
            name: "apply_file_diffs",
            description: "Edit files with search-and-replace, create a file, or delete a file. The user confirms the diff.",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "summary": { "type": "string" },
                    "diffs": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "file_path": { "type": "string" },
                                "search": { "type": "string" },
                                "replace": { "type": "string" }
                            },
                            "required": ["file_path", "search", "replace"]
                        }
                    },
                    "new_files": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "file_path": { "type": "string" },
                                "content": { "type": "string" }
                            },
                            "required": ["file_path", "content"]
                        }
                    },
                    "deleted_files": {
                        "type": "array",
                        "items": { "type": "string" }
                    }
                }
            }),
        },
    ]
}

/// Reads a model tool call. `None` means the name is not one of OwnTerm's tools.
pub fn parse_local_agent_tool(name: &str, arguments_json: &str) -> Option<LocalAgentTool> {
    let args = serde_json::from_str::<Value>(arguments_json).unwrap_or(Value::Null);
    match name {
        "run_shell_command" | "execute_command" | "bash" | "shell" => Some(LocalAgentTool::Shell {
            command: shell_command(&args, arguments_json),
        }),
        "read_files" | "read_file" => Some(LocalAgentTool::ReadFiles {
            files: read_paths(&args, arguments_json),
        }),
        "grep" | "search" => Some(LocalAgentTool::Grep {
            queries: grep_queries(&args, arguments_json),
            path: first_string(&args, &["path", "search_dir"]).unwrap_or_else(|| ".".to_owned()),
        }),
        "file_glob" | "glob" | "find_files" => Some(LocalAgentTool::Glob {
            patterns: glob_patterns(&args, arguments_json),
            search_dir: first_string(&args, &["search_dir", "path"]).unwrap_or_default(),
        }),
        "apply_file_diffs" | "edit_file" | "str_replace" => Some(parse_edit(&args)),
        _ => None,
    }
}

fn shell_command(args: &Value, raw: &str) -> String {
    first_string(args, &["command", "cmd"])
        .filter(|command| !command.trim().is_empty())
        .unwrap_or_else(|| raw.trim().to_owned())
}

fn read_paths(args: &Value, raw: &str) -> Vec<String> {
    let files = string_list_at(args, &["files", "paths"]);
    if !files.is_empty() {
        return files;
    }
    if let Some(path) = first_string(args, &["path", "file_path", "file"]) {
        return vec![path];
    }
    let raw = raw.trim();
    if raw.is_empty() || raw.starts_with('{') {
        Vec::new()
    } else {
        vec![raw.to_owned()]
    }
}

fn grep_queries(args: &Value, raw: &str) -> Vec<String> {
    let queries = string_list_at(args, &["queries", "patterns", "query", "pattern"]);
    if !queries.is_empty() {
        return queries;
    }
    let raw = raw.trim();
    if raw.is_empty() || raw.starts_with('{') {
        Vec::new()
    } else {
        vec![raw.to_owned()]
    }
}

fn glob_patterns(args: &Value, raw: &str) -> Vec<String> {
    let patterns = string_list_at(args, &["patterns", "pattern", "glob"]);
    if !patterns.is_empty() {
        return patterns;
    }
    let raw = raw.trim();
    if raw.is_empty() || raw.starts_with('{') {
        Vec::new()
    } else {
        vec![raw.to_owned()]
    }
}

fn parse_edit(args: &Value) -> LocalAgentTool {
    let summary = first_string(args, &["summary", "title"]).unwrap_or_default();
    let mut replacements = Vec::new();
    let mut new_files = Vec::new();
    let mut deleted_files = string_list_at(args, &["deleted_files", "delete"]);

    if let Some(diffs) = args.get("diffs").and_then(Value::as_array) {
        for diff in diffs {
            push_change(diff, &mut replacements, &mut new_files);
        }
    }
    if let Some(created) = args.get("new_files").and_then(Value::as_array) {
        for file in created {
            if let Some(created) = new_file(file) {
                new_files.push(created);
            }
        }
    }
    if replacements.is_empty() && new_files.is_empty() {
        push_change(args, &mut replacements, &mut new_files);
    }
    deleted_files.retain(|path| !path.is_empty());
    LocalAgentTool::Edit {
        summary,
        replacements,
        new_files,
        deleted_files,
    }
}

fn push_change(value: &Value, replacements: &mut Vec<SearchReplace>, new_files: &mut Vec<NewFile>) {
    let Some(file_path) = first_string(value, &["file_path", "path", "file"]) else {
        return;
    };
    if let (Some(search), Some(replace)) = (
        first_string(value, &["search", "old_string", "old_text"]),
        optional_string(value, &["replace", "new_string", "new_text"]),
    ) {
        replacements.push(SearchReplace {
            file_path,
            search,
            replace,
        });
        return;
    }
    if let Some(created) = new_file(value) {
        new_files.push(created);
    }
}

fn new_file(value: &Value) -> Option<NewFile> {
    let file_path = first_string(value, &["file_path", "path", "file"])?;
    let content = optional_string(value, &["content", "text"])?;
    Some(NewFile { file_path, content })
}

fn first_string(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_owned)
    })
}

fn optional_string(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str).map(str::to_owned))
}

fn string_list_at(value: &Value, keys: &[&str]) -> Vec<String> {
    keys.iter()
        .find_map(|key| value.get(*key).map(string_list))
        .unwrap_or_default()
}

fn string_list(value: &Value) -> Vec<String> {
    match value {
        Value::String(text) => nonempty(text),
        Value::Array(items) => items
            .iter()
            .flat_map(|item| match item {
                Value::String(text) => nonempty(text),
                Value::Object(_) => first_string(item, &["file_path", "path", "file"])
                    .into_iter()
                    .collect(),
                _ => Vec::new(),
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn nonempty(text: &str) -> Vec<String> {
    let text = text.trim();
    if text.is_empty() {
        Vec::new()
    } else {
        vec![text.to_owned()]
    }
}

#[cfg(test)]
#[path = "agent_tools_tests.rs"]
mod tests;
