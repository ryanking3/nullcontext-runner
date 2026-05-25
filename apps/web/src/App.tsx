import { useEffect, useRef, useState } from "react";
import "./App.css";

const API_BASE = "http://127.0.0.1:3333";

type SessionRegistry = {
  sessions: SessionIndexEntry[];
};

type SessionIndexEntry = {
  session_id: string;
  started_at: string;
  security_mode: string;
  prompt_source: string;
  history_stored: boolean;
  backend: string;
  model_path: string;
  workspace: string;
  report_path: string;
  artifacts_detected: number;
  cleanup_attempted: boolean;
  cleanup_successful: boolean;
  workspace_deleted: boolean;
};

type AuditOperation = {
  operation: string;
  status: string;
  details: string;
};

type Theme = "dark" | "light";
type RunStatus = "idle" | "running" | "success" | "failed";
type RuntimeMode = "one-shot" | "active-chat";
type ChatTemplateOption = "auto" | "generic" | "chatml" | "llama3-instruct";
type InspectorView = "audit" | "runtime" | "report" | "stderr";

type StreamPayload = {
  type: string;
  message?: string;
  text?: string;
  operation?: string;
  status?: string;
  details?: string;
  success?: boolean;
};

type ChatStartResponse = {
  session_id: string;
  workspace: string;
  security_mode: string;
  persistent: boolean;
  runtime_active: boolean;
  turns: number;
  chat_template: string;
  chat_context_token_budget: number;
  chat_context_turn_limit: number;
  history_policy: string;
};

type ChatStatusResponse = {
  session_id: string;
  workspace: string;
  security_mode: string;
  persistent: boolean;
  runtime_active: boolean;
  turns: number;
  runtime_duration_ms?: number;
  chat_template?: string;
  chat_context_token_budget?: number;
  chat_context_turn_limit?: number;
  history_policy?: string;
  residual_risk?: string;
};

type ChatEndResponse = {
  session_id: string;
  runtime_stopped: boolean;
  report: unknown;
};

type ApiErrorResponse = {
  error?: string;
};

type ChatMessage = {
  role: "user" | "assistant";
  content: string;
};

function statusClass(status: string): string {
  if (status === "successful") return "pill success";
  if (status === "failed") return "pill failed";
  if (status === "warning") return "pill warning";
  if (status === "not_attempted") return "pill muted";
  return "pill neutral";
}

function shortId(id: string): string {
  return id.slice(0, 8);
}

