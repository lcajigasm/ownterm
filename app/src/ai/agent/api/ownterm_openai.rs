use std::sync::Arc;
use std::time::SystemTime;

use anyhow::{Context, anyhow};
use futures_util::StreamExt;
use prost_types::FieldMask;
use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;
use warp_multi_agent_api::client_action::{AddMessagesToTask, AppendToMessageContent};
use warp_multi_agent_api::message::tool_call::apply_file_diffs::{DeleteFile, FileDiff, NewFile};
use warp_multi_agent_api::message::tool_call::read_files::File as ReadFilesFile;
use warp_multi_agent_api::message::tool_call::{
    ApplyFileDiffs, FileGlobV2, Grep, ReadFiles, RunShellCommand, Tool,
};
use warp_multi_agent_api::message::tool_call_result::Result as ToolResult;
use warp_multi_agent_api::message::{AgentOutput, Message as MessageKind, ToolCall};
use warp_multi_agent_api::response_event::{StreamFinished, StreamInit, stream_finished};
use warp_multi_agent_api::{ClientAction, Message, ResponseEvent, client_action, response_event};

use super::{ConvertToAPITypeError, RequestParams, ResponseStream};
use crate::server::server_api::AIApiError;

const AGENT_OUTPUT_TEXT_PATH: &str = "agent_output.text";

#[derive(Clone, Debug, PartialEq, Eq)]
struct OpenAiCompatibleConfig {
    endpoint: Url,
    model: String,
    api_key: Option<String>,
}

#[derive(Serialize)]
struct ChatCompletionsRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<ToolDefinition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<String>,
    stream: bool,
}

#[derive(Clone, Serialize, Deserialize)]
struct ToolDefinition {
    r#type: &'static str,
    function: FunctionDefinition,
}

#[derive(Clone, Serialize, Deserialize)]
struct FunctionDefinition {
    name: &'static str,
    description: &'static str,
    parameters: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct ChatMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenAiToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct OpenAiToolCall {
    id: String,
    r#type: String,
    function: OpenAiFunctionCall,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct OpenAiFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Deserialize)]
struct ChatCompletionsChunk {
    #[serde(default)]
    choices: Vec<ChatChoice>,
    #[serde(default)]
    error: Option<ChatErrorPayload>,
}

#[derive(Deserialize)]
struct ChatErrorPayload {
    message: String,
}

#[derive(Deserialize, Default)]
struct ChatChoice {
    #[serde(default)]
    delta: ChatDelta,
    #[serde(default)]
    #[allow(dead_code)]
    finish_reason: Option<String>,
}

#[derive(Deserialize, Default)]
struct ChatDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<ChatDeltaToolCall>>,
}

#[derive(Deserialize, Clone, Debug)]
struct ChatDeltaToolCall {
    #[serde(default)]
    index: Option<usize>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    r#type: Option<String>,
    #[serde(default)]
    function: Option<ChatDeltaFunction>,
}

