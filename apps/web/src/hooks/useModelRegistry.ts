import { useState } from "react";
import type { ModelRegistrySnapshot, RegisteredModel } from "../appTypes";
import { buildFilteredModels } from "../appSelectors";
import { readApiError } from "../appUtils";

export function useModelRegistry({ apiBase }: { apiBase: string }) {
  const [models, setModels] = useState<RegisteredModel[]>([]);
  const [runtimeValidation, setRuntimeValidation] =
    useState<ModelRegistrySnapshot["runtime"] | null>(null);
  const [selectedModelId, setSelectedModelId] = useState("");
  const [inspectedModelId, setInspectedModelId] = useState("");
  const [modelsLoadedAt, setModelsLoadedAt] = useState("never");
  const [modelLoadError, setModelLoadError] = useState("");
  const [modelQuery, setModelQuery] = useState("");

  async function loadModels() {
    try {
      const response = await fetch(`${apiBase}/api/models`);

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

  const selectedModel =
    models.find((model) => model.id === selectedModelId) ??
    models.find((model) => model.default_selected) ??
    null;
  const filteredModels = buildFilteredModels(models, modelQuery);
  const inspectedModel =
    filteredModels.find((model) => model.id === inspectedModelId) ??
    models.find((model) => model.id === inspectedModelId) ??
    selectedModel;

  return {
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
  };
}
