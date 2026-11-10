use ai::skills::SkillPathOrigin;
use chrono::Local;
use warp_core::command::ExitCode;
use warp_multi_agent_api::client_action::Action;
use warp_multi_agent_api::{ShellCommandFinished, input_context};
use warpui::{App, EntityId};

use super::*;
use crate::ai::agent::conversation::{AIConversation, UpdateConversationError};
use crate::ai::agent::task::TaskId;
use crate::ai::agent::{
    AIAgentActionId, AIAgentActionResult, AIAgentActionResultType, RequestCommandOutputResult,
    UserQueryMode,
};
use crate::ai::blocklist::history_model::BlocklistAIHistoryModel;
use crate::ai::blocklist::{RequestInput, ResponseStreamId};
use crate::ai::llms::LLMId;
use crate::terminal::model::block::BlockId;
use crate::test_util::settings::initialize_history_persistence_for_tests;

fn user_query(query: &str) -> AIAgentInput {
    AIAgentInput::UserQuery {
        query: query.to_owned(),
        context: Default::default(),
        static_query_type: None,
        referenced_attachments: Default::default(),
        user_query_mode: UserQueryMode::Normal,
        running_command: None,
        intended_agent: None,
        base: None,
    }
}

fn command_result(action_id: &str, task_id: &str, output: &str) -> AIAgentInput {
    AIAgentInput::ActionResult {
        result: AIAgentActionResult {
            id: AIAgentActionId::from(action_id.to_owned()),
            task_id: TaskId::new(task_id.to_owned()),
            result: AIAgentActionResultType::RequestCommandOutput(
                RequestCommandOutputResult::Completed {
                    block_id: BlockId::new(),
                    command: "ls".to_owned(),
                    output: output.to_owned(),
                    exit_code: ExitCode::from(0),
                    start_ts: None,
                    completed_ts: None,
                },
            ),
        },
        context: Default::default(),
    }
}

fn message(kind: MessageKind) -> Message {
    new_message("task", "request", Uuid::new_v4().to_string(), kind)
}

fn shell_call(id: &str, command: &str) -> Message {
    message(MessageKind::ToolCall(ToolCall {
        tool_call_id: id.to_owned(),
        tool: Some(Tool::RunShellCommand(RunShellCommand {
            command: command.to_owned(),
            ..Default::default()
        })),
    }))
}

fn shell_result(id: &str, output: &str, exit_code: i32) -> Message {
    message(MessageKind::ToolCallResult(ToolCallResult {
        tool_call_id: id.to_owned(),
        context: None,
        result: Some(ToolResult::RunShellCommand(RunShellCommandResult {
            command: "ls".to_owned(),
            result: Some(run_shell_command_result::Result::CommandFinished(
                ShellCommandFinished {
                    output: output.to_owned(),
                    exit_code,
                    ..Default::default()
                },
            )),
            ..Default::default()
        })),
    }))
}

fn actions_of(plan: &TurnPlan) -> Vec<&Action> {
    plan.opening_actions
        .iter()
        .filter_map(|action| action.action.as_ref())
        .collect()
}

fn delta(
    index: Option<usize>,
    id: Option<&str>,
    name: Option<&str>,
    arguments: &str,
) -> ChatDeltaToolCall {
    ChatDeltaToolCall {
        index,
        id: id.map(str::to_owned),
        r#type: None,
        function: Some(ChatDeltaFunction {
            name: name.map(str::to_owned),
            arguments: Some(arguments.to_owned()),
        }),
    }
}

#[test]
fn rejects_plain_http_for_remote_hosts() {
    assert!(chat_completions_url("http://example.com/v1").is_err());
    assert_eq!(
        chat_completions_url("http://127.0.0.1:11434/v1")
            .unwrap()
            .as_str(),
        "http://127.0.0.1:11434/v1/chat/completions"
    );
}

