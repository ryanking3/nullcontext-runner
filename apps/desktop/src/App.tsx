import { useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

type RunSessionResponse = {
  stdout: string;
  stderr: string;
  success: boolean;
};

type ParsedRunOutput = {
  lifecycleLogs: string;
  modelOutput: string;
  privacyReport: string;
  rawOutput: string;
  stderr: string;
};

function parseRunOutput(stdout: string, stderr: string): ParsedRunOutput {
  const modelMarker = "--- Model Output ---";
  const reportMarker = "--- Privacy Report v0 ---";

  const modelIndex = stdout.indexOf(modelMarker);
  const reportIndex = stdout.indexOf(reportMarker);

  let lifecycleLogs = stdout;
  let modelOutput = "";
  let privacyReport = "";

  if (modelIndex >= 0) {
    lifecycleLogs = stdout.slice(0, modelIndex).trim();

    if (reportIndex >= 0) {
      modelOutput = stdout
        .slice(modelIndex + modelMarker.length, reportIndex)
        .trim();

      privacyReport = stdout
        .slice(reportIndex + reportMarker.length)
        .trim();
    } else {
      modelOutput = stdout.slice(modelIndex + modelMarker.length).trim();
    }
  }

  if (modelIndex < 0 && reportIndex >= 0) {
    lifecycleLogs = stdout.slice(0, reportIndex).trim();
    privacyReport = stdout.slice(reportIndex + reportMarker.length).trim();
  }

  return {
    lifecycleLogs,
    modelOutput,
    privacyReport,
    rawOutput: stdout,
    stderr,
  };
}

function App() {
  const [prompt, setPrompt] = useState("");
  const [mode, setMode] = useState("secure");
  const [persistent, setPersistent] = useState(false);
  const [runResult, setRunResult] = useState<RunSessionResponse | null>(null);
  const [sessions, setSessions] = useState("");
  const [reportSessionId, setReportSessionId] = useState("");
  const [report, setReport] = useState("");
  const [isRunning, setIsRunning] = useState(false);

  const parsedOutput = useMemo(() => {
    if (!runResult) {
      return null;
    }

    return parseRunOutput(runResult.stdout, runResult.stderr);
  }, [runResult]);

  async function runSession() {
    setIsRunning(true);
    setRunResult(null);

    try {
      const result = await invoke<RunSessionResponse>("run_nullcontext_session", {
        prompt,
        mode,
        persistent,
      });

      setRunResult(result);
    } catch (error) {
      setRunResult({
        stdout: "",
        stderr: String(error),
        success: false,
      });
    } finally {
      setIsRunning(false);
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

        <button onClick={runSession} disabled={isRunning || prompt.trim() === ""}>
          {isRunning ? "Running..." : "Run local session"}
        </button>
      </section>

      {runResult && parsedOutput && (
        <section className="result-grid">
          <div className="card">
            <h2>LLM Response</h2>
            <div className={runResult.success ? "status success" : "status failed"}>
              {runResult.success ? "success" : "failed"}
            </div>
            <pre className="model-output">
              {parsedOutput.modelOutput || "No model output captured."}
            </pre>
          </div>

          <div className="card">
            <h2>Runtime / Lifecycle Logs</h2>
            <pre>{parsedOutput.lifecycleLogs || "No lifecycle logs captured."}</pre>
          </div>

          <div className="card">
            <h2>Privacy Report</h2>
            <pre>{parsedOutput.privacyReport || "No privacy report captured."}</pre>
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