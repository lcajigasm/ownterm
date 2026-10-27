# OwnTerm

<p align="center">
  <strong>The Open-Source, Local-First, Privacy-Focused AI Terminal.</strong><br>
  <em>GPU-accelerated, block-based terminal emulator with native agent tool-calling — zero logins, zero telemetry.</em>
</p>

<p align="center">
  <a href="https://github.com/lcajigasm/ownterm/releases/latest"><img src="https://img.shields.io/github/v/release/lcajigasm/ownterm?color=00ff9d&label=Release" alt="Release"></a>
  <a href="https://lcajigasm.github.io/ownterm/"><img src="https://img.shields.io/badge/Website-lcajigasm.github.io%2Fownterm-00d2ff" alt="Website"></a>
  <a href="LICENSE-AGPL"><img src="https://img.shields.io/badge/License-AGPL%203.0-blue.svg" alt="License: AGPL-3.0"></a>
  <a href="LICENSE-MIT"><img src="https://img.shields.io/badge/UI%20Core-MIT-green.svg" alt="License: MIT"></a>
  <a href="https://github.com/lcajigasm/ownterm/actions/workflows/release-ownterm.yml"><img src="https://github.com/lcajigasm/ownterm/actions/workflows/release-ownterm.yml/badge.svg" alt="Release Build"></a>
  <a href="https://github.com/lcajigasm/ownterm/actions/workflows/audit-services.yml"><img src="https://github.com/lcajigasm/ownterm/actions/workflows/audit-services.yml/badge.svg" alt="Zero-Telemetry Audit"></a>
</p>

---

## 🌐 Official Website & Documentation

