import { useEffect, useState } from "react";
import type {
  RegistryModeFilter,
  RegistryOutcomeFilter,
  RegistrySortOrder,
  SessionIndexEntry,
  SessionLifecycleActionResponse,
  SessionRegistry,
} from "../appTypes";
import { buildFilteredSessions, buildLatestSession } from "../appSelectors";
import { minutesUntil, parsePositiveInteger, readApiError } from "../appUtils";

export function useSessionRegistry({
  apiBase,
  inspectorView,
  onOpenInspectorReport,
}: {
  apiBase: string;
  inspectorView: string;
  onOpenInspectorReport: () => void;
}) {
  const [sessions, setSessions] = useState<SessionIndexEntry[]>([]);
  const [registryLoadedAt, setRegistryLoadedAt] = useState<string>("never");
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

  async function loadSessions() {
    try {
      const response = await fetch(`${apiBase}/api/sessions`);
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

  async function openReport(sessionId: string) {
    setSelectedSessionId(sessionId);
    setShowRawReport(false);

    const session = sessions.find((entry) => entry.session_id === sessionId);
    if (session?.lifecycle.state === "active") {
      setSelectedReport(
        "This persistent active chat is still live. NullContext writes the privacy report when the session ends and sanitization completes."
      );
      return;
    }

    try {
      const response = await fetch(`${apiBase}/api/reports/${sessionId}`);
      const data = await response.json();

      if (!response.ok) {
        const error = typeof data?.error === "string" ? data.error : "Failed to load report.";
        setSelectedReport(error);
        return;
      }

      setSelectedReport(JSON.stringify(data, null, 2));
    } catch (error) {
      setSelectedReport(String(error));
    }
  }

  function openSessionReport(sessionId: string) {
    onOpenInspectorReport();
    void openReport(sessionId);
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
      const response = await fetch(`${apiBase}/api/sessions/${sessionId}/${action}`, {
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

      const response = await fetch(`${apiBase}/api/sessions/${sessionId}/retention`, {
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

  return {
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
    selectedSession,
    selectedLifecycleResult,
    loadSessions,
    openSessionReport,
    runRegistryLifecycleAction,
    saveRegistryRetentionPolicy,
  };
}
