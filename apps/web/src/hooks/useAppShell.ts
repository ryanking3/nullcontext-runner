import { useEffect, useRef, useState } from "react";
import type { StartupStatusResponse, Theme } from "../appTypes";

export function useAppShell({
  apiBase,
  onLoadSessions,
  onLoadModels,
  onLoadCorpora,
  onOpenModelDrawerSelectDefault,
  onOpenRegistryDrawerSelectDefault,
  onOpenCorpusDrawerSelectDefault,
}: {
  apiBase: string;
  onLoadSessions: () => void;
  onLoadModels: () => void;
  onLoadCorpora: () => void;
  onOpenModelDrawerSelectDefault: () => void;
  onOpenRegistryDrawerSelectDefault: () => void;
  onOpenCorpusDrawerSelectDefault: () => void;
}) {
  const commandMenuRef = useRef<HTMLDivElement | null>(null);
  const chatUploadMenuRef = useRef<HTMLDivElement | null>(null);
  const chatUploadInputRef = useRef<HTMLInputElement | null>(null);

  const [theme, setTheme] = useState<Theme>("dark");
  const [serverStatus, setServerStatus] = useState<"checking" | "online" | "offline">("checking");
  const [healthCheckedAt, setHealthCheckedAt] = useState<string>("never");
  const [startupStatus, setStartupStatus] = useState<StartupStatusResponse | null>(null);
  const [startupStatusLoadedAt, setStartupStatusLoadedAt] = useState<string>("never");
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [configDrawerOpen, setConfigDrawerOpen] = useState(false);
  const [modelDrawerOpen, setModelDrawerOpen] = useState(false);
  const [registryDrawerOpen, setRegistryDrawerOpen] = useState(false);
  const [corpusDrawerOpen, setCorpusDrawerOpen] = useState(false);
  const [commandMenuOpen, setCommandMenuOpen] = useState(false);
  const [chatUploadMenuOpen, setChatUploadMenuOpen] = useState(false);
  const [chatUploadAccept, setChatUploadAccept] = useState(
    ".txt,.md,.pdf,text/plain,text/markdown,application/pdf"
  );

  async function checkHealth() {
    setServerStatus("checking");

    try {
      const response = await fetch(`${apiBase}/api/health`);
      setServerStatus(response.ok ? "online" : "offline");
    } catch {
      setServerStatus("offline");
    } finally {
      setHealthCheckedAt(new Date().toLocaleTimeString());
    }
  }

  async function loadStartupStatus() {
    try {
      const response = await fetch(`${apiBase}/api/startup-status`);

      if (!response.ok) {
        throw new Error("Failed to load startup recovery status.");
      }

      const data = (await response.json()) as StartupStatusResponse;
      setStartupStatus(data);
    } catch {
      setStartupStatus(null);
    } finally {
      setStartupStatusLoadedAt(new Date().toLocaleTimeString());
    }
  }

  function closeDrawers() {
    setConfigDrawerOpen(false);
    setModelDrawerOpen(false);
    setRegistryDrawerOpen(false);
    setCorpusDrawerOpen(false);
  }

  function openConfigDrawer() {
    setModelDrawerOpen(false);
    setRegistryDrawerOpen(false);
    setCorpusDrawerOpen(false);
    setConfigDrawerOpen(true);
    setCommandMenuOpen(false);
  }

  function openModelDrawer() {
    setConfigDrawerOpen(false);
    setRegistryDrawerOpen(false);
    setCorpusDrawerOpen(false);
    setModelDrawerOpen(true);
    setCommandMenuOpen(false);
    onOpenModelDrawerSelectDefault();
  }

  function openRegistryDrawer() {
    setConfigDrawerOpen(false);
    setModelDrawerOpen(false);
    setCorpusDrawerOpen(false);
    setRegistryDrawerOpen(true);
    setCommandMenuOpen(false);
    onOpenRegistryDrawerSelectDefault();
  }

  function openCorpusDrawer() {
    setConfigDrawerOpen(false);
    setModelDrawerOpen(false);
    setRegistryDrawerOpen(false);
    setCorpusDrawerOpen(true);
    setCommandMenuOpen(false);
    onOpenCorpusDrawerSelectDefault();
  }

  function openChatUploadPicker(accept: string) {
    setChatUploadAccept(accept);
    setChatUploadMenuOpen(false);
    chatUploadInputRef.current?.click();
  }

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
  }, [theme]);

  useEffect(() => {
    void checkHealth();
    void loadStartupStatus();
    onLoadSessions();
    onLoadModels();
    onLoadCorpora();
  }, []);

  useEffect(() => {
    function closeOnEscape(event: KeyboardEvent) {
      if (event.key === "Escape") {
        closeDrawers();
        setCommandMenuOpen(false);
        setChatUploadMenuOpen(false);
      }
    }

    window.addEventListener("keydown", closeOnEscape);

    return () => window.removeEventListener("keydown", closeOnEscape);
  }, []);

  useEffect(() => {
    function closeCommandMenu(event: MouseEvent) {
      if (
        commandMenuOpen &&
        commandMenuRef.current &&
        !commandMenuRef.current.contains(event.target as Node)
      ) {
        setCommandMenuOpen(false);
      }

      if (
        chatUploadMenuOpen &&
        chatUploadMenuRef.current &&
        !chatUploadMenuRef.current.contains(event.target as Node)
      ) {
        setChatUploadMenuOpen(false);
      }
    }

    window.addEventListener("mousedown", closeCommandMenu);

    return () => window.removeEventListener("mousedown", closeCommandMenu);
  }, [chatUploadMenuOpen, commandMenuOpen]);

  return {
    commandMenuRef,
    chatUploadMenuRef,
    chatUploadInputRef,
    theme,
    setTheme,
    serverStatus,
    healthCheckedAt,
    startupStatus,
    startupStatusLoadedAt,
    sidebarCollapsed,
    setSidebarCollapsed,
    configDrawerOpen,
    modelDrawerOpen,
    registryDrawerOpen,
    corpusDrawerOpen,
    commandMenuOpen,
    setCommandMenuOpen,
    chatUploadMenuOpen,
    setChatUploadMenuOpen,
    chatUploadAccept,
    checkHealth,
    closeDrawers,
    openConfigDrawer,
    openModelDrawer,
    openRegistryDrawer,
    openCorpusDrawer,
    openChatUploadPicker,
  };
}