#[derive(Deserialize, Clone, Debug)]
struct ChatDeltaFunction {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

#[derive(Default, Debug, Clone)]
struct PendingToolCall {
    id: String,
    name: String,
    arguments: String,
}

#[derive(Debug)]
struct ParsedChunk {
    text: Option<String>,
    tool_calls: Option<Vec<ChatDeltaToolCall>>,
}

struct LiveAttempt {
    endpoint: Url,
    model: String,
    api_key: Option<String>,
    reason: String,
    announce: bool,
}

fn live_attempts(params: &RequestParams, prompt: &str) -> anyhow::Result<Vec<LiveAttempt>> {
    match ownterm_provider::choose_model(prompt)? {
        Some(ownterm_provider::ModelChoice::Explicit(endpoint)) => Ok(vec![LiveAttempt {
            endpoint: endpoint.endpoint,
            model: endpoint.model,
            api_key: endpoint.api_key,
            reason: String::new(),
            announce: false,
        }]),
        Some(ownterm_provider::ModelChoice::Routed(attempts)) => Ok(attempts
            .into_iter()
            .map(|attempt| LiveAttempt {
                endpoint: attempt.endpoint,
                model: attempt.model,
                api_key: attempt.api_key,
                reason: attempt.reason,
                announce: true,
            })
            .collect()),
        None => {
            let config = OpenAiCompatibleConfig::from_custom_endpoint(params).ok_or_else(|| {
                anyhow!(
                    "Configure an OpenAI-compatible endpoint in Settings, or set OWNTERM_BASE_URL and OWNTERM_MODEL"
                )
            })?;
            Ok(vec![LiveAttempt {
                endpoint: config.endpoint,
                model: config.model,
                api_key: config.api_key,
                reason: String::new(),
                announce: false,
            }])
        }
    }
}

fn failure_kind(error: &reqwest_eventsource::Error) -> ownterm_provider::AttemptFailure {
    match error {
        reqwest_eventsource::Error::InvalidStatusCode(status, _) => {
            ownterm_provider::classify_failure(Some(status.as_u16()), "")
        }
        reqwest_eventsource::Error::Transport(source) => {
            ownterm_provider::classify_failure(None, &source.to_string())
        }
        other => ownterm_provider::classify_failure(None, &other.to_string()),
    }
}

pub(super) fn generate(
    params: RequestParams,
    cancellation_rx: futures::channel::oneshot::Receiver<()>,
) -> Result<ResponseStream, ConvertToAPITypeError> {
    let prompt = params
        .input
        .iter()
        .rev()
        .find_map(|input| input.display_query())
        .ok_or_else(|| anyhow!("OwnTerm direct inference requires a user prompt"))?;
    let task_id = params
        .tasks
        .first()
        .map(|task| task.id.clone())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let request_id = Uuid::new_v4().to_string();
    let conversation_id = params
        .conversation_token
        .as_ref()
        .map(|token| token.as_str().to_owned())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let message_id = Uuid::new_v4().to_string();
    let messages = build_chat_messages(&params.tasks, &prompt);
    let tools = ownterm_tools();
    let attempts = live_attempts(&params, &prompt)?;

    let output = async_stream::stream! {
        yield Ok(ResponseEvent {
            r#type: Some(response_event::Type::Init(StreamInit {
                request_id: request_id.clone(),
                conversation_id,
                run_id: String::new(),
            })),
        });

        let client = http_client::Client::new_external();
        let mut started_message = false;
        let mut pending_tool_calls: Vec<PendingToolCall> = Vec::new();
        let mut attempt_at = 0usize;

        'attempts: loop {
            let spec = &attempts[attempt_at];
            pending_tool_calls.clear();
            let request = ChatCompletionsRequest {
                model: spec.model.clone(),
                messages: messages.clone(),
                tools: tools.clone(),
                tool_choice: None,
                stream: true,
            };
            let mut builder = client
                .post(spec.endpoint.clone())
                .timeout(std::time::Duration::from_secs(45))
                .json(&request);
            if let Some(ref api_key) = spec.api_key {
                builder = builder.bearer_auth(api_key);
            }
            let mut events = builder.eventsource();
            let mut committed = false;

            while let Some(event) = events.next().await {
                match event {
                    Ok(reqwest_eventsource::Event::Open) => {}
                    Ok(reqwest_eventsource::Event::Message(message)) => {
                        if message.data.trim() == "[DONE]" {
                            break;
                        }
                        match parse_streaming_chunk(&message.data) {
                            Ok(Some(parsed)) => {
                                if !committed && spec.announce {
                                    ownterm_provider::record_last_choice(&spec.reason);
                                    yield Ok(client_action_event(add_text_action(
                                        &task_id,
                                        &request_id,
                                        &message_id,
                                        format!("{}\n", spec.reason),
                                    )));
                                    started_message = true;
                                }
                                committed = true;
                                if let Some(text) = parsed.text {
                                    let action = if started_message {
                                        append_text_action(
                                            &task_id,
                                            &request_id,
                                            &message_id,
                                            text,
                                        )
                                    } else {
                                        started_message = true;
                                        add_text_action(
                                            &task_id,
                                            &request_id,
                                            &message_id,
                                            text,
                                        )
                                    };
                                    yield Ok(client_action_event(action));
                                }
                                if let Some(tool_calls) = parsed.tool_calls {
                                    for delta_call in tool_calls {
                                        let idx = delta_call.index.unwrap_or(0);
                                        if idx >= pending_tool_calls.len() {
                                            pending_tool_calls.resize_with(idx + 1, PendingToolCall::default);
                                        }
                                        let pending = &mut pending_tool_calls[idx];
                                        if let Some(id) = delta_call.id {
                                            pending.id.push_str(&id);
                                        }
                                        if let Some(func) = delta_call.function {
                                            if let Some(name) = func.name {
                                                pending.name.push_str(&name);
                                            }
                                            if let Some(args) = func.arguments {
                                                pending.arguments.push_str(&args);
                                            }
                                        }
                                    }
                                }
                            }
                            Ok(None) => {}
                            Err(error) => {
                                if !committed
                                    && attempt_at + 1 < attempts.len()
                                {
                                    attempt_at += 1;
                                    continue 'attempts;
                                }
                                yield Err(Arc::new(AIApiError::Other(error)));
                                return;
                            }
                        }
                    }
                    Err(error) => {
                        if !committed
                            && attempt_at + 1 < attempts.len()
                            && ownterm_provider::should_fallback(failure_kind(&error))
                        {
                            attempt_at += 1;
                            continue 'attempts;
                        }
                        let api_error =
                            AIApiError::from_stream_error("OwnTermOpenAICompatible", error).await;
                        let sanitized = match api_error {
                            AIApiError::ErrorStatus(status, body) => AIApiError::ErrorStatus(
                                status,
                                sanitize_error_message(body, spec.api_key.as_deref()),
                            ),
                            other => other,
                        };
                        yield Err(Arc::new(sanitized));
                        return;
                    }
                }
            }
            break;
        }

        if !pending_tool_calls.is_empty() {
            let mut tool_messages = Vec::new();
            for pending in pending_tool_calls {
                if let Some(tool) = map_pending_tool_to_proto(&pending.name, &pending.arguments) {
                    let tool_call_id = if pending.id.is_empty() {
                        Uuid::new_v4().to_string()
                    } else {
                        pending.id
                    };
                    tool_messages.push(tool_call_message(
                        &task_id,
                        &request_id,
                        tool_call_id,
                        tool,
                    ));
                }
            }
            if !tool_messages.is_empty() {
                yield Ok(client_action_event(ClientAction {
                    action: Some(client_action::Action::AddMessagesToTask(
                        AddMessagesToTask {
                            task_id: task_id.clone(),
                            messages: tool_messages,
                        },
                    )),
                }));
            }
        }

        yield Ok(ResponseEvent {
            r#type: Some(response_event::Type::Finished(StreamFinished {
                reason: Some(stream_finished::Reason::Done(stream_finished::Done {})),
                ..Default::default()
            })),
        });
    };

    Ok(Box::pin(output.take_until(cancellation_rx)))
}