function formatDuration(ms: number): string {
  const totalSeconds = Math.floor(ms / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;

  return `${minutes}m ${seconds.toString().padStart(2, "0")}s`;
}

function parsePositiveInteger(value: string): number | null {
  const parsed = Number(value);

  if (!Number.isInteger(parsed) || parsed <= 0) {
    return null;
  }

  return parsed;
}

function Hint({ text }: { text: string }) {
  return (
    <span className="hint" tabIndex={0}>
      <span className="hint-trigger">?</span>
      <span className="hint-bubble">{text}</span>
    </span>
  );
}

async function readApiError(response: Response, fallback: string): Promise<string> {
  try {
    const text = await response.text();

    if (!text.trim()) {
      return fallback;
    }

    try {
      const data = JSON.parse(text) as ApiErrorResponse;

      if (typeof data.error === "string" && data.error.trim()) {
        return data.error;
      }
    } catch {
      return text;
    }

    return text;
  } catch {
    return fallback;
  }
}

function formatActiveChatApiError(action: "start" | "message" | "end", message: string): string {
  if (message.includes("generation is still in progress")) {
    return "The active chat runtime is still finishing the current generation. Wait for streaming to settle, or use Stop and then retry End + Sanitize once the session is idle.";
  }

  if (message.includes("already generating")) {
    return "An active chat generation is already in progress for this session. Wait for it to finish before sending another message.";
  }

  if (message.includes("session is ending")) {
    return "This active chat session is already ending. Wait for End + Sanitize to complete before sending another message.";
  }

  if (message.includes("Active chat session not found")) {
    if (action === "end") {
      return "This active chat session is no longer available. Refresh the UI state or start a new session.";
    }

    return "This active chat session is no longer available. Start a new session and try again.";
  }

  return message;
}

function parseSseBlock(block: string): StreamPayload | null {
  const dataLines = block
    .split("\n")
    .filter((line) => line.startsWith("data:"))
    .map((line) => line.replace(/^data:\s?/, ""));

  if (dataLines.length === 0) {
    return null;
  }

  try {
    return JSON.parse(dataLines.join("\n")) as StreamPayload;
  } catch {
    return {
      type: "error",
      message: `Failed to parse stream event: ${dataLines.join("\n")}`,
    };
  }
}

function App() {
  const activeAbortController = useRef<AbortController | null>(null);
  const commandMenuRef = useRef<HTMLDivElement | null>(null);

  const [theme, setTheme] = useState<Theme>("dark");
  const [serverStatus, setServerStatus] = useState<"checking" | "online" | "offline">("checking");
  const [healthCheckedAt, setHealthCheckedAt] = useState<string>("never");
  const [registryLoadedAt, setRegistryLoadedAt] = useState<string>("never");

  const [runtimeMode, setRuntimeMode] = useState<RuntimeMode>("one-shot");
  const [prompt, setPrompt] = useState("");
  const [mode, setMode] = useState("secure");
  const [persistent, setPersistent] = useState(false);
  const [runStatus, setRunStatus] = useState<RunStatus>("idle");
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [inspectorOpen, setInspectorOpen] = useState(true);
  const [configDrawerOpen, setConfigDrawerOpen] = useState(false);
  const [inspectorView, setInspectorView] = useState<InspectorView>("audit");
  const [commandMenuOpen, setCommandMenuOpen] = useState(false);
  const [chatTemplate, setChatTemplate] = useState<ChatTemplateOption>("auto");
  const [chatContextTokenBudget, setChatContextTokenBudget] = useState("2048");
  const [chatContextTurnLimit, setChatContextTurnLimit] = useState("12");

  const [activeChatSessionId, setActiveChatSessionId] = useState("");
  const [activeChatWorkspace, setActiveChatWorkspace] = useState("");
  const [activeChatTurns, setActiveChatTurns] = useState(0);
  const [activeChatRuntimeActive, setActiveChatRuntimeActive] = useState(false);
  const [activeChatStartedAt, setActiveChatStartedAt] = useState<number | null>(null);
  const [activeRuntimeElapsedMs, setActiveRuntimeElapsedMs] = useState(0);
  const [activeChatRisk, setActiveChatRisk] = useState(
    "Runtime is inactive. No active chat KV/cache state is currently held by NullContext."
  );
  const [activeChatStopNotice, setActiveChatStopNotice] = useState("");
  const [activeChatResolvedTemplate, setActiveChatResolvedTemplate] = useState("");
  const [activeChatHistoryPolicy, setActiveChatHistoryPolicy] = useState("");
  const [activeChatContextBudget, setActiveChatContextBudget] = useState<number | null>(null);
  const [activeChatContextTurnLimit, setActiveChatContextTurnLimit] = useState<number | null>(
    null
  );

  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [runtimeLogs, setRuntimeLogs] = useState("");
  const [privacyReport, setPrivacyReport] = useState("");
  const [stderr, setStderr] = useState("");
  const [auditOperations, setAuditOperations] = useState<AuditOperation[]>([]);

  const [sessions, setSessions] = useState<SessionIndexEntry[]>([]);
  const [selectedReport, setSelectedReport] = useState<string>("");
  const [selectedSessionId, setSelectedSessionId] = useState<string>("");

  async function checkHealth() {
    setServerStatus("checking");

    try {
      const response = await fetch(`${API_BASE}/api/health`);
      setServerStatus(response.ok ? "online" : "offline");
    } catch {
      setServerStatus("offline");
    } finally {
      setHealthCheckedAt(new Date().toLocaleTimeString());
    }
  }

  async function loadSessions() {
    try {
      const response = await fetch(`${API_BASE}/api/sessions`);
      const data = (await response.json()) as SessionRegistry;
      setSessions(data.sessions ?? []);
    } catch {
      setSessions([]);
    } finally {
      setRegistryLoadedAt(new Date().toLocaleTimeString());
    }
  }

  function resetRunPanels() {
    setRuntimeLogs("");
    setPrivacyReport("");
    setStderr("");
    setAuditOperations([]);
    setSelectedReport("");
    setSelectedSessionId("");
  }

  function resetOneShotConversation() {
    setMessages([]);
    resetRunPanels();
    setActiveChatStopNotice("");
  }

  function appendAssistantText(text: string) {
    setMessages((current) => {
      const next = [...current];
      const last = next[next.length - 1];

      if (last?.role === "assistant") {
        next[next.length - 1] = {
          ...last,
          content: `${last.content}${text}`,
        };
      } else {
        next.push({
          role: "assistant",
          content: text,
        });
      }

      return next;
    });
  }

  function handleStreamPayload(payload: StreamPayload) {
    switch (payload.type) {
      case "runtime": {
        if (payload.message) {
          setRuntimeLogs((current) => `${current}${payload.message}\n`);
        }
        break;
      }

      case "audit": {
        if (payload.operation && payload.status && payload.details) {
          setAuditOperations((current) => [
            ...current,
            {
              operation: payload.operation ?? "unknown",
              status: payload.status ?? "unknown",
              details: payload.details ?? "",
            },
          ]);
        }
        break;
      }

      case "model": {
        if (payload.text) {
          appendAssistantText(payload.text);
          setRuntimeLogs((current) => {
            if (current.includes("--- Model Output ---\n<RESPONSE>\n")) {
              return current;
            }

            return `${current}--- Model Output ---\n<RESPONSE>\n`;
          });
        }
        break;
      }

      case "report": {
        if (payload.text) {
          setPrivacyReport((current) => `${current}${payload.text}`);
        }
        break;
      }

      case "stderr": {
        if (payload.message) {
          setStderr((current) => `${current}${payload.message}\n`);
        }
        break;
      }

      case "error": {
        if (payload.message) {
          setStderr((current) => `${current}${payload.message}\n`);
        }
        setRunStatus("failed");
        break;
      }

      case "complete": {
        setRunStatus(payload.success ? "success" : "failed");

        if (persistent) {
          loadSessions();
        }

        if (runtimeMode === "active-chat" && payload.success) {
          refreshActiveChatStatus();
        }

        break;
      }

      default: {
        setRuntimeLogs((current) => `${current}${JSON.stringify(payload)}\n`);
      }
    }
  }

  async function consumeSseResponse(response: Response, signal: AbortSignal) {
    if (!response.body) {
      throw new Error("Streaming response body was empty");
    }

    const reader = response.body.getReader();
    const decoder = new TextDecoder();

    let buffer = "";

    while (true) {
      if (signal.aborted) {
        break;
      }

      const { value, done } = await reader.read();

      if (done) {
        break;
      }

      buffer += decoder.decode(value, { stream: true });

      const blocks = buffer.split("\n\n");
      buffer = blocks.pop() ?? "";

      for (const block of blocks) {
        const payload = parseSseBlock(block);

        if (payload) {
          handleStreamPayload(payload);
        }
      }
    }

    if (!signal.aborted && buffer.trim()) {
      const payload = parseSseBlock(buffer);

      if (payload) {
        handleStreamPayload(payload);
      }
    }
  }

  function stopGeneration() {
    activeAbortController.current?.abort();
    activeAbortController.current = null;

    setRunStatus("failed");
    setRuntimeLogs((current) => `${current}Generation stopped by user.\n`);
    setActiveChatStopNotice(
      runtimeMode === "active-chat"
        ? "Stopped this active-chat generation. Any partial assistant text still visible in the transcript was not committed to backend chat history. The runtime remains active until you send another message or use End + Sanitize."
        : ""
    );

    setAuditOperations((current) => [
      ...current,
      {
        operation: "client_generation_stop",
        status: "warning",
        details:
          runtimeMode === "active-chat"
            ? "Client stopped the current active-chat generation. The chat runtime remains active."
            : "Client stopped the one-shot generation. Backend cleanup should continue server-side.",
      },
    ]);
  }

  function readActiveChatConfigInputs() {
    const tokenBudget = parsePositiveInteger(chatContextTokenBudget);
    const turnLimit = parsePositiveInteger(chatContextTurnLimit);

    if (tokenBudget === null) {
      throw new Error("Active chat context token budget must be a whole number greater than 0.");
    }

    if (turnLimit === null) {
      throw new Error("Active chat context turn limit must be a whole number greater than 0.");
    }

    return {
      chat_template: chatTemplate,
      chat_context_token_budget: tokenBudget,
      chat_context_turn_limit: turnLimit,
    };
  }

  async function runOneShot() {
    resetOneShotConversation();
    setRunStatus("running");

    const currentPrompt = prompt;
    const controller = new AbortController();
    activeAbortController.current = controller;

    setMessages([
      {
        role: "user",
        content: currentPrompt,
      },
      {
        role: "assistant",
        content: "",
      },
    ]);

    try {
      const activeChatConfig = readActiveChatConfigInputs();
      setCommandMenuOpen(false);
      const response = await fetch(`${API_BASE}/api/run/stream`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          prompt: currentPrompt,
          mode,
          persistent,
          ...activeChatConfig,
        }),
        signal: controller.signal,
      });

      await consumeSseResponse(response, controller.signal);
    } catch (error) {
      if (controller.signal.aborted) {
        setRunStatus("failed");
      } else {
        setStderr(String(error));
        setRunStatus("failed");
      }
    } finally {
      if (activeAbortController.current === controller) {
        activeAbortController.current = null;
      }
    }
  }

  async function startActiveChat() {
    resetRunPanels();
    setActiveChatStopNotice("");
    setRunStatus("running");
    setConfigDrawerOpen(false);
    setCommandMenuOpen(false);

    try {
      const activeChatConfig = readActiveChatConfigInputs();
      const response = await fetch(`${API_BASE}/api/chat/start`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          mode,
          persistent,
          ...activeChatConfig,
        }),
      });

      if (!response.ok) {
        const error = await readApiError(response, "Failed to start active chat session.");
        throw new Error(formatActiveChatApiError("start", error));
      }

      const data = (await response.json()) as ChatStartResponse;

      setRuntimeMode("active-chat");
      setActiveChatSessionId(data.session_id);
      setActiveChatWorkspace(data.workspace);
      setActiveChatTurns(data.turns);
      setActiveChatRuntimeActive(data.runtime_active);
      setActiveChatStartedAt(Date.now());
      setActiveRuntimeElapsedMs(0);
      setActiveChatRisk(
        "Runtime is active. llama.cpp remains loaded, KV/cache state may remain live, and chat context remains in memory until End + Sanitize."
      );
      setActiveChatResolvedTemplate(data.chat_template);
      setActiveChatHistoryPolicy(data.history_policy);
      setActiveChatContextBudget(data.chat_context_token_budget);
      setActiveChatContextTurnLimit(data.chat_context_turn_limit);
      setRunStatus("success");

      setRuntimeLogs((current) =>
        `${current}Started active chat session ${data.session_id}\nWorkspace: ${data.workspace}\n`
      );
    } catch (error) {
      setStderr(String(error));
      setRunStatus("failed");
    }
  }

  async function refreshActiveChatStatus() {
    if (!activeChatSessionId) {
      return;
    }

    try {
      const response = await fetch(`${API_BASE}/api/chat/${activeChatSessionId}/status`);

      if (!response.ok) {
        return;
      }

      const data = (await response.json()) as ChatStatusResponse;

      setActiveChatTurns(data.turns);
      setActiveChatRuntimeActive(data.runtime_active);
      setActiveChatWorkspace(data.workspace);

      if (typeof data.runtime_duration_ms === "number") {
        setActiveRuntimeElapsedMs(data.runtime_duration_ms);
      }

      if (data.chat_template) {
        setActiveChatResolvedTemplate(data.chat_template);
      }

      if (typeof data.chat_context_token_budget === "number") {
        setActiveChatContextBudget(data.chat_context_token_budget);
      }

      if (typeof data.chat_context_turn_limit === "number") {
        setActiveChatContextTurnLimit(data.chat_context_turn_limit);
      }

      if (data.history_policy) {
        setActiveChatHistoryPolicy(data.history_policy);
      }

      if (data.residual_risk) {
        setActiveChatRisk(data.residual_risk);
      }
    } catch {
      // Non-critical UI refresh failure.
    }
  }

  async function endActiveChat() {
    if (!activeChatSessionId) {
      return;
    }

    setRunStatus("running");
    setActiveChatStopNotice("");
    setCommandMenuOpen(false);

    try {
      const response = await fetch(`${API_BASE}/api/chat/${activeChatSessionId}/end`, {
        method: "POST",
      });

      if (!response.ok) {
        const error = await readApiError(response, "Failed to end active chat session.");
        const friendlyError = formatActiveChatApiError("end", error);
        setActiveChatStopNotice(friendlyError);
        throw new Error(friendlyError);
      }

      const data = (await response.json()) as ChatEndResponse;

      setActiveChatRuntimeActive(false);
      setActiveChatTurns(0);
      setActiveChatStartedAt(null);
      setActiveRuntimeElapsedMs(0);
      setActiveChatRisk(
        "Runtime is inactive. Active chat buffers were finalized according to the session cleanup policy."
      );
      setActiveChatResolvedTemplate("");
      setActiveChatHistoryPolicy("");
      setActiveChatContextBudget(null);
      setActiveChatContextTurnLimit(null);
      setPrivacyReport(JSON.stringify(data.report, null, 2));
      setRuntimeLogs((current) =>
        `${current}Ended active chat session ${data.session_id}\nRuntime stopped: ${data.runtime_stopped}\n`
      );

      setActiveChatSessionId("");
      setActiveChatWorkspace("");
      setRunStatus("success");

      if (persistent) {
        await loadSessions();
      }
    } catch (error) {
      setStderr(String(error));
      setRunStatus("failed");
    }
  }

  async function sendActiveChatMessage() {
    if (!activeChatSessionId) {
      setStderr("No active chat session. Start a session first.");
      setRunStatus("failed");
      return;
    }

    setRunStatus("running");
    setRuntimeLogs("");
    setPrivacyReport("");
    setStderr("");
    setAuditOperations([]);
    setActiveChatStopNotice("");
    setCommandMenuOpen(false);

    const currentPrompt = prompt;
    const controller = new AbortController();
    activeAbortController.current = controller;

    setMessages((current) => [
      ...current,
      {
        role: "user",
        content: currentPrompt,
      },
      {
        role: "assistant",
        content: "",
      },
    ]);

    try {
      const response = await fetch(
        `${API_BASE}/api/chat/${activeChatSessionId}/message/stream`,
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            prompt: currentPrompt,
          }),
          signal: controller.signal,
        }
      );

      if (!response.ok) {
        const error = await readApiError(response, "Failed to send active chat message.");
        throw new Error(formatActiveChatApiError("message", error));
      }

      await consumeSseResponse(response, controller.signal);

      if (!controller.signal.aborted) {
        setPrompt("");
      }
    } catch (error) {
      if (controller.signal.aborted) {
        setRunStatus("failed");
      } else {
        setStderr(String(error));
        setRunStatus("failed");
      }
    } finally {
      if (activeAbortController.current === controller) {
        activeAbortController.current = null;
      }
    }
  }

  async function runSession() {
    if (runtimeMode === "one-shot") {
      await runOneShot();
      return;
    }

    await sendActiveChatMessage();
  }

  async function openReport(sessionId: string) {
    setSelectedSessionId(sessionId);

    try {
      const response = await fetch(`${API_BASE}/api/reports/${sessionId}`);
      const data = await response.json();
      setSelectedReport(JSON.stringify(data, null, 2));
    } catch (error) {
      setSelectedReport(String(error));
    }
  }

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
  }, [theme]);

  useEffect(() => {
    checkHealth();
    loadSessions();
  }, []);

  useEffect(() => {
    if (!activeChatRuntimeActive || activeChatStartedAt === null) {
      return;
    }

    const interval = window.setInterval(() => {
      setActiveRuntimeElapsedMs(Date.now() - activeChatStartedAt);
    }, 1000);

    return () => window.clearInterval(interval);
  }, [activeChatRuntimeActive, activeChatStartedAt]);

  useEffect(() => {
    function warnBeforeUnload(event: BeforeUnloadEvent) {
      if (!activeChatRuntimeActive) {
        return;
      }

      event.preventDefault();
      event.returnValue =
        "An active NullContext chat session is still running. End + Sanitize before leaving.";
    }

    window.addEventListener("beforeunload", warnBeforeUnload);

    return () => window.removeEventListener("beforeunload", warnBeforeUnload);
  }, [activeChatRuntimeActive]);

  useEffect(() => {
    function closeOnEscape(event: KeyboardEvent) {
      if (event.key === "Escape") {
        setConfigDrawerOpen(false);
        setCommandMenuOpen(false);
      }
    }

    window.addEventListener("keydown", closeOnEscape);

    return () => window.removeEventListener("keydown", closeOnEscape);
  }, []);

  useEffect(() => {
    function closeCommandMenu(event: MouseEvent) {
      if (
        commandMenuOpen &&
        commandMenuRef.current &&
        !commandMenuRef.current.contains(event.target as Node)
      ) {
        setCommandMenuOpen(false);
      }
    }

    window.addEventListener("mousedown", closeCommandMenu);

    return () => window.removeEventListener("mousedown", closeCommandMenu);
  }, [commandMenuOpen]);

  const inspectorTabs: Array<{
    id: InspectorView;
    label: string;
    count?: number;
    disabled?: boolean;
  }> = [
    { id: "audit", label: "audit", count: auditOperations.length },
    { id: "runtime", label: "runtime" },
    { id: "report", label: "report" },
    { id: "stderr", label: "stderr", disabled: !stderr },
  ];

  return (
    <main
      className={`shell${sidebarCollapsed ? " sidebar-collapsed" : ""}${
        inspectorOpen ? "" : " inspector-hidden"
      }`}
    >
      <aside className={`sidebar${sidebarCollapsed ? " collapsed" : ""}`}>
        <div className="brand">
          <div className="logo">NC</div>
          {!sidebarCollapsed && (
            <div>
              <h1>NullContext</h1>
              <p>localhost runtime</p>
            </div>
          )}
        </div>

        {sidebarCollapsed ? (
          <div className="sidebar-compact">
            <button
              className="ghost-button"
              onClick={() => setSidebarCollapsed(false)}
              title="Expand sidebar"
            >
              open
            </button>
            <button
              className="ghost-button"
              onClick={() => setConfigDrawerOpen(true)}
              title="Open session config drawer"
            >
              config
            </button>
            <button
              className="ghost-button"
              onClick={() => setInspectorOpen((current) => !current)}
              title={inspectorOpen ? "Hide inspector" : "Show inspector"}
            >
              inspect
            </button>
            <button className="ghost-button" onClick={checkHealth} title="Check server health">
              check
            </button>
            <div className="compact-status" title={`server ${serverStatus}`}>
              <span className={`server-dot ${serverStatus}`} />
              <span>{serverStatus}</span>
            </div>
          </div>
        ) : (
          <>
            <section className="server-line">
              <span className={`server-dot ${serverStatus}`} />
              <span>server:{serverStatus}</span>
              <button className="ghost-button" onClick={checkHealth}>
                check
              </button>
            </section>
            <p className="microcopy">last check: {healthCheckedAt}</p>

            <section className="panel">
              <div className="panel-title">runtime mode</div>

              <div className="segmented">
                <button
                  className={runtimeMode === "one-shot" ? "selected" : ""}
                  onClick={() => setRuntimeMode("one-shot")}
                  disabled={activeChatRuntimeActive}
                >
                  one-shot
                </button>
                <button
                  className={runtimeMode === "active-chat" ? "selected" : ""}
                  onClick={() => setRuntimeMode("active-chat")}
                >
                  active chat
                </button>
              </div>

              <p className="microcopy">
                one-shot cleans up every prompt. active chat keeps the runtime alive until end +
                sanitize.
              </p>
            </section>

            <section className="panel">
              <div className="panel-header">
                <div className="panel-title">session config</div>
                <button className="ghost-button" onClick={() => setConfigDrawerOpen(true)}>
                  open
                </button>
              </div>

              <div className="config-summary">
                <span>mode: {mode}</span>
                <span>persistent: {persistent ? "on" : "off"}</span>
                <span>template: {chatTemplate}</span>
                <span>context: {chatContextTokenBudget} tok / {chatContextTurnLimit} turns</span>
              </div>

              <p className="microcopy">
                Move detailed controls into the config drawer so the main shell stays focused on
                runtime state and conversation flow.
              </p>
            </section>

            <section className="panel">
              <div className="panel-header">
                <div className="panel-title">registry</div>
                <button className="ghost-button" onClick={loadSessions}>
                  refresh
                </button>
              </div>

              <p className="microcopy">last refresh: {registryLoadedAt}</p>

              <div className="session-list">
                {sessions.length === 0 ? (
                  <p className="muted-text">no persistent sessions</p>
                ) : (
                  sessions.map((session) => (
                    <button
                      className={
                        selectedSessionId === session.session_id
                          ? "session-item selected"
                          : "session-item"
                      }
                      key={session.session_id}
                      onClick={() => {
                        setInspectorView("report");
                        setInspectorOpen(true);
                        openReport(session.session_id);
                      }}
                    >
                      <span>{shortId(session.session_id)}</span>
                      <small>{session.security_mode}</small>
                      <small>{new Date(session.started_at).toLocaleString()}</small>
                    </button>
                  ))
                )}
              </div>
            </section>

            <section className="panel">
              <div className="panel-title">theme</div>
              <div className="segmented">
                <button
                  className={theme === "dark" ? "selected" : ""}
                  onClick={() => setTheme("dark")}
                >
                  dark
                </button>
                <button
                  className={theme === "light" ? "selected" : ""}
                  onClick={() => setTheme("light")}
                >
                  light
                </button>
              </div>
            </section>
          </>
        )}
      </aside>

      <section className="main-column">
        <header className="topbar">
          <div>
            <h2>chat</h2>
            <p>
              {runtimeMode === "one-shot"
                ? "one-shot secure inference"
                : "active runtime chat session"}
            </p>
          </div>
          <div className="topbar-actions topbar-status">
            <span className="mini-status">server:{serverStatus}</span>
            <span className="mini-status">
              {runtimeMode === "one-shot" ? "mode:one-shot" : "mode:active-chat"}
            </span>
          </div>
        </header>

        <section
          className={
            activeChatRuntimeActive
              ? "active-runtime-banner active"
              : "active-runtime-banner inactive"
          }
        >
          <div>
            <strong>
              {runtimeMode === "one-shot"
                ? "one-shot runtime"
                : activeChatRuntimeActive
                  ? "active chat runtime"
                  : "active chat runtime inactive"}
            </strong>
            <p>
              {runtimeMode === "one-shot"
                ? "Each prompt starts and ends its own runtime."
                : activeChatRisk}
            </p>
            {runtimeMode === "active-chat" && activeChatWorkspace && (
              <div className="runtime-path truncate" title={activeChatWorkspace}>
                workspace: {activeChatWorkspace}
              </div>
            )}
            {runtimeMode === "active-chat" && activeChatHistoryPolicy && (
              <div className="runtime-meta">
                <div>template: {activeChatResolvedTemplate || "unknown"}</div>
                {activeChatContextBudget !== null && activeChatContextTurnLimit !== null && (
                  <div>
                    context: {activeChatContextBudget} tok / {activeChatContextTurnLimit} turns
                  </div>
                )}
                <div title={activeChatHistoryPolicy}>policy: {activeChatHistoryPolicy}</div>
              </div>
            )}
          </div>

          <div className="runtime-command-strip">
            <div className="runtime-stats">
              <span>turns: {activeChatTurns}</span>
              <span>duration: {formatDuration(activeRuntimeElapsedMs)}</span>
              {activeChatSessionId && <span>id: {shortId(activeChatSessionId)}</span>}
            </div>

            <div className="runtime-actions">
              {runtimeMode === "active-chat" &&
                (!activeChatRuntimeActive ? (
                  <button onClick={startActiveChat} disabled={runStatus === "running"}>
                    start session
                  </button>
                ) : (
                  <button
                    className="danger-button"
                    onClick={endActiveChat}
                    disabled={runStatus === "running"}
                  >
                    end + sanitize
                  </button>
                ))}

              <div className="popup-menu" ref={commandMenuRef}>
                <button
                  className="ghost-button popup-trigger"
                  onClick={() => setCommandMenuOpen((current) => !current)}
                  title="Open runtime actions menu"
                >
                  actions
                </button>

                {commandMenuOpen && (
                  <div className="popup-panel">
                    <button
                      onClick={() => {
                        setConfigDrawerOpen(true);
                        setCommandMenuOpen(false);
                      }}
                    >
                      open config drawer
                    </button>
                    <button
                      onClick={() => {
                        setInspectorOpen((current) => !current);
                        setCommandMenuOpen(false);
                      }}
                    >
                      {inspectorOpen ? "hide inspector" : "show inspector"}
                    </button>
                    <button
                      onClick={() => {
                        setSidebarCollapsed((current) => !current);
                        setCommandMenuOpen(false);
                      }}
                    >
                      {sidebarCollapsed ? "expand sidebar" : "collapse sidebar"}
                    </button>
                    <button
                      onClick={() => {
                        checkHealth();
                        setCommandMenuOpen(false);
                      }}
                    >
                      check server
                    </button>
                    <button
                      onClick={() => {
                        loadSessions();
                        setCommandMenuOpen(false);
                      }}
                    >
                      refresh registry
                    </button>
                  </div>
                )}
              </div>
            </div>
          </div>
        </section>

        <section className="chat-card">
          {activeChatStopNotice && (
            <div className="notice-banner warning-banner">{activeChatStopNotice}</div>
          )}

          <div className="messages">
            {messages.length === 0 && (
              <div className="empty-state">
                <h3>ready</h3>
                <p>
                  {runtimeMode === "one-shot"
                    ? "send a prompt. NullContext will start, infer, audit, report, and clean up."
                    : "start an active chat session, then send multiple prompts through the same runtime."}
                </p>
              </div>
            )}

            {messages.map((message, index) => (
              <div className={`message ${message.role}`} key={`${message.role}-${index}`}>
                <div className="role">{message.role}</div>
                <div className="bubble">
                  {message.content || (runStatus === "running" ? "running..." : "")}
                </div>
              </div>
            ))}
          </div>

          <div className="composer">
            <textarea
              value={prompt}
              onChange={(event) => setPrompt(event.target.value)}
              placeholder={
                runtimeMode === "active-chat" && !activeChatRuntimeActive
                  ? "start a session first..."
                  : "message nullcontext..."
              }
              disabled={runtimeMode === "active-chat" && !activeChatRuntimeActive}
            />

            {runStatus === "running" ? (
              <button className="danger-button" onClick={stopGeneration}>
                stop
              </button>
            ) : (
              <button
                onClick={runSession}
                disabled={
                  prompt.trim() === "" ||
                  (runtimeMode === "active-chat" && !activeChatRuntimeActive)
                }
              >
                send
              </button>
            )}
          </div>
        </section>
      </section>

      {inspectorOpen && (
        <aside className="inspector">
          <section className="panel inspector-shell">
            <div className="panel-header">
              <div className="panel-title">inspector</div>
              <button className="ghost-button" onClick={() => setInspectorOpen(false)}>
                hide
              </button>
            </div>

            <div className="inspector-tabs">
              {inspectorTabs.map((tab) => (
                <button
                  key={tab.id}
                  className={inspectorView === tab.id ? "selected" : ""}
                  disabled={tab.disabled}
                  onClick={() => setInspectorView(tab.id)}
                  title={tab.disabled ? "No data yet" : undefined}
                >
                  {tab.label}
                  {typeof tab.count === "number" ? ` (${tab.count})` : ""}
                </button>
              ))}
            </div>

            <div className="inspector-panel">
              {inspectorView === "audit" && (
                <>
                  {auditOperations.length === 0 ? (
                    <p className="muted-text">audit operations appear during a run</p>
                  ) : (
                    <div className="audit-list">
                      {auditOperations.map((operation, index) => (
                        <details className="audit-item" key={`${operation.operation}-${index}`}>
                          <summary>
                            <code>{operation.operation}</code>
                            <span className={statusClass(operation.status)}>{operation.status}</span>
                          </summary>
                          <p>{operation.details}</p>
                        </details>
                      ))}
                    </div>
                  )}
                </>
              )}

              {inspectorView === "runtime" && <pre>{runtimeLogs || "no runtime logs yet"}</pre>}

              {inspectorView === "report" && (
                <pre>{selectedReport || privacyReport || "no report selected"}</pre>
              )}

              {inspectorView === "stderr" && (
                <pre>{stderr || "no stderr captured"}</pre>
              )}
            </div>
          </section>
        </aside>
      )}

      <div
        className={`drawer-backdrop${configDrawerOpen ? " open" : ""}`}
        onClick={() => setConfigDrawerOpen(false)}
      />
      <aside className={`config-drawer${configDrawerOpen ? " open" : ""}`}>
        <div className="drawer-header">
          <div>
            <h3>session config</h3>
            <p>move detailed controls off the main page and keep the shell focused</p>
          </div>
          <button className="ghost-button" onClick={() => setConfigDrawerOpen(false)}>
            close
          </button>
        </div>

        <div className="drawer-body">
          <section className="panel">
            <label>
              mode
              <select
                value={mode}
                onChange={(event) => setMode(event.target.value)}
                disabled={activeChatRuntimeActive}
              >
                <option value="secure">secure</option>
                <option value="standard">standard</option>
                <option value="air-gapped">air-gapped</option>
              </select>
            </label>

            <label className="checkbox">
              <input
                type="checkbox"
                checked={persistent}
                disabled={mode !== "standard" || activeChatRuntimeActive}
                onChange={(event) => setPersistent(event.target.checked)}
              />
              persistent
              <Hint text="Persistent is only available in standard mode. Secure and air-gapped sessions remain ephemeral." />
            </label>

            <label>
              chat template
              <div className="field-with-hint">
                <select
                  value={chatTemplate}
                  onChange={(event) => setChatTemplate(event.target.value as ChatTemplateOption)}
                  disabled={activeChatRuntimeActive}
                >
                  <option value="auto">auto-detect</option>
                  <option value="generic">generic</option>
                  <option value="chatml">chatml</option>
                  <option value="llama3-instruct">llama3-instruct</option>
                </select>
                <Hint text="Auto-detect resolves a template from the configured model path. Override it if a model needs a specific prompt format." />
              </div>
            </label>

            <label>
              context token budget
              <div className="field-with-hint">
                <input
                  type="number"
                  min={1}
                  step={1}
                  value={chatContextTokenBudget}
                  onChange={(event) => setChatContextTokenBudget(event.target.value)}
                  disabled={activeChatRuntimeActive}
                />
                <Hint text="Approximate recent-context budget used when building active chat prompts. Older turns are dropped first when the window overflows." />
              </div>
            </label>

            <label>
              context turn limit
              <div className="field-with-hint">
                <input
                  type="number"
                  min={1}
                  step={1}
                  value={chatContextTurnLimit}
                  onChange={(event) => setChatContextTurnLimit(event.target.value)}
                  disabled={activeChatRuntimeActive}
                />
                <Hint text="Maximum number of recent prior turns that can be included in the active-chat prompt window." />
              </div>
            </label>

            <p className="microcopy">
              Active chat uses the selected template and bounded recent-context window when the
              session starts. Change settings before starting the runtime.
            </p>
          </section>
        </div>
      </aside>
    </main>
  );
}

export default App;
