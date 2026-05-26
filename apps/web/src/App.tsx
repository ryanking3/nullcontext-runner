import { useEffect, useRef, useState } from "react";
import "./App.css";

const API_BASE = "http://127.0.0.1:3333";

type SessionRegistry = {
  sessions: SessionIndexEntry[];
};

type RegisteredModel = {
  id: string;
  name: string;
  description?: string | null;
  model_path: string;
  max_tokens: number;
  gpu_layers: number;
  chat_template: string;
  chat_context_token_budget: number;
  chat_context_turn_limit: number;
  selectable: boolean;
  validation_status: string;
  validation_message?: string | null;
  default_selected: boolean;
};

type ModelRegistrySnapshot = {
  default_model_id: string;
  runtime: {
    llama_path: string;
    selectable: boolean;
    validation_status: string;
    validation_message?: string | null;
  };
  models: RegisteredModel[];
};

type CorpusRegistrySnapshot = {
  corpora: CorpusIndexEntry[];
};

type CorpusLifecycleMetadata = {
  state: string;
  retention_policy: string;
  retention_deadline?: string | null;
  cleanup_requested_at?: string | null;
  cleanup_completed_at?: string | null;
  cleanup_reason?: string | null;
  updated_at?: string | null;
};

type CorpusIndexEntry = {
  corpus_id: string;
  name: string;
  created_at: string;
  persistent: boolean;
  root_path: string;
  manifest_path: string;
  report_path: string;
  source_count: number;
  chunk_count: number;
  embedding_backend?: string | null;
  embedding_model?: string | null;
  ocr_backend?: string | null;
  lifecycle: CorpusLifecycleMetadata;
};

type SessionLifecycleMetadata = {
  state: string;
  retention_policy: string;
  retention_deadline?: string | null;
  cleanup_requested_at?: string | null;
  cleanup_completed_at?: string | null;
  cleanup_reason?: string | null;
  updated_at?: string | null;
};

type SessionIndexEntry = {
  session_id: string;
  started_at: string;
  security_mode: string;
  prompt_source: string;
  history_stored: boolean;
  backend: string;
  model_id?: string;
  model_name?: string;
  model_path: string;
  workspace: string;
  report_path: string;
  artifacts_detected: number;
  cleanup_attempted: boolean;
  cleanup_successful: boolean;
  workspace_deleted: boolean;
  lifecycle: SessionLifecycleMetadata;
};

type AuditOperation = {
  operation: string;
  status: string;
  details: string;
};

type ArtifactRecord = {
  path: string;
  kind: string;
  size_bytes: number;
};

type CleanupInfo = {
  attempted: boolean;
  successful: boolean;
  workspace_deleted: boolean;
  files_removed: number;
  directories_removed: number;
  artifacts_detected: ArtifactRecord[];
  sanitization_operations: AuditOperation[];
  error?: string | null;
};

type TurnArtifact = {
  turn: number;
  prompt_path: string;
  response_path: string;
};

type SessionProfile = {
  session_kind: string;
  runtime_lifetime: string;
  turn_count: number;
  runtime_duration_ms: number;
  history_policy: string;
  persistence_policy: string;
  prompt_source: string;
  turn_artifacts: TurnArtifact[];
  active_runtime_residual_risk: string;
  grounding_scope?: string | null;
  bound_corpus_id?: string | null;
  bound_corpus_name?: string | null;
  grounded_turn_count?: number;
};

type LifecycleReport = {
  state: string;
  retention_policy: string;
  retention_deadline?: string | null;
  cleanup_requested_at?: string | null;
  cleanup_completed_at?: string | null;
  cleanup_reason?: string | null;
  updated_at?: string | null;
  policy_summary: string;
  decision_summary: string;
};

type RetrievalReportData = {
  corpus_id: string;
  corpus_name: string;
  retrieval_mode: string;
  query: string;
  top_k: number;
  grounded_turns: number;
  retrieved_chunks: number;
  source_paths: string[];
  page_hits: string[];
  context_injected: boolean;
};

type LlamaMemoryDomainReport = {
  domain: string;
  exposure_scope: string;
  cleanup_status: string;
  notes: string;
};

type LlamaRuntimeReportData = {
  runtime_kind: string;
  runtime_pid?: number | null;
  model_id: string;
  model_name: string;
  model_path: string;
  gpu_layers_requested: number;
  gpu_offload_requested: boolean;
  shutdown_method: string;
  process_exit_code?: number | null;
  graceful_shutdown_supported: boolean;
  observed_resident_bytes?: number | null;
  observed_virtual_bytes?: number | null;
  process_memory_source?: string | null;
  observed_gpu_memory_bytes?: number | null;
  gpu_memory_source?: string | null;
  observation_notes: string[];
  cleanup_summary: string;
  residual_risk_summary: string;
  memory_domains: LlamaMemoryDomainReport[];
};

type PrivacyReportData = {
  session_id: string;
  started_at: string;
  history_stored: boolean;
  backend: string;
  security_mode: string;
  gpu_layers: string;
  process_exited_cleanly: boolean;
  cleanup: CleanupInfo;
  session_profile?: SessionProfile | null;
  lifecycle?: LifecycleReport | null;
  llama_runtime?: LlamaRuntimeReportData | null;
  retrieval?: RetrievalReportData | null;
  residual_risk: string;
};

type Theme = "dark" | "light";
type RunStatus = "idle" | "running" | "success" | "failed";
type RuntimeMode = "one-shot" | "active-chat";
type ChatTemplateOption = "auto" | "generic" | "chatml" | "llama3-instruct";
type InspectorView = "audit" | "runtime" | "report" | "stderr";
type RegistryModeFilter = "all" | "secure" | "standard" | "air-gapped";
type RegistryOutcomeFilter =
  | "all"
  | "cleanup-failed"
  | "workspace-retained"
  | "artifacts"
  | "history-stored";
type RegistrySortOrder = "newest" | "oldest";

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
  model_id: string;
  model_name: string;
  corpus_id?: string | null;
  corpus_name?: string | null;
  runtime_active: boolean;
  turns: number;
  grounded_turns: number;
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
  model_id?: string;
  model_name?: string;
  corpus_id?: string | null;
  corpus_name?: string | null;
  runtime_active: boolean;
  turns: number;
  grounded_turns?: number;
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

type ChatCancelResponse = {
  session_id: string;
  generation_active: boolean;
  cancel_requested: boolean;
  message: string;
};

type SessionLifecycleActionResponse = {
  session_id: string;
  lifecycle_state: string;
  retention_policy: string;
  retention_deadline?: string | null;
  cleanup_reason?: string | null;
  cleanup_attempted: boolean;
  cleanup_successful: boolean;
  workspace_deleted: boolean;
  workspace_exists: boolean;
  report_exists: boolean;
  workspace: string;
  report_path: string;
  message: string;
};

type ApiErrorResponse = {
  error?: string;
};

type CorpusIngestionReport = {
  corpus_id: string;
  created_at: string;
  persistent: boolean;
  source_paths_requested: string[];
  files_discovered: number;
  files_ingested: number;
  files_failed: number;
  pdf_pages_seen: number;
  pdf_pages_ocrd: number;
  chunk_count: number;
  ocr_enabled: boolean;
  warnings: string[];
  upload_staging?: {
    staging_root: string;
    staged_files: number;
    staged_bytes: number;
    source_filenames: string[];
    cleaned_up: boolean;
    cleanup_error?: string | null;
  } | null;
  residual_risk: string;
};

type IngestCorpusResponse = {
  corpus: CorpusIndexEntry;
  report: CorpusIngestionReport;
};

