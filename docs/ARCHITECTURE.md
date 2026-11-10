# OwnTerm architecture

OwnTerm is the Warp client built on the `Channel::Oss` channel with every hosted service switched off. This note says where the fork's behavior lives. The gap list is [ROADMAP.md](ROADMAP.md).

## What is switched off, and where

| Concern | Mechanism |
| :--- | :--- |
| Warp servers | `WarpServerConfig::disabled()` and `OzConfig::disabled()` in `crates/warp_core/src/channel/config.rs` point every server URL at `127.0.0.1:9`. The OSS binaries (`app/src/bin/oss.rs`, `crates/warp_tui/src/bin/oss.rs`) use them. |
| Telemetry, crash upload, updater | `telemetry_config`, `crash_reporting_config`, and `autoupdate_config` are `None`. `ChannelState::may_transmit_telemetry()` guards event construction. |
| Cloud features | `OWNTERM_OSS_DISABLED_FEATURES` in `app/src/features.rs` removes cloud, billing, account, updater, and server-backed AI flags after the normal flag set is computed. |
| Sign-in | Startup authentication is skipped for the OSS channel in `app/src/lib.rs`, `app/src/root_view.rs`, and `app/src/tui/mod.rs`. |

`scripts/audit-ownterm-privacy.sh` fails CI when one of these anchors disappears.

## Local agent

Upstream, the client sends a request to Warp's agent service and applies the stream of client actions it gets back. On the OSS channel, `generate_multi_agent_output` (`app/src/ai/agent/api/impl.rs`) calls `ownterm_openai::generate` instead, which plays the service's part on this machine:

```text
input (query or tool results)
        |
   plan_turn ── creates the root task on the first turn and records the input
        |       as task messages, because no server echoes it back
        |
 build_chat_messages ── system prompt + task history -> /chat/completions messages
        |
   stream_turn ── POST to the configured endpoint, SSE in, client actions out
        |
 client runs the tool calls and sends their results as the next request
```

The task's messages are the only conversation state. Each request rebuilds the chat history from them, so a tool result reaches the model on the following request without any state in the adapter.

Rules the adapter keeps:

- A tool call runs only when its name is one of OwnTerm's tools and its arguments parse. Unknown names, malformed arguments, and calls cut off by the output limit become a notice in the transcript.
- The tool calls of one reply share one assistant message, and every call is answered by a tool message, including calls the user cancelled.
- The system message carries the session's working directory, operating system, shell, and project rule files. Attached blocks, selections, and files travel with the user message.
- Large tool results, attachments, and old turns are trimmed before the request is sent (`MAX_TOOL_OUTPUT_CHARS`, `MAX_ATTACHMENT_CHARS`, `OWNTERM_MAX_HISTORY_CHARS`).
- A model that rejects a request listing tools is retried once without them.

## Provider configuration

`crates/ownterm_provider` owns everything about which endpoint and model to call:

- `lib.rs`: provider presets, `~/.ownterm/provider.env`, URL rules (HTTPS unless loopback).
- `auto.rs`: the `auto` model, which ranks `catalog.json` by price among providers that are reachable.
- `agent_tools.rs`: the tool schemas sent to the model and the parser for its tool calls.
- `form.rs`, `workflows.rs`: the Settings form and local workflow files.

The API key is read from an environment variable and is never written to disk. There are no native Anthropic or Google adapters; any endpoint must speak the OpenAI chat-completions format.

## Packaging

`.github/workflows/release.yml` builds `ownterm` and `ownterm-tui` for four targets on a version tag and publishes archives, a macOS disk image, and `SHA256SUMS`. Nothing is code-signed. `script/bump-version.sh` keeps `app/Cargo.toml`, `Cargo.lock`, and the bundle metadata on the same version.
