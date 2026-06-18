# ContextX Intelligence

**By Santhakumar K • Alpha X Solutions**

Local-first **Claude usage monitor + context token saver** for AI tools.

ContextX is a single Rust CLI binary that helps AI users see usage and save tokens at the same time. It combines Claude Usage Monitor-style live usage visibility with reversible context compression, MCP tools, localhost proxy support, CLI wrapping, and safe auto-configuration.

```text
Usage you can see  +  Tokens you can save  =  ContextX Intelligence
```

No Docker. No telemetry. No cloud server. Memory-only by default.

## Quick Install From Source

Best current install path:

```bash
cargo install --git https://github.com/SanthaKumar-K-2004/ContextX-Intelligence.git
```

Then verify:

```bash
contextx doctor
```

Manual source build:

```bash
git clone https://github.com/SanthaKumar-K-2004/ContextX-Intelligence.git
cd ContextX-Intelligence
cargo build --release
```

Make the command available on Linux/macOS:

```bash
mkdir -p ~/.local/bin
cp target/release/contextx ~/.local/bin/contextx
export PATH="$HOME/.local/bin:$PATH"
```

Run with Cargo during development:

```bash
cargo run -- doctor
```

Run the built binary:

```bash
./target/release/contextx doctor
```

## Recommended Distribution Plan

ContextX is a Rust system tool, so the best packaging order is:

| Stage | Package Channel | User Command | Why |
| --- | --- | --- | --- |
| Now | GitHub + Cargo | `cargo install --git https://github.com/SanthaKumar-K-2004/ContextX-Intelligence.git` | Fastest real install for Rust users |
| Next | GitHub Releases | `curl -fsSL .../install.sh \| sh` | Best for normal users, no Rust required |
| Later | Homebrew | `brew install contextx` | Best macOS/Linux developer install |
| Later | npm wrapper | `npm install -g contextx-intelligence` | Good for JS/Node users; wrapper downloads the Rust binary |
| Later | PyPI wrapper | `pipx install contextx-intelligence` | Good for Python users; wrapper downloads the Rust binary |
| Later | Windows Scoop | `scoop install contextx` | Clean Windows install |

Recommended product strategy: keep the core as one Rust binary and use npm/PyPI only as installer wrappers. That keeps ContextX fast, low-RAM, and easy to ship.

## One-Minute Setup

Preview setup first:

```bash
contextx doctor --fix --dry-run
```

Apply safe setup:

```bash
contextx doctor --fix
```

Start the shared local engine:

```bash
contextx daemon
```

Open the dashboard in another terminal:

```bash
contextx tui
```

## Command Cheat Sheet By Tool

### Claude Desktop

```bash
contextx install --client claude-desktop
contextx verify-client --client claude-desktop
contextx daemon
```

Restart Claude Desktop after install.

### Cursor

```bash
contextx install --client cursor
contextx verify-client --client cursor
contextx daemon
```

### VS Code / Cline / Continue

```bash
contextx install --client vscode
contextx verify-client --client vscode
contextx daemon
```

### Zed

```bash
contextx install --client zed
contextx verify-client --client zed
contextx daemon
```

### Claude Code

```bash
contextx daemon
contextx wrap claude
```

### Codex CLI

```bash
contextx daemon
contextx wrap codex
```

### Aider

```bash
contextx daemon
contextx wrap aider
```

### OpenAI SDK / LangChain / Vercel AI SDK

```bash
contextx proxy --port 8787
export OPENAI_BASE_URL=http://127.0.0.1:8787/v1
```

### Dashboard

```bash
contextx daemon
contextx tui
```

### Stats

```bash
contextx stats
contextx stats --watch
```

## What ContextX Shows

```text
Claude / Provider Usage
Input sent:        18,500 tokens
Output received:   4,200 tokens
Burn rate:           820 tokens/min

ContextX Saver
Original input:   120,000 tokens
Compressed sent:   18,500 tokens
Saved:              84.6%
CCR originals:         12 retrievable
```

ContextX can only measure and save traffic it can see through MCP, proxy, wrapper, or direct CLI calls. Exact Claude account quota depends on what Claude exposes to local tools.