impl OpenAiCompatibleConfig {
    fn from_custom_endpoint(params: &RequestParams) -> Option<Self> {
        let selected_model = params.model.to_string();
        params
            .custom_model_providers
            .as_ref()?
            .providers
            .iter()
            .filter(|provider| {
                provider.schema
                    == warp_multi_agent_api::request::settings::custom_model_providers::CustomEndpointSchema::OpenaiChatCompletions
                        as i32
            })
            .find_map(|provider| {
                let model = provider
                    .models
                    .iter()
                    .find(|model| model.config_key == selected_model)?;
                Some(Self {
                    endpoint: chat_completions_url(&provider.base_url).ok()?,
                    model: model.slug.clone(),
                    api_key: (!provider.api_key.trim().is_empty())
                        .then(|| provider.api_key.clone()),
                })
            })
    }
}

fn ownterm_tools() -> Vec<ToolDefinition> {
    ownterm_provider::local_agent_tool_specs()
        .into_iter()
        .map(|spec| ToolDefinition {
            r#type: "function",
            function: FunctionDefinition {
                name: spec.name,
                description: spec.description,
                parameters: spec.parameters,
            },
        })
        .collect()
}

fn resolve_system_prompt() -> Option<String> {
    if let Ok(env_prompt) = std::env::var("OWNTERM_SYSTEM_PROMPT") {
        if !env_prompt.trim().is_empty() {
            return Some(env_prompt.trim().to_owned());
        }
    }

    let local_rules = std::path::Path::new(".ownterm/rules.md");
    if local_rules.is_file() {
        if let Ok(content) = std::fs::read_to_string(local_rules) {
            if !content.trim().is_empty() {
                return Some(content.trim().to_owned());
            }
        }
    }

    if let Some(home) = dirs::home_dir() {
        let home_rules = home.join(".ownterm").join("rules.md");
        if home_rules.is_file() {
            if let Ok(content) = std::fs::read_to_string(home_rules) {
                if !content.trim().is_empty() {
                    return Some(content.trim().to_owned());
                }
            }
        }
    }

    Some(
        "You are OwnTerm, an open-source, local-first terminal assistant. You assist with coding, debugging, and terminal operations. Use the tools on this machine: run_shell_command, read_files, grep, file_glob, and apply_file_diffs. Search with grep or file_glob. Change a file with apply_file_diffs instead of printing a patch."
            .to_owned(),
    )
}