Visit the official website for interactive demos, feature overviews, and release assets:
👉 **[https://lcajigasm.github.io/ownterm/](https://lcajigasm.github.io/ownterm/)**

---

## Table of Contents

- [Why OwnTerm?](#why-ownterm)
- [Key Features](#key-features)
- [Downloads & Installation](#downloads--installation)
- [Bring Your Own Model (BYOM)](#bring-your-own-model-byom)
  - [1. Local Models with Ollama](#1-local-models-with-ollama)
  - [2. Local Models with vLLM / LM Studio / LocalAI](#2-local-models-with-vllm--lm-studio--localai)
  - [3. Cloud Providers (OpenAI, OpenRouter, Groq)](#3-cloud-providers-openai-openrouter-groq)
- [Agent Tool Calling & Rules Configuration](#agent-tool-calling--rules-configuration)
- [Headless TUI (ownterm-tui)](#headless-tui-ownterm-tui)
- [Comparison Matrix](#comparison-matrix)
- [Security & Zero-Telemetry Guarantee](#security--zero-telemetry-guarantee)
- [Building from Source](#building-from-source)
- [Licensing and Trademarks](#licensing-and-trademarks)

---

## Why OwnTerm?

Modern terminals need more than classic 80x24 character grids — they need block-based navigation, searchable history, rich completions, and contextual AI assistance. However, proprietary alternatives lock these features behind **mandatory accounts, telemetry tracking, cloud drives, and paid subscriptions**.

**OwnTerm** is an open-source, local-first fork derived from the [Warp client](https://github.com/warpdotdev/warp). It completely severs all ties to proprietary cloud infrastructure, removes remote billing and analytics, and routes all AI agent queries directly to **your chosen local or remote LLM endpoints**.

### Core Principles

- **Zero Sign-In**: Start the app and immediately use your shell. No signup, no login screens, no authentication gates.
- **Zero Telemetry**: Completely offline-capable. No analytics, tracking pixels, or background data collection.
- **Direct Model Connections**: Connect directly to your local Ollama / vLLM instance or any OpenAI-compatible provider.
- **Keychain Security**: API keys are saved exclusively in your operating system's native credential store (macOS Keychain, Linux Secret Service, Windows Credential Manager).
- **Tool Calling Freedom**: Inspect files, execute commands, and run tests locally with user permission.
- **Open Source**: The main codebase is licensed under **AGPL-3.0**, and the shared UI framework components are licensed under **MIT**.

---

## Key Features

- ⚡ **GPU-Accelerated Rendering**: Lightning-fast performance using native Metal on macOS and OpenGL/Vulkan on Linux and Windows.
- 🧱 **Block-Based Output**: Terminal commands and outputs are grouped into interactive, searchable, and copyable blocks.
- 🤖 **Native AI Agent Mode**: Prompt the agent to inspect build failures, search files, write scripts, or execute shell commands right from the terminal prompt (`Ctrl+Enter` / `Cmd+Enter`).
- 🛠️ **Autonomous Tool Execution**: The agent suggests and executes tools like `run_shell_command` and `read_files`, requiring explicit user confirmation or policy allowance.
- 🎛️ **Project & Global Rules**: Customize model behavior and system instructions using local `.ownterm/rules.md` or global `~/.ownterm/rules.md`.
- 🖥️ **Dual Interfaces**: Choose between the visual desktop GUI (`ownterm`) or the lightweight headless console interface (`ownterm-tui`) for SSH sessions and remote servers.

---

## Downloads & Installation

Precompiled release bundles for **v0.1.0** are available on GitHub Releases:

| Platform | Architecture | Package |
| :--- | :--- | :--- |
| **macOS** | Apple Silicon (M1/M2/M3/M4) | [ownterm-macos-arm64.tar.gz](https://github.com/lcajigasm/ownterm/releases/download/v0.1.0/ownterm-macos-arm64.tar.gz) |
| **macOS** | Intel (x86_64) | [ownterm-macos-x86_64.tar.gz](https://github.com/lcajigasm/ownterm/releases/download/v0.1.0/ownterm-macos-x86_64.tar.gz) |
| **Linux** | x86_64 (glibc 2.31+) | [ownterm-linux-x86_64.tar.gz](https://github.com/lcajigasm/ownterm/releases/download/v0.1.0/ownterm-linux-x86_64.tar.gz) |
| **Windows**| x86_64 (Win 10/11) | [ownterm-windows-x86_64.tar.gz](https://github.com/lcajigasm/ownterm/releases/download/v0.1.0/ownterm-windows-x86_64.tar.gz) |

You can verify package integrity with [SHA256SUMS](https://github.com/lcajigasm/ownterm/releases/download/v0.1.0/SHA256SUMS).

---

## Bring Your Own Model (BYOM)

OwnTerm connects to any inference engine that exposes an OpenAI-compatible `/chat/completions` endpoint. You can configure endpoints via **Environment Variables** or directly in the **Settings GUI** (`Settings -> AI / Provider`).

### 1. Local Models with Ollama

Run completely private, local models with zero internet access:

```bash
# 1. Pull and run your preferred model
ollama run llama3.3

# 2. Start OwnTerm pointing to Ollama
export OWNTERM_BASE_URL="http://localhost:11434/v1"
export OWNTERM_MODEL="llama3.3"
./script/run
```

*(No API key is needed for loopback endpoints.)*

### 2. Local Models with vLLM / LM Studio / LocalAI

- **vLLM**:
  ```bash
  vllm serve Qwen/Qwen2.5-Coder-7B-Instruct --port 8000
  export OWNTERM_BASE_URL="http://localhost:8000/v1"
  export OWNTERM_MODEL="Qwen/Qwen2.5-Coder-7B-Instruct"
  ./script/run
  ```
- **LM Studio**:
  Start the Local Server in LM Studio (default: `http://localhost:1234/v1`) and set:
  ```bash
  export OWNTERM_BASE_URL="http://localhost:1234/v1"
  export OWNTERM_MODEL="qwen2.5-coder-7b-instruct"
  ./script/run
  ```

### 3. Cloud Providers (OpenAI, OpenRouter, Groq)

For remote cloud providers, HTTPS is strictly enforced and API keys are automatically redacted from error traces:

```bash
# OpenRouter example
export OWNTERM_BASE_URL="https://openrouter.ai/api/v1"
export OWNTERM_MODEL="anthropic/claude-3.5-sonnet"
export OWNTERM_API_KEY="sk-or-v1-..."
./script/run

# OpenAI example
export OWNTERM_BASE_URL="https://api.openai.com/v1"
export OWNTERM_MODEL="gpt-4o"
export OWNTERM_API_KEY="sk-proj-..."
./script/run
```

---

## Agent Tool Calling & Rules Configuration

OwnTerm normalizes function calling so any compatible model can interact directly with your terminal and workspace.

### Available Agent Tools

- `run_shell_command`: Executes terminal commands (e.g. running builds, test suites, git commands, and CLI tools). Users can inspect the command, click **Run**, or choose **Always Allow**.
- `read_files`: Inspects file contents in the workspace to analyze errors or provide code fixes.

### Custom Rules and System Prompts

You can steer the agent's behavior and personality using rules files. OwnTerm resolves system instructions with the following order of precedence:

1. `OWNTERM_SYSTEM_PROMPT` environment variable.
2. Local project rules: `.ownterm/rules.md` (in current working directory).
3. Global user rules: `~/.ownterm/rules.md` (in your home directory).
4. Default OwnTerm terminal assistant prompt.

---

## Headless TUI (`ownterm-tui`)

OwnTerm includes a headless terminal client (`crates/warp_tui`) designed for remote servers, SSH sessions, containers, and minimal desktop environments without a window manager:

```bash
# Run headless console client
./script/run-tui
```

`ownterm-tui` uses the cell-grid `TuiElement` rendering system, requiring no GPU or graphical display server.

---

## Comparison Matrix

| Feature | OwnTerm | Upstream Warp | Alacritty | Ghostty |
| :--- | :---: | :---: | :---: | :---: |
| **Account / Login Required** | **Never** ✕ | Mandatory ✓ | **Never** ✕ | **Never** ✕ |
| **Telemetry & Metrics** | **Zero** ✕ | Always on ✓ | **Zero** ✕ | **Zero** ✕ |
| **Local LLMs (Ollama / vLLM)** | **Native 1st-Class** ✓ | Cloud only ✕ | None ✕ | None ✕ |
| **Agent Tool Calling** | **Native** ✓ | Cloud subscription ✓ | None ✕ | None ✕ |
| **Block-Based Terminal** | **Yes** ✓ | Yes ✓ | Classic grid ✕ | Classic grid ✕ |
| **GPU Acceleration** | **Metal / OpenGL** ✓ | Metal / OpenGL ✓ | OpenGL ✓ | Metal / OpenGL ✓ |
| **Headless TUI Included** | **Yes (`ownterm-tui`)** ✓ | No ✕ | No ✕ | No ✕ |
| **License** | **AGPL-3.0 / MIT** | Proprietary services | Apache 2.0 | MIT |

---

## Security & Zero-Telemetry Guarantee

1. **Storage Isolation**: OwnTerm stores data strictly under `~/.ownterm` (GUI) and `~/.ownterm_cli` (TUI), fully isolated from any legacy configuration.
2. **Keyring Credentials**: API keys entered in Settings are written directly to OS-level secure storage, never plain text configuration files.
3. **Loopback-Only HTTP**: Non-HTTPS connections are strictly rejected unless targeting `localhost` or `127.0.0.1`.
4. **Automated Audit Workflow**: Every push and PR is scanned by [`.github/workflows/audit-services.yml`](.github/workflows/audit-services.yml) running [`scripts/audit-warp-services.sh`](scripts/audit-warp-services.sh) to ensure zero upstream cloud service coupling.

---

## Building from Source

### Prerequisites

- **Rust**: Latest stable toolchain (`rustup update stable`).
- **Protobuf Compiler (`protoc`)**:
  - macOS: `brew install protobuf`
  - Linux: `sudo apt-get install -y protobuf-compiler pkg-config libasound2-dev libfontconfig1-dev libfreetype6-dev libx11-dev libxcursor-dev libxi-dev libxrandr-dev libgl1-mesa-dev`
  - Windows: `choco install protoc`

### Build & Run

```bash
# Clone the repository
git clone https://github.com/lcajigasm/ownterm.git
cd ownterm

# Run the GUI desktop app locally
./script/run

# Run the headless console TUI
./script/run-tui

# Build production release binary
cargo build --release -p warp --bin ownterm
```

*(Note for macOS: If you do not have Xcode Metal Developer Tools installed, prepend `WARP_LOCAL_SKIP_METAL=1 ./script/run`)*.

### Running Unit Tests

```bash
# Run core and channel tests
cargo test -p warp_core

# Run OwnTerm inference and agent tests
WARP_LOCAL_SKIP_METAL=1 cargo test -p warp --lib ownterm
```

---

## Licensing and Trademarks

- **Code Licensing**: The main terminal client is licensed under the **GNU Affero General Public License v3.0** ([`LICENSE-AGPL`](LICENSE-AGPL)). Shared UI framework crates (`warpui`, `warpui_core`) retain the **MIT License** ([`LICENSE-MIT`](LICENSE-MIT)).
- **Attribution**: Derived from the open-source client of Warp (© 2026 Warp Services, Inc.).
- **Trademarks & Non-Affiliation**: OwnTerm is an independent open-source project and is not affiliated with, sponsored by, or endorsed by Warp Services, Inc. "Warp" is a trademark of Warp Services, Inc.