## Architecture

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│                         CONTEXTX INTELLIGENCE                               │
│                         Santhakumar K • Alpha X Solutions                    │
│                                                                              │
│  Single local Rust binary: contextx                                          │
│  No Docker • No telemetry • Memory-only by default • Localhost-only API       │
└──────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│ contextx daemon                                                              │
│ Shared local brain on 127.0.0.1:8787                                         │
│                                                                              │
│  ┌────────────────────┐  ┌────────────────────┐  ┌────────────────────┐     │
│  │ Compression Engine │  │ Usage Monitor      │  │ CCR Memory Cache   │     │
│  │ JSON / code / text │  │ session / weekly   │  │ originals in RAM   │     │
│  │ savings gate       │  │ burn rate / agents │  │ retrieve by key    │     │
│  └────────────────────┘  └────────────────────┘  └────────────────────┘     │
│                                                                              │
│  ┌────────────────────┐  ┌────────────────────┐  ┌────────────────────┐     │
│  │ Event Ring Buffer  │  │ Doctor / Installer │  │ Learning Observe   │     │
│  │ recent local stats │  │ safe config edits  │  │ suggestions only   │     │
│  └────────────────────┘  └────────────────────┘  └────────────────────┘     │
└──────────────────────────────────────────────────────────────────────────────┘
          ▲                    ▲                    ▲                    ▲
          │                    │                    │                    │
┌─────────┴─────────┐ ┌────────┴────────┐ ┌────────┴────────┐ ┌────────┴────────┐
│ MCP Mode          │ │ Proxy Mode      │ │ Wrapper Mode    │ │ CLI / TUI       │
│ contextx mcp      │ │ contextx proxy  │ │ contextx wrap   │ │ stats, doctor   │
│                   │ │                 │ │                 │ │ tui, install    │
│ Claude Desktop    │ │ OpenAI SDK      │ │ Claude Code     │ │ user dashboard  │
│ Cursor            │ │ Anthropic API   │ │ Codex CLI       │ │ setup checks    │
│ Cline             │ │ LangChain       │ │ Aider           │ │ safe config     │
│ Continue          │ │ Vercel AI SDK   │ │ other CLIs      │ │ verification    │
│ Zed               │ │ custom apps     │ │                 │ │                 │
└───────────────────┘ └─────────────────┘ └─────────────────┘ └─────────────────┘
```

## Tool Inventory

### MCP Tools

| Tool | What It Does | When To Use |
| --- | --- | --- |
| `contextx_compress` | Compresses messages and returns token savings + CCR keys | Before sending large context to an LLM |
| `contextx_retrieve` | Retrieves original content from RAM using CCR keys | When the model/client needs full details |
| `contextx_stats` | Returns usage, savings, cache, agent, provider, and learning status | Dashboards, client status, debugging |
| `contextx_learn` | Returns observe-only tuning suggestions | Future auto-tuning workflow |

### CLI Commands

| Command | Purpose |
| --- | --- |
| `contextx daemon` | Shared local brain for MCP, proxy, stats, and TUI |
| `contextx mcp` | MCP stdio server for Claude Desktop, Cursor, Cline, Continue, Zed |
| `contextx proxy` | Local OpenAI/Anthropic-compatible proxy |
| `contextx wrap <command>` | Run terminal AI tools through ContextX tracking |
| `contextx tui` | Live terminal dashboard |
| `contextx stats` | Print current memory-only stats |
| `contextx doctor` | Check local setup |
| `contextx doctor --json` | Print machine-readable setup status |
| `contextx doctor --fix` | Apply safe auto-configuration |
| `contextx print-config --client <client>` | Print config without editing files |
| `contextx verify-client --client <client>` | Verify client config contains ContextX |
| `contextx install --client <client>` | Configure one supported client |
| `contextx install --all` | Configure all supported client configs |

## Supported Tools And Setup

### Claude Desktop

Preview config:

```bash
contextx print-config --client claude-desktop
```

Install:

```bash
contextx install --client claude-desktop
```

Verify:

```bash
contextx verify-client --client claude-desktop
```

Run:

```bash
contextx daemon
```

Then restart Claude Desktop and use the MCP tools.

### Cursor

```bash
contextx install --client cursor
contextx verify-client --client cursor
contextx daemon
```

Cursor will see the MCP server as `contextx`.

### VS Code

```bash
contextx install --client vscode
contextx verify-client --client vscode
contextx daemon
```

VS Code-style config uses a `servers` section instead of `mcpServers`.

### Zed

```bash
contextx install --client zed
contextx verify-client --client zed
contextx daemon
```

### Any MCP Client

Use this config shape:

```json
{
  "mcpServers": {
    "contextx": {
      "command": "/absolute/path/to/contextx",
      "args": ["mcp"]
    }
  }
}
```

For VS Code-style clients:

```json
{
  "servers": {
    "contextx": {
      "command": "/absolute/path/to/contextx",
      "args": ["mcp"]
    }
  }
}
```

### OpenAI-Compatible SDKs

Start proxy:

```bash
contextx proxy --port 8787
```

Point your app to ContextX:

```bash
export OPENAI_BASE_URL=http://127.0.0.1:8787/v1
```

Supported local proxy paths:

```text
POST /v1/chat/completions
POST /v1/messages
POST /v1/responses
```

### LangChain / Vercel AI SDK / Custom Apps

Use the same proxy URL:

```bash
http://127.0.0.1:8787/v1
```

Your app sends traffic to ContextX, ContextX compresses visible messages, forwards upstream, and records usage when response fields are available.

### Claude Code / Codex CLI / Aider

Run through wrapper:

```bash
contextx wrap codex
contextx wrap aider
contextx wrap claude
```

Current wrapper behavior is intentionally safe: it starts the command and tracks the session. Deeper request interception is a future layer.

## Common Workflows

### Live Dashboard

Terminal 1:

```bash
contextx daemon
```

Terminal 2:

```bash
contextx tui
```

### Watch JSON Stats

```bash
contextx stats --watch
```

### Print Setup Status As JSON

```bash
contextx doctor --json
```

### Dry-Run All Setup Changes

```bash
contextx doctor --fix --dry-run
```

### Install All Supported Clients

```bash
contextx install --all
```

ContextX creates backups before editing existing config files.

## Local API

When `contextx daemon` or `contextx proxy` is running:

```text
GET  /health
GET  /stats
GET  /v1/contextx/stats
POST /v1/contextx/compress
POST /v1/contextx/retrieve
POST /v1/contextx/learn
```

Example compress request:

```bash
curl -s http://127.0.0.1:8787/v1/contextx/compress \
  -H 'content-type: application/json' \
  -d '{
    "messages": [
      {"role": "user", "content": "large context here"}
    ],
    "agent": "curl",
    "provider": "local-test"
  }'
