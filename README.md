# OwnTerm

OwnTerm is an open-source, local-first terminal derived from the
[Warp client](https://github.com/warpdotdev/warp). Its goal is to keep the
terminal and editor experience while removing Warp-specific commercial,
account, billing, telemetry, cloud-drive, and hosted-agent dependencies.

> **Project status:** active first milestone. The OSS build starts without Warp
> authentication, removes the business/cloud feature surfaces, uses an
> OwnTerm-specific application identity and storage namespace, and routes agent
> requests directly to an OpenAI-compatible endpoint. Broad cross-platform
> packaging and tool-call compatibility remain in progress.

## Principles

- No sign-in required for terminal use.
- No Warp account, billing, credits, teams, Drive, or cloud-agent services.
- No telemetry by default; no hidden network requests.
- Direct connections to user-selected providers.
- API keys stay on the user's machine and are never committed.
- OpenAI-compatible endpoints first, with native provider adapters where useful.
- The derivative remains open under AGPL-3.0; the Warp UI crates retain MIT.

## Local workspace

```text
ownterm/
├── app/                      # OwnTerm GUI and local provider transport
├── crates/                   # terminal, UI, persistence, and shared core
├── docs/                     # architecture and migration decisions
├── scripts/                  # dependency and service-boundary audits
└── .env.example              # names only; never real secrets
```

## Run OwnTerm

```bash
export OWNTERM_BASE_URL="https://api.openai.com/v1"
export OWNTERM_MODEL="gpt-5-mini"
export OWNTERM_API_KEY="..."
./script/run
```

`OWNTERM_API_KEY` may be omitted for loopback services such as Ollama or vLLM.
Plain HTTP is accepted only for loopback addresses. You can also configure a
custom OpenAI Chat Completions endpoint in the Agent settings; its API key is
kept in the operating-system credential store.

The OwnTerm channel uses loopback-disabled placeholders for every inherited
Warp service URL. Direct model traffic uses a separate external HTTP client and
goes only to the endpoint selected by the user.

## First-milestone acceptance criteria

1. The terminal starts without authentication and works offline.
2. Shell sessions, tabs, panes, blocks, themes, search, completions, and the
   command palette remain usable.
3. Packet capture shows no Warp-owned endpoint during startup or ordinary
   terminal use.
4. A user can configure an OpenAI-compatible base URL, model, and API key.
5. Agent requests go directly from the client to that endpoint.
6. Keys are stored in the operating-system credential store, never preferences,
   logs, crash reports, or the repository.
7. Tests cover offline startup, streaming response parsing, cancellation,
   authentication errors, and secret redaction. Tool-call normalization is a
   remaining milestone.

## Licensing and identity

Most of the upstream client is AGPL-3.0-only. `warpui_core` and `warpui` are
MIT-licensed. Copyright and license notices must be preserved. “Warp” and its
logos are upstream trademarks and should not be used as OwnTerm branding.

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) and
[docs/MIGRATION.md](docs/MIGRATION.md) before modifying the client.
