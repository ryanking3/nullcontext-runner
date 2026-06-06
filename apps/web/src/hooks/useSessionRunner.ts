import { useEffect, useRef, useState } from "react";
import type {
  AuditOperation,
  ChatCancelResponse,
  ChatEndResponse,
  ChatMessage,
  ChatStartResponse,
  ChatStatusResponse,
  ChatTemplateOption,
  CorpusIndexEntry,
  RegisteredModel,
  RunStatus,
  RuntimeMode,
  StreamPayload,
} from "../appTypes";
import {
  formatActiveChatApiError,
  parsePositiveInteger,
  parseSseBlock,
  readApiError,
} from "../appUtils";

export function useSessionRunner({
  apiBase,
  runtimeMode,
  setRuntimeMode,
  prompt,
  setPrompt,
  mode,
  persistent,
  selectedModelId,
  selectedCorpusId,
  selectedCorpus,
  selectedModel,
  useModelTemplateDefault,
  useModelContextDefaults,
  chatTemplate,
  chatContextTokenBudget,
  chatContextTurnLimit,
  onLoadSessions,
  onCloseDrawers,
  onCloseCommandMenu,
}: {
  apiBase: string;
  runtimeMode: RuntimeMode;
  setRuntimeMode: (mode: RuntimeMode) => void;
  prompt: string;
  setPrompt: (value: string) => void;
  mode: string;
  persistent: boolean;
  selectedModelId: string;
  selectedCorpusId: string;
  selectedCorpus: CorpusIndexEntry | null;
  selectedModel: RegisteredModel | null;
  useModelTemplateDefault: boolean;
  useModelContextDefaults: boolean;
  chatTemplate: ChatTemplateOption;
  chatContextTokenBudget: string;
  chatContextTurnLimit: string;
  onLoadSessions: () => Promise<void> | void;
  onCloseDrawers: () => void;
  onCloseCommandMenu: () => void;
}) {
  const activeAbortController = useRef<AbortController | null>(null);

  const [runStatus, setRunStatus] = useState<RunStatus>("idle");
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [runtimeLogs, setRuntimeLogs] = useState("");
  const [privacyReport, setPrivacyReport] = useState("");
  const [stderr, setStderr] = useState("");
  const [auditOperations, setAuditOperations] = useState<AuditOperation[]>([]);
  const [activeChatSessionId, setActiveChatSessionId] = useState("");
  const [activeChatWorkspace, setActiveChatWorkspace] = useState("");
  const [activeChatRuntimeEndpoint, setActiveChatRuntimeEndpoint] = useState("");
  const [activeChatModelId, setActiveChatModelId] = useState("");
  const [activeChatModelName, setActiveChatModelName] = useState("");
  const [activeChatCorpusId, setActiveChatCorpusId] = useState("");
  const [activeChatCorpusName, setActiveChatCorpusName] = useState("");
  const [activeChatTurns, setActiveChatTurns] = useState(0);
  const [activeChatGroundedTurns, setActiveChatGroundedTurns] = useState(0);
  const [activeChatRuntimeActive, setActiveChatRuntimeActive] = useState(false);
  const [activeChatStartedAt, setActiveChatStartedAt] = useState<number | null>(null);
  const [activeRuntimeElapsedMs, setActiveRuntimeElapsedMs] = useState(0);
  const [activeChatRisk, setActiveChatRisk] = useState(
    "Runtime is inactive. No active chat KV/cache state is currently held by NullContext."
  );
  const [activeChatStopNotice, setActiveChatStopNotice] = useState("");
  const [activeChatResolvedTemplate, setActiveChatResolvedTemplate] = useState("");
  const [activeChatHistoryPolicy, setActiveChatHistoryPolicy] = useState("");
  const [activeChatContextBudget, setActiveChatContextBudget] = useState<number | null>(null);
  const [activeChatContextTurnLimit, setActiveChatContextTurnLimit] = useState<number | null>(
    null
  );
  const [activeChatCancelPending, setActiveChatCancelPending] = useState(false);

  function resetRunPanels() {
    setRuntimeLogs("");
    setPrivacyReport("");
    setStderr("");
    setAuditOperations([]);
  }

  function resetOneShotConversation() {
    setMessages([]);
    resetRunPanels();
    setActiveChatStopNotice("");
  }

  function appendAssistantText(text: string) {
    setMessages((current) => {
      const next = [...current];
      const last = next[next.length - 1];

      if (last?.role === "assistant") {
        next[next.length - 1] = {
          ...last,
          content: `${last.content}${text}`,
        };
      } else {
        next.push({
          role: "assistant",
          content: text,
        });
      }

      return next;
    });
  }

  async function refreshActiveChatStatus() {
    if (!activeChatSessionId) {
      return;
    }

    try {
      const response = await fetch(`${apiBase}/api/chat/${activeChatSessionId}/status`);

      if (!response.ok) {
        return;
      }

      const data = (await response.json()) as ChatStatusResponse;

      setActiveChatTurns(data.turns);
      if (typeof data.grounded_turns === "number") {
        setActiveChatGroundedTurns(data.grounded_turns);
      }
      setActiveChatRuntimeActive(data.runtime_active);
      setActiveChatWorkspace(data.workspace);
      setActiveChatRuntimeEndpoint(data.runtime_endpoint);

      if (data.model_id) {
        setActiveChatModelId(data.model_id);
      }

      if (data.model_name) {
        setActiveChatModelName(data.model_name);
      }

      if (data.corpus_id) {
        setActiveChatCorpusId(data.corpus_id);
      }

      if (data.corpus_name) {
        setActiveChatCorpusName(data.corpus_name);
      }

      if (typeof data.runtime_duration_ms === "number") {
        setActiveRuntimeElapsedMs(data.runtime_duration_ms);
      }

      if (data.chat_template) {
        setActiveChatResolvedTemplate(data.chat_template);
      }

      if (typeof data.chat_context_token_budget === "number") {
        setActiveChatContextBudget(data.chat_context_token_budget);
      }

      if (typeof data.chat_context_turn_limit === "number") {
        setActiveChatContextTurnLimit(data.chat_context_turn_limit);
      }

      if (data.history_policy) {
        setActiveChatHistoryPolicy(data.history_policy);
      }

      if (data.residual_risk) {
        setActiveChatRisk(data.residual_risk);
      }
    } catch {
      // Non-critical UI refresh failure.
    }
  }

  function handleStreamPayload(payload: StreamPayload) {
    switch (payload.type) {
      case "runtime": {
        if (payload.message) {
          setRuntimeLogs((current) => `${current}${payload.message}\n`);
        }
        break;
      }

      case "audit": {
        if (payload.operation && payload.status && payload.details) {
          setAuditOperations((current) => [
            ...current,
            {
              operation: payload.operation ?? "unknown",
              status: payload.status ?? "unknown",
              details: payload.details ?? "",
            },
          ]);
        }
        break;
      }

      case "model": {
        if (payload.text) {
          appendAssistantText(payload.text);
          setRuntimeLogs((current) => {
            if (current.includes("--- Model Output ---\n<RESPONSE>\n")) {
              return current;
            }

            return `${current}--- Model Output ---\n<RESPONSE>\n`;
          });
        }
        break;
      }

      case "report": {
        if (payload.text) {
          setPrivacyReport((current) => `${current}${payload.text}`);
        }
        break;
      }

      case "stderr": {
        if (payload.message) {
          setStderr((current) => `${current}${payload.message}\n`);
        }
        break;
      }

      case "error": {
        if (payload.message) {
          setStderr((current) => `${current}${payload.message}\n`);
        }
        setActiveChatCancelPending(false);
        setRunStatus("failed");
        break;
      }

      case "complete": {
        if (runtimeMode === "active-chat" && activeChatCancelPending && !payload.success) {
          setRunStatus("idle");
          setActiveChatCancelPending(false);
          setActiveChatStopNotice(
            "Cancelled this active-chat generation. Any partial assistant text still visible in the transcript was not committed to backend chat history. The runtime remains active until you send another message or use End + Sanitize."
          );
          setRuntimeLogs((current) => `${current}Active chat generation cancelled.\n`);
        } else {
          if (runtimeMode === "active-chat" && activeChatCancelPending && payload.success) {
            setActiveChatStopNotice(
              "Cancellation was requested, but the generation finished before the runtime stopped the turn."
            );
          }
          setRunStatus(payload.success ? "success" : "failed");
          setActiveChatCancelPending(false);
        }

        if (persistent) {
          void onLoadSessions();
        }

        if (runtimeMode === "active-chat" && payload.success) {
          void refreshActiveChatStatus();
        }

        break;
      }

      default: {
        setRuntimeLogs((current) => `${current}${JSON.stringify(payload)}\n`);
      }
    }
  }

  async function consumeSseResponse(response: Response, signal: AbortSignal) {
    if (!response.body) {
      throw new Error("Streaming response body was empty");
    }

    const reader = response.body.getReader();
    const decoder = new TextDecoder();
    let buffer = "";

    while (true) {
      if (signal.aborted) {
        break;
      }

      const { value, done } = await reader.read();

      if (done) {
        break;
      }

      buffer += decoder.decode(value, { stream: true });

      const blocks = buffer.split("\n\n");
      buffer = blocks.pop() ?? "";

      for (const block of blocks) {
        const payload = parseSseBlock(block);

        if (payload) {
          handleStreamPayload(payload);
        }
      }
    }

    if (!signal.aborted && buffer.trim()) {
      const payload = parseSseBlock(buffer);

      if (payload) {
        handleStreamPayload(payload);
      }
    }
  }

  function readActiveChatConfigInputs() {
    if (useModelTemplateDefault || useModelContextDefaults) {
      if (!selectedModel) {
        throw new Error("Select a registered model before starting a session.");
      }
    }

    const overrides: {
      chat_template?: ChatTemplateOption;
      chat_context_token_budget?: number;
      chat_context_turn_limit?: number;
    } = {};

    if (!useModelTemplateDefault) {
      overrides.chat_template = chatTemplate;
    }

    if (!useModelContextDefaults) {
      const tokenBudget = parsePositiveInteger(chatContextTokenBudget);
      const turnLimit = parsePositiveInteger(chatContextTurnLimit);

      if (tokenBudget === null) {
        throw new Error(
          "Active chat context token budget must be a whole number greater than 0."
        );
      }

      if (turnLimit === null) {
        throw new Error("Active chat context turn limit must be a whole number greater than 0.");
      }

      overrides.chat_context_token_budget = tokenBudget;
      overrides.chat_context_turn_limit = turnLimit;
    }

    return overrides;
  }

  function selectedCorpusReadinessError() {
    if (!selectedCorpusId) {
      return null;
    }

    if (!selectedCorpus || selectedCorpus.corpus_id !== selectedCorpusId) {
      return "The selected corpus is not currently available in the loaded registry snapshot. Refresh the corpus drawer and choose a ready corpus before running retrieval.";
    }

    if (selectedCorpus.lifecycle.state !== "ready") {
      return `The selected corpus is not ready for retrieval. Current lifecycle state: ${selectedCorpus.lifecycle.state}.`;
    }

    if (!selectedCorpus.root_exists || !selectedCorpus.manifest_exists) {
      return "The selected corpus is missing required retrieval artifacts. Reconcile the corpus registry or choose a different ready corpus before running retrieval.";
    }

    return null;
  }

  async function runOneShot() {
    const corpusReadinessError = selectedCorpusReadinessError();
    if (corpusReadinessError) {
      setStderr(corpusReadinessError);
      setRunStatus("failed");
      return;
    }

    resetOneShotConversation();
    setRunStatus("running");

    const currentPrompt = prompt;
    const controller = new AbortController();
    activeAbortController.current = controller;

    setMessages([
      {
        role: "user",
        content: currentPrompt,
      },
      {
        role: "assistant",
        content: "",
      },
    ]);

    try {
      const activeChatConfig = readActiveChatConfigInputs();
      onCloseCommandMenu();
      const response = await fetch(`${apiBase}/api/run/stream`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          prompt: currentPrompt,
          mode,
          persistent,
          model_id: selectedModelId || undefined,
          corpus_id: selectedCorpusId || undefined,
          ...activeChatConfig,
        }),
        signal: controller.signal,
      });

      await consumeSseResponse(response, controller.signal);
    } catch (error) {
      if (controller.signal.aborted) {
        setRunStatus("failed");
      } else {
        setStderr(String(error));
        setRunStatus("failed");
      }
    } finally {
      if (activeAbortController.current === controller) {
        activeAbortController.current = null;
      }
    }
  }

  async function startActiveChat() {
    const corpusReadinessError = selectedCorpusReadinessError();
    if (corpusReadinessError) {
      setStderr(corpusReadinessError);
      setRunStatus("failed");
      return;
    }

    resetRunPanels();
    setActiveChatStopNotice("");
    setActiveChatCancelPending(false);
    setRunStatus("running");
    onCloseDrawers();
    onCloseCommandMenu();

    try {
      const activeChatConfig = readActiveChatConfigInputs();
      const response = await fetch(`${apiBase}/api/chat/start`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          mode,
          persistent,
          model_id: selectedModelId || undefined,
          corpus_id: selectedCorpusId || undefined,
          ...activeChatConfig,
        }),
      });

      if (!response.ok) {
        const error = await readApiError(response, "Failed to start active chat session.");
        throw new Error(formatActiveChatApiError("start", error));
      }

      const data = (await response.json()) as ChatStartResponse;

      setRuntimeMode("active-chat");
      setActiveChatSessionId(data.session_id);
      setActiveChatWorkspace(data.workspace);
      setActiveChatRuntimeEndpoint(data.runtime_endpoint);
      setActiveChatModelId(data.model_id);
      setActiveChatModelName(data.model_name);
      setActiveChatCorpusId(data.corpus_id || "");
      setActiveChatCorpusName(data.corpus_name || "");
      setActiveChatTurns(data.turns);
      setActiveChatGroundedTurns(data.grounded_turns);
      setActiveChatRuntimeActive(data.runtime_active);
      setActiveChatStartedAt(Date.now());
      setActiveRuntimeElapsedMs(0);
      setActiveChatRisk(
        "Runtime is active. llama.cpp remains loaded, KV/cache state may remain live, and chat context remains in memory until End + Sanitize."
      );
      setActiveChatResolvedTemplate(data.chat_template);
      setActiveChatHistoryPolicy(data.history_policy);
      setActiveChatContextBudget(data.chat_context_token_budget);
      setActiveChatContextTurnLimit(data.chat_context_turn_limit);
      setRunStatus("success");

      setRuntimeLogs((current) =>
        `${current}Started active chat session ${data.session_id}\nWorkspace: ${data.workspace}\n`
      );
    } catch (error) {
      setStderr(String(error));
      setRunStatus("failed");
    }
  }

  async function endActiveChat() {
    if (!activeChatSessionId) {
      return;
    }

    setRunStatus("running");
    setActiveChatStopNotice("");
    setActiveChatCancelPending(false);
    onCloseCommandMenu();

    try {
      const response = await fetch(`${apiBase}/api/chat/${activeChatSessionId}/end`, {
        method: "POST",
      });

      if (!response.ok) {
        const error = await readApiError(response, "Failed to end active chat session.");
        const friendlyError = formatActiveChatApiError("end", error);
        setActiveChatStopNotice(friendlyError);
        throw new Error(friendlyError);
      }

      const data = (await response.json()) as ChatEndResponse;

      setActiveChatRuntimeActive(false);
      setActiveChatTurns(0);
      setActiveChatStartedAt(null);
      setActiveRuntimeElapsedMs(0);
      setActiveChatRisk(
        "Runtime is inactive. Active chat buffers were finalized according to the session cleanup policy."
      );
      setActiveChatResolvedTemplate("");
      setActiveChatHistoryPolicy("");
      setActiveChatContextBudget(null);
      setActiveChatContextTurnLimit(null);
      setActiveChatModelId("");
      setActiveChatModelName("");
      setActiveChatCorpusId("");
      setActiveChatCorpusName("");
      setActiveChatGroundedTurns(0);
      setPrivacyReport(JSON.stringify(data.report, null, 2));
      setRuntimeLogs((current) =>
        `${current}Ended active chat session ${data.session_id}\nRuntime stopped: ${data.runtime_stopped}\n`
      );

      setActiveChatSessionId("");
      setActiveChatWorkspace("");
      setActiveChatRuntimeEndpoint("");
      setRunStatus("success");

      if (persistent) {
        await onLoadSessions();
      }
    } catch (error) {
      setStderr(String(error));
      setRunStatus("failed");
    }
  }

  async function sendActiveChatMessage() {
    if (!activeChatSessionId) {
      setStderr("No active chat session. Start a session first.");
      setRunStatus("failed");
      return;
    }

    setRunStatus("running");
    setRuntimeLogs("");
    setPrivacyReport("");
    setStderr("");
    setAuditOperations([]);
    setActiveChatStopNotice("");
    setActiveChatCancelPending(false);
    onCloseCommandMenu();

    const currentPrompt = prompt;
    const controller = new AbortController();
    activeAbortController.current = controller;

    setMessages((current) => [
      ...current,
      {
        role: "user",
        content: currentPrompt,
      },
      {
        role: "assistant",
        content: "",
      },
    ]);

    try {
      const response = await fetch(`${apiBase}/api/chat/${activeChatSessionId}/message/stream`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          prompt: currentPrompt,
        }),
        signal: controller.signal,
      });

      if (!response.ok) {
        const error = await readApiError(response, "Failed to send active chat message.");
        throw new Error(formatActiveChatApiError("message", error));
      }

      await consumeSseResponse(response, controller.signal);

      if (!controller.signal.aborted) {
        setPrompt("");
      }
    } catch (error) {
      if (controller.signal.aborted) {
        setRunStatus("failed");
      } else {
        setStderr(String(error));
        setRunStatus("failed");
      }
    } finally {
      if (activeAbortController.current === controller) {
        activeAbortController.current = null;
      }
    }
  }

  async function runSession() {
    if (runtimeMode === "one-shot") {
      await runOneShot();
      return;
    }

    await sendActiveChatMessage();
  }

  async function stopGeneration() {
    if (runtimeMode === "active-chat" && activeChatSessionId) {
      setActiveChatStopNotice(
        "Cancellation requested for this active-chat generation. Waiting for the runtime to stop the current turn before clearing it from backend chat history."
      );
      setActiveChatCancelPending(true);

      try {
        const response = await fetch(`${apiBase}/api/chat/${activeChatSessionId}/cancel`, {
          method: "POST",
        });

        if (!response.ok) {
          const error = await readApiError(response, "Failed to cancel active chat generation.");
          throw new Error(formatActiveChatApiError("cancel", error));
        }

        const data = (await response.json()) as ChatCancelResponse;

        setRuntimeLogs((current) => `${current}${data.message}\n`);
        setAuditOperations((current) => [
          ...current,
          {
            operation: "client_generation_cancel_requested",
            status: "warning",
            details:
              "Client requested explicit cancellation for the current active-chat generation. The runtime will stop the current turn without committing it to chat history.",
          },
        ]);

        return;
      } catch (error) {
        setActiveChatCancelPending(false);
        setActiveChatStopNotice(String(error));
        setStderr((current) => `${current}${String(error)}\n`);
        setRuntimeLogs(
          (current) =>
            `${current}Active chat cancel request failed. Closing the client stream as a fallback.\n`
        );
      }
    }

    activeAbortController.current?.abort();
    activeAbortController.current = null;

    setRunStatus("failed");
    setRuntimeLogs((current) => `${current}Generation stopped by user.\n`);
    setActiveChatStopNotice(
      runtimeMode === "active-chat"
        ? "Stopped this active-chat generation by closing the client stream. Any partial assistant text still visible in the transcript was not committed to backend chat history. The runtime remains active until you send another message or use End + Sanitize."
        : ""
    );

    setAuditOperations((current) => [
      ...current,
      {
        operation: "client_generation_stop",
        status: "warning",
        details:
          runtimeMode === "active-chat"
            ? "Client stopped the current active-chat generation by closing the stream. The chat runtime remains active."
            : "Client stopped the one-shot generation. Backend cleanup should continue server-side.",
      },
    ]);
  }

  useEffect(() => {
    if (!activeChatRuntimeActive || activeChatStartedAt === null) {
      return;
    }

    const interval = window.setInterval(() => {
      setActiveRuntimeElapsedMs(Date.now() - activeChatStartedAt);
    }, 1000);

    return () => window.clearInterval(interval);
  }, [activeChatRuntimeActive, activeChatStartedAt]);

  useEffect(() => {
    function warnBeforeUnload(event: BeforeUnloadEvent) {
      if (!activeChatRuntimeActive) {
        return;
      }

      event.preventDefault();
      event.returnValue =
        "An active NullContext chat session is still running. End + Sanitize before leaving.";
    }

    window.addEventListener("beforeunload", warnBeforeUnload);

    return () => window.removeEventListener("beforeunload", warnBeforeUnload);
  }, [activeChatRuntimeActive]);

  return {
    runStatus,
    messages,
    runtimeLogs,
    privacyReport,
    stderr,
    auditOperations,
    activeChatSessionId,
    activeChatWorkspace,
    activeChatRuntimeEndpoint,
    activeChatModelId,
    activeChatModelName,
    activeChatCorpusId,
    activeChatCorpusName,
    activeChatTurns,
    activeChatGroundedTurns,
    activeChatRuntimeActive,
    activeRuntimeElapsedMs,
    activeChatRisk,
    activeChatStopNotice,
    activeChatResolvedTemplate,
    activeChatHistoryPolicy,
    activeChatContextBudget,
    activeChatContextTurnLimit,
    resetRunPanels,
    startActiveChat,
    endActiveChat,
    runSession,
    stopGeneration,
  };
}
