import { useState } from "react";
import { AppSidebar } from "./components/AppSidebar";
import { CorpusDrawer } from "./components/CorpusDrawer";
import { ChatWorkspace } from "./components/ChatWorkspace";
import { ModelRegistryDrawer } from "./components/ModelRegistryDrawer";
import { InspectorPanel } from "./components/InspectorPanel";
import { SessionConfigDrawer } from "./components/SessionConfigDrawer";
import { SessionRegistryDrawer } from "./components/SessionRegistryDrawer";
import { useAppShell } from "./hooks/useAppShell";
import { useAppViewModel } from "./hooks/useAppViewModel";
import { useCorpusManager } from "./hooks/useCorpusManager";
import { useModelRegistry } from "./hooks/useModelRegistry";
import { useSessionRegistry } from "./hooks/useSessionRegistry";
import { useSessionRunner } from "./hooks/useSessionRunner";
import type { ChatTemplateOption, InspectorView, RuntimeMode } from "./appTypes";
import "./App.css";

const API_BASE =
  import.meta.env.VITE_API_BASE?.trim() || "http://127.0.0.1:3333";

function App() {
  const [runtimeMode, setRuntimeMode] = useState<RuntimeMode>("one-shot");
  const [prompt, setPrompt] = useState("");
  const [mode, setMode] = useState("secure");
  const [persistent, setPersistent] = useState(false);
  const [inspectorOpen, setInspectorOpen] = useState(true);
  const [inspectorView, setInspectorView] = useState<InspectorView>("audit");
  const [chatTemplate, setChatTemplate] = useState<ChatTemplateOption>("auto");
  const [chatContextTokenBudget, setChatContextTokenBudget] = useState("2048");
  const [chatContextTurnLimit, setChatContextTurnLimit] = useState("12");
  const [useModelTemplateDefault, setUseModelTemplateDefault] = useState(true);
  const [useModelContextDefaults, setUseModelContextDefaults] = useState(true);
  const {
    models,
    runtimeValidation,
    selectedModelId,
    setSelectedModelId,
    inspectedModelId,
    setInspectedModelId,
    modelsLoadedAt,
    modelLoadError,
    modelQuery,
    setModelQuery,
    selectedModel,
    filteredModels,
    inspectedModel,
    loadModels,
  } = useModelRegistry({
    apiBase: API_BASE,
  });
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

  function handleOpenInspectorReport() {
    setInspectorView("report");
    setInspectorOpen(true);
  }

  const {
    sessions,
    registryLoadedAt,
    selectedReport,
    selectedSessionId,
    setSelectedSessionId,
    showRawReport,
    setShowRawReport,
    registryActionPending,
    registryActionMessage,
    registryActionFailed,
    retentionPolicyDraft,
    setRetentionPolicyDraft,
    retentionMinutesDraft,
    setRetentionMinutesDraft,
    registryQuery,
    setRegistryQuery,
    registryModeFilter,
    setRegistryModeFilter,
    registryOutcomeFilter,
    setRegistryOutcomeFilter,
    registrySortOrder,
    setRegistrySortOrder,
    filteredSessions,
    latestSession,
    latestReportableSession,
    selectedSession,
    selectedLifecycleResult,
    loadSessions,
    openSessionReport,
    runRegistryLifecycleAction,
    saveRegistryRetentionPolicy,
  } = useSessionRegistry({
    apiBase: API_BASE,
    inspectorView,
    onOpenInspectorReport: handleOpenInspectorReport,
  });

  function ensureModelDrawerSelection() {
    if (!inspectedModelId && models.length > 0) {
      setInspectedModelId(selectedModelId || models[0].id);
    }
  }

  function ensureRegistrySessionSelection() {
    if (!selectedSessionId && sessions.length > 0) {
      setSelectedSessionId(sessions[0].session_id);
    }
  }

  function ensureCorpusSelection() {
    if (!selectedCorpusId && corpora.length > 0) {
      setSelectedCorpusId(corpora[0].corpus_id);
    }
  }

  const {
    commandMenuRef,
    chatUploadMenuRef,
    chatUploadInputRef,
    theme,
    setTheme,
    serverStatus,
    healthCheckedAt,
    startupStatus,
    startupStatusLoadedAt,
    sidebarCollapsed,
    setSidebarCollapsed,
    configDrawerOpen,
    modelDrawerOpen,
    registryDrawerOpen,
    corpusDrawerOpen,
    commandMenuOpen,
    setCommandMenuOpen,
    chatUploadMenuOpen,
    setChatUploadMenuOpen,
    chatUploadAccept,
    checkHealth,
    closeDrawers,
    openConfigDrawer,
    openModelDrawer,
    openRegistryDrawer,
    openCorpusDrawer,
    openChatUploadPicker,
  } = useAppShell({
    apiBase: API_BASE,
    onLoadSessions: loadSessions,
    onLoadModels: loadModels,
    onLoadCorpora: loadCorpora,
    onOpenModelDrawerSelectDefault: ensureModelDrawerSelection,
    onOpenRegistryDrawerSelectDefault: ensureRegistrySessionSelection,
    onOpenCorpusDrawerSelectDefault: ensureCorpusSelection,
  });

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

  function handleCloseCommandMenu() {
    setCommandMenuOpen(false);
  }

  const {
    runStatus,
    messages,
    runtimeLogs,
    privacyReport,
    stderr,
    auditOperations,
    activeChatSessionId,
    activeChatWorkspace,
    activeChatRuntimeEndpoint,
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
    selectedCorpus,
    selectedModel,
    useModelTemplateDefault,
    useModelContextDefaults,
    chatTemplate,
    chatContextTokenBudget,
    chatContextTurnLimit,
    onLoadSessions: loadSessions,
    onCloseDrawers: closeDrawers,
    onCloseCommandMenu: handleCloseCommandMenu,
  });
  const {
    effectiveTemplate,
    effectiveContextBudget,
    effectiveContextTurnLimit,
    inspectorTabs,
    activeRuntimeModelName,
    activeRuntimeModelId,
    currentReportRaw,
  } = useAppViewModel({
    selectedModel,
    useModelTemplateDefault,
    chatTemplate,
    useModelContextDefaults,
    chatContextTokenBudget,
    chatContextTurnLimit,
    auditOperationsCount: auditOperations.length,
    stderr,
    activeChatRuntimeActive,
    activeChatModelName,
    activeChatModelId,
    selectedReport,
    privacyReport,
  });

  function handleUseModelForNextSession(modelId: string) {
    setSelectedModelId(modelId);
    closeDrawers();
  }

  function handleSelectModelAndOpenConfig(modelId: string) {
    setSelectedModelId(modelId);
    openConfigDrawer();
  }

  function handleUseCorpusForOneShot(corpusId: string) {
    setSelectedCorpusId(corpusId);
    openConfigDrawer();
  }

  function handleClearRegistryFilters() {
    setRegistryQuery("");
    setRegistryModeFilter("all");
    setRegistryOutcomeFilter("all");
    setRegistrySortOrder("newest");
  }

  function handleOpenLatestReport() {
    if (!latestReportableSession) {
      return;
    }

    setSelectedSessionId(latestReportableSession.session_id);
    openSessionReport(latestReportableSession.session_id);
  }

  function handleToggleRawReport() {
    setShowRawReport((current) => !current);
  }

  const shellClassName = `shell${sidebarCollapsed ? " sidebar-collapsed" : ""}${
    inspectorOpen ? "" : " inspector-hidden"
  }`;
  const drawersOpen =
    configDrawerOpen || modelDrawerOpen || registryDrawerOpen || corpusDrawerOpen;

  return (
    <main className={shellClassName}>
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
        startupStatus={startupStatus}
        startupStatusLoadedAt={startupStatusLoadedAt}
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
        activeChatRuntimeEndpoint={activeChatRuntimeEndpoint}
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
        onToggleRaw={handleToggleRawReport}
        stderr={stderr}
      />

      <div
        className={`drawer-backdrop${drawersOpen ? " open" : ""}`}
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
        onUseForNextSession={handleUseModelForNextSession}
        onSelectAndOpenConfig={handleSelectModelAndOpenConfig}
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
        onUseCorpusForOneShot={handleUseCorpusForOneShot}
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
        latestReportableSession={latestReportableSession}
        onClearFilters={handleClearRegistryFilters}
        onOpenLatestReport={handleOpenLatestReport}
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
