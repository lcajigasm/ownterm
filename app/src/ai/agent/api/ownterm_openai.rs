use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::{Context, anyhow};
use futures_util::StreamExt;
use prost_types::FieldMask;
use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;
use warp_multi_agent_api::client_action::{AddMessagesToTask, AppendToMessageContent, CreateTask};
use warp_multi_agent_api::message::tool_call::apply_file_diffs::{DeleteFile, FileDiff, NewFile};
use warp_multi_agent_api::message::tool_call::read_files::File as ReadFilesFile;
use warp_multi_agent_api::message::tool_call::{
    ApplyFileDiffs, FileGlobV2, Grep, ReadFiles, RunShellCommand, Tool,
};
use warp_multi_agent_api::message::tool_call_result::Result as ToolResult;
use warp_multi_agent_api::message::{
    AgentOutput, Message as MessageKind, ToolCall, ToolCallResult, UserQuery,
};
use warp_multi_agent_api::request::input::tool_call_result::Result as RequestToolResult;
use warp_multi_agent_api::request::input::user_inputs::user_input::Input as UserInput;
use warp_multi_agent_api::request::input::{
    Type as RequestInputType, UserQuery as RequestUserQuery,
};
use warp_multi_agent_api::response_event::{StreamFinished, StreamInit, stream_finished};
use warp_multi_agent_api::{
    ClientAction, InputContext, Message, ResponseEvent, RunShellCommandResult, Task, client_action,
    read_files_result, request, response_event, run_shell_command_result,
};
use warpui::r#async::Timer;

use super::convert_to::convert_input;
use super::{ConvertToAPITypeError, RequestParams, ResponseStream};
use crate::ai::agent::AIAgentInput;
use crate::server::server_api::AIApiError;

const AGENT_OUTPUT_TEXT_PATH: &str = "agent_output.text";

/// How long the endpoint may stay silent before the request is abandoned. Local servers can take
/// minutes to load a model before the first token.
const STREAM_IDLE_TIMEOUT: Duration = Duration::from_secs(300);

/// Largest tool result, in characters, replayed to the model in one message.
const MAX_TOOL_OUTPUT_CHARS: usize = 30_000;

/// Largest attached block, selection, file, or rule file, in characters.
const MAX_ATTACHMENT_CHARS: usize = 12_000;

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
    finish_reason: Option<String>,
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

/// The task messages one request adds before the model answers, and the chat history sent to the
/// model.
#[derive(Debug)]
struct TurnPlan {
    task_id: String,
    opening_actions: Vec<ClientAction>,
    messages: Vec<ChatMessage>,
    /// The latest user query, which model routing classifies.
    routing_prompt: String,
}

/// Plans a request the way the hosted agent service would. No server echoes the request back, so
/// this creates the root task on the first turn and records the incoming query or tool results as
/// task messages; later turns read the conversation from those messages.
fn plan_turn(
    tasks: &[Task],
    input: Vec<AIAgentInput>,
    request_id: &str,
) -> Result<TurnPlan, ConvertToAPITypeError> {
    let root_task = tasks
        .iter()
        .find(|task| {
            task.dependencies
                .as_ref()
                .is_none_or(|dependencies| dependencies.parent_task_id.is_empty())
        })
        .or(tasks.first());
    let task_id = root_task
        .map(|task| task.id.clone())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let new_messages = input_messages(convert_input(input)?, &task_id, request_id)?;

    let mut opening_actions = Vec::new();
    if root_task.is_none() {
        opening_actions.push(ClientAction {
            action: Some(client_action::Action::CreateTask(CreateTask {
                task: Some(Task {
                    id: task_id.clone(),
                    ..Default::default()
                }),
            })),
        });
    }
    if !new_messages.is_empty() {
        opening_actions.push(add_messages_action(&task_id, new_messages.clone()));
    }

    let history: Vec<&Message> = root_task
        .into_iter()
        .flat_map(|task| &task.messages)
        .chain(&new_messages)
        .collect();
    let routing_prompt = history
        .iter()
        .rev()
        .find_map(|message| {
            let MessageKind::UserQuery(query) = message.message.as_ref()? else {
                return None;
            };
            (!query.query.trim().is_empty()).then(|| query.query.clone())
        })
        .ok_or_else(|| anyhow!("OwnTerm direct inference requires a user prompt"))?;

    Ok(TurnPlan {
        task_id,
        opening_actions,
        messages: build_chat_messages(&history),
        routing_prompt,
    })
}

