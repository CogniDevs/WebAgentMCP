# WebAgentMCP

<details open>
<summary><b>English</b></summary>

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

## Dual-Direction Agent Hub

WebAgentMCP supports two complementary agentic workflows:

### 1. Web-to-Local (Control Local Machine from Web Chat)
Work inside web chat interfaces (DeepSeek, Qwen, Claude, AI Studio). The model generates `<tool>...</tool>` tags, and the local `webagentmcp` daemon executes them in your workspace, returning stdout directly into the chat.

### 2. Agent-to-Web (Delegate Heavy Tasks to Free Web Models)
Coding agents (Antigravity, Claude Code, Cursor, Aider) can delegate heavy refactoring, thinking/reasoning, and vibe-coding tasks to free web models (DeepSeek-R1 with reasoning, Qwen 3.7 Max with search) to save paid tokens.

#### Unified OpenAI-compatible Gateway:
```bash
npm run gateway
```
Runs at `http://127.0.0.1:5506/v1` and supports:
- `POST /v1/chat/completions` (JSON & SSE streaming with `reasoning_content`)
- Models: `deepseek-reasoner` (R1), `deepseek-chat` (V3), `qwen3.7-max`, `qwen3.7-plus`, `qwen-turbo`.
- **Anti-Ban Guard**: native WASM PoW solver for DeepSeek, session thread reuse, and human-like request jitter (600–2000ms).

#### Agent MCP Server:
```bash
npm run mcp
```
Provides tools for external coding agents:
- `ask_web_model` — single-turn query with thinking/reasoning process extraction.
- `delegate_vibe_coding` — offload heavy multi-file refactoring and drafting to free web models.

---

## License

MIT License. See [LICENSE](LICENSE).
</details>

<details>
<summary><b>Русский</b></summary>

Автономный локальный мост и хост MCP-серверов для браузерных AI-чатов. Позволяет веб-моделям (Google AI Studio, DeepSeek, ChatGPT, Claude, Gemini, Qwen, Kimi, Z.ai) безопасно работать с локальными файлами, выполнять команды в терминале и вызывать инструменты внешних MCP-серверов без использования платных API-ключей.

Работает через связку легковесного демона на Rust и расширения для браузера. Расширение отслеживает окончание ответа модели, забирает сгенерированные теги вызова инструментов (`<tool>`), демон выполняет их на машине (файлы, терминал, MCP-серверы) и возвращает результат обратно в диалог.

## Поддерживаемые платформы

- Google AI Studio
- DeepSeek
- ChatGPT
- Claude
- Gemini
- Qwen (`chat.qwen.ai`, `chat.qwenlm.ai`)
- Kimi
- Z.ai

---

## Установка

### 1. Сборка из исходников

Требуется установленный Rust:

```bash
cd daemon
cargo build --release
```

Бинарник появится в `daemon/target/release/webagentmcp-daemon` (или `.exe` на Windows).

### 2. Первичная настройка

Запустите бинарник или выполните команду мастера:

```bash
webagentmcp setup
```

Мастер предложит:
1. Выбрать язык (Русский / English).
2. Скопировать бинарник в систему и прописать путь в пользовательский `PATH` (будут доступны команды `webagentmcp` и короткий алиас `wamcp`).
3. Распаковать расширение браузера и скопировать путь к нему в буфер обмена.

### 3. Подключение расширения

- **Chrome / Edge / Brave / Chromium:**
  1. Перейдите по адресу `chrome://extensions/` (или `edge://extensions/`).
  2. Включите **Режим разработчика** в правом верхнем углу.
  3. Нажмите **Загрузить распакованное**.
  4. Вставьте путь из буфера обмена (`~/.webagentmcp/extension/`).

- **Firefox:**
  1. Перейдите по адресу `about:debugging#/runtime/this-firefox`.
  2. Нажмите **Загрузить временное дополнение...** и выберите `manifest.json` в папке расширения.

---

## Использование

1. Откройте терминал в папке нужного проекта и выполните:
   ```bash
   webagentmcp
   ```
   *(демон запустится на `127.0.0.1:5505` и сделает текущую папку рабочей директорией)*.
2. Откройте нужный AI-чат в браузере.
3. Откройте боковую панель расширения (Side Panel) и нажмите **«Инициализировать агента в чате»**.
4. Задайте задачу — модель начнёт автономный цикл чтения, правок и тестирования кода.

### Команды CLI

```bash
webagentmcp                # Запуск сервера в текущей папке
webagentmcp -d /path/dir   # Запуск с указанием рабочей папки
webagentmcp --tray         # Запуск в фоновом режиме со значком в трее
webagentmcp stop           # Остановка запущенного демона
webagentmcp status         # Проверка порта, рабочей папки и активных MCP
webagentmcp workspace get  # Показать текущую рабочую папку
webagentmcp workspace set <path> # Изменить рабочую папку на лету
webagentmcp setup          # Повторный запуск мастера установки/обновления
webagentmcp uninstall      # Полное удаление файлов из ~/.webagentmcp
```

