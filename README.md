<p align="right">
  <b>English</b> | <a href="README.ru.md">Русский</a>
</p>

# WebAgentMCP

Autonomous local bridge and MCP host for web AI chats. Turns browser-based AI chats (Google AI Studio, DeepSeek, ChatGPT, Claude, Gemini, Qwen, Kimi, Z.ai) into autonomous local coding agents without paid API keys.

The system combines a lightweight Rust background daemon with a browser extension. The extension waits for generation to finish, intercepts generated `<tool>` calls, and sends them to the daemon. The daemon executes actions on your machine (filesystem operations, terminal commands, MCP servers) and returns output back into the chat.

## Supported Platforms

- Google AI Studio
- DeepSeek
- ChatGPT
- Claude
- Gemini
- Qwen (`chat.qwen.ai`, `chat.qwenlm.ai`)
- Kimi
- Z.ai

---

## Installation

### 1. Build from Source

Requires a working Rust toolchain:

```bash
cd daemon
cargo build --release
```

Binary will be located at `daemon/target/release/webagentmcp-daemon` (or `.exe` on Windows).

### 2. Setup Wizard

Run the binary or execute the setup command:

```bash
webagentmcp setup
```

The wizard will:

1. Prompt for UI language (English / Russian).
2. Install the binary into `~/.webagentmcp/bin` and add it to user `PATH` (both `webagentmcp` and alias `wamcp` will be available).
3. Extract the browser extension into `~/.webagentmcp/extension/` and copy the path to clipboard.

### 3. Load Extension in Browser

- **Chrome / Edge / Brave / Chromium:**
  1. Open `chrome://extensions/` (or `edge://extensions/`).
  2. Enable **Developer mode** in top right corner.
  3. Click **Load unpacked**.
  4. Paste path from clipboard (`~/.webagentmcp/extension/`).

- **Firefox:**
  1. Open `about:debugging#/runtime/this-firefox`.
  2. Click **Load Temporary Add-on...** and select `manifest.json` inside the extension directory.

---

## Usage

1. Open a terminal inside your target project directory:
   ```bash
   webagentmcp
   ```
   _(daemon binds to `127.0.0.1:5505` and sets current directory as workspace)_.
2. Open any supported AI chat in browser.
3. Open the WebAgentMCP Side Panel in browser and click **"Initialize Agent in Chat"**.
4. Give the model a task — it will begin inspecting, patching, and testing code autonomously.

### CLI Commands

```bash
webagentmcp                # Start daemon in current directory
webagentmcp -d /path/dir   # Start daemon with explicit workspace directory
webagentmcp --tray         # Start detached in background with system tray icon
webagentmcp stop           # Stop running daemon instance
webagentmcp status         # Show status, active port, workspace, and MCP servers
webagentmcp workspace get  # Print current workspace directory
webagentmcp workspace set <path> # Change workspace directory on the fly
webagentmcp setup          # Re-run setup/maintenance wizard
webagentmcp uninstall      # Remove ~/.webagentmcp files
```

### Managing External MCP Servers

The daemon can host external MCP servers via `stdio` (JSON-RPC) and expose their tool schemas directly to the AI chat:

```bash
# Add an MCP server
webagentmcp mcp add db-server npx -y @modelcontextprotocol/server-postgres postgresql://localhost/mydb

# List registered servers
webagentmcp mcp list

# Toggle on/off without removing
webagentmcp mcp toggle db-server off

# Remove server
webagentmcp mcp remove db-server
```

---

## Built-in Tools

- `view_dir` — prints directory tree in ASCII format, respecting `.gitignore` rules and ignoring binaries/caches.
- `read` — reads entire file, redacting exposed secrets/tokens and paginating large files.
- `read_lines` — reads exact line range with 1-based line numbering.
- `search_code` — searches codebase via regex or literal text.
- `write` — writes or overwrites file (automatically strips accidental markdown fences).
- `patch` — surgical search/replace block within file.
- `replace` — bounded line replacement using start/end line bounds.
- `terminal_run` — synchronous terminal execution (up to 120s) with clean UTF-8 capture.
- `terminal_bg` — launches long-running background processes (dev servers, builds) returning PID.
- `terminal_logs` — reads background process stdout/stderr by PID.
- `terminal_kill` — terminates background PID.
- `cron_monitor` — waits for N seconds, then returns latest background process logs.
- `mcp_call` — executes tools on registered external MCP servers.

---

## Notes & Troubleshooting

- **Windows Console Encoding:** internal `cmd.exe` outputs are handled using an OEM codepage (`CP_OEMCP` / CP866) fallback decoder, preventing replacement characters («») in Cyrillic paths and command results.
- **Line Endings (CRLF vs LF):** file patch and replace tools automatically normalize line endings between Windows files and browser text inputs.
- **Large Output Handling:** outputs exceeding browser chat limits are returned as initial readable slices with exact follow-up continuation commands (`<tool='read_lines'>`), while full raw logs are preserved on disk at `.webagentmcp/last_output.txt`.
- **Background Mode:** launching with `--tray` detaches the console window and routes actions to the system tray. Use `webagentmcp stop` to stop it from CLI.

---

## License

MIT License. See [LICENSE](LICENSE).
