import { Hint } from "./Hint";
import type {
  ChatTemplateOption,
  CorpusIndexEntry,
  ModelRegistrySnapshot,
  RegisteredModel,
} from "../appTypes";
import { humanizeSnakeCase } from "../appUtils";

export function SessionConfigDrawer({
  open,
  onClose,
  selectedModelId,
  onSelectedModelIdChange,
  activeChatRuntimeActive,
  models,
  selectedModel,
  modelLoadError,
  runtimeValidation,
  onOpenCorpusDrawer,
  onOpenModelDrawer,
  selectedCorpus,
  corporaLoadedAt,
  corpusLoadError,
  mode,
  onModeChange,
  persistent,
  onPersistentChange,
  chatTemplate,
  onChatTemplateChange,
  useModelTemplateDefault,
  onUseModelTemplateDefaultChange,
  effectiveTemplate,
  chatContextTokenBudget,
  onChatContextTokenBudgetChange,
  chatContextTurnLimit,
  onChatContextTurnLimitChange,
  useModelContextDefaults,
  onUseModelContextDefaultsChange,
  effectiveContextBudget,
  effectiveContextTurnLimit,
  modelsLoadedAt,
}: {
  open: boolean;
  onClose: () => void;
  selectedModelId: string;
  onSelectedModelIdChange: (value: string) => void;
  activeChatRuntimeActive: boolean;
  models: RegisteredModel[];
  selectedModel: RegisteredModel | null;
  modelLoadError: string;
  runtimeValidation: ModelRegistrySnapshot["runtime"] | null;
  onOpenCorpusDrawer: () => void;
  onOpenModelDrawer: () => void;
  selectedCorpus: CorpusIndexEntry | null;
  corporaLoadedAt: string;
  corpusLoadError: string;
  mode: string;
  onModeChange: (value: string) => void;
  persistent: boolean;
  onPersistentChange: (checked: boolean) => void;
  chatTemplate: ChatTemplateOption;
  onChatTemplateChange: (value: ChatTemplateOption) => void;
  useModelTemplateDefault: boolean;
  onUseModelTemplateDefaultChange: (checked: boolean) => void;
  effectiveTemplate: string;
  chatContextTokenBudget: string;
  onChatContextTokenBudgetChange: (value: string) => void;
  chatContextTurnLimit: string;
  onChatContextTurnLimitChange: (value: string) => void;
  useModelContextDefaults: boolean;
  onUseModelContextDefaultsChange: (checked: boolean) => void;
  effectiveContextBudget: number | null;
  effectiveContextTurnLimit: number | null;
  modelsLoadedAt: string;
}) {
  return (
    <aside className={`config-drawer${open ? " open" : ""}`}>
      <div className="drawer-header">
        <div>
          <h3>session config</h3>
          <p>move detailed controls off the main page and keep the shell focused</p>
        </div>
        <button className="ghost-button" onClick={onClose}>
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
                onChange={(event) => onSelectedModelIdChange(event.target.value)}
                disabled={activeChatRuntimeActive || models.length === 0}
              >
                {models.length === 0 ? (
                  <option value="">no models available</option>
                ) : (
                  models.map((model) => (
                    <option key={model.id} value={model.id} disabled={!model.selectable}>
                      {model.name}
                      {model.default_selected ? " | default" : ""}
                      {!model.selectable ? " | unavailable" : ""}
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

          {selectedModel?.description && <p className="microcopy">{selectedModel.description}</p>}

          {selectedModel && (
            <p className="microcopy truncate" title={selectedModel.model_path}>
              {selectedModel.model_path}
            </p>
          )}

          {selectedModel && !selectedModel.selectable && selectedModel.validation_message && (
            <p className="microcopy">{selectedModel.validation_message}</p>
          )}

          {runtimeValidation && !runtimeValidation.selectable && (
            <p className="microcopy">
              {runtimeValidation.validation_message || "llama-server is not ready to launch."}
            </p>
          )}

          <div className="drawer-actions">
            <button className="ghost-button" onClick={onOpenCorpusDrawer}>
              browse corpora
            </button>
            <button className="ghost-button" onClick={onOpenModelDrawer}>
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
                <span>lifecycle: {humanizeSnakeCase(selectedCorpus.lifecycle.state)}</span>
                <span>
                  chunks: {selectedCorpus.chunk_count} | sources: {selectedCorpus.source_count}
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
              onChange={(event) => onModeChange(event.target.value)}
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
              onChange={(event) => onPersistentChange(event.target.checked)}
            />
            persistent
            <Hint text="Persistent is only available in standard mode. Secure and air-gapped sessions remain ephemeral." />
          </label>

          <label>
            chat template
            <div className="field-with-hint">
              <select
                value={chatTemplate}
                onChange={(event) => onChatTemplateChange(event.target.value as ChatTemplateOption)}
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
              onChange={(event) => onUseModelTemplateDefaultChange(event.target.checked)}
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
                onChange={(event) => onChatContextTokenBudgetChange(event.target.value)}
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
                onChange={(event) => onChatContextTurnLimitChange(event.target.value)}
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
              onChange={(event) => onUseModelContextDefaultsChange(event.target.checked)}
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
  );
}
