import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { Dispatch, DragEvent, HTMLAttributes, MutableRefObject, SetStateAction } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";

import AppShell from "./components/layout/AppShell/AppShell";
import Sidebar from "./components/organisms/Sidebar/Sidebar";
import Topbar from "./components/organisms/Topbar/Topbar";
import Toast from "./components/organisms/Toast/Toast";
import AnalyzeView from "./views/AnalyzeView/AnalyzeView";
import CleanView from "./views/CleanView/CleanView";
import LogsView from "./views/LogsView/LogsView";
import { CLEANUP_EMPTY, NAV_ITEMS } from "./constants";
import type {
  CleanFileItem,
  CleanupProgress,
  CleanupState,
  DirectoryAnalysisSummary
} from "./types/cleanup";
import type { MetadataReport, ReportEntry } from "./types/metadata";
import type {
  CleanMode,
  DropTarget,
  ExportFormat,
  LogEntry,
  OfficeField,
  Toast as ToastType,
  ToastKind,
  ViewId
} from "./types/ui";
import { buildOfficeValues, extractSystem, getEntry } from "./utils/metadata";

const IMAGE_EXTENSIONS = new Set(["jpg", "jpeg", "png", "tiff", "tif"]);
const OFFICE_EXTENSIONS = new Set(["docx", "xlsx", "pptx"]);
const NO_EXTENSION_LABEL = "sin extension";
type LogSeverity = "warning" | "error";

const normalizePath = (path: string) => {
  let normalized = path.trim();
  if (normalized.startsWith("\\\\?\\")) {
    normalized = normalized.slice(4);
  }
  return normalized.replace(/\\/g, "/");
};

const getFileName = (path: string) => path.split(/[\\/]/).pop() ?? path;

const getExtension = (path: string) => {
  const base = getFileName(path);
  const parts = base.split(".");
  if (parts.length <= 1) return "";
  return parts[parts.length - 1]?.toLowerCase() ?? "";
};

const isSupportedPath = (path: string) => {
  const ext = getExtension(path);
  const isImage = IMAGE_EXTENSIONS.has(ext);
  const isOffice = OFFICE_EXTENSIONS.has(ext);
  return isImage || isOffice;
};

const buildSummaryFromPaths = (paths: string[]): DirectoryAnalysisSummary => {
  const extensionCounts = new Map<string, number>();
  const imageExtensions = new Set<string>();
  const officeExtensions = new Set<string>();
  let images = 0;
  let office = 0;

  for (const path of paths) {
    const ext = getExtension(path);
    const key = ext || NO_EXTENSION_LABEL;
    extensionCounts.set(key, (extensionCounts.get(key) ?? 0) + 1);
    if (IMAGE_EXTENSIONS.has(ext)) {
      images += 1;
      imageExtensions.add(ext);
    }
    if (OFFICE_EXTENSIONS.has(ext)) {
      office += 1;
      officeExtensions.add(ext);
    }
  }

  const sortedExtensions = Array.from(extensionCounts.entries()).sort((a, b) => {
    if (b[1] !== a[1]) return b[1] - a[1];
    return a[0].localeCompare(b[0]);
  });

  return {
    total_files: paths.length,
    images_count: images,
    office_count: office,
    extension_counts: sortedExtensions,
    image_extensions: Array.from(imageExtensions),
    office_extensions: Array.from(officeExtensions)
  };
};

const buildCleanItems = (paths: string[]): CleanFileItem[] => {
  const uniquePaths = Array.from(new Set(paths));
  return uniquePaths.map((path) => ({
    path,
    name: getFileName(path),
    analysisStatus: "queued",
    analysisError: "",
    report: null,
    cleanupStatus: "idle",
    cleanupError: ""
  }));
};

const formatLogDetail = (detail: unknown) => {
  if (detail === undefined) return "";
  if (detail === null) return "null";
  if (typeof detail === "string") return detail;
  if (detail instanceof Error) {
    return detail.stack || detail.message;
  }
  try {
    return JSON.stringify(
      detail,
      (_key, value) => {
        if (value instanceof Error) {
          return { message: value.message, stack: value.stack };
        }
        return value;
      },
      2
    );
  } catch {
    return String(detail);
  }
};

