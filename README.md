# OwnTerm

OwnTerm is an open-source, local-first terminal derived from the
[Warp client](https://github.com/warpdotdev/warp). Its goal is to keep the
terminal and editor experience while removing Warp-specific commercial,
account, billing, telemetry, cloud-drive, and hosted-agent dependencies.

> **Project status:** foundation and migration plan. The public fork exists at
> <https://github.com/lcajigasm/ownterm>. The upstream source has not yet been
> checked out in this local folder because this execution environment blocks
> GitHub Git and `codeload.github.com` traffic. Do not treat the AI decoupling as
> implemented until the acceptance checks below pass.

## Principles

- No sign-in required for terminal use.
- No Warp account, billing, credits, teams, Drive, or cloud-agent services.
- No telemetry by default; no hidden network requests.
- Direct connections to user-selected providers.
- API keys stay on the user's machine and are never committed.
- OpenAI-compatible endpoints first, with native provider adapters where useful.
- The derivative remains open under AGPL-3.0; the Warp UI crates retain MIT.

## Planned workspace

```text
ownterm/
├── upstream/                 # checkout of lcajigasm/ownterm (Warp fork)
├── docs/                     # architecture and migration decisions
├── scripts/                  # bootstrap and dependency audits
└── .env.example              # names only; never real secrets
```

Create the upstream checkout when network access is available:

```bash
./scripts/bootstrap-upstream.sh
./scripts/audit-warp-services.sh
```

## Definition of done for the first usable release

1. The terminal starts without authentication and works offline.
2. Shell sessions, tabs, panes, blocks, themes, search, completions, and the
   command palette remain usable.
3. Packet capture shows no Warp-owned endpoint during startup or ordinary
   terminal use.
4. A user can configure an OpenAI-compatible base URL, model, and API key.
5. Agent requests go directly from the client to that endpoint.
6. Keys are stored in the operating-system credential store, never preferences,
   logs, crash reports, or the repository.
7. Tests cover offline startup, streaming responses, cancellation, tool calls,
   authentication errors, and secret redaction.

## Licensing and identity

Most of the upstream client is AGPL-3.0-only. `warpui_core` and `warpui` are
MIT-licensed. Copyright and license notices must be preserved. “Warp” and its
logos are upstream trademarks and should not be used as OwnTerm branding.

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) and
[docs/MIGRATION.md](docs/MIGRATION.md) before modifying the client.
