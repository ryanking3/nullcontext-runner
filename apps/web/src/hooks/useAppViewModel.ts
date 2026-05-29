import type { InspectorView, RegisteredModel } from "../appTypes";
import { parsePositiveInteger } from "../appUtils";

export function useAppViewModel({
  selectedModel,
  useModelTemplateDefault,
  chatTemplate,
  useModelContextDefaults,
  chatContextTokenBudget,
  chatContextTurnLimit,
  auditOperationsCount,
  stderr,
  activeChatRuntimeActive,
  activeChatModelName,
  activeChatModelId,
  selectedReport,
  privacyReport,
}: {
  selectedModel: RegisteredModel | null;
  useModelTemplateDefault: boolean;
  chatTemplate: string;
  useModelContextDefaults: boolean;
  chatContextTokenBudget: string;
  chatContextTurnLimit: string;
  auditOperationsCount: number;
  stderr: string;
  activeChatRuntimeActive: boolean;
  activeChatModelName: string;
  activeChatModelId: string;
  selectedReport: string;
  privacyReport: string;
}) {
  const effectiveTemplate = useModelTemplateDefault
    ? selectedModel?.chat_template || "auto"
    : chatTemplate;
  const effectiveContextBudget = useModelContextDefaults
    ? selectedModel?.chat_context_token_budget ?? null
    : parsePositiveInteger(chatContextTokenBudget);
  const effectiveContextTurnLimit = useModelContextDefaults
    ? selectedModel?.chat_context_turn_limit ?? null
    : parsePositiveInteger(chatContextTurnLimit);
  const inspectorTabs: Array<{
    id: InspectorView;
    label: string;
    count?: number;
    disabled?: boolean;
  }> = [
    { id: "audit", label: "audit", count: auditOperationsCount },
    { id: "runtime", label: "runtime" },
    { id: "report", label: "report" },
    { id: "stderr", label: "stderr", disabled: !stderr },
  ];
  const activeRuntimeModelName =
    activeChatRuntimeActive && activeChatModelName
      ? activeChatModelName
      : selectedModel?.name || "unconfigured";
  const activeRuntimeModelId =
    activeChatRuntimeActive && activeChatModelId
      ? activeChatModelId
      : selectedModel?.id || "";
  const currentReportRaw = selectedReport || privacyReport;

  return {
    effectiveTemplate,
    effectiveContextBudget,
    effectiveContextTurnLimit,
    inspectorTabs,
    activeRuntimeModelName,
    activeRuntimeModelId,
    currentReportRaw,
  };
}