fn chat_completions_url(base_url: &str) -> anyhow::Result<Url> {
    ownterm_provider::chat_completions_url(base_url)
}

fn parse_streaming_chunk(data: &str) -> anyhow::Result<Option<ParsedChunk>> {
    let chunk: ChatCompletionsChunk =
        serde_json::from_str(data).context("invalid OpenAI-compatible streaming response")?;
    if let Some(err) = chunk.error {
        return Err(anyhow!("{err}", err = err.message));
    }
    let choice = match chunk.choices.into_iter().next() {
        Some(choice) => choice,
        None => return Ok(None),
    };
    let text = choice.delta.content.filter(|s| !s.is_empty());
    let tool_calls = choice.delta.tool_calls.filter(|t| !t.is_empty());
    if text.is_none() && tool_calls.is_none() {
        return Ok(None);
    }
    Ok(Some(ParsedChunk { text, tool_calls }))
}

fn map_pending_tool_to_proto(name: &str, arguments_json: &str) -> Option<Tool> {
    let Some(tool) = ownterm_provider::parse_local_agent_tool(name, arguments_json) else {
        return Some(Tool::RunShellCommand(RunShellCommand {
            command: format!("{name} {arguments_json}"),
            ..Default::default()
        }));
    };
    Some(match tool {
        ownterm_provider::LocalAgentTool::Shell { command } => {
            Tool::RunShellCommand(RunShellCommand {
                command,
                ..Default::default()
            })
        }
        ownterm_provider::LocalAgentTool::ReadFiles { files } => Tool::ReadFiles(ReadFiles {
            files: files
                .into_iter()
                .map(|name| ReadFilesFile {
                    name,
                    line_ranges: Vec::new(),
                })
                .collect(),
        }),
        ownterm_provider::LocalAgentTool::Grep { queries, path } => {
            Tool::Grep(Grep { queries, path })
        }
        ownterm_provider::LocalAgentTool::Glob {
            patterns,
            search_dir,
        } => Tool::FileGlobV2(FileGlobV2 {
            patterns,
            search_dir,
            ..Default::default()
        }),
        ownterm_provider::LocalAgentTool::Edit {
            summary,
            replacements,
            new_files,
            deleted_files,
        } => Tool::ApplyFileDiffs(ApplyFileDiffs {
            summary,
            diffs: replacements
                .into_iter()
                .map(|edit| FileDiff {
                    file_path: edit.file_path,
                    search: edit.search,
                    replace: edit.replace,
                })
                .collect(),
            new_files: new_files
                .into_iter()
                .map(|file| NewFile {
                    file_path: file.file_path,
                    content: file.content,
                    allow_overwrite: false,
                })
                .collect(),
            deleted_files: deleted_files
                .into_iter()
                .map(|file_path| DeleteFile { file_path })
                .collect(),
            ..Default::default()
        }),
    })
}

