use std::fs;

use super::{
    LocalWorkflow, format_workflow_file, list_local_workflows, parse_workflow_file,
    write_local_workflow,
};

#[test]
fn a_saved_workflow_round_trips_with_its_argument() {
    let workflow = LocalWorkflow::new(
        "Checkout",
        r#"git checkout "{{branch}}" && echo "ok""#,
        "Switch branch",
    )
    .unwrap();
    assert_eq!(workflow.arguments, vec!["branch".to_owned()]);

    let text = format_workflow_file(std::slice::from_ref(&workflow)).unwrap();
    assert!(!text.to_ascii_lowercase().contains("api_key"));
    assert!(text.contains("arg_type: Text"));
    let parsed = parse_workflow_file(&text).unwrap();
    assert_eq!(parsed, vec![workflow]);
}

#[test]
fn the_documented_example_loads() {
    let text = r#"---
name: "Git status"
command: "git status --short"
description: "Show the working tree status"
arguments: []
---
"#;
    let parsed = parse_workflow_file(text).unwrap();
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].name, "Git status");
    assert_eq!(parsed[0].command, "git status --short");
    assert!(parsed[0].arguments.is_empty());
}

#[test]
fn write_lists_and_updates_without_clobbering_another_file() {
    let dir = std::env::temp_dir().join(format!("ownterm-workflows-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let status = LocalWorkflow::new("Git status", "git status --short", "Working tree").unwrap();
    let path = write_local_workflow(&dir, &status).unwrap();
    assert_eq!(path.file_name().unwrap(), "git-status.yaml");

    fs::write(dir.join("notes.txt"), "not a workflow").unwrap();
    fs::write(dir.join("broken.yaml"), "name: only\n").unwrap();

    let checkout = LocalWorkflow::new("Checkout", "git checkout {{branch}}", "").unwrap();
    let other = write_local_workflow(&dir, &checkout).unwrap();
    assert_ne!(path, other);

    let updated = LocalWorkflow::new("Git status", "git status", "Short status").unwrap();
    let updated_path = write_local_workflow(&dir, &updated).unwrap();
    assert_eq!(updated_path, path);

    let listed = list_local_workflows(&dir).unwrap();
    assert_eq!(
        listed
            .iter()
            .map(|workflow| workflow.command.as_str())
            .collect::<Vec<_>>(),
        vec!["git checkout {{branch}}", "git status"]
    );
    assert_eq!(listed[0].arguments, vec!["branch".to_owned()]);

    let missing =
        std::env::temp_dir().join(format!("ownterm-workflows-missing-{}", std::process::id()));
    assert!(list_local_workflows(&missing).unwrap().is_empty());
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn names_and_commands_that_are_not_a_title_are_rejected() {
    assert!(LocalWorkflow::new("", "git status", "").is_err());
    assert!(LocalWorkflow::new("Git", "", "").is_err());
    assert!(LocalWorkflow::new("../secret", "git status", "").is_err());
    assert!(LocalWorkflow::new("a/b", "git status", "").is_err());
    assert!(LocalWorkflow::new("Git", "git status\nid", "").is_err());
    assert!(LocalWorkflow::new("***", "git status", "").is_err());
}

#[test]
fn two_placeholders_are_kept_in_order_once() {
    let workflow =
        LocalWorkflow::new("Deploy", "ship {{env}} {{env}} {{region}}", "Roll out").unwrap();
    assert_eq!(
        workflow.arguments,
        vec!["env".to_owned(), "region".to_owned()]
    );
}