type CorpusLifecycleActionResponse = {
  corpus_id: string;
  lifecycle_state: string;
  retention_policy: string;
  retention_deadline?: string | null;
  cleanup_reason?: string | null;
  root_exists: boolean;
  report_exists: boolean;
  root_path: string;
  manifest_path: string;
  report_path: string;
  message: string;
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

function formatTimestamp(value: string): string {
  const parsed = new Date(value);

  if (Number.isNaN(parsed.getTime())) {
    return value;
  }

  return parsed.toLocaleString();
}

function formatBoolean(value: boolean): string {
  return value ? "yes" : "no";
}

function formatBytes(value: number): string {
  if (!Number.isFinite(value)) {
    return String(value);
  }

  if (value < 1024) {
    return `${value} B`;
  }

  if (value < 1024 * 1024) {
    return `${(value / 1024).toFixed(1)} KB`;
  }

  return `${(value / (1024 * 1024)).toFixed(1)} MB`;
}

function minutesUntil(timestamp: string | null | undefined): string {
  if (!timestamp) {
    return "60";
  }

  const parsed = new Date(timestamp);

  if (Number.isNaN(parsed.getTime())) {
    return "60";
  }

  const minutes = Math.max(1, Math.ceil((parsed.getTime() - Date.now()) / 60000));
  return String(minutes);
}

function humanizeSnakeCase(value: string): string {
  return value.replaceAll("_", " ");
}

function lifecycleStateClass(state: string): string {
  if (state === "cleanup_succeeded") return "pill success";
  if (state === "cleanup_failed") return "pill failed";
  if (state === "cleanup_pending") return "pill warning";
  if (state === "orphaned") return "pill warning";
  if (state === "active") return "pill warning";
  return "pill neutral";
}

function modelValidationClass(status: string): string {
  if (status === "ready") return "pill success";
  if (status === "missing" || status === "not_file") return "pill failed";
  return "pill warning";
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

function formatActiveChatApiError(
  action: "start" | "message" | "cancel" | "end",
  message: string
): string {
  if (message.includes("generation is still in progress")) {
    return "The active chat runtime is still finishing the current generation. Wait for streaming to settle, or use Stop and then retry End + Sanitize once the session is idle.";
  }

  if (message.includes("already generating")) {
    return "An active chat generation is already in progress for this session. Wait for it to finish before sending another message.";
  }

  if (message.includes("session is ending")) {
    return "This active chat session is already ending. Wait for End + Sanitize to complete before sending another message.";
  }

  if (message.includes("No active chat generation is currently running")) {
    return "There is no active chat generation running right now, so there was nothing to cancel.";
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

function parsePrivacyReport(raw: string): PrivacyReportData | null {
  if (!raw.trim()) {
    return null;
  }

  try {
    return JSON.parse(raw) as PrivacyReportData;
  } catch {
    return null;
  }
}

function parseCorpusReport(raw: string): CorpusIngestionReport | null {
  if (!raw.trim()) {
    return null;
  }

  try {
    return JSON.parse(raw) as CorpusIngestionReport;
  } catch {
    return null;
  }
}

function ReportGrid({
  entries,
}: {
  entries: Array<{ label: string; value: string }>;
}) {
  return (
    <dl className="report-grid">
      {entries.map((entry) => (
        <div className="report-grid-row" key={entry.label}>
          <dt>{entry.label}</dt>
          <dd>{entry.value}</dd>
        </div>
      ))}
    </dl>
  );
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
  const [modelDrawerOpen, setModelDrawerOpen] = useState(false);
  const [registryDrawerOpen, setRegistryDrawerOpen] = useState(false);
  const [corpusDrawerOpen, setCorpusDrawerOpen] = useState(false);
  const [inspectorView, setInspectorView] = useState<InspectorView>("audit");
  const [commandMenuOpen, setCommandMenuOpen] = useState(false);
  const [chatTemplate, setChatTemplate] = useState<ChatTemplateOption>("auto");
  const [chatContextTokenBudget, setChatContextTokenBudget] = useState("2048");
  const [chatContextTurnLimit, setChatContextTurnLimit] = useState("12");
  const [useModelTemplateDefault, setUseModelTemplateDefault] = useState(true);
  const [useModelContextDefaults, setUseModelContextDefaults] = useState(true);
  const [models, setModels] = useState<RegisteredModel[]>([]);
  const [runtimeValidation, setRuntimeValidation] =
    useState<ModelRegistrySnapshot["runtime"] | null>(null);
  const [selectedModelId, setSelectedModelId] = useState("");
  const [inspectedModelId, setInspectedModelId] = useState("");
  const [modelsLoadedAt, setModelsLoadedAt] = useState("never");
  const [modelLoadError, setModelLoadError] = useState("");
  const [modelQuery, setModelQuery] = useState("");
  const [corpora, setCorpora] = useState<CorpusIndexEntry[]>([]);
  const [selectedCorpusId, setSelectedCorpusId] = useState("");
  const [corporaLoadedAt, setCorporaLoadedAt] = useState("never");
  const [corpusLoadError, setCorpusLoadError] = useState("");
  const [corpusQuery, setCorpusQuery] = useState("");
  const [corpusIngestName, setCorpusIngestName] = useState("");
  const [corpusIngestPaths, setCorpusIngestPaths] = useState("");
  const [corpusIngestPersistent, setCorpusIngestPersistent] = useState(true);
  const [corpusIngestOcrEnabled, setCorpusIngestOcrEnabled] = useState(true);
  const [corpusIngestPending, setCorpusIngestPending] = useState(false);
  const [corpusIngestMessage, setCorpusIngestMessage] = useState("");
  const [corpusIngestFailed, setCorpusIngestFailed] = useState(false);
  const [corpusUploadFiles, setCorpusUploadFiles] = useState<File[]>([]);
  const [corpusUploadInputKey, setCorpusUploadInputKey] = useState(0);
  const [corpusUploadProgressPercent, setCorpusUploadProgressPercent] = useState<number | null>(
    null
  );
  const [corpusUploadProgressLabel, setCorpusUploadProgressLabel] = useState("");
  const [corpusUploadDragActive, setCorpusUploadDragActive] = useState(false);
  const [lastIngestedCorpusReport, setLastIngestedCorpusReport] =
    useState<CorpusIngestionReport | null>(null);
  const [corpusActionPending, setCorpusActionPending] = useState<string | null>(null);
  const [corpusActionMessage, setCorpusActionMessage] = useState("");
  const [corpusActionFailed, setCorpusActionFailed] = useState(false);
  const [corpusActionResult, setCorpusActionResult] =
    useState<CorpusLifecycleActionResponse | null>(null);
  const [corpusRetentionPolicyDraft, setCorpusRetentionPolicyDraft] =
    useState("retain_until_manual_cleanup");
  const [corpusRetentionMinutesDraft, setCorpusRetentionMinutesDraft] = useState("60");
  const [selectedCorpusReport, setSelectedCorpusReport] = useState("");

  const [activeChatSessionId, setActiveChatSessionId] = useState("");
  const [activeChatWorkspace, setActiveChatWorkspace] = useState("");
  const [activeChatModelId, setActiveChatModelId] = useState("");
  const [activeChatModelName, setActiveChatModelName] = useState("");
  const [activeChatCorpusId, setActiveChatCorpusId] = useState("");
  const [activeChatCorpusName, setActiveChatCorpusName] = useState("");
  const [activeChatTurns, setActiveChatTurns] = useState(0);
  const [activeChatGroundedTurns, setActiveChatGroundedTurns] = useState(0);
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
  const [showRawReport, setShowRawReport] = useState(false);
  const [registryActionPending, setRegistryActionPending] = useState<string | null>(null);
  const [registryActionMessage, setRegistryActionMessage] = useState("");
  const [registryActionFailed, setRegistryActionFailed] = useState(false);
  const [registryActionResult, setRegistryActionResult] =
    useState<SessionLifecycleActionResponse | null>(null);
  const [retentionPolicyDraft, setRetentionPolicyDraft] = useState("retain_until_manual_cleanup");
  const [retentionMinutesDraft, setRetentionMinutesDraft] = useState("60");
  const [activeChatCancelPending, setActiveChatCancelPending] = useState(false);
  const [registryQuery, setRegistryQuery] = useState("");
  const [registryModeFilter, setRegistryModeFilter] = useState<RegistryModeFilter>("all");
  const [registryOutcomeFilter, setRegistryOutcomeFilter] =
    useState<RegistryOutcomeFilter>("all");
  const [registrySortOrder, setRegistrySortOrder] = useState<RegistrySortOrder>("newest");

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
      const nextSessions = data.sessions ?? [];
      setSessions(nextSessions);
      setSelectedSessionId((current) => {
        if (current && nextSessions.some((session) => session.session_id === current)) {
          return current;
        }

        return nextSessions[0]?.session_id ?? "";
      });
    } catch {
      setSessions([]);
      setSelectedSessionId("");
    } finally {
      setRegistryLoadedAt(new Date().toLocaleTimeString());
    }
  }

  async function loadModels() {
    try {
      const response = await fetch(`${API_BASE}/api/models`);

      if (!response.ok) {
        const error = await readApiError(response, "Failed to load model registry.");
        throw new Error(error);
      }

      const data = (await response.json()) as ModelRegistrySnapshot;
      const nextModels = data.models ?? [];

      setModels(nextModels);
      setRuntimeValidation(data.runtime ?? null);
      setModelLoadError("");
      setSelectedModelId((current) => {
        if (current && nextModels.some((model) => model.id === current)) {
          return current;
        }

        if (nextModels.some((model) => model.id === data.default_model_id)) {
          return data.default_model_id;
        }

        return nextModels[0]?.id ?? "";
      });
      setInspectedModelId((current) => {
        if (current && nextModels.some((model) => model.id === current)) {
          return current;
        }

        if (nextModels.some((model) => model.id === data.default_model_id)) {
          return data.default_model_id;
        }

        return nextModels[0]?.id ?? "";
      });
    } catch (error) {
      setModels([]);
      setRuntimeValidation(null);
      setModelLoadError(String(error));
      setSelectedModelId("");
      setInspectedModelId("");
    } finally {
      setModelsLoadedAt(new Date().toLocaleTimeString());
    }
  }

  async function loadCorpora() {
    try {
      const response = await fetch(`${API_BASE}/api/corpora`);

      if (!response.ok) {
        const error = await readApiError(response, "Failed to load corpus registry.");
        throw new Error(error);
      }

      const data = (await response.json()) as CorpusRegistrySnapshot;
      const nextCorpora = data.corpora ?? [];

      setCorpora(nextCorpora);
      setCorpusLoadError("");
      setSelectedCorpusId((current) => {
        if (current && nextCorpora.some((corpus) => corpus.corpus_id === current)) {
          return current;
        }

        return nextCorpora[0]?.corpus_id ?? "";
      });
    } catch (error) {
      setCorpora([]);
      setCorpusLoadError(String(error));
      setSelectedCorpusId("");
    } finally {
      setCorporaLoadedAt(new Date().toLocaleTimeString());
    }
  }

  function resetRunPanels() {
    setRuntimeLogs("");
    setPrivacyReport("");
    setStderr("");
    setAuditOperations([]);
    setSelectedReport("");
    setShowRawReport(false);
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
        setActiveChatCancelPending(false);
        setRunStatus("failed");
        break;
      }

      case "complete": {
        if (runtimeMode === "active-chat" && activeChatCancelPending && !payload.success) {
          setRunStatus("idle");
          setActiveChatCancelPending(false);
          setActiveChatStopNotice(
            "Cancelled this active-chat generation. Any partial assistant text still visible in the transcript was not committed to backend chat history. The runtime remains active until you send another message or use End + Sanitize."
          );
          setRuntimeLogs((current) => `${current}Active chat generation cancelled.\n`);
        } else {
          if (runtimeMode === "active-chat" && activeChatCancelPending && payload.success) {
            setActiveChatStopNotice(
              "Cancellation was requested, but the generation finished before the runtime stopped the turn."
            );
          }
          setRunStatus(payload.success ? "success" : "failed");
          setActiveChatCancelPending(false);
        }

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

  async function stopGeneration() {
    if (runtimeMode === "active-chat" && activeChatSessionId) {
      setActiveChatStopNotice(
        "Cancellation requested for this active-chat generation. Waiting for the runtime to stop the current turn before clearing it from backend chat history."
      );
      setActiveChatCancelPending(true);

      try {
        const response = await fetch(`${API_BASE}/api/chat/${activeChatSessionId}/cancel`, {
          method: "POST",
        });

        if (!response.ok) {
          const error = await readApiError(
            response,
            "Failed to cancel active chat generation."
          );
          throw new Error(formatActiveChatApiError("cancel", error));
        }

        const data = (await response.json()) as ChatCancelResponse;

        setRuntimeLogs((current) => `${current}${data.message}\n`);
        setAuditOperations((current) => [
          ...current,
          {
            operation: "client_generation_cancel_requested",
            status: "warning",
            details:
              "Client requested explicit cancellation for the current active-chat generation. The runtime will stop the current turn without committing it to chat history.",
          },
        ]);

        return;
      } catch (error) {
        setActiveChatCancelPending(false);
        setActiveChatStopNotice(String(error));
        setStderr((current) => `${current}${String(error)}\n`);
        setRuntimeLogs(
          (current) =>
            `${current}Active chat cancel request failed. Closing the client stream as a fallback.\n`
        );
      }
    }

    activeAbortController.current?.abort();
    activeAbortController.current = null;

    setRunStatus("failed");
    setRuntimeLogs((current) => `${current}Generation stopped by user.\n`);
    setActiveChatStopNotice(
      runtimeMode === "active-chat"
        ? "Stopped this active-chat generation by closing the client stream. Any partial assistant text still visible in the transcript was not committed to backend chat history. The runtime remains active until you send another message or use End + Sanitize."
        : ""
    );

    setAuditOperations((current) => [
      ...current,
      {
        operation: "client_generation_stop",
        status: "warning",
        details:
          runtimeMode === "active-chat"
            ? "Client stopped the current active-chat generation by closing the stream. The chat runtime remains active."
            : "Client stopped the one-shot generation. Backend cleanup should continue server-side.",
      },
    ]);
  }

  function readActiveChatConfigInputs() {
    if (useModelTemplateDefault || useModelContextDefaults) {
      if (!selectedModel) {
        throw new Error("Select a registered model before starting a session.");
      }
    }

    const overrides: {
      chat_template?: ChatTemplateOption;
      chat_context_token_budget?: number;
      chat_context_turn_limit?: number;
    } = {};

    if (!useModelTemplateDefault) {
      overrides.chat_template = chatTemplate;
    }

    if (!useModelContextDefaults) {
      const tokenBudget = parsePositiveInteger(chatContextTokenBudget);
      const turnLimit = parsePositiveInteger(chatContextTurnLimit);

      if (tokenBudget === null) {
        throw new Error(
          "Active chat context token budget must be a whole number greater than 0."
        );
      }

      if (turnLimit === null) {
        throw new Error("Active chat context turn limit must be a whole number greater than 0.");
      }

      overrides.chat_context_token_budget = tokenBudget;
      overrides.chat_context_turn_limit = turnLimit;
    }

    return overrides;
  }

  function closeDrawers() {
    setConfigDrawerOpen(false);
    setModelDrawerOpen(false);
    setRegistryDrawerOpen(false);
    setCorpusDrawerOpen(false);
  }

  function openConfigDrawer() {
    setModelDrawerOpen(false);
    setRegistryDrawerOpen(false);
    setConfigDrawerOpen(true);
    setCommandMenuOpen(false);
  }

  function openModelDrawer() {
    setConfigDrawerOpen(false);
    setRegistryDrawerOpen(false);
    setCorpusDrawerOpen(false);
    setModelDrawerOpen(true);
    setCommandMenuOpen(false);

    if (!inspectedModelId && models.length > 0) {
      setInspectedModelId(selectedModelId || models[0].id);
    }
  }

  function openRegistryDrawer() {
    setConfigDrawerOpen(false);
    setModelDrawerOpen(false);
    setCorpusDrawerOpen(false);
    setRegistryDrawerOpen(true);
    setCommandMenuOpen(false);

    if (!selectedSessionId && sessions.length > 0) {
      setSelectedSessionId(sessions[0].session_id);
    }
  }

  function openCorpusDrawer() {
    setConfigDrawerOpen(false);
    setModelDrawerOpen(false);
    setRegistryDrawerOpen(false);
    setCorpusDrawerOpen(true);
    setCommandMenuOpen(false);

    if (!selectedCorpusId && corpora.length > 0) {
      setSelectedCorpusId(corpora[0].corpus_id);
    }
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
          model_id: selectedModelId || undefined,
          corpus_id: selectedCorpusId || undefined,
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
    setActiveChatCancelPending(false);
    setRunStatus("running");
    closeDrawers();
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
          model_id: selectedModelId || undefined,
          corpus_id: selectedCorpusId || undefined,
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
      setActiveChatModelId(data.model_id);
      setActiveChatModelName(data.model_name);
      setActiveChatCorpusId(data.corpus_id || "");
      setActiveChatCorpusName(data.corpus_name || "");
      setActiveChatTurns(data.turns);
      setActiveChatGroundedTurns(data.grounded_turns);
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
      if (typeof data.grounded_turns === "number") {
        setActiveChatGroundedTurns(data.grounded_turns);
      }
      setActiveChatRuntimeActive(data.runtime_active);
      setActiveChatWorkspace(data.workspace);

      if (data.model_id) {
        setActiveChatModelId(data.model_id);
      }

      if (data.model_name) {
        setActiveChatModelName(data.model_name);
      }

      if (data.corpus_id) {
        setActiveChatCorpusId(data.corpus_id);
      }

      if (data.corpus_name) {
        setActiveChatCorpusName(data.corpus_name);
      }

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
    setActiveChatCancelPending(false);
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
      setActiveChatModelId("");
      setActiveChatModelName("");
      setActiveChatCorpusId("");
      setActiveChatCorpusName("");
      setActiveChatGroundedTurns(0);
      setShowRawReport(false);
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
    setActiveChatCancelPending(false);
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
    setShowRawReport(false);

    try {
      const response = await fetch(`${API_BASE}/api/reports/${sessionId}`);
      const data = await response.json();
      setSelectedReport(JSON.stringify(data, null, 2));
    } catch (error) {
      setSelectedReport(String(error));
    }
  }

  function openSessionReport(sessionId: string) {
    setInspectorView("report");
    setInspectorOpen(true);
    openReport(sessionId);
  }

  async function runRegistryLifecycleAction(
    sessionId: string,
    action: "cleanup" | "reconcile"
  ) {
    if (
      action === "cleanup" &&
      !window.confirm(
        "Run lifecycle cleanup for this retained session now? NullContext will try to archive the report first and then delete the session workspace."
      )
    ) {
      return;
    }

    setRegistryActionPending(action);
    setRegistryActionFailed(false);
    setRegistryActionMessage("");

    try {
      const response = await fetch(`${API_BASE}/api/sessions/${sessionId}/${action}`, {
        method: "POST",
      });

      if (!response.ok) {
        const error = await readApiError(
          response,
          `Failed to ${action} registry session lifecycle state.`
        );
        throw new Error(error);
      }

      const data = (await response.json()) as SessionLifecycleActionResponse;
      setRegistryActionResult(data);
      setRegistryActionMessage(data.message);
      setSelectedSessionId(data.session_id);

      await loadSessions();

      if (selectedSessionId === data.session_id && inspectorView === "report") {
        await openReport(data.session_id);
      }
    } catch (error) {
      setRegistryActionFailed(true);
      setRegistryActionMessage(String(error));
    } finally {
      setRegistryActionPending(null);
    }
  }

  async function saveRegistryRetentionPolicy(sessionId: string) {
    setRegistryActionPending("retention");
    setRegistryActionFailed(false);
    setRegistryActionMessage("");

    try {
      const retainForMinutes =
        retentionPolicyDraft === "retain_for_duration"
          ? parsePositiveInteger(retentionMinutesDraft)
          : null;

      if (retentionPolicyDraft === "retain_for_duration" && retainForMinutes === null) {
        throw new Error("Retention minutes must be a whole number greater than 0.");
      }

      const response = await fetch(`${API_BASE}/api/sessions/${sessionId}/retention`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          retention_policy: retentionPolicyDraft,
          retain_for_minutes: retainForMinutes ?? undefined,
        }),
      });

      if (!response.ok) {
        const error = await readApiError(
          response,
          "Failed to update session retention policy."
        );
        throw new Error(error);
      }

      const data = (await response.json()) as SessionLifecycleActionResponse;
      setRegistryActionResult(data);
      setRegistryActionMessage(data.message);
      setSelectedSessionId(data.session_id);

      await loadSessions();
    } catch (error) {
      setRegistryActionFailed(true);
      setRegistryActionMessage(String(error));
    } finally {
      setRegistryActionPending(null);
    }
  }

  async function ingestCorpusFromForm() {
    const paths = corpusIngestPaths
      .split("\n")
      .map((line) => line.trim())
      .filter(Boolean);
    const name = corpusIngestName.trim();

    if (!name) {
      setCorpusIngestFailed(true);
      setCorpusIngestMessage("Corpus name is required.");
      return;
    }

    if (paths.length === 0) {
      setCorpusIngestFailed(true);
      setCorpusIngestMessage("Provide at least one absolute file or directory path to ingest.");
      return;
    }

    setCorpusIngestPending(true);
    setCorpusIngestFailed(false);
    setCorpusIngestMessage("");

    try {
      const response = await fetch(`${API_BASE}/api/corpora`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          name,
          paths,
          persistent: corpusIngestPersistent,
          ocr_enabled: corpusIngestOcrEnabled,
        }),
      });

      if (!response.ok) {
        const error = await readApiError(response, "Failed to ingest corpus.");
        throw new Error(error);
      }

      const data = (await response.json()) as IngestCorpusResponse;

      setLastIngestedCorpusReport(data.report);
      setCorpusIngestMessage(
        `Built corpus ${data.corpus.name} with ${data.report.chunk_count} chunks from ${data.report.files_ingested} ingested file(s).`
      );
      setSelectedCorpusId(data.corpus.corpus_id);
      setCorpusIngestPaths("");
      setCorpusIngestName("");
      await loadCorpora();
    } catch (error) {
      setCorpusIngestFailed(true);
      setCorpusIngestMessage(String(error));
    } finally {
      setCorpusIngestPending(false);
    }
  }

  async function ingestUploadedCorpusFromForm() {
    const name = corpusIngestName.trim();

    if (!name) {
      setCorpusIngestFailed(true);
      setCorpusIngestMessage("Corpus name is required.");
      return;
    }

    if (corpusUploadFiles.length === 0) {
      setCorpusIngestFailed(true);
      setCorpusIngestMessage("Choose at least one .txt, .md, or .pdf file to upload.");
      return;
    }

    setCorpusIngestPending(true);
    setCorpusIngestFailed(false);
    setCorpusIngestMessage("");
    setCorpusUploadProgressPercent(0);
    setCorpusUploadProgressLabel("Preparing upload...");

    try {
      const formData = new FormData();
      formData.append("name", name);
      formData.append("persistent", String(corpusIngestPersistent));
      formData.append("ocr_enabled", String(corpusIngestOcrEnabled));

      for (const file of corpusUploadFiles) {
        formData.append("files", file, file.name);
      }

      const data = await new Promise<IngestCorpusResponse>((resolve, reject) => {
        const request = new XMLHttpRequest();

        request.open("POST", `${API_BASE}/api/corpora/upload`);
        request.responseType = "json";

        request.upload.onprogress = (event) => {
          if (event.lengthComputable) {
            const percent = Math.min(100, Math.round((event.loaded / event.total) * 100));
            setCorpusUploadProgressPercent(percent);
            setCorpusUploadProgressLabel(
              `Uploading ${formatBytes(event.loaded)} of ${formatBytes(event.total)}`
            );
          } else {
            setCorpusUploadProgressLabel("Uploading files...");
          }
        };

        request.onerror = () => {
          reject(new Error("Failed to upload corpus files."));
        };

        request.onload = () => {
          if (request.status < 200 || request.status >= 300) {
            const responseText =
              typeof request.responseText === "string" ? request.responseText : "";

            try {
              const parsed = JSON.parse(responseText) as ApiErrorResponse;
              reject(new Error(parsed.error || "Failed to ingest uploaded corpus."));
            } catch {
              reject(new Error(responseText || "Failed to ingest uploaded corpus."));
            }
            return;
          }

          const response = request.response as IngestCorpusResponse | null;

          if (!response) {
            reject(new Error("Uploaded corpus response was empty."));
            return;
          }

          resolve(response);
        };

        request.send(formData);
      });
      setCorpusUploadProgressPercent(100);
      setCorpusUploadProgressLabel("Upload finished. Finalizing corpus...");

      setLastIngestedCorpusReport(data.report);
      setCorpusIngestMessage(
        `Built uploaded corpus ${data.corpus.name} with ${data.report.chunk_count} chunks from ${data.report.files_ingested} ingested file(s).`
      );
      setSelectedCorpusId(data.corpus.corpus_id);
      setCorpusIngestPaths("");
      setCorpusIngestName("");
      setCorpusUploadFiles([]);
      setCorpusUploadInputKey((current) => current + 1);
      await loadCorpora();
    } catch (error) {
      setCorpusIngestFailed(true);
      setCorpusIngestMessage(String(error));
    } finally {
      setCorpusIngestPending(false);
      setCorpusUploadProgressPercent(null);
      setCorpusUploadProgressLabel("");
    }
  }

  function handleCorpusUploadSelection(files: File[]) {
    setCorpusUploadFiles(
      files.filter((file) =>
        [".txt", ".md", ".pdf"].some((suffix) => file.name.toLowerCase().endsWith(suffix))
      )
    );
  }

  async function openCorpusReport(corpusId: string) {
    try {
      const response = await fetch(`${API_BASE}/api/corpora/${corpusId}/report`);

      if (!response.ok) {
        const error = await readApiError(response, "Failed to load corpus report.");
        throw new Error(error);
      }

      const data = await response.json();
      setSelectedCorpusReport(JSON.stringify(data, null, 2));
    } catch (error) {
      setSelectedCorpusReport(String(error));
    }
  }

  async function runCorpusLifecycleAction(
    corpusId: string,
    action: "cleanup" | "reconcile"
  ) {
    if (
      action === "cleanup" &&
      !window.confirm(
        "Run lifecycle cleanup for this corpus now? NullContext will archive the ingestion report first when possible and then delete the corpus artifacts."
      )
    ) {
      return;
    }

    setCorpusActionPending(action);
    setCorpusActionFailed(false);
    setCorpusActionMessage("");

    try {
      const response = await fetch(`${API_BASE}/api/corpora/${corpusId}/${action}`, {
        method: "POST",
      });

      if (!response.ok) {
        const error = await readApiError(
          response,
          `Failed to ${action} corpus lifecycle state.`
        );
        throw new Error(error);
      }

      const data = (await response.json()) as CorpusLifecycleActionResponse;
      setCorpusActionResult(data);
      setCorpusActionMessage(data.message);
      setSelectedCorpusId(data.corpus_id);
      await loadCorpora();
      await openCorpusReport(data.corpus_id);
    } catch (error) {
      setCorpusActionFailed(true);
      setCorpusActionMessage(String(error));
    } finally {
      setCorpusActionPending(null);
    }
  }

  async function saveCorpusRetentionPolicy(corpusId: string) {
    setCorpusActionPending("retention");
    setCorpusActionFailed(false);
    setCorpusActionMessage("");

    try {
      const retainForMinutes =
        corpusRetentionPolicyDraft === "retain_for_duration"
          ? parsePositiveInteger(corpusRetentionMinutesDraft)
          : null;

      if (corpusRetentionPolicyDraft === "retain_for_duration" && retainForMinutes === null) {
        throw new Error("Retention minutes must be a whole number greater than 0.");
      }

      const response = await fetch(`${API_BASE}/api/corpora/${corpusId}/retention`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          retention_policy: corpusRetentionPolicyDraft,
          retain_for_minutes: retainForMinutes ?? undefined,
        }),
      });

      if (!response.ok) {
        const error = await readApiError(response, "Failed to update corpus retention policy.");
        throw new Error(error);
      }

      const data = (await response.json()) as CorpusLifecycleActionResponse;
      setCorpusActionResult(data);
      setCorpusActionMessage(data.message);
      setSelectedCorpusId(data.corpus_id);
      await loadCorpora();
      await openCorpusReport(data.corpus_id);
    } catch (error) {
      setCorpusActionFailed(true);
      setCorpusActionMessage(String(error));
    } finally {
      setCorpusActionPending(null);
    }
  }

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
  }, [theme]);

  useEffect(() => {
    checkHealth();
    loadSessions();
    loadModels();
    loadCorpora();
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
        closeDrawers();
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

  useEffect(() => {
    const currentSession = sessions.find((session) => session.session_id === selectedSessionId);

    if (!currentSession) {
      return;
    }

    setRetentionPolicyDraft(currentSession.lifecycle.retention_policy);
    setRetentionMinutesDraft(minutesUntil(currentSession.lifecycle.retention_deadline));
  }, [selectedSessionId, sessions]);

  useEffect(() => {
    if (corpora.length === 0) {
      if (selectedCorpusId !== "") {
        setSelectedCorpusId("");
      }
      return;
    }

    if (!corpora.some((corpus) => corpus.corpus_id === selectedCorpusId)) {
      setSelectedCorpusId(corpora[0].corpus_id);
    }
  }, [corpora, selectedCorpusId]);

  useEffect(() => {
    const currentCorpus = corpora.find((corpus) => corpus.corpus_id === selectedCorpusId);

    if (!currentCorpus) {
      return;
    }

    setCorpusRetentionPolicyDraft(currentCorpus.lifecycle.retention_policy);
    setCorpusRetentionMinutesDraft(minutesUntil(currentCorpus.lifecycle.retention_deadline));
  }, [corpora, selectedCorpusId]);

  useEffect(() => {
    const nextSessions = [...sessions]
      .filter((session) => {
        if (registryModeFilter !== "all" && session.security_mode !== registryModeFilter) {
          return false;
        }

        switch (registryOutcomeFilter) {
          case "cleanup-failed":
            if (!session.cleanup_attempted || session.cleanup_successful) {
              return false;
            }
            break;
          case "workspace-retained":
            if (session.workspace_deleted) {
              return false;
            }
            break;
          case "artifacts":
            if (session.artifacts_detected <= 0) {
              return false;
            }
            break;
          case "history-stored":
            if (!session.history_stored) {
              return false;
            }
            break;
          default:
            break;
        }

        const query = registryQuery.trim().toLowerCase();

        if (!query) {
          return true;
        }

        return [
          session.session_id,
          session.security_mode,
          session.prompt_source,
          session.backend,
          session.model_path,
          session.workspace,
          session.report_path,
        ]
          .join(" ")
          .toLowerCase()
          .includes(query);
      })
      .sort((left, right) => {
        const leftTime = new Date(left.started_at).getTime();
        const rightTime = new Date(right.started_at).getTime();

        return registrySortOrder === "newest" ? rightTime - leftTime : leftTime - rightTime;
      });

    if (nextSessions.length === 0) {
      if (selectedSessionId !== "") {
        setSelectedSessionId("");
      }
      return;
    }

    if (!nextSessions.some((session) => session.session_id === selectedSessionId)) {
      setSelectedSessionId(nextSessions[0].session_id);
    }
  }, [
    registryModeFilter,
    registryOutcomeFilter,
    registryQuery,
    registrySortOrder,
    selectedSessionId,
    sessions,
  ]);

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
  const query = registryQuery.trim().toLowerCase();
  const filteredSessions = [...sessions]
    .filter((session) => {
      if (registryModeFilter !== "all" && session.security_mode !== registryModeFilter) {
        return false;
      }

      switch (registryOutcomeFilter) {
        case "cleanup-failed":
          if (!session.cleanup_attempted || session.cleanup_successful) {
            return false;
          }
          break;
        case "workspace-retained":
          if (session.workspace_deleted) {
            return false;
          }
          break;
        case "artifacts":
          if (session.artifacts_detected <= 0) {
            return false;
          }
          break;
        case "history-stored":
          if (!session.history_stored) {
            return false;
          }
          break;
        default:
          break;
      }

      if (!query) {
        return true;
      }

      return [
        session.session_id,
        session.security_mode,
        session.prompt_source,
        session.backend,
        session.model_id ?? "",
        session.model_name ?? "",
        session.model_path,
        session.workspace,
        session.report_path,
      ]
        .join(" ")
        .toLowerCase()
        .includes(query);
    })
    .sort((left, right) => {
      const leftTime = new Date(left.started_at).getTime();
      const rightTime = new Date(right.started_at).getTime();

      return registrySortOrder === "newest" ? rightTime - leftTime : leftTime - rightTime;
    });
  const latestSession = [...sessions].sort((left, right) => {
    const leftTime = new Date(left.started_at).getTime();
    const rightTime = new Date(right.started_at).getTime();

    return rightTime - leftTime;
  })[0];
  const selectedSession =
    filteredSessions.find((session) => session.session_id === selectedSessionId) ?? null;
  const selectedLifecycleResult =
    registryActionResult && registryActionResult.session_id === selectedSessionId
      ? registryActionResult
      : null;
  const selectedModel =
    models.find((model) => model.id === selectedModelId) ??
    models.find((model) => model.default_selected) ??
    null;
  const corpusQueryText = corpusQuery.trim().toLowerCase();
  const filteredCorpora = corpora.filter((corpus) => {
    if (!corpusQueryText) {
      return true;
    }

    return [
      corpus.corpus_id,
      corpus.name,
      corpus.root_path,
      corpus.manifest_path,
      corpus.embedding_backend ?? "",
      corpus.embedding_model ?? "",
      corpus.ocr_backend ?? "",
      corpus.lifecycle.state,
      corpus.lifecycle.retention_policy,
    ]
      .join(" ")
      .toLowerCase()
      .includes(corpusQueryText);
  });
  const selectedCorpus =
    filteredCorpora.find((corpus) => corpus.corpus_id === selectedCorpusId) ??
    corpora.find((corpus) => corpus.corpus_id === selectedCorpusId) ??
    null;
  const selectedCorpusLifecycleResult =
    corpusActionResult && corpusActionResult.corpus_id === selectedCorpusId
      ? corpusActionResult
      : null;
  const modelQueryText = modelQuery.trim().toLowerCase();
  const filteredModels = models.filter((model) => {
    if (!modelQueryText) {
      return true;
    }

    return [
      model.id,
      model.name,
      model.description ?? "",
      model.model_path,
      model.chat_template,
    ]
      .join(" ")
      .toLowerCase()
      .includes(modelQueryText);
  });
  const inspectedModel =
    filteredModels.find((model) => model.id === inspectedModelId) ??
    models.find((model) => model.id === inspectedModelId) ??
    selectedModel;
  const effectiveTemplate = useModelTemplateDefault
    ? selectedModel?.chat_template || "auto"
    : chatTemplate;
  const effectiveContextBudget = useModelContextDefaults
    ? selectedModel?.chat_context_token_budget ?? null
    : parsePositiveInteger(chatContextTokenBudget);
  const effectiveContextTurnLimit = useModelContextDefaults
    ? selectedModel?.chat_context_turn_limit ?? null
    : parsePositiveInteger(chatContextTurnLimit);
  const activeRuntimeModelName =
    activeChatRuntimeActive && activeChatModelName
      ? activeChatModelName
      : selectedModel?.name || "unconfigured";
  const activeRuntimeModelId =
    activeChatRuntimeActive && activeChatModelId
      ? activeChatModelId
      : selectedModel?.id || "";
  const currentReportRaw = selectedReport || privacyReport;
  const currentReport = parsePrivacyReport(currentReportRaw);
  const currentCorpusReport = parseCorpusReport(selectedCorpusReport);

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
              onClick={openConfigDrawer}
              title="Open session config drawer"
            >
              config
            </button>
            <button
              className="ghost-button"
              onClick={openRegistryDrawer}
              title="Open session registry drawer"
            >
              registry
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
                <div className="drawer-actions">
                  <button className="ghost-button" onClick={openCorpusDrawer}>
                    corpora
                  </button>
                  <button className="ghost-button" onClick={openModelDrawer}>
                    models
                  </button>
                  <button className="ghost-button" onClick={openConfigDrawer}>
                    open
                  </button>
                </div>
              </div>

              <div className="config-summary">
                <span>model: {selectedModel?.name || "loading..."}</span>
                <span>mode: {mode}</span>
                <span>persistent: {persistent ? "on" : "off"}</span>
                <span>
                  corpus:{" "}
                  {selectedCorpus
                    ? `${selectedCorpus.name} · ${selectedCorpus.persistent ? "persistent" : "ephemeral"}`
                    : "none"}
                </span>
                <span>
                  template: {effectiveTemplate}
                  {useModelTemplateDefault ? " · model" : " · override"}
                </span>
                <span>
                  context:{" "}
                  {effectiveContextBudget !== null && effectiveContextTurnLimit !== null
                    ? `${effectiveContextBudget} tok / ${effectiveContextTurnLimit} turns`
                    : "invalid"}
                  {useModelContextDefaults ? " · model" : " · override"}
                </span>
              </div>

              <p className="microcopy">
                Move detailed controls into the config drawer so the main shell stays focused on
                runtime state and conversation flow. The selected corpus can ground one-shot runs
                immediately and will bind to active chat when you start a new session.
              </p>
            </section>

            <section className="panel">
              <div className="panel-header">
                <div className="panel-title">registry</div>
                <button className="ghost-button" onClick={openRegistryDrawer}>
                  browse
                </button>
              </div>

              <div className="config-summary">
                <span>
                  sessions: {sessions.length} persistent{sessions.length === 1 ? " run" : " runs"}
                </span>
                <span>last refresh: {registryLoadedAt}</span>
              </div>

              <p className="microcopy">
                Open the registry drawer to inspect retained sessions, workspace paths, cleanup
                outcomes, and stored reports without crowding the main shell.
              </p>
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
            <div className="runtime-meta">
              <div>
                model: {activeRuntimeModelName}
                {activeRuntimeModelId ? ` (${activeRuntimeModelId})` : ""}
              </div>
              {runtimeMode === "active-chat" && activeChatCorpusName && (
                <div className="truncate" title={activeChatCorpusId || activeChatCorpusName}>
                  corpus: {activeChatCorpusName}
                  {activeChatCorpusId ? ` (${activeChatCorpusId})` : ""}
                </div>
              )}
              {runtimeMode === "one-shot" && selectedCorpus && (
                <div className="truncate" title={selectedCorpus.root_path}>
                  corpus: {selectedCorpus.name} ({selectedCorpus.persistent ? "persistent" : "ephemeral"})
                </div>
              )}
              {!activeChatRuntimeActive && selectedModel && (
                <div className="truncate" title={selectedModel.model_path}>
                  path: {selectedModel.model_path}
                </div>
              )}
            </div>
            {runtimeMode === "active-chat" && activeChatHistoryPolicy && (
              <div className="runtime-meta">
                <div>template: {activeChatResolvedTemplate || "unknown"}</div>
                {activeChatContextBudget !== null && activeChatContextTurnLimit !== null && (
                  <div>
                    context: {activeChatContextBudget} tok / {activeChatContextTurnLimit} turns
                  </div>
                )}
                {activeChatCorpusName && (
                  <div>
                    grounded turns: {activeChatGroundedTurns} via {activeChatCorpusName}
                  </div>
                )}
                <div title={activeChatHistoryPolicy}>policy: {activeChatHistoryPolicy}</div>
              </div>
            )}
            {runtimeMode === "one-shot" && (
              <div className="runtime-meta">
                <div>
                  template: {effectiveTemplate}
                  {useModelTemplateDefault ? " (model default)" : " (manual override)"}
                </div>
                {effectiveContextBudget !== null && effectiveContextTurnLimit !== null && (
                  <div>
                    context: {effectiveContextBudget} tok / {effectiveContextTurnLimit} turns
                    {useModelContextDefaults ? " (model default)" : " (manual override)"}
                  </div>
                )}
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
                        openModelDrawer();
                      }}
                    >
                      open model drawer
                    </button>
                    <button
                      onClick={() => {
                        openCorpusDrawer();
                      }}
                    >
                      open corpus drawer
                    </button>
                    <button
                      onClick={() => {
                        openConfigDrawer();
                      }}
                    >
                      open config drawer
                    </button>
                    <button
                      onClick={() => {
                        loadModels();
                        setCommandMenuOpen(false);
                      }}
                    >
                      refresh model registry
                    </button>
                    <button
                      onClick={() => {
                        loadCorpora();
                        setCommandMenuOpen(false);
                      }}
                    >
                      refresh corpus registry
                    </button>
                    <button
                      onClick={() => {
                        openRegistryDrawer();
                      }}
                    >
                      open registry drawer
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
                <>
                  {!currentReportRaw ? (
                    <p className="muted-text">no report selected</p>
                  ) : currentReport ? (
                    <div className="report-viewer">
                      <div className="report-toolbar">
                        <div className="report-toolbar-copy">
                          <strong>privacy report</strong>
                          <span>
                            session {shortId(currentReport.session_id)} ·{" "}
                            {formatTimestamp(currentReport.started_at)}
                          </span>
                        </div>

                        <button
                          className={showRawReport ? "selected" : ""}
                          onClick={() => setShowRawReport((current) => !current)}
                        >
                          {showRawReport ? "hide raw json" : "view raw json"}
                        </button>
                      </div>

                      <section className="report-section">
                        <div className="panel-title">summary</div>
                        <ReportGrid
                          entries={[
                            { label: "session id", value: currentReport.session_id },
                            { label: "started", value: formatTimestamp(currentReport.started_at) },
                            { label: "security mode", value: currentReport.security_mode },
                            { label: "backend", value: currentReport.backend },
                            { label: "gpu layers", value: currentReport.gpu_layers },
                            {
                              label: "history stored",
                              value: formatBoolean(currentReport.history_stored),
                            },
                            {
                              label: "process exited cleanly",
                              value: formatBoolean(currentReport.process_exited_cleanly),
                            },
                          ]}
                        />
                      </section>

                      {currentReport.lifecycle && (
                        <section className="report-section">
                          <div className="panel-title">lifecycle policy</div>
                          <ReportGrid
                            entries={[
                              {
                                label: "state",
                                value: humanizeSnakeCase(currentReport.lifecycle.state),
                              },
                              {
                                label: "retention policy",
                                value: humanizeSnakeCase(
                                  currentReport.lifecycle.retention_policy
                                ),
                              },
                              {
                                label: "retention deadline",
                                value: currentReport.lifecycle.retention_deadline
                                  ? formatTimestamp(currentReport.lifecycle.retention_deadline)
                                  : "none",
                              },
                              {
                                label: "cleanup requested",
                                value: currentReport.lifecycle.cleanup_requested_at
                                  ? formatTimestamp(currentReport.lifecycle.cleanup_requested_at)
                                  : "none",
                              },
                              {
                                label: "cleanup completed",
                                value: currentReport.lifecycle.cleanup_completed_at
                                  ? formatTimestamp(currentReport.lifecycle.cleanup_completed_at)
                                  : "none",
                              },
                              {
                                label: "cleanup reason",
                                value: currentReport.lifecycle.cleanup_reason
                                  ? humanizeSnakeCase(currentReport.lifecycle.cleanup_reason)
                                  : "none",
                              },
                              {
                                label: "lifecycle updated",
                                value: currentReport.lifecycle.updated_at
                                  ? formatTimestamp(currentReport.lifecycle.updated_at)
                                  : "none",
                              },
                            ]}
                          />

                          <div className="report-risk-block">
                            <p>
                              <strong>policy summary:</strong>{" "}
                              {currentReport.lifecycle.policy_summary}
                            </p>
                            <p>
                              <strong>decision summary:</strong>{" "}
                              {currentReport.lifecycle.decision_summary}
                            </p>
                          </div>
                        </section>
                      )}

                      {currentReport.session_profile && (
                        <section className="report-section">
                          <div className="panel-title">session profile</div>
                          <ReportGrid
                            entries={[
                              {
                                label: "session kind",
                                value: currentReport.session_profile.session_kind,
                              },
                              {
                                label: "runtime lifetime",
                                value: currentReport.session_profile.runtime_lifetime,
                              },
                              {
                                label: "turn count",
                                value: String(currentReport.session_profile.turn_count),
                              },
                              {
                                label: "runtime duration",
                                value: formatDuration(
                                  currentReport.session_profile.runtime_duration_ms
                                ),
                              },
                              {
                                label: "history policy",
                                value: currentReport.session_profile.history_policy,
                              },
                              {
                                label: "persistence policy",
                                value: currentReport.session_profile.persistence_policy,
                              },
                              {
                                label: "prompt source",
                                value: currentReport.session_profile.prompt_source,
                              },
                              {
                                label: "grounding scope",
                                value:
                                  currentReport.session_profile.grounding_scope || "none",
                              },
                              {
                                label: "bound corpus",
                                value:
                                  currentReport.session_profile.bound_corpus_name ||
                                  currentReport.session_profile.bound_corpus_id ||
                                  "none",
                              },
                              {
                                label: "grounded turns",
                                value: String(
                                  currentReport.session_profile.grounded_turn_count ?? 0
                                ),
                              },
                            ]}
                          />

                          <details className="report-detail" open>
                            <summary>
                              <span>turn artifacts</span>
                              <span className="pill neutral">
                                {currentReport.session_profile.turn_artifacts.length}
                              </span>
                            </summary>
                            {currentReport.session_profile.turn_artifacts.length === 0 ? (
                              <p className="muted-text">no turn artifacts recorded</p>
                            ) : (
                              <div className="report-list">
                                {currentReport.session_profile.turn_artifacts.map((artifact) => (
                                  <div className="report-item" key={artifact.turn}>
                                    <div className="report-item-header">
                                      <strong>turn {artifact.turn}</strong>
                                    </div>
                                    <div className="report-path-list">
                                      <div>{artifact.prompt_path}</div>
                                      <div>{artifact.response_path}</div>
                                    </div>
                                  </div>
                                ))}
                              </div>
                            )}
                          </details>
                        </section>
                      )}

                      {currentReport.llama_runtime && (
                        <section className="report-section">
                          <div className="panel-title">llama runtime exposure</div>
                          <ReportGrid
                            entries={[
                              {
                                label: "runtime",
                                value: currentReport.llama_runtime.runtime_kind,
                              },
                              {
                                label: "runtime pid",
                                value:
                                  currentReport.llama_runtime.runtime_pid?.toString() || "none",
                              },
                              {
                                label: "model",
                                value: `${currentReport.llama_runtime.model_name} (${currentReport.llama_runtime.model_id})`,
                              },
                              {
                                label: "gpu layers requested",
                                value: String(
                                  currentReport.llama_runtime.gpu_layers_requested
                                ),
                              },
                              {
                                label: "gpu offload requested",
                                value: formatBoolean(
                                  currentReport.llama_runtime.gpu_offload_requested
                                ),
                              },
                              {
                                label: "shutdown method",
                                value: humanizeSnakeCase(
                                  currentReport.llama_runtime.shutdown_method
                                ),
                              },
                              {
                                label: "process exit code",
                                value:
                                  currentReport.llama_runtime.process_exit_code?.toString() ||
                                  "unknown",
                              },
                              {
                                label: "graceful shutdown supported",
                                value: formatBoolean(
                                  currentReport.llama_runtime.graceful_shutdown_supported
                                ),
                              },
                              {
                                label: "observed rss",
                                value: currentReport.llama_runtime.observed_resident_bytes
                                  ? formatBytes(
                                      currentReport.llama_runtime.observed_resident_bytes
                                    )
                                  : "unavailable",
                              },
                              {
                                label: "observed virtual",
                                value: currentReport.llama_runtime.observed_virtual_bytes
                                  ? formatBytes(
                                      currentReport.llama_runtime.observed_virtual_bytes
                                    )
                                  : "unavailable",
                              },
                              {
                                label: "observed gpu memory",
                                value: currentReport.llama_runtime.observed_gpu_memory_bytes
                                  ? formatBytes(
                                      currentReport.llama_runtime.observed_gpu_memory_bytes
                                    )
                                  : "unavailable",
                              },
                              {
                                label: "memory source",
                                value:
                                  currentReport.llama_runtime.process_memory_source || "none",
                              },
                              {
                                label: "gpu source",
                                value: currentReport.llama_runtime.gpu_memory_source || "none",
                              },
                              {
                                label: "model path",
                                value: currentReport.llama_runtime.model_path,
                              },
                            ]}
                          />

                          <div className="report-risk-block">
                            <p>
                              <strong>cleanup boundary:</strong>{" "}
                              {currentReport.llama_runtime.cleanup_summary}
                            </p>
                            <p>
                              <strong>runtime-specific residual risk:</strong>{" "}
                              {currentReport.llama_runtime.residual_risk_summary}
                            </p>
                          </div>

                          <details className="report-detail" open>
                            <summary>
                              <span>memory domains</span>
                              <span className="pill neutral">
                                {currentReport.llama_runtime.memory_domains.length}
                              </span>
                            </summary>
                            <div className="audit-list">
                              {currentReport.llama_runtime.memory_domains.map((domain) => (
                                <details
                                  className="audit-item"
                                  key={`${domain.domain}-${domain.exposure_scope}`}
                                >
                                  <summary>
                                    <code>{domain.domain}</code>
                                    <span className={statusClass(domain.cleanup_status)}>
                                      {domain.cleanup_status}
                                    </span>
                                  </summary>
                                  <p>
                                    <strong>scope:</strong> {domain.exposure_scope}
                                  </p>
                                  <p>{domain.notes}</p>
                                </details>
                              ))}
                            </div>
                          </details>

                          {currentReport.llama_runtime.observation_notes.length > 0 && (
                            <details className="report-detail" open>
                              <summary>
                                <span>observation notes</span>
                                <span className="pill neutral">
                                  {currentReport.llama_runtime.observation_notes.length}
                                </span>
                              </summary>
                              <div className="report-risk-block">
                                {currentReport.llama_runtime.observation_notes.map((note) => (
                                  <p key={note}>{note}</p>
                                ))}
                              </div>
                            </details>
                          )}
                        </section>
                      )}

                      {currentReport.retrieval && (
                        <section className="report-section">
                          <div className="panel-title">retrieval provenance</div>
                          <ReportGrid
                            entries={[
                              {
                                label: "corpus",
                                value: `${currentReport.retrieval.corpus_name} (${currentReport.retrieval.corpus_id})`,
                              },
                              {
                                label: "mode",
                                value: humanizeSnakeCase(currentReport.retrieval.retrieval_mode),
                              },
                              {
                                label: "grounded turns",
                                value: String(currentReport.retrieval.grounded_turns),
                              },
                              {
                                label: "retrieved chunks",
                                value: String(currentReport.retrieval.retrieved_chunks),
                              },
                              {
                                label: "top k",
                                value: String(currentReport.retrieval.top_k),
                              },
                              {
                                label: "context injected",
                                value: formatBoolean(currentReport.retrieval.context_injected),
                              },
                            ]}
                          />

                          <div className="report-risk-block">
                            <p>
                              <strong>latest grounded query:</strong>{" "}
                              {currentReport.retrieval.query}
                            </p>
                            <p>
                              <strong>source files touched:</strong>{" "}
                              {currentReport.retrieval.source_paths.length}
                            </p>
                            {currentReport.retrieval.page_hits.length > 0 && (
                              <p>
                                <strong>page-level hits:</strong>{" "}
                                {currentReport.retrieval.page_hits.length}
                              </p>
                            )}
                          </div>
                        </section>
                      )}

                      <section className="report-section">
                        <div className="panel-title">cleanup</div>
                        <ReportGrid
                          entries={[
                            {
                              label: "attempted",
                              value: formatBoolean(currentReport.cleanup.attempted),
                            },
                            {
                              label: "successful",
                              value: formatBoolean(currentReport.cleanup.successful),
                            },
                            {
                              label: "workspace deleted",
                              value: formatBoolean(currentReport.cleanup.workspace_deleted),
                            },
                            {
                              label: "files removed",
                              value: String(currentReport.cleanup.files_removed),
                            },
                            {
                              label: "directories removed",
                              value: String(currentReport.cleanup.directories_removed),
                            },
                            {
                              label: "cleanup error",
                              value: currentReport.cleanup.error || "none",
                            },
                          ]}
                        />
                      </section>

                      <details className="report-detail" open>
                        <summary>
                          <span>artifacts detected</span>
                          <span className="pill neutral">
                            {currentReport.cleanup.artifacts_detected.length}
                          </span>
                        </summary>
                        {currentReport.cleanup.artifacts_detected.length === 0 ? (
                          <p className="muted-text">no artifacts detected</p>
                        ) : (
                          <div className="report-list">
                            {currentReport.cleanup.artifacts_detected.map((artifact) => (
                              <div
                                className="report-item"
                                key={`${artifact.path}-${artifact.kind}-${artifact.size_bytes}`}
                              >
                                <div className="report-item-header">
                                  <strong>{artifact.kind}</strong>
                                  <span className="pill neutral">
                                    {formatBytes(artifact.size_bytes)}
                                  </span>
                                </div>
                                <div className="report-path-list">
                                  <div>{artifact.path}</div>
                                </div>
                              </div>
                            ))}
                          </div>
                        )}
                      </details>

                      <details className="report-detail" open>
                        <summary>
                          <span>sanitization operations</span>
                          <span className="pill neutral">
                            {currentReport.cleanup.sanitization_operations.length}
                          </span>
                        </summary>
                        {currentReport.cleanup.sanitization_operations.length === 0 ? (
                          <p className="muted-text">no sanitization operations recorded</p>
                        ) : (
                          <div className="audit-list">
                            {currentReport.cleanup.sanitization_operations.map(
                              (operation, index) => (
                                <details
                                  className="audit-item"
                                  key={`${operation.operation}-${index}`}
                                >
                                  <summary>
                                    <code>{operation.operation}</code>
                                    <span className={statusClass(operation.status)}>
                                      {operation.status}
                                    </span>
                                  </summary>
                                  <p>{operation.details}</p>
                                </details>
                              )
                            )}
                          </div>
                        )}
                      </details>

                      <section className="report-section">
                        <div className="panel-title">residual risks</div>
                        <div className="report-risk-block">
                          <p>{currentReport.residual_risk}</p>
                          {currentReport.session_profile?.active_runtime_residual_risk && (
                            <p>{currentReport.session_profile.active_runtime_residual_risk}</p>
                          )}
                        </div>
                      </section>

                      {showRawReport && <pre>{currentReportRaw}</pre>}
                    </div>
                  ) : (
                    <div className="report-viewer">
                      <div className="report-toolbar">
                        <div className="report-toolbar-copy">
                          <strong>privacy report</strong>
                          <span>raw output</span>
                        </div>
                      </div>
                      <pre>{currentReportRaw}</pre>
                    </div>
                  )}
                </>
              )}

              {inspectorView === "stderr" && (
                <pre>{stderr || "no stderr captured"}</pre>
              )}
            </div>
          </section>
        </aside>
      )}

      <div
        className={`drawer-backdrop${
          configDrawerOpen || modelDrawerOpen || registryDrawerOpen || corpusDrawerOpen
            ? " open"
            : ""
        }`}
        onClick={closeDrawers}
      />
      <aside className={`model-drawer${modelDrawerOpen ? " open" : ""}`}>
        <div className="drawer-header">
          <div>
            <h3>model registry</h3>
            <p>inspect registered models, compare runtime defaults, and choose the next session model</p>
          </div>
          <div className="drawer-actions">
            <button className="ghost-button" onClick={loadModels}>
              refresh
            </button>
            <button className="ghost-button" onClick={closeDrawers}>
              close
            </button>
          </div>
        </div>

        <div className="drawer-body model-drawer-body">
          <section className="panel model-list-panel">
            {runtimeValidation && (
              <div className="registry-action-banner">
                <div className="registry-lifecycle-summary">
                  <span className={modelValidationClass(runtimeValidation.validation_status)}>
                    runtime {runtimeValidation.validation_status.replaceAll("_", " ")}
                  </span>
                  {runtimeValidation.selectable ? (
                    <span className="pill success">launcher ready</span>
                  ) : (
                    <span className="pill failed">launcher unavailable</span>
                  )}
                </div>
                <div className="registry-path">{runtimeValidation.llama_path}</div>
                {runtimeValidation.validation_message && (
                  <div className="microcopy">{runtimeValidation.validation_message}</div>
                )}
              </div>
            )}

            <div className="panel-header">
              <div className="panel-title">models</div>
              <span className="mini-status">loaded:{modelsLoadedAt}</span>
            </div>

            <div className="registry-toolbar">
              <label>
                search
                <input
                  type="search"
                  value={modelQuery}
                  onChange={(event) => setModelQuery(event.target.value)}
                  placeholder="name, id, path, template..."
                />
              </label>
            </div>

            {modelLoadError ? (
              <p className="muted-text">model registry unavailable: {modelLoadError}</p>
            ) : filteredModels.length === 0 ? (
              <p className="muted-text">
                {models.length === 0
                  ? "no models are registered"
                  : "no models match the current search"}
              </p>
            ) : (
              <div className="session-list model-session-list">
                {filteredModels.map((model) => (
                  <button
                    className={inspectedModelId === model.id ? "session-item selected" : "session-item"}
                    key={model.id}
                    onClick={() => setInspectedModelId(model.id)}
                  >
                    <div className="registry-session-header">
                      <span>{model.name}</span>
                      <div className="drawer-actions">
                        <span className={modelValidationClass(model.validation_status)}>
                          {model.validation_status.replaceAll("_", " ")}
                        </span>
                        {selectedModelId === model.id ? (
                          <span className="pill success">selected</span>
                        ) : model.default_selected ? (
                          <span className="pill neutral">default</span>
                        ) : null}
                      </div>
                    </div>
                    <div className="registry-session-meta">
                      <small>{model.id}</small>
                      <small>{model.chat_template}</small>
                    </div>
                    <small>{model.max_tokens} tok · {model.gpu_layers} gpu layers</small>
                  </button>
                ))}
              </div>
            )}
          </section>

          <section className="panel model-detail-panel">
            <div className="panel-header">
              <div className="panel-title">details</div>
              {inspectedModel && <span className="mini-status">id:{inspectedModel.id}</span>}
            </div>

            {!inspectedModel ? (
              <p className="muted-text">select a model to inspect its runtime defaults</p>
            ) : (
              <>
                <div className="registry-lifecycle-summary">
                  <span className={modelValidationClass(inspectedModel.validation_status)}>
                    {inspectedModel.validation_status.replaceAll("_", " ")}
                  </span>
                  {selectedModelId === inspectedModel.id ? (
                    <span className="pill success">selected for next session</span>
                  ) : (
                    <span className="pill neutral">available</span>
                  )}
                  {inspectedModel.default_selected && <span className="pill neutral">default</span>}
                </div>

                <dl className="registry-detail-grid">
                  <dt>name</dt>
                  <dd>{inspectedModel.name}</dd>
                  <dt>id</dt>
                  <dd>{inspectedModel.id}</dd>
                  <dt>default template</dt>
                  <dd>{inspectedModel.chat_template}</dd>
                  <dt>launchable</dt>
                  <dd>{inspectedModel.selectable ? "yes" : "no"}</dd>
                  <dt>max tokens</dt>
                  <dd>{inspectedModel.max_tokens}</dd>
                  <dt>gpu layers</dt>
                  <dd>{inspectedModel.gpu_layers}</dd>
                  <dt>context budget</dt>
                  <dd>{inspectedModel.chat_context_token_budget}</dd>
                  <dt>context turn limit</dt>
                  <dd>{inspectedModel.chat_context_turn_limit}</dd>
                  <dt>model path</dt>
                  <dd className="registry-path">{inspectedModel.model_path}</dd>
                </dl>

                {inspectedModel.description && (
                  <div className="report-risk-block model-description-block">
                    <p>{inspectedModel.description}</p>
                  </div>
                )}

                {inspectedModel.validation_message && (
                  <div className="registry-action-banner failed">
                    {inspectedModel.validation_message}
                  </div>
                )}

                <div className="registry-actions">
                  <button
                    onClick={() => {
                      setSelectedModelId(inspectedModel.id);
                      setModelDrawerOpen(false);
                    }}
                    disabled={!inspectedModel.selectable}
                  >
                    use for next session
                  </button>
                  <button
                    className="ghost-button"
                    onClick={() => {
                      setSelectedModelId(inspectedModel.id);
                      openConfigDrawer();
                    }}
                    disabled={!inspectedModel.selectable}
                  >
                    select + open config
                  </button>
                </div>

                <p className="microcopy">
                  Model selection stays separate from live runtime state so you can inspect paths,
                  templates, token limits, and context defaults before launching the next session.
                </p>
              </>
            )}
          </section>
        </div>
      </aside>
      <aside className={`config-drawer${configDrawerOpen ? " open" : ""}`}>
        <div className="drawer-header">
          <div>
            <h3>session config</h3>
            <p>move detailed controls off the main page and keep the shell focused</p>
          </div>
          <button className="ghost-button" onClick={closeDrawers}>
            close
          </button>
        </div>

        <div className="drawer-body">
          <section className="panel">
            <label>
              model
              <div className="field-with-hint">
                <select
                  value={selectedModelId}
                  onChange={(event) => setSelectedModelId(event.target.value)}
                  disabled={activeChatRuntimeActive || models.length === 0}
                >
                  {models.length === 0 ? (
                    <option value="">no models available</option>
                  ) : (
                    models.map((model) => (
                      <option key={model.id} value={model.id} disabled={!model.selectable}>
                        {model.name}
                        {model.default_selected ? " · default" : ""}
                        {!model.selectable ? " · unavailable" : ""}
                      </option>
                    ))
                  )}
                </select>
                <Hint text="Pick which registered model this session should launch. Model selection is resolved before the runtime starts and stays fixed for the active chat session." />
              </div>
            </label>

            {selectedModel ? (
              <div className="config-summary">
                <span>id: {selectedModel.id}</span>
                <span>max tokens: {selectedModel.max_tokens}</span>
                <span>gpu layers: {selectedModel.gpu_layers}</span>
                <span>default template: {selectedModel.chat_template}</span>
              </div>
            ) : (
              <p className="microcopy">
                {modelLoadError
                  ? `Model registry unavailable: ${modelLoadError}`
                  : "No registered models were returned by the backend."}
              </p>
            )}

            {selectedModel?.description && (
              <p className="microcopy">{selectedModel.description}</p>
            )}

            {selectedModel && (
              <p className="microcopy truncate" title={selectedModel.model_path}>
                {selectedModel.model_path}
              </p>
            )}

            {selectedModel && !selectedModel.selectable && selectedModel.validation_message && (
              <p className="microcopy">{selectedModel.validation_message}</p>
            )}

            {runtimeValidation && !runtimeValidation.selectable && (
              <p className="microcopy">{runtimeValidation.validation_message || "llama-server is not ready to launch."}</p>
            )}

            <div className="drawer-actions">
              <button className="ghost-button" onClick={openCorpusDrawer}>
                browse corpora
              </button>
              <button className="ghost-button" onClick={openModelDrawer}>
                browse model registry
              </button>
            </div>

            <section className="corpus-config-block">
              <div className="panel-header">
                <div className="panel-title">grounding corpus</div>
                <span className="mini-status">loaded:{corporaLoadedAt}</span>
              </div>

              {selectedCorpus ? (
                <div className="config-summary">
                  <span>name: {selectedCorpus.name}</span>
                  <span>id: {selectedCorpus.corpus_id}</span>
                  <span>
                    lifecycle: {humanizeSnakeCase(selectedCorpus.lifecycle.state)}
                  </span>
                  <span>
                    chunks: {selectedCorpus.chunk_count} · sources: {selectedCorpus.source_count}
                  </span>
                </div>
              ) : (
                <p className="microcopy">
                  {corpusLoadError
                    ? `Corpus registry unavailable: ${corpusLoadError}`
                    : "No corpus selected. One-shot runs will use the raw prompt only."}
                </p>
              )}

              {selectedCorpus && (
                <p className="microcopy">
                  Selected corpus will be used for one-shot grounded runs immediately and will bind
                  to any new active chat session you start after changing it here.
                </p>
              )}
            </section>

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
                  disabled={activeChatRuntimeActive || useModelTemplateDefault}
                >
                  <option value="auto">auto-detect</option>
                  <option value="generic">generic</option>
                  <option value="chatml">chatml</option>
                  <option value="llama3-instruct">llama3-instruct</option>
                </select>
                <Hint text="Auto-detect resolves a template from the configured model path. Override it if a model needs a specific prompt format." />
              </div>
            </label>

            <label className="checkbox">
              <input
                type="checkbox"
                checked={useModelTemplateDefault}
                disabled={activeChatRuntimeActive || !selectedModel}
                onChange={(event) => setUseModelTemplateDefault(event.target.checked)}
              />
              use selected model template default
            </label>
            <p className="microcopy">
              effective template: {effectiveTemplate}
              {useModelTemplateDefault ? " from model registry" : " from manual override"}
            </p>

            <label>
              context token budget
              <div className="field-with-hint">
                <input
                  type="number"
                  min={1}
                  step={1}
                  value={chatContextTokenBudget}
                  onChange={(event) => setChatContextTokenBudget(event.target.value)}
                  disabled={activeChatRuntimeActive || useModelContextDefaults}
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
                  disabled={activeChatRuntimeActive || useModelContextDefaults}
                />
                <Hint text="Maximum number of recent prior turns that can be included in the active-chat prompt window." />
              </div>
            </label>

            <label className="checkbox">
              <input
                type="checkbox"
                checked={useModelContextDefaults}
                disabled={activeChatRuntimeActive || !selectedModel}
                onChange={(event) => setUseModelContextDefaults(event.target.checked)}
              />
              use selected model context defaults
            </label>
            <p className="microcopy">
              effective context:{" "}
              {effectiveContextBudget !== null && effectiveContextTurnLimit !== null
                ? `${effectiveContextBudget} tok / ${effectiveContextTurnLimit} turns`
                : "invalid manual override"}
              {useModelContextDefaults ? " from model registry" : " from manual override"}
            </p>

            <p className="microcopy">
              Active chat uses the selected template and bounded recent-context window when the
              session starts. Change settings before starting the runtime.
            </p>
            <p className="microcopy">models loaded: {modelsLoadedAt}</p>
          </section>
        </div>
      </aside>

      <aside className={`corpus-drawer${corpusDrawerOpen ? " open" : ""}`}>
        <div className="drawer-header">
          <div>
            <h3>corpus registry</h3>
            <p>ingest local documents, inspect corpus artifacts, and choose a grounded one-shot corpus</p>
          </div>
          <div className="drawer-actions">
            <button className="ghost-button" onClick={loadCorpora}>
              refresh
            </button>
            <button className="ghost-button" onClick={closeDrawers}>
              close
            </button>
          </div>
        </div>

        <div className="drawer-body corpus-drawer-body">
          <section className="panel corpus-list-panel">
            <div className="panel-header">
              <div className="panel-title">corpora</div>
              <span className="mini-status">loaded:{corporaLoadedAt}</span>
            </div>

            <div className="registry-toolbar">
              <label>
                search
                <input
                  type="search"
                  value={corpusQuery}
                  onChange={(event) => setCorpusQuery(event.target.value)}
                  placeholder="name, id, path, backend..."
                />
              </label>
            </div>

            {corpusLoadError ? (
              <p className="muted-text">corpus registry unavailable: {corpusLoadError}</p>
            ) : filteredCorpora.length === 0 ? (
              <p className="muted-text">
                {corpora.length === 0 ? "no corpora have been ingested yet" : "no corpora match the current search"}
              </p>
            ) : (
              <div className="session-list model-session-list">
                {filteredCorpora.map((corpus) => (
                  <button
                    className={
                      selectedCorpusId === corpus.corpus_id ? "session-item selected" : "session-item"
                    }
                    key={corpus.corpus_id}
                    onClick={() => setSelectedCorpusId(corpus.corpus_id)}
                  >
                    <div className="registry-session-header">
                      <span>{corpus.name}</span>
                      <span className={lifecycleStateClass(corpus.lifecycle.state)}>
                        {humanizeSnakeCase(corpus.lifecycle.state)}
                      </span>
                    </div>
                    <div className="registry-session-meta">
                      <small>{corpus.persistent ? "persistent" : "ephemeral"}</small>
                      <small>{new Date(corpus.created_at).toLocaleString()}</small>
                    </div>
                    <small>{shortId(corpus.corpus_id)}</small>
                    <small>
                      {corpus.source_count} sources · {corpus.chunk_count} chunks
                    </small>
                  </button>
                ))}
              </div>
            )}
          </section>

          <section className="panel corpus-detail-panel">
            <div className="detail-stack">
              <section className="corpus-detail-block">
                <div className="panel-header">
                  <div className="panel-title">details</div>
                  {selectedCorpus && (
                    <span className="mini-status">id:{shortId(selectedCorpus.corpus_id)}</span>
                  )}
                </div>

                {!selectedCorpus ? (
                  <p className="muted-text">select a corpus to inspect its lifecycle and artifact paths</p>
                ) : (
                  <>
                    <div className="registry-lifecycle-summary">
                      <span className={lifecycleStateClass(selectedCorpus.lifecycle.state)}>
                        {humanizeSnakeCase(selectedCorpus.lifecycle.state)}
                      </span>
                      <span className="pill neutral">
                        {selectedCorpus.persistent ? "persistent" : "ephemeral"}
                      </span>
                    </div>

                    {corpusActionMessage && (
                      <div
                        className={
                          corpusActionFailed ? "registry-action-banner failed" : "registry-action-banner"
                        }
                      >
                        {corpusActionMessage}
                      </div>
                    )}

                    <dl className="registry-detail-grid">
                      <dt>name</dt>
                      <dd>{selectedCorpus.name}</dd>
                      <dt>created</dt>
                      <dd>{formatTimestamp(selectedCorpus.created_at)}</dd>
                      <dt>retention policy</dt>
                      <dd>{humanizeSnakeCase(selectedCorpus.lifecycle.retention_policy)}</dd>
                      <dt>retention deadline</dt>
                      <dd>
                        {selectedCorpus.lifecycle.retention_deadline
                          ? formatTimestamp(selectedCorpus.lifecycle.retention_deadline)
                          : "none"}
                      </dd>
                      <dt>cleanup requested</dt>
                      <dd>
                        {selectedCorpus.lifecycle.cleanup_requested_at
                          ? formatTimestamp(selectedCorpus.lifecycle.cleanup_requested_at)
                          : "not requested"}
                      </dd>
                      <dt>cleanup completed</dt>
                      <dd>
                        {selectedCorpus.lifecycle.cleanup_completed_at
                          ? formatTimestamp(selectedCorpus.lifecycle.cleanup_completed_at)
                          : "not completed"}
                      </dd>
                      <dt>cleanup reason</dt>
                      <dd>
                        {selectedCorpus.lifecycle.cleanup_reason
                          ? humanizeSnakeCase(selectedCorpus.lifecycle.cleanup_reason)
                          : "none"}
                      </dd>
                      <dt>lifecycle updated</dt>
                      <dd>
                        {selectedCorpus.lifecycle.updated_at
                          ? formatTimestamp(selectedCorpus.lifecycle.updated_at)
                          : "unknown"}
                      </dd>
                      <dt>sources</dt>
                      <dd>{selectedCorpus.source_count}</dd>
                      <dt>chunks</dt>
                      <dd>{selectedCorpus.chunk_count}</dd>
                      <dt>embedding backend</dt>
                      <dd>{selectedCorpus.embedding_backend || "unknown"}</dd>
                      <dt>embedding model</dt>
                      <dd>{selectedCorpus.embedding_model || "unknown"}</dd>
                      <dt>ocr backend</dt>
                      <dd>{selectedCorpus.ocr_backend || "unknown"}</dd>
                      <dt>root path</dt>
                      <dd className="registry-path">{selectedCorpus.root_path}</dd>
                      <dt>manifest path</dt>
                      <dd className="registry-path">{selectedCorpus.manifest_path}</dd>
                      <dt>report path</dt>
                      <dd className="registry-path">{selectedCorpus.report_path}</dd>
                      {selectedCorpusLifecycleResult && (
                        <>
                          <dt>root exists</dt>
                          <dd>{formatBoolean(selectedCorpusLifecycleResult.root_exists)}</dd>
                          <dt>report exists</dt>
                          <dd>{formatBoolean(selectedCorpusLifecycleResult.report_exists)}</dd>
                        </>
                      )}
                    </dl>

                    <section className="registry-retention-controls">
                      <div className="panel-title">retention policy</div>
                      <div className="registry-filter-row">
                        <label>
                          policy
                          <select
                            value={corpusRetentionPolicyDraft}
                            onChange={(event) => setCorpusRetentionPolicyDraft(event.target.value)}
                            disabled={corpusActionPending !== null}
                          >
                            <option value="retain_until_manual_cleanup">manual cleanup</option>
                            <option value="retain_for_duration">timed retention</option>
                            <option value="ephemeral_immediate">ephemeral immediate</option>
                          </select>
                        </label>

                        {corpusRetentionPolicyDraft === "retain_for_duration" && (
                          <label>
                            minutes
                            <input
                              type="number"
                              min={1}
                              step={1}
                              value={corpusRetentionMinutesDraft}
                              onChange={(event) => setCorpusRetentionMinutesDraft(event.target.value)}
                              disabled={corpusActionPending !== null}
                            />
                          </label>
                        )}
                      </div>

                      <div className="registry-retention-meta">
                        <span>
                          current deadline:{" "}
                          {selectedCorpus.lifecycle.retention_deadline
                            ? formatTimestamp(selectedCorpus.lifecycle.retention_deadline)
                            : "none"}
                        </span>
                        <button
                          onClick={() => saveCorpusRetentionPolicy(selectedCorpus.corpus_id)}
                          disabled={corpusActionPending !== null}
                        >
                          {corpusActionPending === "retention"
                            ? "saving..."
                            : "save retention policy"}
                        </button>
                      </div>
                    </section>

                    <div className="registry-actions">
                      <button
                        onClick={() => {
                          setSelectedCorpusId(selectedCorpus.corpus_id);
                          openConfigDrawer();
                        }}
                      >
                        use for one-shot
                      </button>
                      <button
                        onClick={() => openCorpusReport(selectedCorpus.corpus_id)}
                        disabled={corpusActionPending !== null}
                      >
                        load report
                      </button>
                      <button
                        onClick={() =>
                          runCorpusLifecycleAction(selectedCorpus.corpus_id, "reconcile")
                        }
                        disabled={corpusActionPending !== null}
                      >
                        {corpusActionPending === "reconcile" ? "reconciling..." : "reconcile"}
                      </button>
                      <button
                        className="danger-button"
                        onClick={() =>
                          runCorpusLifecycleAction(selectedCorpus.corpus_id, "cleanup")
                        }
                        disabled={corpusActionPending !== null}
                      >
                        {corpusActionPending === "cleanup" ? "cleaning up..." : "cleanup now"}
                      </button>
                    </div>

                    {selectedCorpusReport && (
                      <>
                        {currentCorpusReport?.upload_staging && (
                          <section className="report-section">
                            <div className="panel-title">upload staging</div>
                            <ReportGrid
                              entries={[
                                {
                                  label: "staging root",
                                  value: currentCorpusReport.upload_staging.staging_root,
                                },
                                {
                                  label: "staged files",
                                  value: String(currentCorpusReport.upload_staging.staged_files),
                                },
                                {
                                  label: "staged bytes",
                                  value: formatBytes(currentCorpusReport.upload_staging.staged_bytes),
                                },
                                {
                                  label: "cleanup status",
                                  value: currentCorpusReport.upload_staging.cleaned_up
                                    ? "cleaned up"
                                    : "retained/failed",
                                },
                                {
                                  label: "cleanup error",
                                  value:
                                    currentCorpusReport.upload_staging.cleanup_error || "none",
                                },
                              ]}
                            />

                            <details className="report-detail" open>
                              <summary>
                                <span>uploaded filenames</span>
                                <span className="pill neutral">
                                  {currentCorpusReport.upload_staging.source_filenames.length}
                                </span>
                              </summary>
                              <div className="report-list">
                                {currentCorpusReport.upload_staging.source_filenames.map((name) => (
                                  <div className="report-item" key={name}>
                                    <div className="report-path-list">
                                      <div>{name}</div>
                                    </div>
                                  </div>
                                ))}
                              </div>
                            </details>
                          </section>
                        )}

                        <details className="report-detail" open>
                          <summary>
                            <span>corpus report</span>
                            <span className="pill neutral">json</span>
                          </summary>
                          <pre>{selectedCorpusReport}</pre>
                        </details>
                      </>
                    )}
                  </>
                )}
              </section>

              <section className="corpus-ingest-panel">
                <div className="panel-header">
                  <div className="panel-title">ingest corpus</div>
                  <span className="mini-status">txt · md · pdf</span>
                </div>

                {corpusIngestMessage && (
                  <div
                    className={
                      corpusIngestFailed ? "registry-action-banner failed" : "registry-action-banner"
                    }
                  >
                    {corpusIngestMessage}
                  </div>
                )}

                <label>
                  corpus name
                  <input
                    value={corpusIngestName}
                    onChange={(event) => setCorpusIngestName(event.target.value)}
                    placeholder="incident-response-briefing"
                    disabled={corpusIngestPending}
                  />
                </label>

                <label>
                  local paths
                  <textarea
                    value={corpusIngestPaths}
                    onChange={(event) => setCorpusIngestPaths(event.target.value)}
                    placeholder={"/Users/you/docs/briefing.pdf\n/Users/you/docs/notes"}
                    disabled={corpusIngestPending}
                  />
                </label>

                <label>
                  upload files
                  <div
                    className={`upload-dropzone${corpusUploadDragActive ? " active" : ""}${
                      corpusIngestPending ? " disabled" : ""
                    }`}
                    onDragEnter={(event) => {
                      event.preventDefault();
                      if (!corpusIngestPending) {
                        setCorpusUploadDragActive(true);
                      }
                    }}
                    onDragOver={(event) => {
                      event.preventDefault();
                      if (!corpusIngestPending) {
                        setCorpusUploadDragActive(true);
                      }
                    }}
                    onDragLeave={(event) => {
                      event.preventDefault();
                      if (event.currentTarget.contains(event.relatedTarget as Node | null)) {
                        return;
                      }
                      setCorpusUploadDragActive(false);
                    }}
                    onDrop={(event) => {
                      event.preventDefault();
                      setCorpusUploadDragActive(false);
                      if (corpusIngestPending) {
                        return;
                      }
                      handleCorpusUploadSelection(Array.from(event.dataTransfer.files ?? []));
                    }}
                  >
                    <input
                      key={corpusUploadInputKey}
                      className="upload-input"
                      type="file"
                      accept=".txt,.md,.pdf,text/plain,text/markdown,application/pdf"
                      multiple
                      disabled={corpusIngestPending}
                      onChange={(event) =>
                        handleCorpusUploadSelection(Array.from(event.target.files ?? []))
                      }
                    />
                    <div className="upload-dropzone-copy">
                      <strong>drop files here or click to browse</strong>
                      <span>Finder / File Explorer upload for .txt, .md, and .pdf</span>
                    </div>
                  </div>
                </label>

                {corpusUploadFiles.length > 0 && (
                  <div className="config-summary corpus-upload-summary">
                    <span>
                      selected: {corpusUploadFiles.length} file
                      {corpusUploadFiles.length === 1 ? "" : "s"}
                    </span>
                    {corpusUploadFiles.slice(0, 4).map((file) => (
                      <span key={`${file.name}-${file.size}`}>
                        {file.name} · {formatBytes(file.size)}
                      </span>
                    ))}
                    {corpusUploadFiles.length > 4 && (
                      <span>…and {corpusUploadFiles.length - 4} more</span>
                    )}
                  </div>
                )}

                <label className="checkbox">
                  <input
                    type="checkbox"
                    checked={corpusIngestPersistent}
                    onChange={(event) => setCorpusIngestPersistent(event.target.checked)}
                    disabled={corpusIngestPending}
                  />
                  persistent corpus
                </label>

                <label className="checkbox">
                  <input
                    type="checkbox"
                    checked={corpusIngestOcrEnabled}
                    onChange={(event) => setCorpusIngestOcrEnabled(event.target.checked)}
                    disabled={corpusIngestPending}
                  />
                  enable hybrid OCR for sparse PDF pages
                </label>

                <div className="registry-actions">
                  <button onClick={ingestCorpusFromForm} disabled={corpusIngestPending}>
                    {corpusIngestPending ? "ingesting..." : "ingest from paths"}
                  </button>
                  <button
                    onClick={ingestUploadedCorpusFromForm}
                    disabled={corpusIngestPending || corpusUploadFiles.length === 0}
                  >
                    {corpusIngestPending ? "uploading..." : "upload + ingest"}
                  </button>
                </div>

                {corpusIngestPending && corpusUploadProgressPercent !== null && (
                  <div className="upload-progress">
                    <div className="upload-progress-bar">
                      <div
                        className="upload-progress-fill"
                        style={{ width: `${corpusUploadProgressPercent}%` }}
                      />
                    </div>
                    <div className="upload-progress-meta">
                      <span>{corpusUploadProgressPercent}%</span>
                      <span>{corpusUploadProgressLabel || "Uploading corpus files..."}</span>
                    </div>
                  </div>
                )}

                <p className="microcopy">
                  Upload files to open Finder/File Explorer like a normal chatbot, or enter
                  absolute local file and directory paths if you want an operator-style ingest.
                  PDF ingestion uses native text extraction first, then OCRs sparse pages when enabled.
                </p>

                {lastIngestedCorpusReport && (
                  <div className="config-summary corpus-ingest-summary">
                    <span>last ingest: {shortId(lastIngestedCorpusReport.corpus_id)}</span>
                    <span>
                      discovered: {lastIngestedCorpusReport.files_discovered} · ingested:{" "}
                      {lastIngestedCorpusReport.files_ingested}
                    </span>
                    <span>
                      pdf pages: {lastIngestedCorpusReport.pdf_pages_seen} · OCR:{" "}
                      {lastIngestedCorpusReport.pdf_pages_ocrd}
                    </span>
                    <span>chunks: {lastIngestedCorpusReport.chunk_count}</span>
                    {lastIngestedCorpusReport.upload_staging && (
                      <span>
                        upload staging:{" "}
                        {lastIngestedCorpusReport.upload_staging.cleaned_up
                          ? "cleaned"
                          : "retained/failed"}
                      </span>
                    )}
                  </div>
                )}

                {lastIngestedCorpusReport?.upload_staging && (
                  <section className="report-section compact-report-section">
                    <div className="panel-title">latest upload staging</div>
                    <ReportGrid
                      entries={[
                        {
                          label: "staging root",
                          value: lastIngestedCorpusReport.upload_staging.staging_root,
                        },
                        {
                          label: "staged files",
                          value: String(lastIngestedCorpusReport.upload_staging.staged_files),
                        },
                        {
                          label: "staged bytes",
                          value: formatBytes(lastIngestedCorpusReport.upload_staging.staged_bytes),
                        },
                        {
                          label: "cleanup status",
                          value: lastIngestedCorpusReport.upload_staging.cleaned_up
                            ? "cleaned up"
                            : "retained/failed",
                        },
                        {
                          label: "cleanup error",
                          value: lastIngestedCorpusReport.upload_staging.cleanup_error || "none",
                        },
                      ]}
                    />
                  </section>
                )}
              </section>
            </div>
          </section>
        </div>
      </aside>

      <aside className={`registry-drawer${registryDrawerOpen ? " open" : ""}`}>
        <div className="drawer-header">
          <div>
            <h3>session registry</h3>
            <p>browse retained sessions and open stored reports without packing the sidebar</p>
          </div>
          <div className="drawer-actions">
            <button className="ghost-button" onClick={loadSessions}>
              refresh
            </button>
            <button className="ghost-button" onClick={closeDrawers}>
              close
            </button>
          </div>
        </div>

        <div className="drawer-body registry-drawer-body">
          <section className="panel registry-list-panel">
            <div className="panel-header">
              <div className="panel-title">sessions</div>
              <span className="mini-status">loaded:{registryLoadedAt}</span>
            </div>

            <div className="registry-toolbar">
              <label>
                search
                <input
                  type="search"
                  value={registryQuery}
                  onChange={(event) => setRegistryQuery(event.target.value)}
                  placeholder="session id, backend, path..."
                />
              </label>

              <div className="registry-filter-row">
                <label>
                  mode
                  <select
                    value={registryModeFilter}
                    onChange={(event) =>
                      setRegistryModeFilter(event.target.value as RegistryModeFilter)
                    }
                  >
                    <option value="all">all</option>
                    <option value="secure">secure</option>
                    <option value="standard">standard</option>
                    <option value="air-gapped">air-gapped</option>
                  </select>
                </label>

                <label>
                  outcome
                  <select
                    value={registryOutcomeFilter}
                    onChange={(event) =>
                      setRegistryOutcomeFilter(event.target.value as RegistryOutcomeFilter)
                    }
                  >
                    <option value="all">all</option>
                    <option value="cleanup-failed">cleanup failed</option>
                    <option value="workspace-retained">workspace retained</option>
                    <option value="artifacts">artifacts &gt; 0</option>
                    <option value="history-stored">history stored</option>
                  </select>
                </label>

                <label>
                  sort
                  <select
                    value={registrySortOrder}
                    onChange={(event) =>
                      setRegistrySortOrder(event.target.value as RegistrySortOrder)
                    }
                  >
                    <option value="newest">newest first</option>
                    <option value="oldest">oldest first</option>
                  </select>
                </label>
              </div>

              <div className="registry-chip-row">
                <button
                  className={
                    registryOutcomeFilter === "cleanup-failed" ? "selected chip-button" : "chip-button"
                  }
                  onClick={() =>
                    setRegistryOutcomeFilter((current) =>
                      current === "cleanup-failed" ? "all" : "cleanup-failed"
                    )
                  }
                >
                  cleanup failed
                </button>
                <button
                  className={
                    registryOutcomeFilter === "workspace-retained"
                      ? "selected chip-button"
                      : "chip-button"
                  }
                  onClick={() =>
                    setRegistryOutcomeFilter((current) =>
                      current === "workspace-retained" ? "all" : "workspace-retained"
                    )
                  }
                >
                  workspace retained
                </button>
                <button
                  className={
                    registryOutcomeFilter === "artifacts" ? "selected chip-button" : "chip-button"
                  }
                  onClick={() =>
                    setRegistryOutcomeFilter((current) =>
                      current === "artifacts" ? "all" : "artifacts"
                    )
                  }
                >
                  artifacts &gt; 0
                </button>
                <button
                  className={
                    registryOutcomeFilter === "history-stored"
                      ? "selected chip-button"
                      : "chip-button"
                  }
                  onClick={() =>
                    setRegistryOutcomeFilter((current) =>
                      current === "history-stored" ? "all" : "history-stored"
                    )
                  }
                >
                  history stored
                </button>
              </div>

              <div className="registry-toolbar-meta">
                <span>
                  showing {filteredSessions.length} of {sessions.length}
                </span>
                <div className="registry-toolbar-actions">
                  <button
                    className="ghost-button"
                    onClick={() => {
                      setRegistryQuery("");
                      setRegistryModeFilter("all");
                      setRegistryOutcomeFilter("all");
                      setRegistrySortOrder("newest");
                    }}
                  >
                    clear filters
                  </button>
                  <button
                    className="ghost-button"
                    disabled={!latestSession}
                    onClick={() => {
                      if (!latestSession) {
                        return;
                      }

                      setSelectedSessionId(latestSession.session_id);
                      openSessionReport(latestSession.session_id);
                    }}
                  >
                    latest report
                  </button>
                </div>
              </div>
            </div>

            {sessions.length === 0 ? (
              <p className="muted-text">no persistent sessions</p>
            ) : filteredSessions.length === 0 ? (
              <p className="muted-text">no sessions match the current registry filters</p>
            ) : (
              <div className="session-list registry-session-list">
                {filteredSessions.map((session) => (
                  <button
                    className={
                      selectedSessionId === session.session_id
                        ? "session-item selected"
                        : "session-item"
                    }
                    key={session.session_id}
                    onClick={() => setSelectedSessionId(session.session_id)}
                  >
                    <div className="registry-session-header">
                      <span>{shortId(session.session_id)}</span>
                      <span className={lifecycleStateClass(session.lifecycle.state)}>
                        {humanizeSnakeCase(session.lifecycle.state)}
                      </span>
                    </div>
                    <div className="registry-session-meta">
                      <small>{session.security_mode}</small>
                      <small>{new Date(session.started_at).toLocaleString()}</small>
                    </div>
                    {session.model_name && <small>{session.model_name}</small>}
                    <small>{humanizeSnakeCase(session.lifecycle.retention_policy)}</small>
                  </button>
                ))}
              </div>
            )}
          </section>

          <section className="panel registry-detail-panel">
            <div className="panel-header">
              <div className="panel-title">details</div>
              {selectedSession && (
                <span className="mini-status">id:{shortId(selectedSession.session_id)}</span>
              )}
            </div>

            {!selectedSession ? (
              <p className="muted-text">
                {filteredSessions.length === 0 && sessions.length > 0
                  ? "adjust the registry filters to inspect a matching session"
                  : "select a persistent session to inspect its metadata"}
              </p>
            ) : (
              <>
                <div className="registry-lifecycle-summary">
                  <span className={lifecycleStateClass(selectedSession.lifecycle.state)}>
                    {humanizeSnakeCase(selectedSession.lifecycle.state)}
                  </span>
                  <span className="pill neutral">
                    {humanizeSnakeCase(selectedSession.lifecycle.retention_policy)}
                  </span>
                </div>

                {registryActionMessage && (
                  <div
                    className={
                      registryActionFailed
                        ? "registry-action-banner failed"
                        : "registry-action-banner"
                    }
                  >
                    {registryActionMessage}
                  </div>
                )}

                <dl className="registry-detail-grid">
                  <dt>started</dt>
                  <dd>{new Date(selectedSession.started_at).toLocaleString()}</dd>
                  <dt>lifecycle state</dt>
                  <dd>{humanizeSnakeCase(selectedSession.lifecycle.state)}</dd>
                  <dt>retention policy</dt>
                  <dd>{humanizeSnakeCase(selectedSession.lifecycle.retention_policy)}</dd>
                  <dt>retention deadline</dt>
                  <dd>
                    {selectedSession.lifecycle.retention_deadline
                      ? formatTimestamp(selectedSession.lifecycle.retention_deadline)
                      : "none"}
                  </dd>
                  <dt>cleanup requested</dt>
                  <dd>
                    {selectedSession.lifecycle.cleanup_requested_at
                      ? formatTimestamp(selectedSession.lifecycle.cleanup_requested_at)
                      : "not requested"}
                  </dd>
                  <dt>cleanup completed</dt>
                  <dd>
                    {selectedSession.lifecycle.cleanup_completed_at
                      ? formatTimestamp(selectedSession.lifecycle.cleanup_completed_at)
                      : "not completed"}
                  </dd>
                  <dt>cleanup reason</dt>
                  <dd>
                    {selectedSession.lifecycle.cleanup_reason
                      ? humanizeSnakeCase(selectedSession.lifecycle.cleanup_reason)
                      : "none"}
                  </dd>
                  <dt>lifecycle updated</dt>
                  <dd>
                    {selectedSession.lifecycle.updated_at
                      ? formatTimestamp(selectedSession.lifecycle.updated_at)
                      : "unknown"}
                  </dd>
                  <dt>mode</dt>
                  <dd>{selectedSession.security_mode}</dd>
                  <dt>prompt source</dt>
                  <dd>{selectedSession.prompt_source}</dd>
                  <dt>history stored</dt>
                  <dd>{selectedSession.history_stored ? "yes" : "no"}</dd>
                  <dt>backend</dt>
                  <dd>{selectedSession.backend}</dd>
                  <dt>model id</dt>
                  <dd>{selectedSession.model_id || "legacy/default"}</dd>
                  <dt>model name</dt>
                  <dd>{selectedSession.model_name || "unknown"}</dd>
                  <dt>artifacts</dt>
                  <dd>{selectedSession.artifacts_detected}</dd>
                  <dt>cleanup attempted</dt>
                  <dd>{selectedSession.cleanup_attempted ? "yes" : "no"}</dd>
                  <dt>cleanup successful</dt>
                  <dd>{selectedSession.cleanup_successful ? "yes" : "no"}</dd>
                  <dt>workspace deleted</dt>
                  <dd>{selectedSession.workspace_deleted ? "yes" : "no"}</dd>
                  <dt>workspace</dt>
                  <dd className="registry-path">{selectedSession.workspace}</dd>
                  <dt>model path</dt>
                  <dd className="registry-path">{selectedSession.model_path}</dd>
                  <dt>report path</dt>
                  <dd className="registry-path">{selectedSession.report_path}</dd>
                  {selectedLifecycleResult && (
                    <>
                      <dt>workspace exists</dt>
                      <dd>{formatBoolean(selectedLifecycleResult.workspace_exists)}</dd>
                      <dt>report exists</dt>
                      <dd>{formatBoolean(selectedLifecycleResult.report_exists)}</dd>
                    </>
                  )}
                </dl>

                <section className="registry-retention-controls">
                  <div className="panel-title">retention policy</div>
                  <div className="registry-filter-row">
                    <label>
                      policy
                      <select
                        value={retentionPolicyDraft}
                        onChange={(event) => setRetentionPolicyDraft(event.target.value)}
                        disabled={registryActionPending !== null}
                      >
                        <option value="retain_until_manual_cleanup">manual cleanup</option>
                        <option value="retain_for_duration">timed retention</option>
                      </select>
                    </label>

                    {retentionPolicyDraft === "retain_for_duration" && (
                      <label>
                        minutes
                        <input
                          type="number"
                          min={1}
                          step={1}
                          value={retentionMinutesDraft}
                          onChange={(event) => setRetentionMinutesDraft(event.target.value)}
                          disabled={registryActionPending !== null}
                        />
                      </label>
                    )}
                  </div>

                  <div className="registry-retention-meta">
                    <span>
                      current deadline:{" "}
                      {selectedSession.lifecycle.retention_deadline
                        ? formatTimestamp(selectedSession.lifecycle.retention_deadline)
                        : "none"}
                    </span>
                    <button
                      onClick={() => saveRegistryRetentionPolicy(selectedSession.session_id)}
                      disabled={registryActionPending !== null}
                    >
                      {registryActionPending === "retention"
                        ? "saving..."
                        : "save retention policy"}
                    </button>
                  </div>
                </section>

                <div className="registry-actions">
                  <button onClick={() => openSessionReport(selectedSession.session_id)}>
                    open report in inspector
                  </button>
                  <button
                    onClick={() =>
                      runRegistryLifecycleAction(selectedSession.session_id, "reconcile")
                    }
                    disabled={registryActionPending !== null}
                  >
                    {registryActionPending === "reconcile" ? "reconciling..." : "reconcile"}
                  </button>
                  <button
                    className="danger-button"
                    onClick={() =>
                      runRegistryLifecycleAction(selectedSession.session_id, "cleanup")
                    }
                    disabled={registryActionPending !== null}
                  >
                    {registryActionPending === "cleanup" ? "cleaning up..." : "cleanup now"}
                  </button>
                </div>

                <p className="microcopy">
                  Registry browsing stays separate from the live runtime shell so report
                  inspection and lifecycle actions don&apos;t compete with conversation and runtime
                  controls.
                </p>
              </>
            )}
          </section>
        </div>
      </aside>
    </main>
  );
}

export default App;
