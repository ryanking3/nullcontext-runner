import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./App.css";

type RunSessionResponse = {
  stdout: string;
  stderr: string;
  success: boolean;
};

type StreamChunk = {
  stream: string;
  chunk: string;
};

type AuditOperation = {
  operation: string;
  status: string;
  details: string;
};

type ParsedRunOutput = {
  lifecycleLogs: string;
  auditOperations: AuditOperation[];
  modelOutput: string;
  privacyReport: string;
  rawOutput: string;
  stderr: string;
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

function parseRunOutput(stdout: string, stderr: string): ParsedRunOutput {
  const modelMarker = "--- Model Output ---";
  const reportMarker = "--- Privacy Report v0 ---";

  const modelIndex = stdout.indexOf(modelMarker);
  const reportIndex = stdout.indexOf(reportMarker);

  let beforeModel = stdout;
  let modelOutput = "";
  let privacyReport = "";

  if (modelIndex >= 0) {
    beforeModel = stdout.slice(0, modelIndex);

    if (reportIndex >= 0) {
      modelOutput = stdout.slice(modelIndex + modelMarker.length, reportIndex).trim();
      privacyReport = stdout.slice(reportIndex + reportMarker.length).trim();
    } else {
      modelOutput = stdout.slice(modelIndex + modelMarker.length).trim();
    }
  }

  if (modelIndex < 0 && reportIndex >= 0) {
    beforeModel = stdout.slice(0, reportIndex);
    privacyReport = stdout.slice(reportIndex + reportMarker.length).trim();
  }

  const allNonReportOutput = reportIndex >= 0 ? stdout.slice(0, reportIndex) : stdout;

  const auditOperations: AuditOperation[] = [];
  const lifecycleLines: string[] = [];

  for (const line of allNonReportOutput.split("\n")) {
    const audit = parseAuditLine(line.trim());

    if (audit) {
      auditOperations.push(audit);
    } else if (!line.includes(modelMarker)) {
      lifecycleLines.push(line);
    }
  }

  return {
    lifecycleLogs: lifecycleLines.join("\n").trim(),
    auditOperations,
    modelOutput,
    privacyReport,
    rawOutput: stdout,
    stderr,
  };
}

function statusClass(status: string) {
  if (status === "successful") {
    return "op-status success";
  }

  if (status === "failed") {
    return "op-status failed";
  }

  if (status === "warning") {
    return "op-status warning";
  }

  return "op-status neutral";
}

function App() {
  const [prompt, setPrompt] = useState("");
  const [mode, setMode] = useState("secure");
  const [persistent, setPersistent] = useState(false);

  const [streamStdout, setStreamStdout] = useState("");
  const [streamStderr, setStreamStderr] = useState("");
  const [runStatus, setRunStatus] = useState<"idle" | "running" | "success" | "failed">("idle");

  const [sessions, setSessions] = useState("");
  const [reportSessionId, setReportSessionId] = useState("");
  const [report, setReport] = useState("");

  const parsedOutput = useMemo(() => {
    if (!streamStdout && !streamStderr) {
      return null;
    }

    return parseRunOutput(streamStdout, streamStderr);
  }, [streamStdout, streamStderr]);

  useEffect(() => {
    const unlistenChunkPromise = listen<StreamChunk>("nullcontext://stream-chunk", (event) => {
      if (event.payload.stream === "stdout") {
        setStreamStdout((current) => `${current}${event.payload.chunk}`);
      } else {
        setStreamStderr((current) => `${current}${event.payload.chunk}`);
      }
    });

    const unlistenErrorPromise = listen<StreamChunk>("nullcontext://stream-error", (event) => {
      setStreamStderr((current) => `${current}${event.payload.chunk}\n`);
      setRunStatus("failed");
    });

    const unlistenCompletePromise = listen<RunSessionResponse>(
      "nullcontext://stream-complete",
      (event) => {
        setRunStatus(event.payload.success ? "success" : "failed");
      }
    );

    return () => {
      unlistenChunkPromise.then((unlisten) => unlisten());
      unlistenErrorPromise.then((unlisten) => unlisten());
      unlistenCompletePromise.then((unlisten) => unlisten());
    };
  }, []);

  async function runSession() {
    setStreamStdout("");
    setStreamStderr("");
    setRunStatus("running");

    try {
      await invoke("run_nullcontext_session_streaming", {
        prompt,
        mode,
        persistent,
      });
    } catch (error) {
      setStreamStderr(String(error));
      setRunStatus("failed");
    }
  }

  async function listSessions() {
    setSessions("");

    try {
      const result = await invoke<RunSessionResponse>("list_nullcontext_sessions");

      setSessions(
        [
          result.success ? "Status: success" : "Status: failed",
          "",
          result.stdout,
          result.stderr,
        ].join("\n")
      );
    } catch (error) {
      setSessions(`Error: ${String(error)}`);
    }
  }

  async function showReport() {
    setReport("");

    try {
      const result = await invoke<RunSessionResponse>("show_nullcontext_report", {
        sessionId: reportSessionId,
      });

      setReport(
        [
          result.success ? "Status: success" : "Status: failed",
          "",
          result.stdout,
          result.stderr,
        ].join("\n")
      );
    } catch (error) {
      setReport(`Error: ${String(error)}`);
    }
  }

  return (
    <main className="app">
      <section className="header">
        <div>
          <h1>NullContext</h1>
          <p>Secure local LLM session shell</p>
        </div>
        <span className="badge">local-only</span>
      </section>

      <section className="card">
        <h2>Run Session</h2>

        <label>
          Prompt
          <textarea
            value={prompt}
            onChange={(event) => setPrompt(event.target.value)}
            placeholder="Enter prompt..."
          />
        </label>

        <div className="row">
          <label>
            Security mode
            <select value={mode} onChange={(event) => setMode(event.target.value)}>
              <option value="standard">standard</option>
              <option value="secure">secure</option>
              <option value="air-gapped">air-gapped</option>
            </select>
          </label>

          <label className="checkbox">
            <input
              type="checkbox"
              checked={persistent}
              onChange={(event) => setPersistent(event.target.checked)}
              disabled={mode !== "standard"}
            />
            Persistent session
          </label>
        </div>

        <button onClick={runSession} disabled={runStatus === "running" || prompt.trim() === ""}>
          {runStatus === "running" ? "Running..." : "Run local session"}
        </button>
      </section>

      {parsedOutput && (
        <section className="result-grid">
          <div className="card">
            <h2>LLM Response</h2>
            <div
              className={
                runStatus === "success"
                  ? "status success"
                  : runStatus === "failed"
                    ? "status failed"
                    : "status running"
              }
            >
              {runStatus}
            </div>
            <pre className="model-output">
              {parsedOutput.modelOutput || "Waiting for model output..."}
            </pre>
          </div>

          <div className="card">
            <h2>Audit Operations</h2>

            {parsedOutput.auditOperations.length === 0 ? (
              <p className="muted">Waiting for audit operations...</p>
            ) : (
              <div className="audit-list">
                {parsedOutput.auditOperations.map((operation, index) => (
                  <div className="audit-item" key={`${operation.operation}-${index}`}>
                    <div className="audit-item-header">
                      <code>{operation.operation}</code>
                      <span className={statusClass(operation.status)}>{operation.status}</span>
                    </div>
                    <p>{operation.details}</p>
                  </div>
                ))}
              </div>
            )}
          </div>

          <div className="card">
            <h2>Runtime / Lifecycle Logs</h2>
            <pre>{parsedOutput.lifecycleLogs || "Waiting for lifecycle logs..."}</pre>
          </div>

          <div className="card">
            <h2>Privacy Report</h2>
            <pre>{parsedOutput.privacyReport || "Waiting for privacy report..."}</pre>
          </div>

          {parsedOutput.stderr.trim() && (
            <div className="card danger">
              <h2>Errors / STDERR</h2>
              <pre>{parsedOutput.stderr}</pre>
            </div>
          )}

          <details className="card">
            <summary>Raw output</summary>
            <pre>{parsedOutput.rawOutput}</pre>
          </details>
        </section>
      )}

      <section className="card">
        <h2>Session Registry</h2>

        <button onClick={listSessions}>List persistent sessions</button>

        {sessions && <pre>{sessions}</pre>}

        <div className="row">
          <label>
            Session ID
            <input
              value={reportSessionId}
              onChange={(event) => setReportSessionId(event.target.value)}
              placeholder="session id"
            />
          </label>

          <button onClick={showReport} disabled={reportSessionId.trim() === ""}>
            Show report
          </button>
        </div>

        {report && <pre>{report}</pre>}
      </section>
    </main>
  );
}

export default App;