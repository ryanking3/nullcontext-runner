import type { CorpusIndexEntry, RegisteredModel, RuntimeMode, Theme } from "../appTypes";

export function AppSidebar({
  sidebarCollapsed,
  onSidebarCollapsedChange,
  openConfigDrawer,
  openRegistryDrawer,
  inspectorOpen,
  onInspectorOpenChange,
  checkHealth,
  serverStatus,
  healthCheckedAt,
  runtimeMode,
  onRuntimeModeChange,
  activeChatRuntimeActive,
  openCorpusDrawer,
  openModelDrawer,
  selectedModel,
  mode,
  persistent,
  selectedCorpus,
  effectiveTemplate,
  useModelTemplateDefault,
  effectiveContextBudget,
  effectiveContextTurnLimit,
  useModelContextDefaults,
  sessionsCount,
  registryLoadedAt,
  theme,
  onThemeChange,
}: {
  sidebarCollapsed: boolean;
  onSidebarCollapsedChange: (collapsed: boolean) => void;
  openConfigDrawer: () => void;
  openRegistryDrawer: () => void;
  inspectorOpen: boolean;
  onInspectorOpenChange: (open: boolean) => void;
  checkHealth: () => void;
  serverStatus: "checking" | "online" | "offline";
  healthCheckedAt: string;
  runtimeMode: RuntimeMode;
  onRuntimeModeChange: (mode: RuntimeMode) => void;
  activeChatRuntimeActive: boolean;
  openCorpusDrawer: () => void;
  openModelDrawer: () => void;
  selectedModel: RegisteredModel | null;
  mode: string;
  persistent: boolean;
  selectedCorpus: CorpusIndexEntry | null;
  effectiveTemplate: string;
  useModelTemplateDefault: boolean;
  effectiveContextBudget: number | null;
  effectiveContextTurnLimit: number | null;
  useModelContextDefaults: boolean;
  sessionsCount: number;
  registryLoadedAt: string;
  theme: Theme;
  onThemeChange: (theme: Theme) => void;
}) {
  return (
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
            onClick={() => onSidebarCollapsedChange(false)}
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
            onClick={() => onInspectorOpenChange(!inspectorOpen)}
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
                onClick={() => onRuntimeModeChange("one-shot")}
                disabled={activeChatRuntimeActive}
              >
                one-shot
              </button>
              <button
                className={runtimeMode === "active-chat" ? "selected" : ""}
                onClick={() => onRuntimeModeChange("active-chat")}
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
                sessions: {sessionsCount} persistent{sessionsCount === 1 ? " run" : " runs"}
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
                onClick={() => onThemeChange("dark")}
              >
                dark
              </button>
              <button
                className={theme === "light" ? "selected" : ""}
                onClick={() => onThemeChange("light")}
              >
                light
              </button>
            </div>
          </section>
        </>
      )}
    </aside>
  );
}
