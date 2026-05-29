import { useEffect, useState } from "react";
import type {
  ApiErrorResponse,
  CorpusIndexEntry,
  CorpusIngestionReport,
  CorpusLifecycleActionResponse,
  CorpusRegistrySnapshot,
  IngestCorpusResponse,
  RuntimeMode,
} from "../appTypes";
import { buildFilteredCorpora } from "../appSelectors";
import { formatBytes, minutesUntil, parseCorpusReport, parsePositiveInteger, readApiError } from "../appUtils";

export function useCorpusManager({
  apiBase,
  runtimeMode,
  activeChatRuntimeActive,
}: {
  apiBase: string;
  runtimeMode: RuntimeMode;
  activeChatRuntimeActive: boolean;
}) {
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

  async function loadCorpora() {
    try {
      const response = await fetch(`${apiBase}/api/corpora`);

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

      request.open("POST", `${apiBase}/api/corpora/upload`);
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
      const response = await fetch(`${apiBase}/api/corpora`, {
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

  function handleCorpusUploadSelection(files: File[]) {
    setCorpusUploadFiles(filterSupportedCorpusFiles(files));
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
    }
  }

  async function openCorpusReport(corpusId: string) {
    try {
      const response = await fetch(`${apiBase}/api/corpora/${corpusId}/report`);

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

  async function runCorpusLifecycleAction(corpusId: string, action: "cleanup" | "reconcile") {
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
      const response = await fetch(`${apiBase}/api/corpora/${corpusId}/${action}`, {
        method: "POST",
      });

      if (!response.ok) {
        const error = await readApiError(response, `Failed to ${action} corpus lifecycle state.`);
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

      const response = await fetch(`${apiBase}/api/corpora/${corpusId}/retention`, {
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

  const filteredCorpora = buildFilteredCorpora(corpora, corpusQuery);
  const selectedCorpus =
    filteredCorpora.find((corpus) => corpus.corpus_id === selectedCorpusId) ??
    corpora.find((corpus) => corpus.corpus_id === selectedCorpusId) ??
    null;
  const selectedCorpusLifecycleResult =
    corpusActionResult && corpusActionResult.corpus_id === selectedCorpusId
      ? corpusActionResult
      : null;
  const currentCorpusReport = parseCorpusReport(selectedCorpusReport);

  return {
    corpora,
    setCorpora,
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
    ingestUploadedCorpusFromChat,
    openCorpusReport,
    runCorpusLifecycleAction,
    saveCorpusRetentionPolicy,
  };
}
