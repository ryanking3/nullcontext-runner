import { useEffect, useState } from "react";
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
};

type ChatStatusResponse = {
  session_id: string;
  workspace: string;
  security_mode: string;
  persistent: boolean;
  runtime_active: boolean;
  turns: number;
};

type ChatEndResponse = {
  session_id: string;
  runtime_stopped: boolean;
  report: unknown;
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
  const [theme, setTheme] = useState<Theme>("dark");
  const [serverStatus, setServerStatus] = useState<"checking" | "online" | "offline">("checking");
  const [healthCheckedAt, setHealthCheckedAt] = useState<string>("never");
  const [registryLoadedAt, setRegistryLoadedAt] = useState<string>("never");

  const [runtimeMode, setRuntimeMode] = useState<RuntimeMode>("one-shot");
  const [prompt, setPrompt] = useState("");
  const [mode, setMode] = useState("secure");
  const [persistent, setPersistent] = useState(false);
  const [runStatus, setRunStatus] = useState<RunStatus>("idle");

  const [activeChatSessionId, setActiveChatSessionId] = useState("");
  const [activeChatWorkspace, setActiveChatWorkspace] = useState("");
  const [activeChatTurns, setActiveChatTurns] = useState(0);
  const [activeChatRuntimeActive, setActiveChatRuntimeActive] = useState(false);

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

  async function consumeSseResponse(response: Response) {
    if (!response.body) {
      throw new Error("Streaming response body was empty");
    }

    const reader = response.body.getReader();
    const decoder = new TextDecoder();

    let buffer = "";

    while (true) {
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

    if (buffer.trim()) {
      const payload = parseSseBlock(buffer);

      if (payload) {
        handleStreamPayload(payload);
      }
    }
  }

  async function runOneShot() {
    resetOneShotConversation();
    setRunStatus("running");

    const currentPrompt = prompt;

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
      const response = await fetch(`${API_BASE}/api/run/stream`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          prompt: currentPrompt,
          mode,
          persistent,
        }),
      });

      await consumeSseResponse(response);
    } catch (error) {
      setStderr(String(error));
      setRunStatus("failed");
    }
  }

  async function startActiveChat() {
    resetRunPanels();
    setRunStatus("running");

    try {
      const response = await fetch(`${API_BASE}/api/chat/start`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          mode,
          persistent,
        }),
      });

      if (!response.ok) {
        const error = await response.text();
        throw new Error(error);
      }

      const data = (await response.json()) as ChatStartResponse;

      setActiveChatSessionId(data.session_id);
      setActiveChatWorkspace(data.workspace);
      setActiveChatTurns(data.turns);
      setActiveChatRuntimeActive(data.runtime_active);
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
    } catch {
      // Non-critical UI refresh failure.
    }
  }

  async function endActiveChat() {
    if (!activeChatSessionId) {
      return;
    }

    setRunStatus("running");

    try {
      const response = await fetch(`${API_BASE}/api/chat/${activeChatSessionId}/end`, {
        method: "POST",
      });

      if (!response.ok) {
        const error = await response.text();
        throw new Error(error);
      }

      const data = (await response.json()) as ChatEndResponse;

      setActiveChatRuntimeActive(false);
      setActiveChatTurns(0);
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

    const currentPrompt = prompt;

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
        }
      );

      await consumeSseResponse(response);
      setPrompt("");
    } catch (error) {
      setStderr(String(error));
      setRunStatus("failed");
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

  return (
    <main className="shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="logo">NC</div>
          <div>
            <h1>NullContext</h1>
            <p>localhost runtime</p>
          </div>
        </div>

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
          <div className="panel-title">session</div>

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
          </label>

          {runtimeMode === "active-chat" && (
            <div className="active-chat-controls">
              {!activeChatRuntimeActive ? (
                <button onClick={startActiveChat} disabled={runStatus === "running"}>
                  start session
                </button>
              ) : (
                <button onClick={endActiveChat} disabled={runStatus === "running"}>
                  end + sanitize
                </button>
              )}

              {activeChatSessionId && (
                <div className="session-meta">
                  <div>id: {shortId(activeChatSessionId)}</div>
                  <div>runtime: {activeChatRuntimeActive ? "active" : "stopped"}</div>
                  <div>turns: {activeChatTurns}</div>
                  <div className="truncate">workspace: {activeChatWorkspace}</div>
                </div>
              )}
            </div>
          )}

          <p className="microcopy">
            secure and air-gapped sessions are ephemeral. standard can retain workspace artifacts.
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
                  onClick={() => openReport(session.session_id)}
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
        </header>

        <section className="chat-card">
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

            <button
              onClick={runSession}
              disabled={
                runStatus === "running" ||
                prompt.trim() === "" ||
                (runtimeMode === "active-chat" && !activeChatRuntimeActive)
              }
            >
              {runStatus === "running" ? "running" : "send"}
            </button>
          </div>
        </section>
      </section>

      <aside className="inspector">
        <details className="panel" open>
          <summary>audit operations ({auditOperations.length})</summary>

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
        </details>

        <details className="panel" open>
          <summary>runtime logs</summary>
          <pre>{runtimeLogs || "no runtime logs yet"}</pre>
        </details>

        <details className="panel" open>
          <summary>privacy report</summary>
          <pre>{selectedReport || privacyReport || "no report selected"}</pre>
        </details>

        {stderr && (
          <details className="panel danger" open>
            <summary>stderr</summary>
            <pre>{stderr}</pre>
          </details>
        )}
      </aside>
    </main>
  );
}

export default App;