/// Converts a request's input into the task messages the hosted service would echo back.
fn input_messages(
    input: request::Input,
    task_id: &str,
    request_id: &str,
) -> Result<Vec<Message>, ConvertToAPITypeError> {
    // The request carries one context for all of its inputs; the first message records it.
    let mut context = input.context;
    let mut kinds = Vec::new();
    match input.r#type {
        Some(RequestInputType::UserInputs(inputs)) => {
            for input in inputs.inputs {
                match input.input {
                    Some(UserInput::UserQuery(query)) => {
                        kinds.push(user_query_kind(query, context.take()));
                    }
                    Some(UserInput::CliAgentUserQuery(query)) => {
                        if let Some(query) = query.user_query {
                            kinds.push(user_query_kind(query, context.take()));
                        }
                    }
                    Some(UserInput::ToolCallResult(result)) => {
                        kinds.push(MessageKind::ToolCallResult(ToolCallResult {
                            tool_call_id: result.tool_call_id,
                            context: context.take(),
                            result: Some(
                                result
                                    .result
                                    .map(message_tool_result)
                                    .unwrap_or(ToolResult::Cancel(())),
                            ),
                        }));
                    }
                    Some(
                        UserInput::MessagesReceivedFromAgents(_)
                        | UserInput::EventsFromAgents(_)
                        | UserInput::PassiveSuggestionResult(_)
                        | UserInput::OrchestrationConfigUpdate(_)
                        | UserInput::ConversationHandoff(_)
                        | UserInput::AgentWake(_),
                    )
                    | None => {}
                }
            }
        }
        Some(RequestInputType::QueryWithCannedResponse(query)) => {
            kinds.push(plain_query_kind(query.query, context.take()));
        }
        Some(RequestInputType::AutoCodeDiffQuery(query)) => {
            kinds.push(plain_query_kind(query.query, context.take()));
        }
        Some(RequestInputType::CreateNewProject(query)) => {
            kinds.push(plain_query_kind(query.query, context.take()));
        }
        Some(RequestInputType::ResumeConversation(_)) => {}
        // Every other request type asks for a hosted-service feature (passive suggestions,
        // summaries, cloud environments, skills) that has no local counterpart.
        _ => {
            return Err(anyhow!("The local agent does not support this kind of request").into());
        }
    }
    Ok(kinds
        .into_iter()
        .map(|kind| new_message(task_id, request_id, Uuid::new_v4().to_string(), kind))
        .collect())
}

fn user_query_kind(query: RequestUserQuery, context: Option<InputContext>) -> MessageKind {
    MessageKind::UserQuery(UserQuery {
        query: query.query,
        context,
        referenced_attachments: query.referenced_attachments,
        mode: query.mode,
        intended_agent: query.intended_agent,
        origin: query.origin,
        author: query.author,
        source_message: query.source_message,
    })
}

fn plain_query_kind(query: String, context: Option<InputContext>) -> MessageKind {
    MessageKind::UserQuery(UserQuery {
        query,
        context,
        ..Default::default()
    })
}

