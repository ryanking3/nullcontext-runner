import type {
  CorpusIndexEntry,
  RegisteredModel,
  RegistryModeFilter,
  RegistryOutcomeFilter,
  RegistrySortOrder,
  SessionIndexEntry,
} from "./appTypes";

export function buildFilteredSessions(
  sessions: SessionIndexEntry[],
  registryQuery: string,
  registryModeFilter: RegistryModeFilter,
  registryOutcomeFilter: RegistryOutcomeFilter,
  registrySortOrder: RegistrySortOrder
): SessionIndexEntry[] {
  const query = registryQuery.trim().toLowerCase();

  return [...sessions]
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
}

export function buildLatestSession(sessions: SessionIndexEntry[]): SessionIndexEntry | undefined {
  return [...sessions].sort((left, right) => {
    const leftTime = new Date(left.started_at).getTime();
    const rightTime = new Date(right.started_at).getTime();

    return rightTime - leftTime;
  })[0];
}

export function buildFilteredCorpora(
  corpora: CorpusIndexEntry[],
  corpusQuery: string
): CorpusIndexEntry[] {
  const query = corpusQuery.trim().toLowerCase();

  return corpora.filter((corpus) => {
    if (!query) {
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
      .includes(query);
  });
}

export function buildFilteredModels(
  models: RegisteredModel[],
  modelQuery: string
): RegisteredModel[] {
  const query = modelQuery.trim().toLowerCase();

  return models.filter((model) => {
    if (!query) {
      return true;
    }

    return [model.id, model.name, model.description ?? "", model.model_path, model.chat_template]
      .join(" ")
      .toLowerCase()
      .includes(query);
  });
}