fn build_chat_messages(tasks: &[warp_multi_agent_api::Task], prompt: &str) -> Vec<ChatMessage> {
    let mut messages = Vec::new();
    if let Some(system_prompt) = resolve_system_prompt() {
        messages.push(ChatMessage {
            role: "system".to_owned(),
            content: Some(system_prompt),
            tool_calls: None,
            tool_call_id: None,
        });
    }

    for task in tasks {
        for message in &task.messages {
            match &message.message {
                Some(MessageKind::UserQuery(uq)) if !uq.query.trim().is_empty() => {
                    messages.push(ChatMessage {
                        role: "user".to_owned(),
                        content: Some(uq.query.clone()),
                        tool_calls: None,
                        tool_call_id: None,
                    });
                }
                Some(MessageKind::AgentOutput(ao)) if !ao.text.trim().is_empty() => {
                    messages.push(ChatMessage {
                        role: "assistant".to_owned(),
                        content: Some(ao.text.clone()),
                        tool_calls: None,
                        tool_call_id: None,
                    });
                }
                Some(MessageKind::ToolCall(tc)) => {
                    let (name, arguments) = match &tc.tool {
                        Some(Tool::RunShellCommand(cmd)) => (
                            "run_shell_command",
                            serde_json::to_string(&serde_json::json!({ "command": cmd.command }))
                                .unwrap_or_default(),
                        ),
                        Some(Tool::ReadFiles(rf)) => {
                            let files: Vec<&str> =
                                rf.files.iter().map(|f| f.name.as_str()).collect();
                            (
                                "read_files",
                                serde_json::to_string(&serde_json::json!({ "files": files }))
                                    .unwrap_or_default(),
                            )
                        }
                        Some(Tool::Grep(grep)) => (
                            "grep",
                            serde_json::to_string(&serde_json::json!({
                                "queries": grep.queries,
                                "path": grep.path,
                            }))
                            .unwrap_or_default(),
                        ),
                        Some(Tool::FileGlobV2(glob)) => (
                            "file_glob",
                            serde_json::to_string(&serde_json::json!({
                                "patterns": glob.patterns,
                                "search_dir": glob.search_dir,
                            }))
                            .unwrap_or_default(),
                        ),
                        Some(Tool::ApplyFileDiffs(diffs)) => (
                            "apply_file_diffs",
                            serde_json::to_string(&serde_json::json!({
                                "summary": diffs.summary,
                                "diffs": diffs.diffs.iter().map(|diff| serde_json::json!({
                                    "file_path": diff.file_path,
                                    "search": diff.search,
                                    "replace": diff.replace,
                                })).collect::<Vec<_>>(),
                                "new_files": diffs.new_files.iter().map(|file| serde_json::json!({
                                    "file_path": file.file_path,
                                    "content": file.content,
                                })).collect::<Vec<_>>(),
                                "deleted_files": diffs.deleted_files.iter().map(|file| file.file_path.clone()).collect::<Vec<_>>(),
                            }))
                            .unwrap_or_default(),
                        ),
                        Some(Tool::CallMcpTool(mcp)) => (mcp.name.as_str(), "{}".to_owned()),
                        _ => ("generic_tool", "{}".to_owned()),
                    };
                    messages.push(ChatMessage {
                        role: "assistant".to_owned(),
                        content: None,
                        tool_calls: Some(vec![OpenAiToolCall {
                            id: tc.tool_call_id.clone(),
                            r#type: "function".to_owned(),
                            function: OpenAiFunctionCall {
                                name: name.to_owned(),
                                arguments,
                            },
                        }]),
                        tool_call_id: None,
                    });
                }
                Some(MessageKind::ToolCallResult(tcr)) => {
                    let content = match &tcr.result {
                        Some(ToolResult::RunShellCommand(res)) =>
                        {
                            #[allow(deprecated)]
                            if res.exit_code == 0 {
                                res.output.clone()
                            } else {
                                format!("(exit code: {})\n{}", res.exit_code, res.output)
                            }
                        }
                        Some(ToolResult::ReadFiles(res)) => match &res.result {
                            Some(
                                warp_multi_agent_api::read_files_result::Result::TextFilesSuccess(
                                    success,
                                ),
                            ) => {
                                let mut out = String::new();
                                for f in &success.files {
                                    out.push_str(&format!("--- {}\n{}\n", f.file_path, f.content));
                                }
                                out
                            }
                            Some(warp_multi_agent_api::read_files_result::Result::Error(err)) => {
                                format!("Error reading files: {}", err.message)
                            }
                            _ => "Files read successfully".to_owned(),
                        },
                        Some(ToolResult::Grep(result)) => format_grep_result(result),
                        Some(ToolResult::FileGlobV2(result)) => format_glob_result(result),
                        Some(ToolResult::ApplyFileDiffs(result)) => format_diff_result(result),
                        _ => "Tool executed successfully".to_owned(),
                    };
                    messages.push(ChatMessage {
                        role: "tool".to_owned(),
                        content: Some(content),
                        tool_calls: None,
                        tool_call_id: Some(tcr.tool_call_id.clone()),
                    });
                }
                _ => {}
            }
        }
    }

    let last_matches_prompt = messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .is_some_and(|m| m.content.as_deref() == Some(prompt));
    if !last_matches_prompt {
        messages.push(ChatMessage {
            role: "user".to_owned(),
            content: Some(prompt.to_owned()),
            tool_calls: None,
            tool_call_id: None,
        });
    }
    messages
}