fn message_tool_result(result: RequestToolResult) -> ToolResult {
    match result {
        RequestToolResult::RunShellCommand(r) => ToolResult::RunShellCommand(r),
        RequestToolResult::ReadFiles(r) => ToolResult::ReadFiles(r),
        RequestToolResult::SearchCodebase(r) => ToolResult::SearchCodebase(r),
        RequestToolResult::ApplyFileDiffs(r) => ToolResult::ApplyFileDiffs(r),
        RequestToolResult::SuggestPlan(r) => ToolResult::SuggestPlan(r),
        RequestToolResult::SuggestCreatePlan(r) => ToolResult::SuggestCreatePlan(r),
        RequestToolResult::Grep(r) => ToolResult::Grep(r),
        #[allow(deprecated)]
        RequestToolResult::FileGlob(r) => ToolResult::FileGlob(r),
        RequestToolResult::ReadMcpResource(r) => ToolResult::ReadMcpResource(r),
        RequestToolResult::CallMcpTool(r) => ToolResult::CallMcpTool(r),
        RequestToolResult::WriteToLongRunningShellCommand(r) => {
            ToolResult::WriteToLongRunningShellCommand(r)
        }
        RequestToolResult::SuggestNewConversation(r) => ToolResult::SuggestNewConversation(r),
        RequestToolResult::FileGlobV2(r) => ToolResult::FileGlobV2(r),
        RequestToolResult::SuggestPrompt(r) => ToolResult::SuggestPrompt(r),
        RequestToolResult::OpenCodeReview(r) => ToolResult::OpenCodeReview(r),
        RequestToolResult::InitProject(r) => ToolResult::InitProject(r),
        RequestToolResult::ReadDocuments(r) => ToolResult::ReadDocuments(r),
        RequestToolResult::EditDocuments(r) => ToolResult::EditDocuments(r),
        RequestToolResult::CreateDocuments(r) => ToolResult::CreateDocuments(r),
        RequestToolResult::ReadShellCommandOutput(r) => ToolResult::ReadShellCommandOutput(r),
        RequestToolResult::UseComputer(r) => ToolResult::UseComputer(r),
        RequestToolResult::InsertReviewComments(r) => ToolResult::InsertReviewComments(r),
        RequestToolResult::RequestComputerUse(r) => ToolResult::RequestComputerUseResult(r),
        RequestToolResult::ReadSkill(r) => ToolResult::ReadSkill(r),
        RequestToolResult::FetchConversation(r) => ToolResult::FetchConversation(r),
        RequestToolResult::SendMessageToAgent(r) => ToolResult::SendMessageToAgent(r),
        RequestToolResult::TransferShellCommandControlToUser(r) => {
            ToolResult::TransferShellCommandControlToUser(r)
        }
        RequestToolResult::AskUserQuestion(r) => ToolResult::AskUserQuestion(r),
        RequestToolResult::UploadFileArtifact(r) => ToolResult::UploadFileArtifact(r),
        RequestToolResult::RunAgentsResult(r) => ToolResult::RunAgentsResult(r),
        RequestToolResult::WaitForEvents(r) => ToolResult::WaitForEvents(r),
        RequestToolResult::StartRecording(r) => ToolResult::StartRecording(r),
        RequestToolResult::StopRecording(r) => ToolResult::StopRecording(r),
    }
}

pub(super) fn generate(
    mut params: RequestParams,
    cancellation_rx: futures::channel::oneshot::Receiver<()>,
) -> Result<ResponseStream, ConvertToAPITypeError> {
    let request_id = Uuid::new_v4().to_string();
    let conversation_id = params
        .conversation_token
        .as_ref()
        .map(|token| token.as_str().to_owned())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let input = std::mem::take(&mut params.input);
    let plan = plan_turn(&params.tasks, input, &request_id)?;
    let attempts = live_attempts(&params, &plan.routing_prompt)?;
    if attempts.is_empty() {
        return Err(anyhow!("No model is available for this request").into());
    }
    Ok(stream_turn(
        plan,
        attempts,
        request_id,
        conversation_id,
        cancellation_rx,
    ))
}

/// Waits for the next stream item, or `None` when the endpoint stays silent for `idle`.
async fn next_or_idle<S>(stream: &mut S, idle: Duration) -> Option<Option<S::Item>>
where
    S: futures::Stream + Unpin,
{
    match futures::future::select(stream.next(), Box::pin(Timer::after(idle))).await {
        futures::future::Either::Left((item, _)) => Some(item),
        futures::future::Either::Right(_) => None,
    }
}

/// Merges one streamed tool-call fragment into the calls collected so far.
fn merge_tool_call_delta(pending: &mut Vec<PendingToolCall>, delta: ChatDeltaToolCall) {
    let id = delta.id.filter(|id| !id.is_empty());
    let index = delta.index.unwrap_or_else(|| {
        // Some servers omit the index; a fragment that names a different call id starts a new
        // call, and anything else continues the latest one.
        let starts_new_call = match (&id, pending.last()) {
            (Some(id), Some(last)) => !last.id.is_empty() && last.id != *id,
            (Some(_) | None, None) | (None, Some(_)) => false,
        };
        if starts_new_call {
            pending.len()
        } else {
            pending.len().saturating_sub(1)
        }
    });
    if index >= pending.len() {
        pending.resize_with(index + 1, PendingToolCall::default);
    }
    let call = &mut pending[index];
    // Servers that repeat the id or name on every fragment must not have them concatenated.
    if let Some(id) = id
        && call.id.is_empty()
    {
        call.id = id;
    }
    if let Some(function) = delta.function {
        if let Some(name) = function.name.filter(|name| !name.is_empty())
            && call.name.is_empty()
        {
            call.name = name;
        }
        if let Some(arguments) = function.arguments {
            call.arguments.push_str(&arguments);
        }
    }
}

