import { useEffect, useRef, useState } from "react";
import { Hint } from "./components/Hint";
import { ModelRegistryDrawer } from "./components/ModelRegistryDrawer";
import { PrivacyReportViewer } from "./components/PrivacyReportViewer";
import { ReportGrid } from "./components/ReportGrid";
import { SessionRegistryDrawer } from "./components/SessionRegistryDrawer";
import type {
  ApiErrorResponse,
  AuditOperation,
  ChatCancelResponse,
  ChatEndResponse,
  ChatMessage,
  ChatStartResponse,
  ChatStatusResponse,
  ChatTemplateOption,
  CorpusIndexEntry,
  CorpusIngestionReport,
  CorpusLifecycleActionResponse,
  CorpusRegistrySnapshot,
  InspectorView,
  IngestCorpusResponse,
  ModelRegistrySnapshot,
  RegisteredModel,
  RegistryModeFilter,
  RegistryOutcomeFilter,
  RegistrySortOrder,
  RunStatus,
  RuntimeMode,
  SessionIndexEntry,
  SessionLifecycleActionResponse,
  SessionRegistry,
  StreamPayload,
  Theme,
} from "./appTypes";
import {
  formatActiveChatApiError,
  formatBoolean,
  formatBytes,
  formatDuration,
  formatTimestamp,
  humanizeSnakeCase,
  lifecycleStateClass,
  minutesUntil,
  parseCorpusReport,
  parsePositiveInteger,
  parseSseBlock,
  readApiError,
  shortId,
  statusClass,
} from "./appUtils";
import "./App.css";

const API_BASE = "http://127.0.0.1:3333";

