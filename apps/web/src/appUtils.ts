import type {
  ApiErrorResponse,
  CorpusIngestionReport,
  PrivacyReportData,
  StreamPayload,
} from "./appTypes";

export function statusClass(status: string): string {
  if (status === "successful") return "pill success";
  if (status === "failed") return "pill failed";
  if (status === "warning") return "pill warning";
  if (status === "not_attempted") return "pill muted";
  return "pill neutral";
}

export function inspectionStatusClass(status: string): string {
  if (status === "markers_detected_in_scanned_memory") {
    return "pill failed";
  }

  if (status === "no_markers_detected_in_scanned_regions") {
    return "pill success";
  }

  if (status.includes("startup_failed")) {
    return "pill failed";
  }

  if (
    status.includes("visibility_limited") ||
    status.includes("memory_bytes_unavailable")
  ) {
    return "pill warning";
  }

  if (
    status.includes("not_observed_after_shutdown") ||
    status === "gpu_offload_not_requested"
  ) {
    return "pill success";
  }

  if (status.includes("still_observable_after_shutdown")) {
    return "pill failed";
  }

  if (
    status.includes("inconclusive") ||
    status.includes("unavailable") ||
    status.includes("unsupported") ||
    status.includes("skipped") ||
    status.includes("not_observable") ||
    status.includes("not_completed") ||
    status.includes("not_implemented")
  ) {
    return "pill warning";
  }

  if (status.includes("failed")) {
    return "pill failed";
  }

  return "pill neutral";
}

export function shortId(id: string): string {
  return id.slice(0, 8);
}

export function formatDuration(ms: number): string {
  const totalSeconds = Math.floor(ms / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;

  return `${minutes}m ${seconds.toString().padStart(2, "0")}s`;
}

export function formatTimestamp(value: string): string {
  const parsed = new Date(value);

  if (Number.isNaN(parsed.getTime())) {
    return value;
  }

  return parsed.toLocaleString();
}

export function formatBoolean(value: boolean): string {
  return value ? "yes" : "no";
}

export function formatBytes(value: number): string {
  if (!Number.isFinite(value)) {
    return String(value);
  }

  if (value < 1024) {
    return `${value} B`;
  }

  if (value < 1024 * 1024) {
    return `${(value / 1024).toFixed(1)} KB`;
  }

  return `${(value / (1024 * 1024)).toFixed(1)} MB`;
}

export function formatSignedBytesDelta(value: number): string {
  const prefix = value > 0 ? "+" : "";
  return `${prefix}${formatBytes(Math.abs(value))}${value === 0 ? "" : value > 0 ? " increase" : " decrease"}`;
}

export function minutesUntil(timestamp: string | null | undefined): string {
  if (!timestamp) {
    return "60";
  }

  const parsed = new Date(timestamp);

  if (Number.isNaN(parsed.getTime())) {
    return "60";
  }

  const minutes = Math.max(1, Math.ceil((parsed.getTime() - Date.now()) / 60000));
  return String(minutes);
}

export function humanizeSnakeCase(value: string): string {
  return value.replaceAll("_", " ");
}

export function lifecycleStateClass(state: string): string {
  if (state === "cleanup_succeeded") return "pill success";
  if (state === "cleanup_failed") return "pill failed";
  if (state === "cleanup_pending") return "pill warning";
  if (state === "abandoned_active") return "pill warning";
  if (state === "orphaned") return "pill warning";
  if (state === "active") return "pill warning";
  return "pill neutral";
}

export function modelValidationClass(status: string): string {
  if (status === "ready") return "pill success";
  if (status === "missing" || status === "not_file") return "pill failed";
  return "pill warning";
}

export function parsePositiveInteger(value: string): number | null {
  const parsed = Number(value);

  if (!Number.isInteger(parsed) || parsed <= 0) {
    return null;
  }

  return parsed;
}

export async function readApiError(response: Response, fallback: string): Promise<string> {
  try {
    const text = await response.text();

    if (!text.trim()) {
      return fallback;
    }

    try {
      const data = JSON.parse(text) as ApiErrorResponse;

      if (typeof data.error === "string" && data.error.trim()) {
        return data.error;
      }
    } catch {
      return text;
    }

    return text;
  } catch {
    return fallback;
  }
}

export function formatActiveChatApiError(
  action: "start" | "message" | "cancel" | "end",
  message: string
): string {
  if (
    action === "start" &&
    message.includes("Active chat startup failed before a live session could be created")
  ) {
    return `${message} NullContext did not leave an active chat session running. Review the startup diagnostics and retry after correcting the runtime issue.`;
  }

  if (message.includes("generation is still in progress")) {
    return "The active chat runtime is still finishing the current generation. Wait for streaming to settle, or use Stop and then retry End + Sanitize once the session is idle.";
  }

  if (message.includes("already generating")) {
    return "An active chat generation is already in progress for this session. Wait for it to finish before sending another message.";
  }

  if (message.includes("session is ending")) {
    return "This active chat session is already ending. Wait for End + Sanitize to complete before sending another message.";
  }

  if (message.includes("No active chat generation is currently running")) {
    return "There is no active chat generation running right now, so there was nothing to cancel.";
  }

  if (message.includes("Active chat session not found")) {
    if (action === "end") {
      return "This active chat session is no longer available. Refresh the UI state or start a new session.";
    }

    return "This active chat session is no longer available. Start a new session and try again.";
  }

  return message;
}

export function parseSseBlock(block: string): StreamPayload | null {
  const dataLines = block
    .split("\n")
    .filter((line) => line.startsWith("data:"))
    .map((line) => line.replace(/^data:\s?/, ""));

  if (dataLines.length === 0) {
    return null;
  }

  try {
    return JSON.parse(dataLines.join("\n")) as StreamPayload;
  } catch {
    return {
      type: "error",
      message: `Failed to parse stream event: ${dataLines.join("\n")}`,
    };
  }
}

export function parsePrivacyReport(raw: string): PrivacyReportData | null {
  if (!raw.trim()) {
    return null;
  }

  try {
    return JSON.parse(raw) as PrivacyReportData;
  } catch {
    return null;
  }
}

export function parseCorpusReport(raw: string): CorpusIngestionReport | null {
  if (!raw.trim()) {
    return null;
  }

  try {
    return JSON.parse(raw) as CorpusIngestionReport;
  } catch {
    return null;
  }
}