fn stream_turn(
    plan: TurnPlan,
    attempts: Vec<LiveAttempt>,
    request_id: String,
    conversation_id: String,
    cancellation_rx: futures::channel::oneshot::Receiver<()>,
) -> ResponseStream {
    let TurnPlan {
        task_id,
        opening_actions,
        messages,
        routing_prompt: _,
    } = plan;
    let message_id = Uuid::new_v4().to_string();
    let tools = ownterm_tools();

    let output = async_stream::stream! {
        yield Ok(ResponseEvent {
            r#type: Some(response_event::Type::Init(StreamInit {
                request_id: request_id.clone(),
                conversation_id,
                run_id: String::new(),
            })),
        });
        if !opening_actions.is_empty() {
            yield Ok(ResponseEvent {
                r#type: Some(response_event::Type::ClientActions(
                    response_event::ClientActions {
                        actions: opening_actions,
                    },
                )),
            });
        }

        let client = http_client::Client::new_external();
        let mut started_message = false;
        let mut pending_tool_calls: Vec<PendingToolCall> = Vec::new();
        let mut finish_reason: Option<String>;
        let mut attempt_at = 0usize;
        let mut send_tools = true;

        'attempts: loop {
            let spec = &attempts[attempt_at];
            pending_tool_calls.clear();
            finish_reason = None;
            let request = ChatCompletionsRequest {
                model: spec.model.clone(),
                messages: messages.clone(),
                tools: if send_tools { tools.clone() } else { Vec::new() },
                tool_choice: None,
                stream: true,
            };
            let mut builder = client.post(spec.endpoint.clone()).json(&request);
            if let Some(ref api_key) = spec.api_key {
                builder = builder.bearer_auth(api_key);
            }
            let mut events = builder.eventsource();
            let mut committed = false;

            loop {
                let Some(event) = next_or_idle(&mut events, STREAM_IDLE_TIMEOUT).await else {
                    if !committed && attempt_at + 1 < attempts.len() {
                        attempt_at += 1;
                        send_tools = true;
                        continue 'attempts;
                    }
                    yield Err(Arc::new(AIApiError::Other(anyhow!(
                        "The model endpoint sent nothing for {} seconds",
                        STREAM_IDLE_TIMEOUT.as_secs()
                    ))));
                    return;
                };
                let Some(event) = event else {
                    break;
                };
                match event {
                    Ok(reqwest_eventsource::Event::Open) => {}
                    Ok(reqwest_eventsource::Event::Message(message)) => {
                        if message.data.trim() == "[DONE]" {
                            break;
                        }
                        match parse_streaming_chunk(&message.data) {
                            Ok(Some(parsed)) => {
                                if let Some(reason) = parsed.finish_reason {
                                    finish_reason = Some(reason);
                                }
                                if parsed.text.is_none() && parsed.tool_calls.is_none() {
                                    continue;
                                }
                                if !committed && spec.announce {
                                    ownterm_provider::record_last_choice(&spec.reason);
                                    yield Ok(client_action_event(text_action(
                                        &mut started_message,
                                        &task_id,
                                        &request_id,
                                        &message_id,
                                        format!("{}\n", spec.reason),
                                    )));
                                }
                                committed = true;
                                if let Some(text) = parsed.text {
                                    yield Ok(client_action_event(text_action(
                                        &mut started_message,
                                        &task_id,
                                        &request_id,
                                        &message_id,
                                        text,
                                    )));
                                }
                                for delta in parsed.tool_calls.into_iter().flatten() {
                                    merge_tool_call_delta(&mut pending_tool_calls, delta);
                                }
                            }
                            Ok(None) => {}
                            Err(error) => {
                                if !committed && attempt_at + 1 < attempts.len() {
                                    attempt_at += 1;
                                    send_tools = true;
                                    continue 'attempts;
                                }
                                yield Err(Arc::new(AIApiError::Other(anyhow!(
                                    sanitize_error_message(
                                        format!("{error:#}"),
                                        spec.api_key.as_deref(),
                                    )
                                ))));
                                return;
                            }
                        }
                    }
                    Err(error) => {
                        // A model without tool support rejects any request that lists tools;
                        // the same model can still answer in plain text.
                        if !committed
                            && send_tools
                            && matches!(
                                &error,
                                reqwest_eventsource::Error::InvalidStatusCode(status, _)
                                    if status.as_u16() == 400
                            )
                        {
                            send_tools = false;
                            continue 'attempts;
                        }
                        if !committed
                            && attempt_at + 1 < attempts.len()
                            && ownterm_provider::should_fallback(failure_kind(&error))
                        {
                            attempt_at += 1;
                            send_tools = true;
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

        let (tool_messages, notices) = finished_tool_calls(
            pending_tool_calls,
            finish_reason.as_deref(),
            &task_id,
            &request_id,
        );
        if !notices.is_empty() {
            let separator = if started_message { "\n\n" } else { "" };
            yield Ok(client_action_event(text_action(
                &mut started_message,
                &task_id,
                &request_id,
                &message_id,
                format!("{separator}{}", notices.join("\n")),
            )));
        }
        if !tool_messages.is_empty() {
            yield Ok(client_action_event(add_messages_action(&task_id, tool_messages)));
        }

        yield Ok(ResponseEvent {
            r#type: Some(response_event::Type::Finished(StreamFinished {
                reason: Some(stream_finished::Reason::Done(stream_finished::Done {})),
                ..Default::default()
            })),
        });
    };

    Box::pin(output.take_until(cancellation_rx))
}

