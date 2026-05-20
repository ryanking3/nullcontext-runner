import { useEffect, useMemo, useState } from "react";

const API_BASE = "http://127.0.0.1:3333";

type RunResponse = {
  success: boolean;
  stdout: string;
  stderr: string;
};

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

type ParsedOutput = {
  lifecycleLogs: string;
  modelOutput: string;
  privacyReport: string;
  auditOperations: AuditOperation[];
};

function parseAuditLine(line: string): AuditOperation | null {
  if (!line.startsWith("[audit] ")) {
    return null;
  }

  const payload = line.replace("[audit] ", "");
  const parts = payload.split(" | ");

  if (parts.length < 3) {
    return {
      operation: "unknown",
      status: "unknown",
      details: payload,
    };
  }

  return {
    operation: parts[0]?.trim() ?? "unknown",
    status: parts[1]?.trim() ?? "unknown",
    details: parts.slice(2).join(" | ").trim(),
  };
}

function parseRunOutput(stdout: string): ParsedOutput {
  const modelMarker = "--- Model Output ---";
  const reportMarker = "--- Privacy Report v0 ---";

  const modelIndex = stdout.indexOf(modelMarker);
  const reportIndex = stdout.indexOf(reportMarker);

  let lifecycleSection = stdout;
  let modelOutput = "";
  let privacyReport = "";

  if (modelIndex >= 0) {
    lifecycleSection = stdout.slice(0, modelIndex);

    if (reportIndex >= 0) {
      modelOutput = stdout.slice(modelIndex + modelMarker.length, reportIndex).trim();
      privacyReport = stdout.slice(reportIndex + reportMarker.length).trim();
    } else {
      modelOutput = stdout.slice(modelIndex + modelMarker.length).trim();
    }
  }

  if (modelIndex < 0 && reportIndex >= 0) {
    lifecycleSection = stdout.slice(0, reportIndex);
    privacyReport = stdout.slice(reportIndex + reportMarker.length).trim();
  }

  const allBeforeReport = reportIndex >= 0 ? stdout.slice(0, reportIndex) : stdout;
  const auditOperations: AuditOperation[] = [];
  const lifecycleLines: string[] = [];

  for (const line of allBeforeReport.split("\n")) {
    const audit = parseAuditLine(line.trim());

    if (audit) {
      auditOperations.push(audit);
      continue;
    }

    if (!line.includes(modelMarker)) {
      lifecycleLines.push(line);
    }
  }

  return {
    lifecycleLogs: lifecycleLines.join("\n").trim(),
    modelOutput,
    privacyReport,
    auditOperations,
  };
}

function statusClass(status: string): string {
  if (status === "successful") return "pill success";
  if (status === "failed") return "pill failed";
  if (status === "warning") return "pill warning";
  if (status === "not_attempted") return "pill muted";
  return "pill neutral";
}

