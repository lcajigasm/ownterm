# OwnTerm architecture

## Boundary

The terminal core should not know about accounts, plans, credits, workspaces,
or a particular model vendor. Provider integration sits behind a small local
interface and returns a provider-neutral event stream.

```text
Terminal UI / command blocks / editor
                  |
             Local agent UI
                  |
          Provider abstraction
          /        |          \
 OpenAI-compatible Anthropic   Google
          |
   HTTPS directly from device
```

There is deliberately no OwnTerm proxy in the default path. A user may point
the OpenAI-compatible adapter at OpenAI, OpenRouter, LiteLLM, Ollama, vLLM, or
another compatible endpoint.

## Proposed Rust interface

The final names should follow the upstream crate conventions, but the boundary
should be equivalent to:

```rust
#[async_trait]
pub trait ModelProvider: Send + Sync {
    async fn models(&self) -> Result<Vec<Model>, ProviderError>;
    async fn stream(
        &self,
        request: ChatRequest,
    ) -> Result<ProviderEventStream, ProviderError>;
}
```

`ProviderEventStream` should normalize text deltas, reasoning metadata when
available, tool calls, usage, completion, cancellation, and errors. Provider
adapters own wire formats; terminal and agent views do not.

## Configuration

Non-secret configuration may live in OwnTerm preferences:

- provider kind and display name;
- HTTPS base URL;
- model identifier;
- optional request headers whose values are not secrets;
- timeout and context limits.

Secrets must be referenced by an opaque credential ID. Store their values in
macOS Keychain, Windows Credential Manager, or Linux Secret Service. Environment
variables are a development-only fallback.

## Network policy

- Deny Warp domains in the OwnTerm build after the migration.
- No analytics, remote feature flags, hosted configuration, update channel, or
  crash upload without an explicit new design and opt-in.
- Validate custom URLs and show the exact destination before saving.
- Require TLS except for loopback endpoints such as local Ollama or vLLM.
- Redact authorization headers and provider payloads from logs.

## Packaging

Application IDs, executable names, update metadata, icons, URLs, and support
links must use OwnTerm-controlled values. Preserve upstream copyright and
license attribution separately from product identity.