/// Turns the tool calls a finished stream collected into task messages. A call that cannot be
/// run safely is dropped and explained in a notice instead.
fn finished_tool_calls(
    pending: Vec<PendingToolCall>,
    finish_reason: Option<&str>,
    task_id: &str,
    request_id: &str,
) -> (Vec<Message>, Vec<String>) {
    let mut messages = Vec::new();
    let mut notices = Vec::new();
    let pending: Vec<_> = pending
        .into_iter()
        .filter(|call| !call.name.is_empty() || !call.arguments.is_empty())
        .collect();
    if finish_reason == Some("length") && !pending.is_empty() {
        // The arguments of a call cut off by the output limit are incomplete.
        notices.push(
            "The model's reply hit its output limit before a tool call was complete, so the call was not run."
                .to_owned(),
        );
        return (messages, notices);
    }
    for call in pending {
        match local_tool_call(&call.name, &call.arguments) {
            Ok(tool) => {
                let tool_call_id = if call.id.is_empty() {
                    Uuid::new_v4().to_string()
                } else {
                    call.id
                };
                messages.push(new_message(
                    task_id,
                    request_id,
                    Uuid::new_v4().to_string(),
                    MessageKind::ToolCall(ToolCall {
                        tool_call_id,
                        tool: Some(tool),
                    }),
                ));
            }
            Err(notice) => notices.push(notice),
        }
    }
    (messages, notices)
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

const DEFAULT_SYSTEM_PROMPT: &str = "You are OwnTerm, an open-source, local-first terminal assistant. You assist with coding, debugging, and terminal operations. Use the tools on this machine: run_shell_command, read_files, grep, file_glob, and apply_file_diffs. Search with grep or file_glob. Change a file with apply_file_diffs instead of printing a patch.";

fn read_rules(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let content = content.trim();
    (!content.is_empty()).then(|| content.to_owned())
}

/// Resolves the base instructions: `OWNTERM_SYSTEM_PROMPT`, then `.ownterm/rules.md` in the
/// session's working directory, then `~/.ownterm/rules.md`, then the built-in prompt.
fn resolve_base_prompt(working_directory: Option<&str>) -> String {
    if let Ok(env_prompt) = std::env::var("OWNTERM_SYSTEM_PROMPT")
        && !env_prompt.trim().is_empty()
    {
        return env_prompt.trim().to_owned();
    }
    let project_rules = working_directory
        .filter(|directory| !directory.is_empty())
        .map(|directory| Path::new(directory).join(".ownterm/rules.md"))
        .unwrap_or_else(|| PathBuf::from(".ownterm/rules.md"));
    read_rules(&project_rules)
        .or_else(|| read_rules(&dirs::home_dir()?.join(".ownterm").join("rules.md")))
        .unwrap_or_else(|| DEFAULT_SYSTEM_PROMPT.to_owned())
}

/// Builds the system message from the base instructions plus the session facts and project rule
/// files in the conversation's latest context.
fn system_prompt(context: Option<&InputContext>) -> String {
    let working_directory = context
        .and_then(|context| context.directory.as_ref())
        .map(|directory| directory.pwd.as_str());
    let mut prompt = resolve_base_prompt(working_directory);
    let Some(context) = context else {
        return prompt;
    };

    let mut facts = Vec::new();
    if let Some(directory) = working_directory.filter(|directory| !directory.is_empty()) {
        facts.push(format!("Working directory: {directory}"));
    }
    if let Some(os) = &context.operating_system {
        let os = format!("{} {}", os.platform, os.distribution);
        if !os.trim().is_empty() {
            facts.push(format!("Operating system: {}", os.trim()));
        }
    }
    if let Some(shell) = &context.shell {
        let shell = format!("{} {}", shell.name, shell.version);
        if !shell.trim().is_empty() {
            facts.push(format!("Shell: {}", shell.trim()));
        }
    }
    if !facts.is_empty() {
        prompt.push_str("\n\n");
        prompt.push_str(&facts.join("\n"));
    }
    for rule_file in context
        .project_rules
        .iter()
        .flat_map(|rules| &rules.active_rule_files)
    {
        if rule_file.content.trim().is_empty() {
            continue;
        }
        prompt.push_str(&format!(
            "\n\nProject rules from {}:\n{}",
            rule_file.file_path,
            truncate_middle(rule_file.content.trim(), MAX_ATTACHMENT_CHARS)
        ));
    }
    prompt
}

/// Keeps the start and end of `text` when it is longer than `max_chars`.
fn truncate_middle(text: &str, max_chars: usize) -> String {
    let total = text.chars().count();
    if total <= max_chars {
        return text.to_owned();
    }
    let head_len = max_chars * 2 / 3;
    let tail_len = max_chars - head_len;
    let head: String = text.chars().take(head_len).collect();
    let tail: String = text.chars().skip(total - tail_len).collect();
    format!(
        "{head}\n[... {} characters omitted ...]\n{tail}",
        total - max_chars
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
    let finish_reason = choice.finish_reason.filter(|reason| !reason.is_empty());
    if text.is_none() && tool_calls.is_none() && finish_reason.is_none() {
        return Ok(None);
    }
    Ok(Some(ParsedChunk {
        text,
        tool_calls,
        finish_reason,
    }))
}

/// Maps a model tool call to a client tool. The error is a notice for the user: a call with an
/// unknown name, malformed arguments, or nothing to do is never run.
fn local_tool_call(name: &str, arguments_json: &str) -> Result<Tool, String> {
    let arguments = arguments_json.trim();
    if (arguments.starts_with('{') || arguments.starts_with('['))
        && serde_json::from_str::<serde_json::Value>(arguments).is_err()
    {
        return Err(format!(
            "The model sent malformed arguments for `{name}`, so the call was not run."
        ));
    }
    let Some(tool) = ownterm_provider::parse_local_agent_tool(name, arguments_json) else {
        return Err(format!(
            "The model called `{name}`, which is not a tool OwnTerm provides, so the call was not run."
        ));
    };
    let empty =
        || format!("The model called `{name}` without usable arguments, so the call was not run.");
    Ok(match tool {
        ownterm_provider::LocalAgentTool::Shell { command } => {
            if command.trim().is_empty() {
                return Err(empty());
            }
            Tool::RunShellCommand(RunShellCommand {
                command,
                ..Default::default()
            })
        }
        ownterm_provider::LocalAgentTool::ReadFiles { files } => {
            if files.is_empty() {
                return Err(empty());
            }
            Tool::ReadFiles(ReadFiles {
                files: files
                    .into_iter()
                    .map(|name| ReadFilesFile {
                        name,
                        line_ranges: Vec::new(),
                    })
                    .collect(),
            })
        }
        ownterm_provider::LocalAgentTool::Grep { queries, path } => {
            if queries.is_empty() {
                return Err(empty());
            }
            Tool::Grep(Grep { queries, path })
        }
        ownterm_provider::LocalAgentTool::Glob {
            patterns,
            search_dir,
        } => {
            if patterns.is_empty() {
                return Err(empty());
            }
            Tool::FileGlobV2(FileGlobV2 {
                patterns,
                search_dir,
                ..Default::default()
            })
        }
        ownterm_provider::LocalAgentTool::Edit {
            summary,
            replacements,
            new_files,
            deleted_files,
        } => {
            if replacements.is_empty() && new_files.is_empty() && deleted_files.is_empty() {
                return Err(empty());
            }
            Tool::ApplyFileDiffs(ApplyFileDiffs {
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
            })
        }
    })
}

impl ChatMessage {
    fn text(role: &str, content: String) -> Self {
        Self {
            role: role.to_owned(),
            content: Some(content),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    fn tool_result(tool_call_id: String, content: String) -> Self {
        Self {
            role: "tool".to_owned(),
            content: Some(content),
            tool_calls: None,
            tool_call_id: Some(tool_call_id),
        }
    }
}

fn message_context(message: &Message) -> Option<&InputContext> {
    match message.message.as_ref()? {
        MessageKind::UserQuery(query) => query.context.as_ref(),
        MessageKind::ToolCallResult(result) => result.context.as_ref(),
        _ => None,
    }
}

/// Builds the chat history from task messages, in the shape `/chat/completions` accepts: the tool
/// calls of one reply share an assistant message, and each of them is answered by a tool message.
fn build_chat_messages(history: &[&Message]) -> Vec<ChatMessage> {
    let latest_context = history
        .iter()
        .rev()
        .find_map(|message| message_context(message));
    let mut messages = vec![ChatMessage::text("system", system_prompt(latest_context))];

    for message in history {
        match &message.message {
            Some(MessageKind::UserQuery(query)) if !query.query.trim().is_empty() => {
                messages.push(ChatMessage::text("user", render_user_query(query)));
            }
            Some(MessageKind::AgentOutput(output)) if !output.text.trim().is_empty() => {
                messages.push(ChatMessage::text("assistant", output.text.clone()));
            }
            Some(MessageKind::ToolCall(tool_call)) => {
                let (name, arguments) = tool_call_request(tool_call);
                let call = OpenAiToolCall {
                    id: tool_call.tool_call_id.clone(),
                    r#type: "function".to_owned(),
                    function: OpenAiFunctionCall { name, arguments },
                };
                match messages.last_mut() {
                    Some(last) if last.role == "assistant" => {
                        last.tool_calls.get_or_insert_default().push(call);
                    }
                    Some(_) | None => messages.push(ChatMessage {
                        role: "assistant".to_owned(),
                        content: None,
                        tool_calls: Some(vec![call]),
                        tool_call_id: None,
                    }),
                }
            }
            Some(MessageKind::ToolCallResult(result)) => {
                messages.push(ChatMessage::tool_result(
                    result.tool_call_id.clone(),
                    truncate_middle(&render_tool_result(result), MAX_TOOL_OUTPUT_CHARS),
                ));
            }
            _ => {}
        }
    }
    pair_tool_calls_with_results(messages)
}

/// Reorders tool messages to follow the assistant message that made their calls, answers calls
/// that never got a result, and drops results with no call. Strict servers reject anything else.
fn pair_tool_calls_with_results(messages: Vec<ChatMessage>) -> Vec<ChatMessage> {
    let mut results: HashMap<String, String> = HashMap::new();
    for message in &messages {
        if let (Some(id), Some(content)) = (&message.tool_call_id, &message.content) {
            results.insert(id.clone(), content.clone());
        }
    }
    let mut paired = Vec::with_capacity(messages.len());
    for message in messages {
        if message.tool_call_id.is_some() {
            continue;
        }
        let call_ids: Vec<String> = message
            .tool_calls
            .iter()
            .flatten()
            .map(|call| call.id.clone())
            .collect();
        paired.push(message);
        for id in call_ids {
            let content = results.remove(&id).unwrap_or_else(|| {
                "No result: the call was cancelled before it finished.".to_owned()
            });
            paired.push(ChatMessage::tool_result(id, content));
        }
    }
    paired
}

fn render_user_query(query: &UserQuery) -> String {
    let Some(context) = &query.context else {
        return query.query.clone();
    };
    let mut attached = Vec::new();
    for selection in &context.selected_text {
        if !selection.text.trim().is_empty() {
            attached.push(format!(
                "Selected text:\n{}",
                truncate_middle(&selection.text, MAX_ATTACHMENT_CHARS)
            ));
        }
    }
    #[allow(deprecated)]
    let blocks = &context.executed_shell_commands;
    for command in blocks {
        attached.push(format!(
            "Terminal block:\n$ {}\n{}\n(exit code {})",
            command.command,
            truncate_middle(command.output.trim_end(), MAX_ATTACHMENT_CHARS),
            command.exit_code
        ));
    }
    for file in context
        .files
        .iter()
        .filter_map(|file| file.content.as_ref())
    {
        attached.push(format!(
            "File {}:\n{}",
            file.file_path,
            truncate_middle(&file.content, MAX_ATTACHMENT_CHARS)
        ));
    }
    if attached.is_empty() {
        return query.query.clone();
    }
    format!(
        "{}\n\nAttached context:\n\n{}",
        query.query,
        attached.join("\n\n")
    )
}

/// The function name and JSON arguments that reproduce a recorded tool call for the model.
fn tool_call_request(tool_call: &ToolCall) -> (String, String) {
    let (name, arguments) = match &tool_call.tool {
        Some(Tool::RunShellCommand(command)) => (
            "run_shell_command",
            serde_json::json!({ "command": command.command }),
        ),
        Some(Tool::ReadFiles(read)) => (
            "read_files",
            serde_json::json!({
                "files": read.files.iter().map(|file| file.name.as_str()).collect::<Vec<_>>(),
            }),
        ),
        Some(Tool::Grep(grep)) => (
            "grep",
            serde_json::json!({ "queries": grep.queries, "path": grep.path }),
        ),
        Some(Tool::FileGlobV2(glob)) => (
            "file_glob",
            serde_json::json!({ "patterns": glob.patterns, "search_dir": glob.search_dir }),
        ),
        Some(Tool::ApplyFileDiffs(diffs)) => (
            "apply_file_diffs",
            serde_json::json!({
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
                "deleted_files": diffs
                    .deleted_files
                    .iter()
                    .map(|file| file.file_path.as_str())
                    .collect::<Vec<_>>(),
            }),
        ),
        // The local agent only issues the calls above; anything else comes from a conversation
        // recorded by another agent and is replayed without arguments.
        _ => ("unsupported_tool", serde_json::json!({})),
    };
    (name.to_owned(), arguments.to_string())
}

fn render_tool_result(result: &ToolCallResult) -> String {
    match &result.result {
        Some(ToolResult::RunShellCommand(result)) => render_shell_result(result),
        Some(ToolResult::ReadFiles(result)) => match &result.result {
            Some(read_files_result::Result::TextFilesSuccess(success)) => success
                .files
                .iter()
                .map(|file| format!("--- {}\n{}\n", file.file_path, file.content))
                .collect(),
            Some(read_files_result::Result::Error(error)) => {
                format!("Error reading files: {}", error.message)
            }
            _ => "The files were read, but their content is not text.".to_owned(),
        },
        Some(ToolResult::Grep(result)) => format_grep_result(result),
        Some(ToolResult::FileGlobV2(result)) => format_glob_result(result),
        Some(ToolResult::ApplyFileDiffs(result)) => format_diff_result(result),
        Some(ToolResult::Cancel(())) | None => {
            "The user cancelled this tool call. Do not retry it unless asked.".to_owned()
        }
        // Results of tools the local agent does not offer.
        Some(_) => "The tool finished, but its result cannot be shown to this model.".to_owned(),
    }
}

fn render_shell_result(result: &RunShellCommandResult) -> String {
    match &result.result {
        Some(run_shell_command_result::Result::CommandFinished(finished)) => {
            if finished.exit_code == 0 {
                finished.output.clone()
            } else {
                format!("(exit code: {})\n{}", finished.exit_code, finished.output)
            }
        }
        Some(run_shell_command_result::Result::LongRunningCommandSnapshot(snapshot)) => format!(
            "The command is still running. Output so far:\n{}",
            snapshot.output
        ),
        Some(run_shell_command_result::Result::PermissionDenied(_)) => {
            "The user did not allow this command to run.".to_owned()
        }
        Some(run_shell_command_result::Result::TerminalBusy(_)) => {
            "The terminal is busy with another command, so this one did not run.".to_owned()
        }
        None =>
        {
            #[allow(deprecated)]
            if result.exit_code == 0 {
                result.output.clone()
            } else {
                format!("(exit code: {})\n{}", result.exit_code, result.output)
            }
        }
    }
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

fn add_messages_action(task_id: &str, messages: Vec<Message>) -> ClientAction {
    ClientAction {
        action: Some(client_action::Action::AddMessagesToTask(
            AddMessagesToTask {
                task_id: task_id.to_owned(),
                messages,
            },
        )),
    }
}

/// Streams agent text: the first chunk adds the message and later chunks append to it.
fn text_action(
    started_message: &mut bool,
    task_id: &str,
    request_id: &str,
    message_id: &str,
    text: String,
) -> ClientAction {
    let message = new_message(
        task_id,
        request_id,
        message_id.to_owned(),
        MessageKind::AgentOutput(AgentOutput { text }),
    );
    if !*started_message {
        *started_message = true;
        return add_messages_action(task_id, vec![message]);
    }
    ClientAction {
        action: Some(client_action::Action::AppendToMessageContent(
            AppendToMessageContent {
                task_id: task_id.to_owned(),
                message: Some(message),
                mask: Some(FieldMask {
                    paths: vec![AGENT_OUTPUT_TEXT_PATH.to_owned()],
                }),
            },
        )),
    }
}

fn new_message(task_id: &str, request_id: &str, id: String, kind: MessageKind) -> Message {
    Message {
        id,
        task_id: task_id.to_owned(),
        request_id: request_id.to_owned(),
        timestamp: Some(prost_types::Timestamp::from(SystemTime::now())),
        message: Some(kind),
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
#[path = "ownterm_openai_tests.rs"]
mod tests;
