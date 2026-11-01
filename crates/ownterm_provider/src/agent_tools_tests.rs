use super::{
    LocalAgentTool, NewFile, SearchReplace, local_agent_tool_specs, parse_local_agent_tool,
};

#[test]
fn advertised_tools_are_the_local_set() {
    let names: Vec<_> = local_agent_tool_specs()
        .into_iter()
        .map(|spec| spec.name)
        .collect();
    assert_eq!(
        names,
        vec![
            "run_shell_command",
            "read_files",
            "grep",
            "file_glob",
            "apply_file_diffs",
        ]
    );
}

#[test]
fn shell_reads_command_and_a_raw_line() {
    assert_eq!(
        parse_local_agent_tool("bash", r#"{"cmd":"git status"}"#),
        Some(LocalAgentTool::Shell {
            command: "git status".to_owned(),
        })
    );
    assert_eq!(
        parse_local_agent_tool("run_shell_command", "echo hi"),
        Some(LocalAgentTool::Shell {
            command: "echo hi".to_owned(),
        })
    );
}

#[test]
fn read_files_accepts_a_list_or_one_path() {
    assert_eq!(
        parse_local_agent_tool("read_files", r#"{"files":["a.rs","b.rs"]}"#),
        Some(LocalAgentTool::ReadFiles {
            files: vec!["a.rs".to_owned(), "b.rs".to_owned()],
        })
    );
    assert_eq!(
        parse_local_agent_tool("read_file", r#"{"path":"Cargo.toml"}"#),
        Some(LocalAgentTool::ReadFiles {
            files: vec!["Cargo.toml".to_owned()],
        })
    );
}

#[test]
fn grep_and_glob_stay_on_this_machine() {
    assert_eq!(
        parse_local_agent_tool("search", r#"{"pattern":"fn main","path":"src"}"#),
        Some(LocalAgentTool::Grep {
            queries: vec!["fn main".to_owned()],
            path: "src".to_owned(),
        })
    );
    assert_eq!(
        parse_local_agent_tool("grep", "TODO"),
        Some(LocalAgentTool::Grep {
            queries: vec!["TODO".to_owned()],
            path: ".".to_owned(),
        })
    );
    assert_eq!(
        parse_local_agent_tool(
            "file_glob",
            r#"{"patterns":["**/*.rs"],"search_dir":"app"}"#
        ),
        Some(LocalAgentTool::Glob {
            patterns: vec!["**/*.rs".to_owned()],
            search_dir: "app".to_owned(),
        })
    );
}

#[test]
fn edit_accepts_replace_create_and_delete() {
    assert_eq!(
        parse_local_agent_tool(
            "str_replace",
            r#"{"path":"src/main.rs","old_string":"foo","new_string":"bar"}"#,
        ),
        Some(LocalAgentTool::Edit {
            summary: String::new(),
            replacements: vec![SearchReplace {
                file_path: "src/main.rs".to_owned(),
                search: "foo".to_owned(),
                replace: "bar".to_owned(),
            }],
            new_files: vec![],
            deleted_files: vec![],
        })
    );
    assert_eq!(
        parse_local_agent_tool(
            "apply_file_diffs",
            r#"{"summary":"add note","new_files":[{"file_path":"NOTE","content":"hi"}],"deleted_files":["old.txt"]}"#,
        ),
        Some(LocalAgentTool::Edit {
            summary: "add note".to_owned(),
            replacements: vec![],
            new_files: vec![NewFile {
                file_path: "NOTE".to_owned(),
                content: "hi".to_owned(),
            }],
            deleted_files: vec!["old.txt".to_owned()],
        })
    );
}

#[test]
fn an_unknown_tool_is_not_rewritten() {
    assert_eq!(
        parse_local_agent_tool("use_computer", r#"{"action":"click"}"#),
        None
    );
}