#[test]
fn parses_text_tool_call_and_finish_reason_chunks() {
    let text = parse_streaming_chunk(r#"{"choices":[{"delta":{"content":"hola"}}]}"#)
        .unwrap()
        .unwrap();
    assert_eq!(text.text.as_deref(), Some("hola"));

    assert!(
        parse_streaming_chunk(r#"{"choices":[{"delta":{"role":"assistant"}}]}"#)
            .unwrap()
            .is_none()
    );

    let finished = parse_streaming_chunk(r#"{"choices":[{"delta":{},"finish_reason":"length"}]}"#)
        .unwrap()
        .unwrap();
    assert_eq!(finished.finish_reason.as_deref(), Some("length"));

    let error =
        parse_streaming_chunk(r#"{"error":{"message":"Incorrect API key provided"}}"#).unwrap_err();
    assert!(error.to_string().contains("Incorrect API key provided"));
}

#[test]
fn redacts_api_key_from_error_message() {
    let secret = "sk-proj-supersecret123456789";
    let sanitized = sanitize_error_message(format!("bad key {secret}"), Some(secret));
    assert_eq!(sanitized, "bad key [REDACTED]");
}

#[test]
fn unknown_or_malformed_tool_calls_are_never_run() {
    assert!(matches!(
        local_tool_call("run_shell_command", r#"{"command":"cargo check"}"#),
        Ok(Tool::RunShellCommand(command)) if command.command == "cargo check"
    ));
    assert!(local_tool_call("rm_everything", r#"{"path":"/"}"#).is_err());
    assert!(local_tool_call("run_shell_command", r#"{"command":"rm -rf /tm"#).is_err());
    assert!(local_tool_call("run_shell_command", r#"{"command":""}"#).is_err());
    assert!(local_tool_call("read_files", "{}").is_err());
}

#[test]
fn repeated_ids_and_names_are_not_concatenated() {
    let mut pending = Vec::new();
    merge_tool_call_delta(
        &mut pending,
        delta(Some(0), Some("call_1"), Some("grep"), r#"{"queries":"#),
    );
    merge_tool_call_delta(
        &mut pending,
        delta(Some(0), Some("call_1"), Some("grep"), r#"["main"]}"#),
    );

    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, "call_1");
    assert_eq!(pending[0].name, "grep");
    assert_eq!(pending[0].arguments, r#"{"queries":["main"]}"#);
}

#[test]
fn fragments_without_an_index_split_on_a_new_call_id() {
    let mut pending = Vec::new();
    merge_tool_call_delta(&mut pending, delta(None, Some("a"), Some("grep"), "{"));
    merge_tool_call_delta(&mut pending, delta(None, None, None, "}"));
    merge_tool_call_delta(
        &mut pending,
        delta(None, Some("b"), Some("file_glob"), "{}"),
    );

    assert_eq!(pending.len(), 2);
    assert_eq!(pending[0].arguments, "{}");
    assert_eq!(pending[1].name, "file_glob");
}

#[test]
fn a_tool_call_cut_off_by_the_output_limit_is_dropped() {
    let pending = vec![PendingToolCall {
        id: "call_1".to_owned(),
        name: "run_shell_command".to_owned(),
        arguments: r#"{"command":"rm -rf build"}"#.to_owned(),
    }];

    let (messages, notices) = finished_tool_calls(pending.clone(), Some("length"), "task", "req");
    assert!(messages.is_empty());
    assert_eq!(notices.len(), 1);

    let (messages, notices) = finished_tool_calls(pending, Some("tool_calls"), "task", "req");
    assert_eq!(messages.len(), 1);
    assert!(notices.is_empty());
}

#[test]
fn parallel_tool_calls_share_one_assistant_message() {
    let history = [
        message(plain_query_kind("list both".to_owned(), None)),
        message(MessageKind::AgentOutput(AgentOutput {
            text: "Running both.".to_owned(),
        })),
        shell_call("a", "ls src"),
        shell_call("b", "ls docs"),
        shell_result("b", "usage.md", 0),
        shell_result("a", "main.rs", 0),
    ];
    let history: Vec<&Message> = history.iter().collect();

    let messages = build_chat_messages(&history);
    let roles: Vec<&str> = messages
        .iter()
        .map(|message| message.role.as_str())
        .collect();
    assert_eq!(roles, ["system", "user", "assistant", "tool", "tool"]);

    let assistant = &messages[2];
    assert_eq!(assistant.content.as_deref(), Some("Running both."));
    let call_ids: Vec<&str> = assistant
        .tool_calls
        .iter()
        .flatten()
        .map(|call| call.id.as_str())
        .collect();
    assert_eq!(call_ids, ["a", "b"]);
    assert_eq!(messages[3].tool_call_id.as_deref(), Some("a"));
    assert_eq!(messages[3].content.as_deref(), Some("main.rs"));
    assert_eq!(messages[4].content.as_deref(), Some("usage.md"));
}

#[test]
fn a_tool_call_without_a_result_is_answered_as_cancelled() {
    let history = [
        message(plain_query_kind("run it".to_owned(), None)),
        shell_call("a", "make"),
        message(plain_query_kind("never mind".to_owned(), None)),
    ];
    let history: Vec<&Message> = history.iter().collect();

    let messages = build_chat_messages(&history);
    let roles: Vec<&str> = messages
        .iter()
        .map(|message| message.role.as_str())
        .collect();
    assert_eq!(roles, ["system", "user", "assistant", "tool", "user"]);
    assert!(
        messages[3]
            .content
            .as_deref()
            .unwrap()
            .contains("cancelled")
    );
}

#[test]
fn shell_results_report_output_and_exit_code() {
    let history = [
        message(plain_query_kind("build".to_owned(), None)),
        shell_call("a", "make"),
        shell_result("a", "error: missing target", 2),
    ];
    let history: Vec<&Message> = history.iter().collect();

    let messages = build_chat_messages(&history);
    assert_eq!(
        messages[3].content.as_deref(),
        Some("(exit code: 2)\nerror: missing target")
    );
}

#[test]
fn session_context_reaches_the_model() {
    let context = InputContext {
        directory: Some(input_context::Directory {
            pwd: "/work/project".to_owned(),
            ..Default::default()
        }),
        selected_text: vec![input_context::SelectedText {
            text: "panicked at src/main.rs:4".to_owned(),
        }],
        ..Default::default()
    };
    let history = [message(plain_query_kind(
        "why did this fail?".to_owned(),
        Some(context),
    ))];
    let history: Vec<&Message> = history.iter().collect();

    let messages = build_chat_messages(&history);
    assert!(
        messages[0]
            .content
            .as_deref()
            .unwrap()
            .contains("Working directory: /work/project")
    );
    let user = messages[1].content.as_deref().unwrap();
    assert!(user.starts_with("why did this fail?"));
    assert!(user.contains("panicked at src/main.rs:4"));
}

#[test]
fn long_tool_output_keeps_its_start_and_end() {
    let text = format!("start{}end", "x".repeat(1_000));
    let truncated = truncate_middle(&text, 100);
    assert!(truncated.starts_with("start"));
    assert!(truncated.ends_with("end"));
    assert!(truncated.contains("characters omitted"));
    assert_eq!(truncate_middle("short", 100), "short");
}

#[test]
fn an_oversized_history_drops_its_oldest_turns_whole() {
    let history = [
        message(plain_query_kind("first question".to_owned(), None)),
        shell_call("a", "ls"),
        shell_result("a", &"x".repeat(500), 0),
        message(plain_query_kind("second question".to_owned(), None)),
        message(MessageKind::AgentOutput(AgentOutput {
            text: "second answer".to_owned(),
        })),
        message(plain_query_kind("third question".to_owned(), None)),
    ];
    let history: Vec<&Message> = history.iter().collect();
    let messages = build_chat_messages(&history);
    let system_chars = chat_message_chars(&messages[0]);

    let fitted = fit_history(messages.clone(), system_chars + 100);
    let contents: Vec<&str> = fitted[1..]
        .iter()
        .filter_map(|message| message.content.as_deref())
        .collect();
    assert_eq!(fitted[0].role, "system");
    assert_eq!(
        contents,
        ["second question", "second answer", "third question"]
    );

    let latest_only = fit_history(messages, 1);
    assert_eq!(
        latest_only.last().unwrap().content.as_deref(),
        Some("third question")
    );
    assert_eq!(latest_only.len(), 2);
}

#[test]
fn first_turn_creates_the_task_and_records_the_query() {
    let plan = plan_turn(&[], vec![user_query("list files")], "req-1").unwrap();

    let actions = actions_of(&plan);
    assert_eq!(actions.len(), 2);
    let Action::CreateTask(create) = actions[0] else {
        panic!("expected CreateTask first, got {:?}", actions[0]);
    };
    assert_eq!(create.task.as_ref().unwrap().id, plan.task_id);
    let Action::AddMessagesToTask(added) = actions[1] else {
        panic!("expected AddMessagesToTask, got {:?}", actions[1]);
    };
    assert_eq!(added.task_id, plan.task_id);
    assert!(matches!(
        &added.messages[0].message,
        Some(MessageKind::UserQuery(query)) if query.query == "list files"
    ));
    assert_eq!(plan.routing_prompt, "list files");
    assert_eq!(plan.messages.last().unwrap().role, "user");
}

#[test]
fn a_tool_result_turn_continues_the_conversation() {
    let task = Task {
        id: "root".to_owned(),
        messages: vec![
            message(plain_query_kind("list files".to_owned(), None)),
            shell_call("call-1", "ls"),
        ],
        ..Default::default()
    };

    let plan = plan_turn(
        &[task],
        vec![command_result("call-1", "root", "Cargo.toml")],
        "req-2",
    )
    .unwrap();

    let actions = actions_of(&plan);
    assert_eq!(actions.len(), 1, "an existing task must not be recreated");
    assert!(matches!(
        actions[0],
        Action::AddMessagesToTask(added)
            if matches!(&added.messages[0].message, Some(MessageKind::ToolCallResult(_)))
    ));
    assert_eq!(plan.task_id, "root");
    assert_eq!(plan.routing_prompt, "list files");
    let last = plan.messages.last().unwrap();
    assert_eq!(last.role, "tool");
    assert_eq!(last.tool_call_id.as_deref(), Some("call-1"));
    assert_eq!(last.content.as_deref(), Some("Cargo.toml"));
}

#[test]
fn a_request_with_no_query_anywhere_is_rejected() {
    let result = plan_turn(
        &[],
        vec![command_result("call-1", "root", "Cargo.toml")],
        "req-1",
    );
    assert!(result.is_err());
}

fn request_input(
    conversation: &AIConversation,
    task_id: TaskId,
    input: AIAgentInput,
) -> RequestInput {
    RequestInput {
        conversation_id: conversation.id(),
        input_messages: Vec::from([(task_id, vec![input])]),
        working_directory: None,
        model_id: LLMId::from("test-model"),
        coding_model_id: LLMId::from("test-coding-model"),
        cli_agent_model_id: LLMId::from("test-cli-agent-model"),
        computer_use_model_id: LLMId::from("test-computer-use-model"),
        shared_session_response_initiator: None,
        request_start_ts: Local::now(),
        supported_tools_override: None,
    }
}

/// Drives two turns through a real conversation: the events the adapter emits must be accepted
/// by the client, and the second request must see the first turn's history.
#[test]
fn the_conversation_accepts_two_turns_with_a_tool_call() {
    App::test((), |mut app| async move {
        initialize_history_persistence_for_tests(&mut app);
        let history_model =
            app.add_singleton_model(|_| BlocklistAIHistoryModel::new(vec![], vec![], &[]));
        let surface = EntityId::new();
        let mut conversation = AIConversation::new(false, false);
        let optimistic_root = conversation.get_root_task_id().clone();

        // Turn 1: a query, answered with text and one tool call.
        let stream_1 = ResponseStreamId::new_for_test();
        let input = user_query("list files");
        history_model.update(&mut app, |_, ctx| {
            conversation
                .update_for_new_request_input(
                    request_input(&conversation, optimistic_root, input.clone()),
                    stream_1.clone(),
                    surface,
                    ctx,
                )
                .expect("new request input should apply");
        });
        assert!(
            conversation.compute_active_tasks().is_empty(),
            "a new conversation sends no tasks, so the adapter must create one"
        );
        let plan = plan_turn(&conversation.compute_active_tasks(), vec![input], "req-1").unwrap();
        let task_id = plan.task_id.clone();
        let mut started = false;
        let mut turn_1: Vec<ClientAction> = plan.opening_actions;
        turn_1.push(text_action(
            &mut started,
            &task_id,
            "req-1",
            "message-1",
            "Listing".to_owned(),
        ));
        turn_1.push(text_action(
            &mut started,
            &task_id,
            "req-1",
            "message-1",
            " them.".to_owned(),
        ));
        let (tool_messages, notices) = finished_tool_calls(
            vec![PendingToolCall {
                id: "call-1".to_owned(),
                name: "run_shell_command".to_owned(),
                arguments: r#"{"command":"ls"}"#.to_owned(),
            }],
            Some("tool_calls"),
            &task_id,
            "req-1",
        );
        assert!(notices.is_empty());
        turn_1.push(add_messages_action(&task_id, tool_messages));

        history_model.update(&mut app, |_, ctx| {
            conversation
                .initialize_output_for_response_stream(
                    &stream_1,
                    StreamInit {
                        request_id: "req-1".to_owned(),
                        conversation_id: "local-conversation".to_owned(),
                        run_id: String::new(),
                    },
                    surface,
                    ctx,
                )
                .expect("stream init should apply");

            // Without the task the adapter now creates, the client rejects the output.
            let orphan = add_messages_action("unknown-task", Vec::new());
            assert!(matches!(
                conversation.apply_client_action(
                    &stream_1,
                    surface,
                    orphan.action.unwrap(),
                    &SkillPathOrigin::Unavailable,
                    ctx,
                ),
                Err(UpdateConversationError::TaskNotFound)
            ));

            for action in turn_1 {
                conversation
                    .apply_client_action(
                        &stream_1,
                        surface,
                        action.action.unwrap(),
                        &SkillPathOrigin::Unavailable,
                        ctx,
                    )
                    .expect("adapter output should apply");
            }
            conversation
                .mark_request_completed(&stream_1, surface, ctx)
                .expect("request should complete");
        });

        // Turn 2: the tool result comes back with no user query.
        let tasks = conversation.compute_active_tasks();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, task_id);
        let stream_2 = ResponseStreamId::new_for_test();
        let result = command_result("call-1", &task_id, "Cargo.toml");
        history_model.update(&mut app, |_, ctx| {
            conversation
                .update_for_new_request_input(
                    request_input(&conversation, TaskId::new(task_id.clone()), result.clone()),
                    stream_2.clone(),
                    surface,
                    ctx,
                )
                .expect("tool result input should apply");
        });
        let plan = plan_turn(&tasks, vec![result], "req-2").unwrap();

        let roles: Vec<&str> = plan
            .messages
            .iter()
            .map(|message| message.role.as_str())
            .collect();
        assert_eq!(roles, ["system", "user", "assistant", "tool"]);
        assert_eq!(plan.messages[1].content.as_deref(), Some("list files"));
        assert_eq!(plan.messages[2].content.as_deref(), Some("Listing them."));
        assert_eq!(plan.messages[3].content.as_deref(), Some("Cargo.toml"));

        history_model.update(&mut app, |_, ctx| {
            conversation
                .initialize_output_for_response_stream(
                    &stream_2,
                    StreamInit {
                        request_id: "req-2".to_owned(),
                        conversation_id: "local-conversation".to_owned(),
                        run_id: String::new(),
                    },
                    surface,
                    ctx,
                )
                .expect("stream init should apply");
            for action in plan.opening_actions {
                conversation
                    .apply_client_action(
                        &stream_2,
                        surface,
                        action.action.unwrap(),
                        &SkillPathOrigin::Unavailable,
                        ctx,
                    )
                    .expect("tool result echo should apply");
            }
        });
    });
}

fn sse(chunks: &[&str]) -> String {
    chunks
        .iter()
        .map(|chunk| format!("data: {chunk}\n\n"))
        .collect()
}

fn attempt(server: &mockito::ServerGuard) -> LiveAttempt {
    LiveAttempt {
        endpoint: chat_completions_url(&format!("{}/v1", server.url())).unwrap(),
        model: "test-model".to_owned(),
        api_key: None,
        reason: String::new(),
        announce: false,
    }
}

async fn collect_actions(stream: ResponseStream) -> Vec<Action> {
    let events: Vec<_> = stream.collect().await;
    let mut actions = Vec::new();
    for event in events {
        let event = event.expect("stream should not fail");
        if let Some(response_event::Type::ClientActions(batch)) = event.r#type {
            actions.extend(batch.actions.into_iter().filter_map(|action| action.action));
        }
    }
    actions
}

#[tokio::test]
async fn streams_text_and_a_tool_call_from_an_sse_endpoint() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/chat/completions")
        .with_header("content-type", "text/event-stream")
        .with_body(sse(&[
            r#"{"choices":[{"delta":{"role":"assistant","content":"On it."}}]}"#,
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"run_shell_command","arguments":"{\"comm"}}]}}]}"#,
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"and\":\"ls\"}"}}]}}]}"#,
            r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#,
            "[DONE]",
        ]))
        .create_async()
        .await;
    let plan = plan_turn(&[], vec![user_query("list files")], "req-1").unwrap();
    let (_cancel, cancellation_rx) = futures::channel::oneshot::channel();

    let actions = collect_actions(stream_turn(
        plan,
        vec![attempt(&server)],
        "req-1".to_owned(),
        "conversation".to_owned(),
        cancellation_rx,
    ))
    .await;

    mock.assert_async().await;
    assert!(matches!(actions[0], Action::CreateTask(_)));
    assert!(matches!(
        &actions[1],
        Action::AddMessagesToTask(added)
            if matches!(&added.messages[0].message, Some(MessageKind::UserQuery(_)))
    ));
    assert!(matches!(
        &actions[2],
        Action::AddMessagesToTask(added)
            if matches!(
                &added.messages[0].message,
                Some(MessageKind::AgentOutput(output)) if output.text == "On it."
            )
    ));
    let Some(Action::AddMessagesToTask(added)) = actions.last() else {
        panic!("expected the tool call last, got {:?}", actions.last());
    };
    assert!(matches!(
        &added.messages[0].message,
        Some(MessageKind::ToolCall(call))
            if call.tool_call_id == "call_1"
                && matches!(&call.tool, Some(Tool::RunShellCommand(command)) if command.command == "ls")
    ));
}

