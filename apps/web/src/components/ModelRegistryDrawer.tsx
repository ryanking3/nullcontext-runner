import type { ModelRegistrySnapshot, RegisteredModel } from "../appTypes";
import { modelValidationClass } from "../appUtils";

export function ModelRegistryDrawer({
  open,
  onRefresh,
  onClose,
  runtimeValidation,
  modelsLoadedAt,
  modelQuery,
  onModelQueryChange,
  modelLoadError,
  models,
  filteredModels,
  inspectedModelId,
  onInspectModel,
  selectedModelId,
  inspectedModel,
  onUseForNextSession,
  onSelectAndOpenConfig,
}: {
  open: boolean;
  onRefresh: () => void;
  onClose: () => void;
  runtimeValidation: ModelRegistrySnapshot["runtime"] | null;
  modelsLoadedAt: string;
  modelQuery: string;
  onModelQueryChange: (value: string) => void;
  modelLoadError: string;
  models: RegisteredModel[];
  filteredModels: RegisteredModel[];
  inspectedModelId: string;
  onInspectModel: (modelId: string) => void;
  selectedModelId: string;
  inspectedModel: RegisteredModel | null;
  onUseForNextSession: (modelId: string) => void;
  onSelectAndOpenConfig: (modelId: string) => void;
}) {
  return (
    <aside className={`model-drawer${open ? " open" : ""}`}>
      <div className="drawer-header">
        <div>
          <h3>model registry</h3>
          <p>inspect registered models, compare runtime defaults, and choose the next session model</p>
        </div>
        <div className="drawer-actions">
          <button className="ghost-button" onClick={onRefresh}>
            refresh
          </button>
          <button className="ghost-button" onClick={onClose}>
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
                onChange={(event) => onModelQueryChange(event.target.value)}
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
                  onClick={() => onInspectModel(model.id)}
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
                  onClick={() => onUseForNextSession(inspectedModel.id)}
                  disabled={!inspectedModel.selectable}
                >
                  use for next session
                </button>
                <button
                  className="ghost-button"
                  onClick={() => onSelectAndOpenConfig(inspectedModel.id)}
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
  );
}