### Управление MCP-серверами (Model Context Protocol)

Демон умеет подключать внешние MCP-серверы, работающие через `stdio` (JSON-RPC), и пробрасывать их схемы инструментов в чат:

```bash
# Добавить сервер (например, PostgreSQL)
webagentmcp mcp add db-server npx -y @modelcontextprotocol/server-postgres postgresql://localhost/mydb

# Список настроенных серверов
webagentmcp mcp list

# Включение / отключение без удаления
webagentmcp mcp toggle db-server off

# Удаление сервера
webagentmcp mcp remove db-server
```

---

## Встроенные инструменты агента

- `view_dir` — вывод дерева проекта в формате ASCII с фильтрацией бинарников, локфайлов и правил `.gitignore`.
- `read` — чтение файла целиком с маскированием секретов и пагинацией, если файл слишком большой.
- `read_lines` — построчное чтение заданного диапазона строк.
- `search_code` — поиск по кодовой базе (регулярное выражение или подстрока).
- `write` — создание или полная перезапись файла (с очисткой случайных markdown-обёрток).
- `patch` — точечная замена блока кода по поисковому фрагменту.
- `replace` — замена блока с ограничением номерами начальной и конечной строк.
- `terminal_run` — синхронное выполнение команд (сборка, тесты, git) с тайм-аутом до 120 с.
- `terminal_bg` — запуск долгоживущих процессов (dev-серверы, npm install) с возвратом PID.
- `terminal_logs` — чтение логов фонового процесса по его PID.
- `terminal_kill` — завершение процесса по PID.
- `cron_monitor` — пауза на N секунд с последующим автоматическим возвратом вывода фонового процесса.
- `mcp_call` — вызов инструментов из подключенных MCP-серверов.

---

## Особенности работы и устранение проблем

- **Кодировка консоли на Windows:** демон использует декодер `CP_OEMCP` (CP866), предотвращая появление знаков «» в кириллических путях и выводе `cmd.exe`.
- **Переносы строк CRLF / LF:** в инструментах `patch` и `replace` переносы строк нормализуются автоматически, исключая сбои сопоставления между файлами на диске и текстом из браузера.
- **Слишком большой вывод:** если результат команды или файл превышает допустимый лимит чата, демон отдаёт первую часть с указанием общего числа строк и точной командой для дочитывания следующего куска (`<tool='read_lines'>`), а полный лог сохраняет в `.webagentmcp/last_output.txt`.
- **Фоновый режим:** при запуске с флагом `--tray` окно консоли отсоединяется, а управление переходит в системный трей. Остановка из терминала выполняется командой `webagentmcp stop`.

---

## Двусторонний режим (Dual-Direction Agent Hub)

WebAgentMCP поддерживает два дополняющих друг друга сценария:

### 1. Web-to-Local (Управление компьютером из браузера)
Вы находитесь в веб-интерфейсе (DeepSeek, Qwen, Claude, AI Studio), отправляете запрос модели, модель генерирует теги `<tool>...</tool>`, локальный демон `webagentmcp` выполняет действия в терминале или файлах и возвращает результат в диалог.

### 2. Agent-to-Web (Делегирование задач веб-моделям для экономии токенов)
Внешние агенты (Antigravity, Claude Code, Cursor, Aider) могут использовать бесплатные веб-модели (DeepSeek-R1 с reasoning, Qwen 3.7 Max с поиском) для тяжелых вычислительных задач и вайбкодинга без затрат на платные токены.

#### Запуск единого шлюза (OpenAI-compatible):
```bash
npm run gateway
```
Шлюз поднимается на `http://127.0.0.1:5506/v1` и поддерживает:
- `POST /v1/chat/completions` (с потоковым SSE и передачей `reasoning_content`)
- Модели: `deepseek-reasoner` (R1), `deepseek-chat` (V3), `qwen3.7-max`, `qwen3.7-plus`, `qwen-turbo`.
- **Anti-Ban Guard**: автоматический расчет WASM PoW для DeepSeek, сессионный троттлинг и рандомизированный джиттер между запросами.

#### Использование через MCP Server:
```bash
npm run mcp
```
Предоставляет агентам инструменты:
- `ask_web_model` — разовый запрос к DeepSeek/Qwen с получением цепочки рассуждений.
- `delegate_vibe_coding` — отправка объемной задачи на рефакторинг/написание кода с получением готового решения.

---

## Лицензия

MIT License. См. файл [LICENSE](LICENSE).
</details>