```

## Privacy And Safety

Default behavior:

- No Docker.
- No telemetry.
- No cloud service.
- No prompt history database.
- CCR originals are stored only in process RAM.
- Stats store counts, hashes, timing, model, provider, agent, and project metadata.
- HTTP services bind to `127.0.0.1` by default.
- Non-localhost binding is blocked unless `--allow-non-localhost` is explicitly passed.

Important limit:

Prompts still go to the AI provider selected by your client or proxy upstream. ContextX protects its own local processing; it does not make third-party providers private.

## Safe Auto-Configuration

ContextX edits config carefully:

- Reads existing JSON.
- Keeps existing settings.
- Keeps other MCP servers.
- Adds only the `contextx` server entry.
- Creates a backup before editing existing files.

Example before:

```json
{
  "mcpServers": {
    "github": {
      "command": "github-mcp"
    }
  }
}
```

After:

```json
{
  "mcpServers": {
    "github": {
      "command": "github-mcp"
    },
    "contextx": {
      "command": "/absolute/path/to/contextx",
      "args": ["mcp"]
    }
  }
}
```

## Development

```bash
cargo fmt --check
cargo test
cargo run -- doctor
cargo run -- mcp
cargo run -- proxy --port 8787
```

## Project Identity

Project: **ContextX Intelligence**

Creator: **Santhakumar K**

Company: **Alpha X Solutions**

Repository: <https://github.com/SanthaKumar-K-2004/ContextX-Intelligence>