export default function App() {
  const [view, setView] = useState<ViewId>("analyze");
  const [filePath, setFilePath] = useState("");
  const [includeHash, setIncludeHash] = useState(true);
  const [report, setReport] = useState<MetadataReport | null>(null);
  const [reportError, setReportError] = useState("");
  const [officeValues, setOfficeValues] = useState<Record<OfficeField, string>>({
    author: "",
    title: "",
    subject: "",
    company: ""
  });

  const [cleanMode, setCleanMode] = useState<CleanMode>("directory");
  const [dirPath, setDirPath] = useState("");
  const [selectedFiles, setSelectedFiles] = useState<string[]>([]);
  const [recursive, setRecursive] = useState(false);
  const [exportFormat, setExportFormat] = useState<ExportFormat>("json");
  const [dirSummary, setDirSummary] = useState<DirectoryAnalysisSummary | null>(null);
  const [fileSummary, setFileSummary] = useState<DirectoryAnalysisSummary | null>(null);
  const [cleanup, setCleanup] = useState<CleanupState>(CLEANUP_EMPTY);
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [dirItems, setDirItems] = useState<CleanFileItem[]>([]);
  const [fileItems, setFileItems] = useState<CleanFileItem[]>([]);

  const [toast, setToast] = useState<ToastType | null>(null);
  const toastTimer = useRef<number | null>(null);
  const lastDropRef = useRef<{ signature: string; time: number } | null>(null);
  const pendingDropPathsRef = useRef<string[]>([]);
  const logIndexRef = useRef(0);
  const fileAnalysisTokenRef = useRef(0);
  const dirAnalysisTokenRef = useRef(0);
  const dirLoadTokenRef = useRef(0);
  const cleanupTargetsRef = useRef<Set<string>>(new Set());
  const cleanupOrderRef = useRef<string[]>([]);
  const cleanupIndexRef = useRef(0);

  const [dropTarget, setDropTarget] = useState<DropTarget | null>(null);
  const dropTargetRef = useRef<DropTarget | null>(null);

  const [busy, setBusy] = useState({
    analyze: false,
    dirAnalyze: false,
    fileAnalyze: false,
    cleanup: false,
    remove: false,
    edit: false,
    export: false
  });

  const logEvent = useCallback(
    (level: LogSeverity, message: string, detail?: unknown, context = "app") => {
      const entry: LogEntry = {
        id: `${Date.now()}-${logIndexRef.current++}`,
        time: new Date().toISOString(),
        level,
        context,
        message,
        detail: formatLogDetail(detail)
      };
      setLogs((prev) => [...prev, entry]);
    },
    []
  );

  const systemEntries = useMemo(() => extractSystem(report), [report]);
  const mimeEntry = useMemo<ReportEntry | null>(() => getEntry(report, "Tipo MIME"), [report]);
  const typeEntry = useMemo<ReportEntry | null>(() => {
    return getEntry(report, "Tipo de archivo") ?? getEntry(report, "Tipo");
  }, [report]);

  const extensionCounts = useMemo(() => {
    const summary = cleanMode === "directory" ? dirSummary : fileSummary;
    if (!summary) return [] as [string, number][];
    return summary.extension_counts.slice(0, 8);
  }, [cleanMode, dirSummary, fileSummary]);

  const isOffice = useMemo(() => {
    const extension = filePath.split(".").pop()?.toLowerCase();
    if (extension && ["docx", "xlsx", "pptx"].includes(extension)) return true;
    if (mimeEntry?.value) {
      return (
        mimeEntry.value.includes("officedocument") ||
        mimeEntry.value.includes("msword") ||
        mimeEntry.value.includes("ms-excel") ||
        mimeEntry.value.includes("ms-powerpoint")
      );
    }
    return false;
  }, [filePath, mimeEntry]);

  const updateItemsByPaths = (
    paths: string[],
    updater: (item: CleanFileItem) => CleanFileItem
  ) => {
    const normalized = paths.map(normalizePath).filter(Boolean);
    if (!normalized.length) return;
    const pathSet = new Set(normalized);
    setFileItems((prev) =>
      prev.map((item) =>
        pathSet.has(normalizePath(item.path)) ? updater(item) : item
      )
    );
    setDirItems((prev) =>
      prev.map((item) =>
        pathSet.has(normalizePath(item.path)) ? updater(item) : item
      )
    );
  };

  const updateItemByPath = (path: string, updater: (item: CleanFileItem) => CleanFileItem) => {
    updateItemsByPaths([path], updater);
  };

  const runAnalysisQueue = async (
    paths: string[],
    setItems: Dispatch<SetStateAction<CleanFileItem[]>>,
    tokenRef: MutableRefObject<number>
  ) => {
    const token = ++tokenRef.current;
    for (const path of paths) {
      if (tokenRef.current !== token) return;
      setItems((prev) =>
        prev.map((item) =>
          item.path === path
            ? { ...item, analysisStatus: "analyzing", analysisError: "" }
            : item
        )
      );
      try {
        const report = await invoke<MetadataReport>("analyze_file", {
          path,
          includeHash: true
        });
        if (tokenRef.current !== token) return;
        setItems((prev) =>
          prev.map((item) =>
            item.path === path
              ? { ...item, analysisStatus: "ready", report, analysisError: "" }
              : item
          )
        );
      } catch (error) {
        if (tokenRef.current !== token) return;
        setItems((prev) =>
          prev.map((item) =>
            item.path === path
              ? { ...item, analysisStatus: "error", analysisError: String(error) }
              : item
          )
        );
        logEvent("error", "Analisis fallo", { path, error }, "clean-analyze");
      }
    }
  };

  const refreshFileItems = (paths: string[]) => {
    const filtered = paths.filter((path) => isSupportedPath(path));
    if (!filtered.length) {
      fileAnalysisTokenRef.current += 1;
      setFileItems([]);
      setFileSummary(null);
      if (paths.length) {
        showToast("warning", "No hay archivos compatibles para limpiar");
        logEvent("warning", "Sin archivos compatibles para limpiar", { paths }, "clean");
      }
      return;
    }
    setFileSummary(buildSummaryFromPaths(filtered));
    setFileItems(buildCleanItems(filtered));
    void runAnalysisQueue(filtered, setFileItems, fileAnalysisTokenRef);
  };

  const refreshDirectoryItems = async (path: string) => {
    const token = ++dirLoadTokenRef.current;
    setDirItems([]);
    setDirSummary(null);
    dirAnalysisTokenRef.current += 1;
    try {
      const files = await invoke<string[]>("list_cleanup_files", {
        path,
        recursive,
        filter: "all"
      });
      if (dirLoadTokenRef.current !== token) return;
      if (!files.length) {
        showToast("warning", "No hay archivos compatibles en el directorio");
        logEvent("warning", "Directorio sin archivos compatibles", { path }, "clean");
        return;
      }
      setDirSummary(buildSummaryFromPaths(files));
      setDirItems(buildCleanItems(files));
      void runAnalysisQueue(files, setDirItems, dirAnalysisTokenRef);
    } catch (error) {
      if (dirLoadTokenRef.current !== token) return;
      showToast("error", `No se pudo cargar el directorio: ${error}`);
      logEvent("error", "Error al cargar directorio", { path, error }, "clean");
    }
  };

  useEffect(() => {
    setOfficeValues(buildOfficeValues(report));
  }, [report]);

  useEffect(() => {
    let stop: (() => void) | null = null;
    listen<CleanupProgress>("cleanup://progress", (event) => {
      const payload = event.payload;
      if (payload.type === "failure") {
        logEvent("error", "Limpieza fallo", payload, "cleanup");
      }
      if (payload.type === "finished" && payload.failures > 0) {
        logEvent("warning", "Limpieza finalizada con errores", payload, "cleanup");
      }
      if (payload.type === "started") {
        setCleanup({
          running: true,
          total: payload.total,
          index: 0,
          successes: 0,
          failures: 0,
          current: "",
          lastError: "",
          finished: false
        });
        cleanupIndexRef.current = 0;
        const targets = Array.from(cleanupTargetsRef.current);
        if (targets.length) {
          updateItemsByPaths(targets, (item) => ({
            ...item,
            cleanupStatus: "queued",
            cleanupError: ""
          }));
        }
      }
      if (payload.type === "processing") {
        setCleanup((prev) => ({
          ...prev,
          running: true,
          total: payload.total,
          index: payload.index,
          current: payload.path
        }));
        cleanupIndexRef.current = payload.index;
        const fallbackPath = cleanupOrderRef.current[payload.index - 1];
        updateItemsByPaths([payload.path, fallbackPath ?? ""], (item) => ({
          ...item,
          cleanupStatus: "cleaning",
          cleanupError: ""
        }));
      }
      if (payload.type === "success") {
        setCleanup((prev) => ({
          ...prev,
          successes: prev.successes + 1,
          current: payload.path
        }));
        const fallbackPath = cleanupOrderRef.current[cleanupIndexRef.current - 1];
        updateItemsByPaths([payload.path, fallbackPath ?? ""], (item) => ({
          ...item,
          cleanupStatus: "success",
          cleanupError: ""
        }));
      }
      if (payload.type === "failure") {
        setCleanup((prev) => ({
          ...prev,
          failures: prev.failures + 1,
          current: payload.path,
          lastError: payload.error
        }));
        const fallbackPath = cleanupOrderRef.current[cleanupIndexRef.current - 1];
        updateItemsByPaths([payload.path, fallbackPath ?? ""], (item) => ({
          ...item,
          cleanupStatus: "error",
          cleanupError: payload.error
        }));
      }
      if (payload.type === "finished") {
        setCleanup((prev) => ({
          ...prev,
          running: false,
          finished: true,
          successes: payload.successes,
          failures: payload.failures
        }));
        cleanupTargetsRef.current = new Set();
        cleanupOrderRef.current = [];
        cleanupIndexRef.current = 0;
        setBusy((prev) => ({ ...prev, cleanup: false }));
        showToast(
          payload.failures > 0 ? "warning" : "success",
          `Limpieza completa: ${payload.successes} ok, ${payload.failures} errores`
        );
      }
    })
      .then((unlisten) => {
        stop = unlisten;
      })
      .catch((error) => {
        showToast("error", "No se pudo suscribir a eventos de limpieza");
        logEvent("error", "Suscripcion fallida a eventos de limpieza", error, "cleanup");
      });

    return () => {
      if (stop) stop();
    };
  }, []);

  const showToast = (kind: ToastKind, message: string) => {
    if (toastTimer.current) {
      window.clearTimeout(toastTimer.current);
    }
    setToast({ kind, message });
    toastTimer.current = window.setTimeout(() => setToast(null), 3600);
    if (kind === "warning" || kind === "error") {
      logEvent(kind, `Toast: ${message}`, undefined, "toast");
    }
  };

  useEffect(() => {
    if (cleanMode !== "files") return;
    if (!selectedFiles.length) {
      fileAnalysisTokenRef.current += 1;
      setFileItems([]);
      setFileSummary(null);
      return;
    }
    refreshFileItems(selectedFiles);
  }, [cleanMode, selectedFiles]);

  useEffect(() => {
    if (cleanMode !== "directory") return;
    if (!dirPath.trim()) {
      dirAnalysisTokenRef.current += 1;
      setDirItems([]);
      setDirSummary(null);
      return;
    }
    void refreshDirectoryItems(dirPath);
  }, [cleanMode, dirPath, recursive]);

  const setDropTargetState = (target: DropTarget | null) => {
    dropTargetRef.current = target;
    setDropTarget(target);
  };

  const resolveDropTarget = () => {
    if (view === "analyze") return "analyze-file";
    return cleanMode === "directory" ? "clean-directory" : "clean-files";
  };

  const applyFilePath = (path: string, message = "Archivo seleccionado") => {
    setFilePath(path);
    setReport(null);
    setReportError("");
    showToast("info", message);
  };

  const applyDirectoryPath = (path: string, message = "Directorio seleccionado") => {
    setDirPath(path);
    setDirSummary(null);
    setDirItems([]);
    dirAnalysisTokenRef.current += 1;
    setCleanup(CLEANUP_EMPTY);
    cleanupTargetsRef.current = new Set();
    cleanupOrderRef.current = [];
    cleanupIndexRef.current = 0;
    showToast("info", message);
  };

  const applyFiles = (paths: string[], message?: string) => {
    setSelectedFiles(paths);
    setFileSummary(null);
    setFileItems([]);
    fileAnalysisTokenRef.current += 1;
    setCleanup(CLEANUP_EMPTY);
    cleanupTargetsRef.current = new Set();
    cleanupOrderRef.current = [];
    cleanupIndexRef.current = 0;
    showToast("info", message ?? `${paths.length} archivos seleccionados`);
  };

  const applyDroppedPaths = (paths: string[], target: DropTarget) => {
    if (!paths.length) return;
    if (target === "analyze-file") {
      applyFilePath(paths[0], "Archivo cargado");
      return;
    }
    if (target === "clean-directory") {
      applyDirectoryPath(paths[0], "Directorio cargado");
      return;
    }
    applyFiles(paths, paths.length === 1 ? "Archivo cargado" : "Archivos cargados");
  };

  const applyDroppedPathsOnce = (paths: string[], target: DropTarget) => {
    if (!paths.length) return;
    const signature = paths.join("|");
    const now = Date.now();
    const lastDrop = lastDropRef.current;
    if (lastDrop && lastDrop.signature === signature && now - lastDrop.time < 400) {
      return;
    }
    lastDropRef.current = { signature, time: now };
    applyDroppedPaths(paths, target);
  };

  const extractPathsFromEvent = (event: DragEvent<HTMLDivElement>) => {
    const files = Array.from(event.dataTransfer?.files ?? []);
    return files
      .map((file) => (file as File & { path?: string }).path)
      .filter((path): path is string => Boolean(path));
  };

  const dropZoneHandlers = (target: DropTarget): HTMLAttributes<HTMLDivElement> => ({
    onDragEnter: (event: DragEvent<HTMLDivElement>) => {
      event.preventDefault();
      setDropTargetState(target);
    },
    onDragOver: (event: DragEvent<HTMLDivElement>) => {
      event.preventDefault();
      if (dropTargetRef.current !== target) {
        setDropTargetState(target);
      }
    },
    onDragLeave: (event: DragEvent<HTMLDivElement>) => {
      const relatedTarget = event.relatedTarget as Node | null;
      if (relatedTarget && event.currentTarget.contains(relatedTarget)) {
        return;
      }
      setDropTargetState(null);
    },
    onDrop: (event: DragEvent<HTMLDivElement>) => {
      event.preventDefault();
      const droppedPaths = extractPathsFromEvent(event);
      if (droppedPaths.length) {
        applyDroppedPathsOnce(droppedPaths, target);
      }
      setDropTargetState(null);
    }
  });

  useEffect(() => {
    let stopWebview: (() => void) | null = null;
    let stopWindow: (() => void) | null = null;

    const handleDragDropEvent = (event: { payload: { type: string; paths?: string[] } }) => {
      if (event.payload.type === "enter" || event.payload.type === "over") {
        if (event.payload.paths?.length) {
          pendingDropPathsRef.current = event.payload.paths;
        }
        setDropTargetState(resolveDropTarget());
        return;
      }
      if (event.payload.type === "drop") {
        const target = dropTargetRef.current ?? resolveDropTarget();
        const paths =
          event.payload.paths && event.payload.paths.length > 0
            ? event.payload.paths
            : pendingDropPathsRef.current;
        applyDroppedPathsOnce(paths ?? [], target);
        setDropTargetState(null);
        pendingDropPathsRef.current = [];
        return;
      }
      if (event.payload.type === "leave") {
        setDropTargetState(null);
        pendingDropPathsRef.current = [];
      }
    };

    getCurrentWebview()
      .onDragDropEvent(handleDragDropEvent)
      .then((unlisten) => {
        stopWebview = unlisten;
      })
      .catch((error) => {
        showToast("error", "No se pudo habilitar arrastrar y soltar");
      });

    getCurrentWindow()
      .onDragDropEvent(handleDragDropEvent)
      .then((unlisten) => {
        stopWindow = unlisten;
      })
      .catch((error) => {
        showToast("error", "No se pudo habilitar arrastrar y soltar");
      });

    return () => {
      if (stopWebview) stopWebview();
      if (stopWindow) stopWindow();
    };
  }, [view, cleanMode]);

  const handlePickFile = async () => {
    try {
      const selected = await invoke<string | null>("pick_file");
      if (selected) {
        applyFilePath(selected);
      } else {
        logEvent("warning", "Selector de archivo cancelado", undefined, "picker");
      }
    } catch (error) {
      showToast("error", `No se pudo abrir el selector: ${error}`);
      logEvent("error", "Error en selector de archivo", error, "picker");
    }
  };

  const handlePickDirectory = async () => {
    try {
      const selected = await invoke<string | null>("pick_directory");
      if (selected) {
        applyDirectoryPath(selected);
      } else {
        logEvent("warning", "Selector de directorio cancelado", undefined, "picker");
      }
    } catch (error) {
      showToast("error", `No se pudo abrir el selector: ${error}`);
      logEvent("error", "Error en selector de directorio", error, "picker");
    }
  };

  const handlePickFiles = async () => {
    try {
      const selected = await invoke<string[] | null>("pick_files");
      if (selected && selected.length) {
        applyFiles(selected);
      } else {
        logEvent("warning", "Selector de archivos cancelado", undefined, "picker");
      }
    } catch (error) {
      showToast("error", `No se pudo abrir el selector: ${error}`);
      logEvent("error", "Error en selector de archivos", error, "picker");
    }
  };

  const handleAnalyze = async () => {
    if (!filePath.trim()) {
      showToast("warning", "Selecciona un archivo");
      logEvent("warning", "Analisis solicitado sin archivo", undefined, "analyze");
      return;
    }
    setBusy((prev) => ({ ...prev, analyze: true }));
    setReportError("");
    try {
      const result = await invoke<MetadataReport>("analyze_file", {
        path: filePath,
        includeHash: includeHash
      });
      setReport(result);
      showToast("success", "Analisis completado");
    } catch (error) {
      setReportError(String(error));
      showToast("error", "No se pudo analizar el archivo");
      logEvent("error", "Analisis fallo", error, "analyze");
    } finally {
      setBusy((prev) => ({ ...prev, analyze: false }));
    }
  };

  const handleCleanItem = async (path: string) => {
    if (busy.cleanup) {
      showToast("warning", "Ya hay una limpieza en curso");
      logEvent("warning", "Limpieza solicitada con otra en curso", { path }, "cleanup");
      return;
    }
    cleanupTargetsRef.current = new Set([path]);
    cleanupOrderRef.current = [path];
    cleanupIndexRef.current = 0;
    updateItemByPath(path, (item) => ({
      ...item,
      cleanupStatus: "queued",
      cleanupError: ""
    }));
    setBusy((prev) => ({ ...prev, cleanup: true }));
    setCleanup(CLEANUP_EMPTY);
    try {
      await invoke("start_cleanup_files", {
        paths: [path],
        filter: "all"
      });
      showToast("info", "Limpieza iniciada");
    } catch (error) {
      setBusy((prev) => ({ ...prev, cleanup: false }));
      cleanupTargetsRef.current = new Set();
      cleanupOrderRef.current = [];
      cleanupIndexRef.current = 0;
      updateItemByPath(path, (item) => ({ ...item, cleanupStatus: "idle" }));
      showToast("error", `No se pudo iniciar la limpieza: ${error}`);
      logEvent("error", "Fallo iniciar limpieza individual", { path, error }, "cleanup");
    }
  };

  const handleCleanAll = async () => {
    if (busy.cleanup) {
      showToast("warning", "Ya hay una limpieza en curso");
      logEvent("warning", "Limpieza global solicitada con otra en curso", undefined, "cleanup");
      return;
    }
    const activeItems = cleanMode === "directory" ? dirItems : fileItems;
    const readyItems = activeItems.filter((item) => item.analysisStatus === "ready");
    if (!readyItems.length) {
      showToast("warning", "No hay archivos listos para limpiar");
      logEvent("warning", "Limpieza global sin archivos listos", undefined, "cleanup");
      return;
    }
    const paths = readyItems.map((item) => item.path);
    cleanupTargetsRef.current = new Set(paths);
    cleanupOrderRef.current = paths;
    cleanupIndexRef.current = 0;
    updateItemsByPaths(paths, (item) => ({
      ...item,
      cleanupStatus: "queued",
      cleanupError: ""
    }));
    setBusy((prev) => ({ ...prev, cleanup: true }));
    setCleanup(CLEANUP_EMPTY);
    try {
      await invoke("start_cleanup_files", {
        paths,
        filter: "all"
      });
      showToast("info", "Limpieza iniciada");
    } catch (error) {
      setBusy((prev) => ({ ...prev, cleanup: false }));
      cleanupTargetsRef.current = new Set();
      cleanupOrderRef.current = [];
      cleanupIndexRef.current = 0;
      updateItemsByPaths(paths, (item) => ({ ...item, cleanupStatus: "idle" }));
      showToast("error", `No se pudo iniciar la limpieza: ${error}`);
      logEvent("error", "Fallo iniciar limpieza global", { paths, error }, "cleanup");
    }
  };

  const handleRemoveMetadata = async () => {
    if (!filePath.trim()) {
      showToast("warning", "Selecciona un archivo");
      logEvent("warning", "Eliminar metadata sin archivo", undefined, "analyze");
      return;
    }
    if (!report) {
      showToast("warning", "Analiza el archivo antes de limpiar metadata");
      logEvent("warning", "Eliminar metadata sin analisis", { path: filePath }, "analyze");
      return;
    }
    setBusy((prev) => ({ ...prev, remove: true }));
    try {
      await invoke("remove_metadata", { path: filePath });
      showToast("success", "Metadata eliminada");
    } catch (error) {
      showToast("error", `No se pudo eliminar: ${error}`);
      logEvent("error", "Eliminar metadata fallo", { path: filePath, error }, "analyze");
    } finally {
      setBusy((prev) => ({ ...prev, remove: false }));
    }
  };

  const handleExportReport = async () => {
    if (!report) {
      showToast("warning", "Analiza un archivo antes de exportar");
      logEvent("warning", "Exportar sin analisis", { path: filePath }, "export");
      return;
    }
    const reportName = getEntry(report, "Nombre")?.value;
    const pathName = filePath.split(/[\\/]/).pop() || "";
    const rawName = (reportName || pathName || "archivo").trim();
    const nameWithoutExt = rawName.replace(/\.[^.]+$/, "") || "archivo";
    const baseName = nameWithoutExt.toLowerCase().endsWith("-metadata")
      ? nameWithoutExt
      : `${nameWithoutExt}-metadata`;
    const suggestedName = `${baseName}.${exportFormat}`;

    setBusy((prev) => ({ ...prev, export: true }));
    try {
      const savedPath = await invoke<string | null>("export_report", {
        report,
        format: exportFormat,
        suggested_name: suggestedName
      });
      if (savedPath) {
        showToast("success", `Exportado en ${savedPath}`);
      } else {
        logEvent("warning", "Exportacion cancelada", undefined, "export");
      }
    } catch (error) {
      const message = String(error);
      if (!message.toLowerCase().includes("cancel")) {
        showToast("error", `No se pudo exportar: ${message}`);
      }
      logEvent("error", "Exportar reporte fallo", { error }, "export");
    } finally {
      setBusy((prev) => ({ ...prev, export: false }));
    }
  };

  const handleEditField = async (field: OfficeField) => {
    if (!filePath.trim()) {
      showToast("warning", "Selecciona un archivo");
      logEvent("warning", "Editar metadata sin archivo", { field }, "office");
      return;
    }
    if (!report) {
      showToast("warning", "Analiza el archivo antes de editar");
      logEvent("warning", "Editar metadata sin analisis", { path: filePath, field }, "office");
      return;
    }
    const value = officeValues[field]?.trim();
    if (!value) {
      showToast("warning", "Ingresa un valor valido");
      logEvent("warning", "Editar metadata sin valor", { field }, "office");
      return;
    }
    setBusy((prev) => ({ ...prev, edit: true }));
    try {
      await invoke("edit_office_metadata", {
        path: filePath,
        field,
        value
      });
      showToast("success", "Metadata actualizada");
    } catch (error) {
      showToast("error", `No se pudo actualizar: ${error}`);
      logEvent("error", "Actualizar metadata fallo", { path: filePath, field, error }, "office");
    } finally {
      setBusy((prev) => ({ ...prev, edit: false }));
    }
  };

  return (
    <AppShell>
      <Sidebar items={NAV_ITEMS} active={view} onSelect={setView} running={cleanup.running} />
      <main className="main">
        <Topbar title={NAV_ITEMS.find((item) => item.id === view)?.label ?? ""} />
        <section className="content">
          {view === "analyze" && (
            <AnalyzeView
              filePath={filePath}
              includeHash={includeHash}
              report={report}
              reportError={reportError}
              systemEntries={systemEntries}
              typeEntry={typeEntry}
              mimeEntry={mimeEntry}
              isOffice={isOffice}
              officeValues={officeValues}
              exportFormat={exportFormat}
              exporting={busy.export}
              busy={{ analyze: busy.analyze, remove: busy.remove, edit: busy.edit }}
              dropActive={dropTarget === "analyze-file"}
              dropHandlers={dropZoneHandlers("analyze-file")}
              onPickFile={handlePickFile}
              onToggleHash={() => setIncludeHash((prev) => !prev)}
              onAnalyze={handleAnalyze}
              onRemoveMetadata={handleRemoveMetadata}
              onEditField={handleEditField}
              onExportFormatChange={setExportFormat}
              onExport={handleExportReport}
              onOfficeValueChange={(field, value) =>
                setOfficeValues((prev) => ({
                  ...prev,
                  [field]: value
                }))
              }
            />
          )}
          {view === "clean" && (
            <CleanView
              cleanMode={cleanMode}
              dirPath={dirPath}
              selectedFiles={selectedFiles}
              recursive={recursive}
              dirSummary={dirSummary}
              fileSummary={fileSummary}
              extensionCounts={extensionCounts}
              cleanupRunning={cleanup.running || busy.cleanup}
              dirItems={dirItems}
              fileItems={fileItems}
              dropTarget={dropTarget}
              dropHandlers={dropZoneHandlers}
              onSetCleanMode={setCleanMode}
              onPickDirectory={handlePickDirectory}
              onPickFiles={handlePickFiles}
              onToggleRecursive={() => setRecursive((prev) => !prev)}
              onCleanItem={handleCleanItem}
              onCleanAll={handleCleanAll}
            />
          )}
          {view === "logs" && <LogsView logs={logs} />}
        </section>
      </main>
      {toast && <Toast toast={toast} />}
    </AppShell>
  );
}