fn format_grep_result(result: &warp_multi_agent_api::GrepResult) -> String {
    match &result.result {
        Some(warp_multi_agent_api::grep_result::Result::Success(success)) => {
            if success.matched_files.is_empty() {
                return "No matches.".to_owned();
            }
            let mut out = String::new();
            for file in &success.matched_files {
                if file.matched_lines.is_empty() {
                    out.push_str(&file.file_path);
                    out.push('\n');
                    continue;
                }
                for line in &file.matched_lines {
                    out.push_str(&format!("{}:{}\n", file.file_path, line.line_number));
                }
            }
            out
        }
        Some(warp_multi_agent_api::grep_result::Result::Error(error)) => {
            format!("Grep error: {}", error.message)
        }
        None => "Grep finished.".to_owned(),
    }
}

fn format_glob_result(result: &warp_multi_agent_api::FileGlobV2Result) -> String {
    match &result.result {
        Some(warp_multi_agent_api::file_glob_v2_result::Result::Success(success)) => {
            if success.matched_files.is_empty() {
                "No files.".to_owned()
            } else {
                success
                    .matched_files
                    .iter()
                    .map(|file| file.file_path.as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
        Some(warp_multi_agent_api::file_glob_v2_result::Result::Error(error)) => {
            format!("Glob error: {}", error.message)
        }
        None => "Glob finished.".to_owned(),
    }
}

fn format_diff_result(result: &warp_multi_agent_api::ApplyFileDiffsResult) -> String {
    match &result.result {
        Some(warp_multi_agent_api::apply_file_diffs_result::Result::Success(success)) => {
            let updated = success.updated_files_v2.len();
            let deleted = success.deleted_files.len();
            format!("Applied diffs. Updated {updated} file(s), deleted {deleted}.")
        }
        Some(warp_multi_agent_api::apply_file_diffs_result::Result::Error(error)) => {
            format!("Diff failed: {}", error.message)
        }
        None => "Diff finished.".to_owned(),
    }
}

fn sanitize_error_message(mut message: String, api_key: Option<&str>) -> String {
    if let Some(key) = api_key {
        let trimmed = key.trim();
        if !trimmed.is_empty() {
            message = message.replace(trimmed, "[REDACTED]");
        }
    }
    message
}

fn add_text_action(
    task_id: &str,
    request_id: &str,
    message_id: &str,
    text: String,
) -> ClientAction {
    ClientAction {
        action: Some(client_action::Action::AddMessagesToTask(
            AddMessagesToTask {
                task_id: task_id.to_owned(),
                messages: vec![text_message(task_id, request_id, message_id, text)],
            },
        )),
    }
}

fn append_text_action(
    task_id: &str,
    request_id: &str,
    message_id: &str,
    text: String,
) -> ClientAction {
    ClientAction {
        action: Some(client_action::Action::AppendToMessageContent(
            AppendToMessageContent {
                task_id: task_id.to_owned(),
                message: Some(text_message(task_id, request_id, message_id, text)),
                mask: Some(FieldMask {
                    paths: vec![AGENT_OUTPUT_TEXT_PATH.to_owned()],
                }),
            },
        )),
    }
}

fn text_message(task_id: &str, request_id: &str, message_id: &str, text: String) -> Message {
    Message {
        id: message_id.to_owned(),
        task_id: task_id.to_owned(),
        request_id: request_id.to_owned(),
        timestamp: Some(prost_types::Timestamp::from(SystemTime::now())),
        message: Some(MessageKind::AgentOutput(AgentOutput { text })),
        ..Default::default()
    }
}

fn tool_call_message(task_id: &str, request_id: &str, tool_call_id: String, tool: Tool) -> Message {
    Message {
        id: Uuid::new_v4().to_string(),
        task_id: task_id.to_owned(),
        request_id: request_id.to_owned(),
        timestamp: Some(prost_types::Timestamp::from(SystemTime::now())),
        message: Some(MessageKind::ToolCall(ToolCall {
            tool_call_id,
            tool: Some(tool),
        })),
        ..Default::default()
    }
}

fn client_action_event(action: ClientAction) -> ResponseEvent {
    ResponseEvent {
        r#type: Some(response_event::Type::ClientActions(
            response_event::ClientActions {
                actions: vec![action],
            },
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_chat_completions_url_from_api_root() {
        assert_eq!(
            chat_completions_url("https://example.com/v1")
                .unwrap()
                .as_str(),
            "https://example.com/v1/chat/completions"
        );
    }

    #[test]
    fn preserves_complete_chat_completions_url() {
        assert_eq!(
            chat_completions_url("https://example.com/v1/chat/completions")
                .unwrap()
                .as_str(),
            "https://example.com/v1/chat/completions"
        );
    }

    #[test]
    fn rejects_plain_http_for_remote_hosts() {
        assert!(chat_completions_url("http://example.com/v1").is_err());
        assert!(chat_completions_url("http://127.0.0.1:11434/v1").is_ok());
    }

    #[test]
    fn parses_streaming_text_delta() {
        let chunk = parse_streaming_chunk(r#"{"choices":[{"delta":{"content":"hola"}}]}"#).unwrap();
        assert_eq!(chunk.unwrap().text, Some("hola".to_owned()));
    }

    #[test]
    fn accepts_role_only_streaming_chunk() {
        let chunk =
            parse_streaming_chunk(r#"{"choices":[{"delta":{"role":"assistant"}}]}"#).unwrap();
        assert!(chunk.is_none());
    }

    #[test]
    fn parses_streaming_tool_call_delta() {
        let chunk = parse_streaming_chunk(
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"run_shell_command","arguments":"{\"command\":\"ls\"}"}}]}}]}"#,
        )
        .unwrap()
        .unwrap();

        let tool_calls = chunk.tool_calls.expect("expected tool calls");
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].id.as_deref(), Some("call_1"));
        let func = tool_calls[0].function.as_ref().unwrap();
        assert_eq!(func.name.as_deref(), Some("run_shell_command"));
        assert_eq!(func.arguments.as_deref(), Some("{\"command\":\"ls\"}"));
    }

    #[test]
    fn parses_streaming_error_payload() {
        let err = parse_streaming_chunk(
            r#"{"error":{"message":"Incorrect API key provided","type":"invalid_request_error"}}"#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("Incorrect API key provided"));
    }

    #[test]
    fn redacts_api_key_from_error_message() {
        let secret = "sk-proj-supersecret123456789";
        let raw_error = format!("Authentication error with key {secret}: invalid credentials");
        let sanitized = sanitize_error_message(raw_error, Some(secret));
        assert!(!sanitized.contains(secret));
        assert!(sanitized.contains("[REDACTED]"));
    }

    #[test]
    fn maps_pending_tool_to_shell_command() {
        let tool = map_pending_tool_to_proto("run_shell_command", r#"{"command":"cargo check"}"#);
        assert!(matches!(
            tool,
            Some(Tool::RunShellCommand(cmd)) if cmd.command == "cargo check"
        ));
    }

    #[test]
    fn maps_grep_glob_and_search_replace() {
        match map_pending_tool_to_proto("grep", r#"{"queries":["fn main"],"path":"src"}"#) {
            Some(Tool::Grep(grep)) => {
                assert_eq!(grep.queries, vec!["fn main".to_owned()]);
                assert_eq!(grep.path, "src");
            }
            other => panic!("expected grep, got {other:?}"),
        }
        match map_pending_tool_to_proto("file_glob", r#"{"patterns":["**/*.rs"]}"#) {
            Some(Tool::FileGlobV2(glob)) => {
                assert_eq!(glob.patterns, vec!["**/*.rs".to_owned()]);
            }
            other => panic!("expected glob, got {other:?}"),
        }
        match map_pending_tool_to_proto(
            "apply_file_diffs",
            r#"{"diffs":[{"file_path":"a.rs","search":"old","replace":"new"}]}"#,
        ) {
            Some(Tool::ApplyFileDiffs(diffs)) => {
                assert_eq!(diffs.diffs.len(), 1);
                assert_eq!(diffs.diffs[0].file_path, "a.rs");
                assert_eq!(diffs.diffs[0].search, "old");
                assert_eq!(diffs.diffs[0].replace, "new");
            }
            other => panic!("expected diffs, got {other:?}"),
        }
    }

    #[test]
    fn maps_pending_tool_to_read_files() {
        let tool =
            map_pending_tool_to_proto("read_files", r#"{"files":["src/main.rs","Cargo.toml"]}"#);
        match tool {
            Some(Tool::ReadFiles(rf)) => {
                assert_eq!(rf.files.len(), 2);
                assert_eq!(rf.files[0].name, "src/main.rs");
                assert_eq!(rf.files[1].name, "Cargo.toml");
            }
            _ => panic!("expected Tool::ReadFiles"),
        }
    }

    #[test]
    fn builds_chat_messages_from_conversation_history() {
        let tasks = vec![warp_multi_agent_api::Task {
            id: "task-1".to_owned(),
            messages: vec![
                Message {
                    id: "msg-1".to_owned(),
                    message: Some(MessageKind::UserQuery(
                        warp_multi_agent_api::message::UserQuery {
                            query: "What is OwnTerm?".to_owned(),
                            ..Default::default()
                        },
                    )),
                    ..Default::default()
                },
                Message {
                    id: "msg-2".to_owned(),
                    message: Some(MessageKind::AgentOutput(AgentOutput {
                        text: "OwnTerm is a local-first terminal.".to_owned(),
                    })),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }];

        let messages = build_chat_messages(&tasks, "Tell me more");
        // May contain system prompt at index 0
        let user_msgs: Vec<_> = messages.iter().filter(|m| m.role == "user").collect();
        let assistant_msgs: Vec<_> = messages.iter().filter(|m| m.role == "assistant").collect();

        assert_eq!(user_msgs.len(), 2);
        assert_eq!(user_msgs[0].content.as_deref(), Some("What is OwnTerm?"));
        assert_eq!(user_msgs[1].content.as_deref(), Some("Tell me more"));
        assert_eq!(assistant_msgs.len(), 1);
        assert_eq!(
            assistant_msgs[0].content.as_deref(),
            Some("OwnTerm is a local-first terminal.")
        );
    }

    #[test]
    #[allow(deprecated)]
    fn builds_chat_messages_with_tool_call_and_result() {
        let tasks = vec![warp_multi_agent_api::Task {
            id: "task-1".to_owned(),
            messages: vec![
                Message {
                    id: "msg-1".to_owned(),
                    message: Some(MessageKind::UserQuery(
                        warp_multi_agent_api::message::UserQuery {
                            query: "Run cargo check".to_owned(),
                            ..Default::default()
                        },
                    )),
                    ..Default::default()
                },
                Message {
                    id: "msg-2".to_owned(),
                    message: Some(MessageKind::ToolCall(ToolCall {
                        tool_call_id: "call_abc".to_owned(),
                        tool: Some(Tool::RunShellCommand(RunShellCommand {
                            command: "cargo check".to_owned(),
                            ..Default::default()
                        })),
                    })),
                    ..Default::default()
                },
                Message {
                    id: "msg-3".to_owned(),
                    message: Some(MessageKind::ToolCallResult(
                        warp_multi_agent_api::message::ToolCallResult {
                            tool_call_id: "call_abc".to_owned(),
                            context: None,
                            result: Some(ToolResult::RunShellCommand(
                                warp_multi_agent_api::RunShellCommandResult {
                                    command: "cargo check".to_owned(),
                                    output: "Finished dev target(s)".to_owned(),
                                    exit_code: 0,
                                    result: None,
                                },
                            )),
                        },
                    )),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }];

        let messages = build_chat_messages(&tasks, "What is next?");
        let tool_call_msg = messages
            .iter()
            .find(|m| m.tool_calls.is_some())
            .expect("should find tool call message");
        assert_eq!(tool_call_msg.role, "assistant");
        let calls = tool_call_msg.tool_calls.as_ref().unwrap();
        assert_eq!(calls[0].id, "call_abc");
        assert_eq!(calls[0].function.name, "run_shell_command");

        let tool_result_msg = messages
            .iter()
            .find(|m| m.role == "tool")
            .expect("should find tool result message");
        assert_eq!(tool_result_msg.tool_call_id.as_deref(), Some("call_abc"));
        assert_eq!(
            tool_result_msg.content.as_deref(),
            Some("Finished dev target(s)")
        );
    }

    #[tokio::test]
    async fn stream_cancellation_terminates_stream() {
        let (tx, rx) = futures::channel::oneshot::channel();
        let infinite_stream = async_stream::stream! {
            yield 1;
            yield 2;
            yield 3;
            yield 4;
        };

        let mut stream = Box::pin(infinite_stream.take_until(rx));
        assert_eq!(stream.next().await, Some(1));
        let _ = tx.send(());
        assert_eq!(stream.next().await, None);
    }
}
