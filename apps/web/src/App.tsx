import { useEffect, useRef, useState } from "react";
import { AppSidebar } from "./components/AppSidebar";
import { CorpusDrawer } from "./components/CorpusDrawer";
import { ChatWorkspace } from "./components/ChatWorkspace";
import { ModelRegistryDrawer } from "./components/ModelRegistryDrawer";
import { InspectorPanel } from "./components/InspectorPanel";
import { SessionConfigDrawer } from "./components/SessionConfigDrawer";
import { SessionRegistryDrawer } from "./components/SessionRegistryDrawer";
import { useCorpusManager } from "./hooks/useCorpusManager";
import { useSessionRunner } from "./hooks/useSessionRunner";
import type {
  ChatTemplateOption,
  InspectorView,
  ModelRegistrySnapshot,
  RegisteredModel,
  RegistryModeFilter,
  RegistryOutcomeFilter,
  RegistrySortOrder,
  RuntimeMode,
  SessionIndexEntry,
  SessionLifecycleActionResponse,
  SessionRegistry,
  Theme,
} from "./appTypes";
import {
  buildFilteredModels,
  buildFilteredSessions,
  buildLatestSession,
} from "./appSelectors";
import {
  minutesUntil,
  parsePositiveInteger,
  readApiError,
} from "./appUtils";
import "./App.css";

const API_BASE = "http://127.0.0.1:3333";