function App() {
  const activeAbortController = useRef<AbortController | null>(null);
  const commandMenuRef = useRef<HTMLDivElement | null>(null);
  const chatUploadMenuRef = useRef<HTMLDivElement | null>(null);
  const chatUploadInputRef = useRef<HTMLInputElement | null>(null);

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
  const [chatUploadMenuOpen, setChatUploadMenuOpen] = useState(false);
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
  const [chatUploadAccept, setChatUploadAccept] = useState(
    ".txt,.md,.pdf,text/plain,text/markdown,application/pdf"
  );
  const [corpusUploadProgressPercent, setCorpusUploadProgressPercent] = useState<number | null>(
    null
  );
  const [corpusUploadProgressLabel, setCorpusUploadProgressLabel] = useState("");
  const [corpusUploadDragActive, setCorpusUploadDragActive] = useState(false);
  const [chatUploadNotice, setChatUploadNotice] = useState("");
  const [chatUploadFailed, setChatUploadFailed] = useState(false);
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

    try {
      const data = await uploadCorpusFiles({
        files: corpusUploadFiles,
        name,
        persistent: corpusIngestPersistent,
        ocrEnabled: corpusIngestOcrEnabled,
      });

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

  function filterSupportedCorpusFiles(files: File[]) {
    return files.filter((file) =>
      [".txt", ".md", ".pdf"].some((suffix) => file.name.toLowerCase().endsWith(suffix))
    );
  }

  function buildQuickUploadCorpusName(files: File[]) {
    const first = files[0]?.name ?? "chat-upload";
    const base = first.replace(/\.[^/.]+$/, "");
    const safeBase = base
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "")
      .slice(0, 36);

    if (files.length === 1) {
      return `${safeBase || "chat-upload"}-upload`;
    }

    return `${safeBase || "chat-upload"}-${files.length}-files`;
  }

  async function uploadCorpusFiles({
    files,
    name,
    persistent,
    ocrEnabled,
  }: {
    files: File[];
    name: string;
    persistent: boolean;
    ocrEnabled: boolean;
  }) {
    setCorpusUploadProgressPercent(0);
    setCorpusUploadProgressLabel("Preparing upload...");

    const formData = new FormData();
    formData.append("name", name);
    formData.append("persistent", String(persistent));
    formData.append("ocr_enabled", String(ocrEnabled));

    for (const file of files) {
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
          const responseText = typeof request.responseText === "string" ? request.responseText : "";

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
    return data;
  }

  function handleCorpusUploadSelection(files: File[]) {
    setCorpusUploadFiles(filterSupportedCorpusFiles(files));
  }

  function openChatUploadPicker(accept: string) {
    setChatUploadAccept(accept);
    setChatUploadMenuOpen(false);
    chatUploadInputRef.current?.click();
  }

  async function ingestUploadedCorpusFromChat(files: File[]) {
    const supportedFiles = filterSupportedCorpusFiles(files);

    if (supportedFiles.length === 0) {
      setChatUploadFailed(true);
      setChatUploadNotice("Choose at least one .txt, .md, or .pdf file to upload.");
      return;
    }

    setCorpusUploadFiles(supportedFiles);
    setCorpusIngestPending(true);
    setChatUploadFailed(false);
    setChatUploadNotice("");

    try {
      const data = await uploadCorpusFiles({
        files: supportedFiles,
        name: buildQuickUploadCorpusName(supportedFiles),
        persistent: false,
        ocrEnabled: true,
      });

      setLastIngestedCorpusReport(data.report);
      setSelectedCorpusId(data.corpus.corpus_id);
      setChatUploadNotice(
        runtimeMode === "active-chat" && activeChatRuntimeActive
          ? `Uploaded ${data.corpus.name}. It is selected for one-shot runs and for the next active chat session you start.`
          : `Uploaded ${data.corpus.name}. It is now the selected grounding corpus for your next run.`
      );
      await loadCorpora();
    } catch (error) {
      setChatUploadFailed(true);
      setChatUploadNotice(String(error));
    } finally {
      setCorpusIngestPending(false);
      setCorpusUploadProgressPercent(null);
      setCorpusUploadProgressLabel("");
      setChatUploadMenuOpen(false);
    }
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
        setChatUploadMenuOpen(false);
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

      if (
        chatUploadMenuOpen &&
        chatUploadMenuRef.current &&
        !chatUploadMenuRef.current.contains(event.target as Node)
      ) {
        setChatUploadMenuOpen(false);
      }
    }

    window.addEventListener("mousedown", closeCommandMenu);

    return () => window.removeEventListener("mousedown", closeCommandMenu);
  }, [chatUploadMenuOpen, commandMenuOpen]);

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
            <div className="popup-menu composer-attach-menu" ref={chatUploadMenuRef}>
              <button
                className="ghost-button popup-trigger composer-attach-button"
                disabled={
                  corpusIngestPending ||
                  runStatus === "running" ||
                  (runtimeMode === "active-chat" && activeChatRuntimeActive)
                }
                onClick={() => setChatUploadMenuOpen((current) => !current)}
                title={
                  runtimeMode === "active-chat" && activeChatRuntimeActive
                    ? "Uploads bind to new active chat sessions, not the one already running."
                    : "Attach files as a grounding corpus."
                }
              >
                +
              </button>
              {chatUploadMenuOpen && (
                <div className="popup-panel">
                  <button onClick={() => openChatUploadPicker(".pdf,application/pdf")}>
                    upload pdf
                  </button>
                  <button onClick={() => openChatUploadPicker(".md,text/markdown")}>
                    upload markdown
                  </button>
                  <button onClick={() => openChatUploadPicker(".txt,text/plain")}>
                    upload text
                  </button>
                  <button
                    onClick={() =>
                      openChatUploadPicker(
                        ".txt,.md,.pdf,text/plain,text/markdown,application/pdf"
                      )
                    }
                  >
                    upload any supported file
                  </button>
                </div>
              )}
              <input
                ref={chatUploadInputRef}
                className="upload-input"
                type="file"
                accept={chatUploadAccept}
                multiple
                disabled={
                  corpusIngestPending ||
                  runStatus === "running" ||
                  (runtimeMode === "active-chat" && activeChatRuntimeActive)
                }
                onChange={(event) => {
                  const files = Array.from(event.target.files ?? []);
                  event.target.value = "";
                  void ingestUploadedCorpusFromChat(files);
                }}
              />
            </div>
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

            {(selectedCorpus || chatUploadNotice || corpusIngestPending) && (
              <div className="composer-meta">
                {selectedCorpus && (
                  <span className="composer-corpus-chip">
                    corpus: {selectedCorpus.name} ·{" "}
                    {selectedCorpus.persistent ? "persistent" : "ephemeral"}
                  </span>
                )}
                {corpusIngestPending && corpusUploadProgressPercent !== null && (
                  <div className="composer-upload-progress">
                    <div className="upload-progress-bar">
                      <div
                        className="upload-progress-fill"
                        style={{ width: `${corpusUploadProgressPercent}%` }}
                      />
                    </div>
                    <span>
                      {corpusUploadProgressPercent}% ·{" "}
                      {corpusUploadProgressLabel || "Uploading corpus files..."}
                    </span>
                  </div>
                )}
                {chatUploadNotice && (
                  <span className={chatUploadFailed ? "failed-note" : "muted-text"}>
                    {chatUploadNotice}
                  </span>
                )}
              </div>
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
                <PrivacyReportViewer
                  rawReport={currentReportRaw}
                  showRawReport={showRawReport}
                  onToggleRaw={() => setShowRawReport((current) => !current)}
                />
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
      <ModelRegistryDrawer
        open={modelDrawerOpen}
        onRefresh={loadModels}
        onClose={closeDrawers}
        runtimeValidation={runtimeValidation}
        modelsLoadedAt={modelsLoadedAt}
        modelQuery={modelQuery}
        onModelQueryChange={setModelQuery}
        modelLoadError={modelLoadError}
        models={models}
        filteredModels={filteredModels}
        inspectedModelId={inspectedModelId}
        onInspectModel={setInspectedModelId}
        selectedModelId={selectedModelId}
        inspectedModel={inspectedModel}
        onUseForNextSession={(modelId) => {
          setSelectedModelId(modelId);
          setModelDrawerOpen(false);
        }}
        onSelectAndOpenConfig={(modelId) => {
          setSelectedModelId(modelId);
          openConfigDrawer();
        }}
      />
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

      <SessionRegistryDrawer
        open={registryDrawerOpen}
        onRefresh={loadSessions}
        onClose={closeDrawers}
        registryLoadedAt={registryLoadedAt}
        registryQuery={registryQuery}
        onRegistryQueryChange={setRegistryQuery}
        registryModeFilter={registryModeFilter}
        onRegistryModeFilterChange={setRegistryModeFilter}
        registryOutcomeFilter={registryOutcomeFilter}
        onRegistryOutcomeFilterChange={setRegistryOutcomeFilter}
        registrySortOrder={registrySortOrder}
        onRegistrySortOrderChange={setRegistrySortOrder}
        filteredSessions={filteredSessions}
        sessions={sessions}
        latestSession={latestSession}
        onClearFilters={() => {
          setRegistryQuery("");
          setRegistryModeFilter("all");
          setRegistryOutcomeFilter("all");
          setRegistrySortOrder("newest");
        }}
        onOpenLatestReport={() => {
          if (!latestSession) {
            return;
          }

          setSelectedSessionId(latestSession.session_id);
          openSessionReport(latestSession.session_id);
        }}
        selectedSessionId={selectedSessionId}
        onSelectSession={setSelectedSessionId}
        selectedSession={selectedSession}
        selectedLifecycleResult={selectedLifecycleResult}
        registryActionMessage={registryActionMessage}
        registryActionFailed={registryActionFailed}
        retentionPolicyDraft={retentionPolicyDraft}
        onRetentionPolicyDraftChange={setRetentionPolicyDraft}
        retentionMinutesDraft={retentionMinutesDraft}
        onRetentionMinutesDraftChange={setRetentionMinutesDraft}
        registryActionPending={registryActionPending}
        onSaveRetentionPolicy={saveRegistryRetentionPolicy}
        onOpenSessionReport={openSessionReport}
        onRunLifecycleAction={runRegistryLifecycleAction}
      />
    </main>
  );
}

export default App;