function App() {
  const [serverStatus, setServerStatus] = useState<"checking" | "online" | "offline">("checking");
  const [prompt, setPrompt] = useState("");
  const [mode, setMode] = useState("secure");
  const [persistent, setPersistent] = useState(false);
  const [isRunning, setIsRunning] = useState(false);
  const [runResponse, setRunResponse] = useState<RunResponse | null>(null);
  const [sessions, setSessions] = useState<SessionIndexEntry[]>([]);
  const [selectedReport, setSelectedReport] = useState<string>("");
  const [selectedSessionId, setSelectedSessionId] = useState<string>("");

  const parsed = useMemo(() => {
    if (!runResponse) return null;
    return parseRunOutput(runResponse.stdout);
  }, [runResponse]);

  async function checkHealth() {
    try {
      const response = await fetch(`${API_BASE}/api/health`);
      setServerStatus(response.ok ? "online" : "offline");
    } catch {
      setServerStatus("offline");
    }
  }

  async function loadSessions() {
    try {
      const response = await fetch(`${API_BASE}/api/sessions`);
      const data = (await response.json()) as SessionRegistry;
      setSessions(data.sessions ?? []);
    } catch {
      setSessions([]);
    }
  }

  async function runSession() {
    setIsRunning(true);
    setRunResponse(null);
    setSelectedReport("");
    setSelectedSessionId("");

    try {
      const response = await fetch(`${API_BASE}/api/run`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          prompt,
          mode,
          persistent,
        }),
      });

      const data = (await response.json()) as RunResponse;
      setRunResponse(data);

      if (persistent) {
        await loadSessions();
      }
    } catch (error) {
      setRunResponse({
        success: false,
        stdout: "",
        stderr: String(error),
      });
    } finally {
      setIsRunning(false);
    }
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
            <p>local secure inference</p>
          </div>
        </div>

        <div className="server-card">
          <span className={`server-dot ${serverStatus}`} />
          <span>server: {serverStatus}</span>
          <button onClick={checkHealth}>check</button>
        </div>

        <section className="panel">
          <h2>Session</h2>

          <label>
            Security mode
            <select value={mode} onChange={(event) => setMode(event.target.value)}>
              <option value="secure">secure</option>
              <option value="standard">standard</option>
              <option value="air-gapped">air-gapped</option>
            </select>
          </label>

          <label className="checkbox">
            <input
              type="checkbox"
              checked={persistent}
              disabled={mode !== "standard"}
              onChange={(event) => setPersistent(event.target.checked)}
            />
            persistent
          </label>

          <div className="hint">
            Secure and air-gapped sessions are ephemeral. Standard mode can retain reports and
            workspace artifacts.
          </div>
        </section>

        <section className="panel">
          <div className="panel-header">
            <h2>Registry</h2>
            <button onClick={loadSessions}>refresh</button>
          </div>

          <div className="session-list">
            {sessions.length === 0 ? (
              <p className="muted-text">No persistent sessions.</p>
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
                  <span>{session.session_id.slice(0, 8)}</span>
                  <small>{session.security_mode}</small>
                  <small>{new Date(session.started_at).toLocaleString()}</small>
                </button>
              ))
            )}
          </div>
        </section>
      </aside>

      <section className="main-column">
        <header className="topbar">
          <div>
            <h2>Chat</h2>
            <p>localhost-only web UI for NullContext sessions</p>
          </div>

          <div className="topbar-actions">
            <span className="badge">GGUF</span>
            <span className="badge">llama.cpp</span>
            <span className="badge">offline-capable</span>
          </div>
        </header>

        <section className="chat-card">
          <div className="messages">
            {!runResponse && (
              <div className="empty-state">
                <h3>Start a local session</h3>
                <p>
                  Enter a prompt below. NullContext will run local inference, track artifacts,
                  emit audit operations, and generate a privacy report.
                </p>
              </div>
            )}

            {runResponse && (
              <>
                <div className="message user">
                  <div className="role">user</div>
                  <div className="bubble">{prompt}</div>
                </div>

                <div className="message assistant">
                  <div className="role">assistant</div>
                  <div className="bubble">
                    {parsed?.modelOutput || "No model output captured."}
                  </div>
                </div>
              </>
            )}
          </div>

          <div className="composer">
            <textarea
              value={prompt}
              onChange={(event) => setPrompt(event.target.value)}
              placeholder="Message NullContext..."
            />

            <button onClick={runSession} disabled={isRunning || prompt.trim() === ""}>
              {isRunning ? "running..." : "send"}
            </button>
          </div>
        </section>
      </section>

      <aside className="inspector">
        <section className="panel">
          <h2>Audit operations</h2>

          {!parsed || parsed.auditOperations.length === 0 ? (
            <p className="muted-text">Audit operations appear after a run.</p>
          ) : (
            <div className="audit-list">
              {parsed.auditOperations.map((operation, index) => (
                <div className="audit-item" key={`${operation.operation}-${index}`}>
                  <div className="audit-row">
                    <code>{operation.operation}</code>
                    <span className={statusClass(operation.status)}>{operation.status}</span>
                  </div>
                  <p>{operation.details}</p>
                </div>
              ))}
            </div>
          )}
        </section>

        <section className="panel">
          <h2>Runtime logs</h2>
          <pre>{parsed?.lifecycleLogs || "No runtime logs yet."}</pre>
        </section>

        <section className="panel">
          <h2>Privacy report</h2>
          <pre>{selectedReport || parsed?.privacyReport || "No report selected."}</pre>
        </section>

        {runResponse?.stderr && (
          <section className="panel danger">
            <h2>stderr</h2>
            <pre>{runResponse.stderr}</pre>
          </section>
        )}
      </aside>
    </main>
  );
}

export default App;