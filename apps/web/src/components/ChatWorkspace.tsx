import type { RefObject } from "react";
import type {
  ChatMessage,
  CorpusIndexEntry,
  RegisteredModel,
  RunStatus,
  RuntimeMode,
} from "../appTypes";
import { formatDuration, shortId } from "../appUtils";

export function ChatWorkspace({
  runtimeMode,
  serverStatus,
  activeChatRuntimeActive,
  activeChatRisk,
  activeChatWorkspace,
  activeChatRuntimeEndpoint,
  activeRuntimeModelName,
  activeRuntimeModelId,
  activeChatCorpusName,
  activeChatCorpusId,
  selectedCorpus,
  selectedModel,
  activeChatHistoryPolicy,
  activeChatResolvedTemplate,
  activeChatContextBudget,
  activeChatContextTurnLimit,
  activeChatGroundedTurns,
  effectiveTemplate,
  useModelTemplateDefault,
  effectiveContextBudget,
  effectiveContextTurnLimit,
  useModelContextDefaults,
  activeChatTurns,
  activeRuntimeElapsedMs,
  activeChatSessionId,
  startActiveChat,
  endActiveChat,
  runStatus,
  commandMenuRef,
  commandMenuOpen,
  onCommandMenuOpenChange,
  onOpenModelDrawer,
  onOpenCorpusDrawer,
  onOpenConfigDrawer,
  onRefreshModels,
  onRefreshCorpora,
  onOpenRegistryDrawer,
  inspectorOpen,
  onInspectorOpenChange,
  sidebarCollapsed,
  onSidebarCollapsedChange,
  onCheckHealth,
  activeChatStopNotice,
  messages,
  chatUploadMenuRef,
  chatUploadMenuOpen,
  onChatUploadMenuOpenChange,
  corpusIngestPending,
  openChatUploadPicker,
  chatUploadInputRef,
  chatUploadAccept,
  ingestUploadedCorpusFromChat,
  prompt,
  onPromptChange,
  stopGeneration,
  runSession,
  chatUploadNotice,
  chatUploadFailed,
  corpusUploadProgressPercent,
  corpusUploadProgressLabel,
}: {
  runtimeMode: RuntimeMode;
  serverStatus: "checking" | "online" | "offline";
  activeChatRuntimeActive: boolean;
  activeChatRisk: string;
  activeChatWorkspace: string;
  activeChatRuntimeEndpoint: string;
  activeRuntimeModelName: string;
  activeRuntimeModelId: string;
  activeChatCorpusName: string;
  activeChatCorpusId: string;
  selectedCorpus: CorpusIndexEntry | null;
  selectedModel: RegisteredModel | null;
  activeChatHistoryPolicy: string;
  activeChatResolvedTemplate: string;
  activeChatContextBudget: number | null;
  activeChatContextTurnLimit: number | null;
  activeChatGroundedTurns: number;
  effectiveTemplate: string;
  useModelTemplateDefault: boolean;
  effectiveContextBudget: number | null;
  effectiveContextTurnLimit: number | null;
  useModelContextDefaults: boolean;
  activeChatTurns: number;
  activeRuntimeElapsedMs: number;
  activeChatSessionId: string;
  startActiveChat: () => void;
  endActiveChat: () => void;
  runStatus: RunStatus;
  commandMenuRef: RefObject<HTMLDivElement | null>;
  commandMenuOpen: boolean;
  onCommandMenuOpenChange: (open: boolean) => void;
  onOpenModelDrawer: () => void;
  onOpenCorpusDrawer: () => void;
  onOpenConfigDrawer: () => void;
  onRefreshModels: () => void;
  onRefreshCorpora: () => void;
  onOpenRegistryDrawer: () => void;
  inspectorOpen: boolean;
  onInspectorOpenChange: (open: boolean) => void;
  sidebarCollapsed: boolean;
  onSidebarCollapsedChange: (collapsed: boolean) => void;
  onCheckHealth: () => void;
  activeChatStopNotice: string;
  messages: ChatMessage[];
  chatUploadMenuRef: RefObject<HTMLDivElement | null>;
  chatUploadMenuOpen: boolean;
  onChatUploadMenuOpenChange: (open: boolean) => void;
  corpusIngestPending: boolean;
  openChatUploadPicker: (accept: string) => void;
  chatUploadInputRef: RefObject<HTMLInputElement | null>;
  chatUploadAccept: string;
  ingestUploadedCorpusFromChat: (files: File[]) => Promise<void>;
  prompt: string;
  onPromptChange: (value: string) => void;
  stopGeneration: () => void;
  runSession: () => void;
  chatUploadNotice: string;
  chatUploadFailed: boolean;
  corpusUploadProgressPercent: number | null;
  corpusUploadProgressLabel: string;
}) {
  return (
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
          {runtimeMode === "active-chat" && activeChatRuntimeEndpoint && (
            <div className="runtime-path truncate" title={activeChatRuntimeEndpoint}>
              endpoint: {activeChatRuntimeEndpoint}
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
                corpus: {selectedCorpus.name} (
                {selectedCorpus.persistent ? "persistent" : "ephemeral"})
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
                onClick={() => onCommandMenuOpenChange(!commandMenuOpen)}
                title="Open runtime actions menu"
              >
                actions
              </button>

              {commandMenuOpen && (
                <div className="popup-panel">
                  <button onClick={onOpenModelDrawer}>open model drawer</button>
                  <button onClick={onOpenCorpusDrawer}>open corpus drawer</button>
                  <button onClick={onOpenConfigDrawer}>open config drawer</button>
                  <button
                    onClick={() => {
                      onRefreshModels();
                      onCommandMenuOpenChange(false);
                    }}
                  >
                    refresh model registry
                  </button>
                  <button
                    onClick={() => {
                      onRefreshCorpora();
                      onCommandMenuOpenChange(false);
                    }}
                  >
                    refresh corpus registry
                  </button>
                  <button onClick={onOpenRegistryDrawer}>open registry drawer</button>
                  <button
                    onClick={() => {
                      onInspectorOpenChange(!inspectorOpen);
                      onCommandMenuOpenChange(false);
                    }}
                  >
                    {inspectorOpen ? "hide inspector" : "show inspector"}
                  </button>
                  <button
                    onClick={() => {
                      onSidebarCollapsedChange(!sidebarCollapsed);
                      onCommandMenuOpenChange(false);
                    }}
                  >
                    {sidebarCollapsed ? "expand sidebar" : "collapse sidebar"}
                  </button>
                  <button
                    onClick={() => {
                      onCheckHealth();
                      onCommandMenuOpenChange(false);
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
              <div className="bubble">{message.content || (runStatus === "running" ? "running..." : "")}</div>
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
              onClick={() => onChatUploadMenuOpenChange(!chatUploadMenuOpen)}
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
            onChange={(event) => onPromptChange(event.target.value)}
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
              disabled={prompt.trim() === "" || (runtimeMode === "active-chat" && !activeChatRuntimeActive)}
            >
              send
            </button>
          )}

          {(selectedCorpus || chatUploadNotice || corpusIngestPending) && (
            <div className="composer-meta">
              {selectedCorpus && (
                <span className="composer-corpus-chip">
                  corpus: {selectedCorpus.name} |{" "}
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
                    {corpusUploadProgressPercent}% |{" "}
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
  );
}
