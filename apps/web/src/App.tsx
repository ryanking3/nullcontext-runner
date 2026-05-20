import { useEffect, useMemo, useState } from "react";
import "./App.css";

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

type Theme = "dark" | "light";

function stripAuditLines(text: string): string {
  return text
    .split("\n")
    .filter((line) => !line.trim().startsWith("[audit] "))
    .join("\n")
    .trim();
}

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

function extractAuditOperationsFromReport(report: string): AuditOperation[] {
  if (!report.trim()) {
    return [];
  }

  try {
    const parsed = JSON.parse(report);
    const operations = parsed?.cleanup?.sanitization_operations;

    if (!Array.isArray(operations)) {
      return [];
    }

    return operations
      .filter((operation) => {
        return (
          typeof operation.operation === "string" &&
          typeof operation.status === "string" &&
          typeof operation.details === "string"
        );
      })
      .map((operation) => ({
        operation: operation.operation,
        status: operation.status,
        details: operation.details,
      }));
  } catch {
    return [];
  }
}

function firstExistingIndex(source: string, markers: string[], start: number): number {
  const indexes = markers
    .map((marker) => source.indexOf(marker, start))
    .filter((index) => index >= 0);

  if (indexes.length === 0) {
    return -1;
  }

  return Math.min(...indexes);
}

function parseRunOutput(stdout: string): ParsedOutput {
  const modelMarker = "--- Model Output ---";
  const reportMarker = "--- Privacy Report v0 ---";

  const cleanupMarkers = [
    "\nSanitizing Rust-owned buffers...",
    "\nSession mode:",
    "\n[audit] prompt_buffer_ram_zeroization_verification",
    "\n[audit] response_buffer_ram_zeroization_verification",
    "\n[audit] explicit_sensitive_byte_buffer_zeroization",
  ];

  const modelIndex = stdout.indexOf(modelMarker);
  const reportIndex = stdout.indexOf(reportMarker);

  let modelOutput = "";
  let privacyReport = "";

  if (reportIndex >= 0) {
    privacyReport = stdout.slice(reportIndex + reportMarker.length).trim();
  }

  let lifecycleSource = reportIndex >= 0 ? stdout.slice(0, reportIndex) : stdout;

  if (modelIndex >= 0) {
    const modelStart = modelIndex + modelMarker.length;
    const modelEnd = firstExistingIndex(stdout, cleanupMarkers, modelStart);

    if (modelEnd >= 0) {
      modelOutput = stripAuditLines(stdout.slice(modelStart, modelEnd));
      lifecycleSource = [
        stdout.slice(0, modelIndex),
        `${modelMarker}\n<RESPONSE>`,
        stdout.slice(modelEnd, reportIndex >= 0 ? reportIndex : stdout.length),
      ].join("\n");
    } else if (reportIndex >= 0) {
      modelOutput = stripAuditLines(stdout.slice(modelStart, reportIndex));
      lifecycleSource = [
        stdout.slice(0, modelIndex),
        `${modelMarker}\n<RESPONSE>`,
      ].join("\n");
    } else {
      modelOutput = stripAuditLines(stdout.slice(modelStart));
      lifecycleSource = [
        stdout.slice(0, modelIndex),
        `${modelMarker}\n<RESPONSE>`,
      ].join("\n");
    }
  }

  const auditOperations: AuditOperation[] = [];
  const lifecycleLines: string[] = [];

  for (const line of lifecycleSource.split("\n")) {
    const trimmed = line.trim();
    const audit = parseAuditLine(trimmed);

    if (audit) {
      auditOperations.push(audit);
      continue;
    }

    lifecycleLines.push(line);
  }

  const reportAuditOperations = extractAuditOperationsFromReport(privacyReport);

  return {
    lifecycleLogs: lifecycleLines.join("\n").trim(),
    modelOutput,
    privacyReport,
    auditOperations:
      reportAuditOperations.length > 0 ? reportAuditOperations : auditOperations,
  };
}

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

function App() {
  const [theme, setTheme] = useState<Theme>("dark");
  const [serverStatus, setServerStatus] = useState<"checking" | "online" | "offline">("checking");
  const [healthCheckedAt, setHealthCheckedAt] = useState<string>("never");
  const [registryLoadedAt, setRegistryLoadedAt] = useState<string>("never");

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
          <div className="panel-title">session</div>

          <label>
            mode
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
            <p>local inference with lifecycle visibility</p>
          </div>
        </header>

        <section className="chat-card">
          <div className="messages">
            {!runResponse && (
              <div className="empty-state">
                <h3>ready</h3>
                <p>
                  enter a prompt. NullContext will run local inference, scan artifacts, emit audit
                  operations, and generate a privacy report.
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
                    {parsed?.modelOutput || "no model output captured"}
                  </div>
                </div>
              </>
            )}
          </div>

          <div className="composer">
            <textarea
              value={prompt}
              onChange={(event) => setPrompt(event.target.value)}
              placeholder="message nullcontext..."
            />

            <button onClick={runSession} disabled={isRunning || prompt.trim() === ""}>
              {isRunning ? "running" : "send"}
            </button>
          </div>
        </section>
      </section>

      <aside className="inspector">
        <details className="panel" open>
          <summary>audit operations ({parsed?.auditOperations.length ?? 0})</summary>

          {!parsed || parsed.auditOperations.length === 0 ? (
            <p className="muted-text">audit operations appear after a run</p>
          ) : (
            <div className="audit-list">
              {parsed.auditOperations.map((operation, index) => (
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
          <pre>{parsed?.lifecycleLogs || "no runtime logs yet"}</pre>
        </details>

        <details className="panel" open>
          <summary>privacy report</summary>
          <pre>{selectedReport || parsed?.privacyReport || "no report selected"}</pre>
        </details>

        {runResponse?.stderr && (
          <details className="panel danger" open>
            <summary>stderr</summary>
            <pre>{runResponse.stderr}</pre>
          </details>
        )}
      </aside>
    </main>
  );
}

export default App;