function App() {
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
  const [chatUploadAccept, setChatUploadAccept] = useState(
    ".txt,.md,.pdf,text/plain,text/markdown,application/pdf"
  );
  const {
    corpora,
    selectedCorpusId,
    setSelectedCorpusId,
    corporaLoadedAt,
    corpusLoadError,
    corpusQuery,
    setCorpusQuery,
    corpusIngestName,
    setCorpusIngestName,
    corpusIngestPaths,
    setCorpusIngestPaths,
    corpusIngestPersistent,
    setCorpusIngestPersistent,
    corpusIngestOcrEnabled,
    setCorpusIngestOcrEnabled,
    corpusIngestPending,
    corpusIngestMessage,
    corpusIngestFailed,
    corpusUploadFiles,
    corpusUploadInputKey,
    corpusUploadProgressPercent,
    corpusUploadProgressLabel,
    corpusUploadDragActive,
    setCorpusUploadDragActive,
    chatUploadNotice,
    chatUploadFailed,
    lastIngestedCorpusReport,
    corpusActionPending,
    corpusActionMessage,
    corpusActionFailed,
    corpusRetentionPolicyDraft,
    setCorpusRetentionPolicyDraft,
    corpusRetentionMinutesDraft,
    setCorpusRetentionMinutesDraft,
    selectedCorpusReport,
    filteredCorpora,
    selectedCorpus,
    selectedCorpusLifecycleResult,
    currentCorpusReport,
    loadCorpora,
    ingestCorpusFromForm,
    ingestUploadedCorpusFromForm,
    handleCorpusUploadSelection,
    ingestUploadedCorpusFromChat: ingestUploadedCorpusFromChatAction,
    openCorpusReport,
    runCorpusLifecycleAction,
    saveCorpusRetentionPolicy,
  } = useCorpusManager({
    apiBase: API_BASE,
  });

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

  function openChatUploadPicker(accept: string) {
    setChatUploadAccept(accept);
    setChatUploadMenuOpen(false);
    chatUploadInputRef.current?.click();
  }

  async function ingestUploadedCorpusFromChat(files: File[]) {
    try {
      await ingestUploadedCorpusFromChatAction(files, {
        runtimeMode,
        activeChatRuntimeActive,
      });
    } finally {
      setChatUploadMenuOpen(false);
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
    const nextSessions = buildFilteredSessions(
      sessions,
      registryQuery,
      registryModeFilter,
      registryOutcomeFilter,
      registrySortOrder
    );

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

  const selectedModel =
    models.find((model) => model.id === selectedModelId) ??
    models.find((model) => model.default_selected) ??
    null;
  const filteredModels = buildFilteredModels(models, modelQuery);
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
  const {
    runStatus,
    messages,
    runtimeLogs,
    privacyReport,
    stderr,
    auditOperations,
    activeChatSessionId,
    activeChatWorkspace,
    activeChatModelId,
    activeChatModelName,
    activeChatCorpusId,
    activeChatCorpusName,
    activeChatTurns,
    activeChatGroundedTurns,
    activeChatRuntimeActive,
    activeRuntimeElapsedMs,
    activeChatRisk,
    activeChatStopNotice,
    activeChatResolvedTemplate,
    activeChatHistoryPolicy,
    activeChatContextBudget,
    activeChatContextTurnLimit,
    startActiveChat,
    endActiveChat,
    runSession,
    stopGeneration,
  } = useSessionRunner({
    apiBase: API_BASE,
    runtimeMode,
    setRuntimeMode,
    prompt,
    setPrompt,
    mode,
    persistent,
    selectedModelId,
    selectedCorpusId,
    selectedModel,
    useModelTemplateDefault,
    useModelContextDefaults,
    chatTemplate,
    chatContextTokenBudget,
    chatContextTurnLimit,
    onLoadSessions: loadSessions,
    onCloseDrawers: closeDrawers,
    onCloseCommandMenu: () => setCommandMenuOpen(false),
  });
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
  const filteredSessions = buildFilteredSessions(
    sessions,
    registryQuery,
    registryModeFilter,
    registryOutcomeFilter,
    registrySortOrder
  );
  const latestSession = buildLatestSession(sessions);
  const selectedSession =
    filteredSessions.find((session) => session.session_id === selectedSessionId) ?? null;
  const selectedLifecycleResult =
    registryActionResult && registryActionResult.session_id === selectedSessionId
      ? registryActionResult
      : null;
  const activeRuntimeModelName =
    activeChatRuntimeActive && activeChatModelName
      ? activeChatModelName
      : selectedModel?.name || "unconfigured";
  const activeRuntimeModelId =
    activeChatRuntimeActive && activeChatModelId
      ? activeChatModelId
      : selectedModel?.id || "";
  const currentReportRaw = selectedReport || privacyReport;

  return (
    <main
      className={`shell${sidebarCollapsed ? " sidebar-collapsed" : ""}${
        inspectorOpen ? "" : " inspector-hidden"
      }`}
    >
      <AppSidebar
        sidebarCollapsed={sidebarCollapsed}
        onSidebarCollapsedChange={setSidebarCollapsed}
        openConfigDrawer={openConfigDrawer}
        openRegistryDrawer={openRegistryDrawer}
        inspectorOpen={inspectorOpen}
        onInspectorOpenChange={setInspectorOpen}
        checkHealth={checkHealth}
        serverStatus={serverStatus}
        healthCheckedAt={healthCheckedAt}
        runtimeMode={runtimeMode}
        onRuntimeModeChange={setRuntimeMode}
        activeChatRuntimeActive={activeChatRuntimeActive}
        openCorpusDrawer={openCorpusDrawer}
        openModelDrawer={openModelDrawer}
        selectedModel={selectedModel}
        mode={mode}
        persistent={persistent}
        selectedCorpus={selectedCorpus}
        effectiveTemplate={effectiveTemplate}
        useModelTemplateDefault={useModelTemplateDefault}
        effectiveContextBudget={effectiveContextBudget}
        effectiveContextTurnLimit={effectiveContextTurnLimit}
        useModelContextDefaults={useModelContextDefaults}
        sessionsCount={sessions.length}
        registryLoadedAt={registryLoadedAt}
        theme={theme}
        onThemeChange={setTheme}
      />

      <ChatWorkspace
        runtimeMode={runtimeMode}
        serverStatus={serverStatus}
        activeChatRuntimeActive={activeChatRuntimeActive}
        activeChatRisk={activeChatRisk}
        activeChatWorkspace={activeChatWorkspace}
        activeRuntimeModelName={activeRuntimeModelName}
        activeRuntimeModelId={activeRuntimeModelId}
        activeChatCorpusName={activeChatCorpusName}
        activeChatCorpusId={activeChatCorpusId}
        selectedCorpus={selectedCorpus}
        selectedModel={selectedModel}
        activeChatHistoryPolicy={activeChatHistoryPolicy}
        activeChatResolvedTemplate={activeChatResolvedTemplate}
        activeChatContextBudget={activeChatContextBudget}
        activeChatContextTurnLimit={activeChatContextTurnLimit}
        activeChatGroundedTurns={activeChatGroundedTurns}
        effectiveTemplate={effectiveTemplate}
        useModelTemplateDefault={useModelTemplateDefault}
        effectiveContextBudget={effectiveContextBudget}
        effectiveContextTurnLimit={effectiveContextTurnLimit}
        useModelContextDefaults={useModelContextDefaults}
        activeChatTurns={activeChatTurns}
        activeRuntimeElapsedMs={activeRuntimeElapsedMs}
        activeChatSessionId={activeChatSessionId}
        startActiveChat={startActiveChat}
        endActiveChat={endActiveChat}
        runStatus={runStatus}
        commandMenuRef={commandMenuRef}
        commandMenuOpen={commandMenuOpen}
        onCommandMenuOpenChange={setCommandMenuOpen}
        onOpenModelDrawer={openModelDrawer}
        onOpenCorpusDrawer={openCorpusDrawer}
        onOpenConfigDrawer={openConfigDrawer}
        onRefreshModels={loadModels}
        onRefreshCorpora={loadCorpora}
        onOpenRegistryDrawer={openRegistryDrawer}
        inspectorOpen={inspectorOpen}
        onInspectorOpenChange={setInspectorOpen}
        sidebarCollapsed={sidebarCollapsed}
        onSidebarCollapsedChange={setSidebarCollapsed}
        onCheckHealth={checkHealth}
        activeChatStopNotice={activeChatStopNotice}
        messages={messages}
        chatUploadMenuRef={chatUploadMenuRef}
        chatUploadMenuOpen={chatUploadMenuOpen}
        onChatUploadMenuOpenChange={setChatUploadMenuOpen}
        corpusIngestPending={corpusIngestPending}
        openChatUploadPicker={openChatUploadPicker}
        chatUploadInputRef={chatUploadInputRef}
        chatUploadAccept={chatUploadAccept}
        ingestUploadedCorpusFromChat={ingestUploadedCorpusFromChat}
        prompt={prompt}
        onPromptChange={setPrompt}
        stopGeneration={stopGeneration}
        runSession={runSession}
        chatUploadNotice={chatUploadNotice}
        chatUploadFailed={chatUploadFailed}
        corpusUploadProgressPercent={corpusUploadProgressPercent}
        corpusUploadProgressLabel={corpusUploadProgressLabel}
      />

      <InspectorPanel
        open={inspectorOpen}
        onClose={() => setInspectorOpen(false)}
        tabs={inspectorTabs}
        inspectorView={inspectorView}
        onInspectorViewChange={setInspectorView}
        auditOperations={auditOperations}
        runtimeLogs={runtimeLogs}
        currentReportRaw={currentReportRaw}
        showRawReport={showRawReport}
        onToggleRaw={() => setShowRawReport((current) => !current)}
        stderr={stderr}
      />

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
      <SessionConfigDrawer
        open={configDrawerOpen}
        onClose={closeDrawers}
        selectedModelId={selectedModelId}
        onSelectedModelIdChange={setSelectedModelId}
        activeChatRuntimeActive={activeChatRuntimeActive}
        models={models}
        selectedModel={selectedModel}
        modelLoadError={modelLoadError}
        runtimeValidation={runtimeValidation}
        onOpenCorpusDrawer={openCorpusDrawer}
        onOpenModelDrawer={openModelDrawer}
        selectedCorpus={selectedCorpus}
        corporaLoadedAt={corporaLoadedAt}
        corpusLoadError={corpusLoadError}
        mode={mode}
        onModeChange={setMode}
        persistent={persistent}
        onPersistentChange={setPersistent}
        chatTemplate={chatTemplate}
        onChatTemplateChange={setChatTemplate}
        useModelTemplateDefault={useModelTemplateDefault}
        onUseModelTemplateDefaultChange={setUseModelTemplateDefault}
        effectiveTemplate={effectiveTemplate}
        chatContextTokenBudget={chatContextTokenBudget}
        onChatContextTokenBudgetChange={setChatContextTokenBudget}
        chatContextTurnLimit={chatContextTurnLimit}
        onChatContextTurnLimitChange={setChatContextTurnLimit}
        useModelContextDefaults={useModelContextDefaults}
        onUseModelContextDefaultsChange={setUseModelContextDefaults}
        effectiveContextBudget={effectiveContextBudget}
        effectiveContextTurnLimit={effectiveContextTurnLimit}
        modelsLoadedAt={modelsLoadedAt}
      />

      <CorpusDrawer
        open={corpusDrawerOpen}
        onRefresh={loadCorpora}
        onClose={closeDrawers}
        corporaLoadedAt={corporaLoadedAt}
        corpusQuery={corpusQuery}
        onCorpusQueryChange={setCorpusQuery}
        corpusLoadError={corpusLoadError}
        corpora={corpora}
        filteredCorpora={filteredCorpora}
        selectedCorpusId={selectedCorpusId}
        onSelectCorpus={setSelectedCorpusId}
        selectedCorpus={selectedCorpus}
        selectedCorpusLifecycleResult={selectedCorpusLifecycleResult}
        corpusActionMessage={corpusActionMessage}
        corpusActionFailed={corpusActionFailed}
        corpusRetentionPolicyDraft={corpusRetentionPolicyDraft}
        onCorpusRetentionPolicyDraftChange={setCorpusRetentionPolicyDraft}
        corpusRetentionMinutesDraft={corpusRetentionMinutesDraft}
        onCorpusRetentionMinutesDraftChange={setCorpusRetentionMinutesDraft}
        corpusActionPending={corpusActionPending}
        onSaveCorpusRetentionPolicy={saveCorpusRetentionPolicy}
        onUseCorpusForOneShot={(corpusId) => {
          setSelectedCorpusId(corpusId);
          openConfigDrawer();
        }}
        onOpenCorpusReport={openCorpusReport}
        onRunCorpusLifecycleAction={runCorpusLifecycleAction}
        selectedCorpusReport={selectedCorpusReport}
        currentCorpusReport={currentCorpusReport}
        corpusIngestMessage={corpusIngestMessage}
        corpusIngestFailed={corpusIngestFailed}
        corpusIngestName={corpusIngestName}
        onCorpusIngestNameChange={setCorpusIngestName}
        corpusIngestPaths={corpusIngestPaths}
        onCorpusIngestPathsChange={setCorpusIngestPaths}
        corpusUploadDragActive={corpusUploadDragActive}
        corpusIngestPending={corpusIngestPending}
        onCorpusUploadDragActiveChange={setCorpusUploadDragActive}
        onCorpusUploadSelection={handleCorpusUploadSelection}
        corpusUploadInputKey={corpusUploadInputKey}
        corpusUploadFiles={corpusUploadFiles}
        corpusIngestPersistent={corpusIngestPersistent}
        onCorpusIngestPersistentChange={setCorpusIngestPersistent}
        corpusIngestOcrEnabled={corpusIngestOcrEnabled}
        onCorpusIngestOcrEnabledChange={setCorpusIngestOcrEnabled}
        onIngestCorpusFromPaths={ingestCorpusFromForm}
        onIngestUploadedCorpus={ingestUploadedCorpusFromForm}
        corpusUploadProgressPercent={corpusUploadProgressPercent}
        corpusUploadProgressLabel={corpusUploadProgressLabel}
        lastIngestedCorpusReport={lastIngestedCorpusReport}
      />

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