#[tokio::test]
async fn retries_without_tools_when_the_model_rejects_them() {
    let mut server = mockito::Server::new_async().await;
    let rejected = server
        .mock("POST", "/v1/chat/completions")
        .match_body(mockito::Matcher::Regex("\"tools\"".to_owned()))
        .with_status(400)
        .with_body(r#"{"error":{"message":"model does not support tools"}}"#)
        .create_async()
        .await;
    let plain = server
        .mock("POST", "/v1/chat/completions")
        .with_header("content-type", "text/event-stream")
        .with_body(sse(&[
            r#"{"choices":[{"delta":{"content":"hello"}}]}"#,
            "[DONE]",
        ]))
        .create_async()
        .await;
    let plan = plan_turn(&[], vec![user_query("hi")], "req-1").unwrap();
    let (_cancel, cancellation_rx) = futures::channel::oneshot::channel();

    let actions = collect_actions(stream_turn(
        plan,
        vec![attempt(&server)],
        "req-1".to_owned(),
        "conversation".to_owned(),
        cancellation_rx,
    ))
    .await;

    rejected.assert_async().await;
    plain.assert_async().await;
    assert!(matches!(
        actions.last(),
        Some(Action::AddMessagesToTask(added))
            if matches!(
                &added.messages[0].message,
                Some(MessageKind::AgentOutput(output)) if output.text == "hello"
            )
    ));
}
