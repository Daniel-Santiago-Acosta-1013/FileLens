import { useEffect, useMemo, useRef, useState } from "react";
import type { DragEvent, HTMLAttributes } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";

import AppShell from "./components/layout/AppShell/AppShell";
import Sidebar from "./components/organisms/Sidebar/Sidebar";
import Topbar from "./components/organisms/Topbar/Topbar";
import Toast from "./components/organisms/Toast/Toast";
import AnalyzeView from "./views/AnalyzeView/AnalyzeView";
import CleanView from "./views/CleanView/CleanView";
import { CLEANUP_EMPTY, NAV_ITEMS } from "./constants";
import type { CleanupProgress, CleanupState, DirectoryAnalysisSummary } from "./types/cleanup";
import type { MetadataReport, ReportEntry } from "./types/metadata";
import type {
  CleanMode,
  DropTarget,
  Filter,
  OfficeField,
  Toast as ToastType,
  ToastKind,
  ViewId
} from "./types/ui";
import { buildOfficeValues, extractSystem, getEntry } from "./utils/metadata";

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
  const [filter, setFilter] = useState<Filter>("all");
  const [dirSummary, setDirSummary] = useState<DirectoryAnalysisSummary | null>(null);
  const [fileSummary, setFileSummary] = useState<DirectoryAnalysisSummary | null>(null);
  const [cleanup, setCleanup] = useState<CleanupState>(CLEANUP_EMPTY);

  const [toast, setToast] = useState<ToastType | null>(null);
  const toastTimer = useRef<number | null>(null);

  const [dropTarget, setDropTarget] = useState<DropTarget | null>(null);
  const dropTargetRef = useRef<DropTarget | null>(null);

  const [busy, setBusy] = useState({
    analyze: false,
    dirAnalyze: false,
    fileAnalyze: false,
    cleanup: false,
    remove: false,
    edit: false
  });

  const systemEntries = useMemo(() => extractSystem(report), [report]);
  const mimeEntry = useMemo<ReportEntry | null>(() => getEntry(report, "Tipo MIME"), [report]);
  const typeEntry = useMemo<ReportEntry | null>(() => getEntry(report, "Tipo"), [report]);

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

  useEffect(() => {
    setOfficeValues(buildOfficeValues(report));
  }, [report]);

  useEffect(() => {
    let stop: (() => void) | null = null;
    listen<CleanupProgress>("cleanup://progress", (event) => {
      const payload = event.payload;
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
      }
      if (payload.type === "processing") {
        setCleanup((prev) => ({
          ...prev,
          running: true,
          total: payload.total,
          index: payload.index,
          current: payload.path
        }));
      }
      if (payload.type === "success") {
        setCleanup((prev) => ({
          ...prev,
          successes: prev.successes + 1,
          current: payload.path
        }));
      }
      if (payload.type === "failure") {
        setCleanup((prev) => ({
          ...prev,
          failures: prev.failures + 1,
          current: payload.path,
          lastError: payload.error
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
      .catch(() => {
        showToast("error", "No se pudo suscribir a eventos de limpieza");
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
  };

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
    setCleanup(CLEANUP_EMPTY);
    showToast("info", message);
  };

  const applyFiles = (paths: string[], message?: string) => {
    setSelectedFiles(paths);
    setFileSummary(null);
    setCleanup(CLEANUP_EMPTY);
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
      setDropTargetState(null);
    }
  });

  useEffect(() => {
    let stop: (() => void) | null = null;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type === "drop") {
          const target = dropTargetRef.current ?? resolveDropTarget();
          applyDroppedPaths(event.payload.paths, target);
          setDropTargetState(null);
        }
        if (event.payload.type === "leave") {
          setDropTargetState(null);
        }
      })
      .then((unlisten) => {
        stop = unlisten;
      })
      .catch(() => {
        showToast("error", "No se pudo habilitar arrastrar y soltar");
      });

    return () => {
      if (stop) stop();
    };
  }, [view, cleanMode]);

  const handlePickFile = async () => {
    const selected = await invoke<string | null>("pick_file");
    if (selected) {
      applyFilePath(selected);
    }
  };

  const handlePickDirectory = async () => {
    const selected = await invoke<string | null>("pick_directory");
    if (selected) {
      applyDirectoryPath(selected);
    }
  };

  const handlePickFiles = async () => {
    const selected = await invoke<string[] | null>("pick_files");
    if (selected && selected.length) {
      applyFiles(selected);
    }
  };

  const handleAnalyze = async () => {
    if (!filePath.trim()) {
      showToast("warning", "Selecciona un archivo");
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
    } finally {
      setBusy((prev) => ({ ...prev, analyze: false }));
    }
  };

  const handleAnalyzeDirectory = async () => {
    if (!dirPath.trim()) {
      showToast("warning", "Selecciona un directorio");
      return;
    }
    setBusy((prev) => ({ ...prev, dirAnalyze: true }));
    try {
      const summary = await invoke<DirectoryAnalysisSummary>("analyze_directory", {
        path: dirPath,
        recursive
      });
      setDirSummary(summary);
      showToast("success", "Analisis completado");
    } catch (error) {
      showToast("error", `No se pudo analizar: ${error}`);
    } finally {
      setBusy((prev) => ({ ...prev, dirAnalyze: false }));
    }
  };

  const handleAnalyzeFiles = async () => {
    if (!selectedFiles.length) {
      showToast("warning", "Selecciona archivos");
      return;
    }
    setBusy((prev) => ({ ...prev, fileAnalyze: true }));
    try {
      const summary = await invoke<DirectoryAnalysisSummary>("analyze_files", {
        paths: selectedFiles
      });
      setFileSummary(summary);
      showToast("success", "Analisis completado");
    } catch (error) {
      showToast("error", `No se pudo analizar: ${error}`);
    } finally {
      setBusy((prev) => ({ ...prev, fileAnalyze: false }));
    }
  };

  const handleStartCleanup = async () => {
    if (cleanMode === "directory") {
      if (!dirPath.trim()) {
        showToast("warning", "Selecciona un directorio");
        return;
      }
      if (!dirSummary) {
        showToast("warning", "Ejecuta el analisis antes de limpiar");
        return;
      }
      setBusy((prev) => ({ ...prev, cleanup: true }));
      setCleanup(CLEANUP_EMPTY);
      try {
        await invoke("start_cleanup", {
          path: dirPath,
          recursive,
          filter
        });
        showToast("info", "Limpieza iniciada");
      } catch (error) {
        setBusy((prev) => ({ ...prev, cleanup: false }));
        showToast("error", `No se pudo iniciar la limpieza: ${error}`);
      }
      return;
    }

    if (!selectedFiles.length) {
      showToast("warning", "Selecciona archivos");
      return;
    }
    if (!fileSummary) {
      showToast("warning", "Ejecuta el analisis antes de limpiar");
      return;
    }
    setBusy((prev) => ({ ...prev, cleanup: true }));
    setCleanup(CLEANUP_EMPTY);
    try {
      await invoke("start_cleanup_files", {
        paths: selectedFiles,
        filter
      });
      showToast("info", "Limpieza iniciada");
    } catch (error) {
      setBusy((prev) => ({ ...prev, cleanup: false }));
      showToast("error", `No se pudo iniciar la limpieza: ${error}`);
    }
  };

  const handleRemoveMetadata = async () => {
    if (!filePath.trim()) {
      showToast("warning", "Selecciona un archivo");
      return;
    }
    if (!report) {
      showToast("warning", "Analiza el archivo antes de limpiar metadata");
      return;
    }
    setBusy((prev) => ({ ...prev, remove: true }));
    try {
      await invoke("remove_metadata", { path: filePath });
      showToast("success", "Metadata eliminada");
    } catch (error) {
      showToast("error", `No se pudo eliminar: ${error}`);
    } finally {
      setBusy((prev) => ({ ...prev, remove: false }));
    }
  };

  const handleEditField = async (field: OfficeField) => {
    if (!filePath.trim()) {
      showToast("warning", "Selecciona un archivo");
      return;
    }
    if (!report) {
      showToast("warning", "Analiza el archivo antes de editar");
      return;
    }
    const value = officeValues[field]?.trim();
    if (!value) {
      showToast("warning", "Ingresa un valor valido");
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
          {view === "analyze" ? (
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
              busy={{ analyze: busy.analyze, remove: busy.remove, edit: busy.edit }}
              dropActive={dropTarget === "analyze-file"}
              dropHandlers={dropZoneHandlers("analyze-file")}
              onPickFile={handlePickFile}
              onToggleHash={() => setIncludeHash((prev) => !prev)}
              onAnalyze={handleAnalyze}
              onRemoveMetadata={handleRemoveMetadata}
              onEditField={handleEditField}
              onOfficeValueChange={(field, value) =>
                setOfficeValues((prev) => ({
                  ...prev,
                  [field]: value
                }))
              }
            />
          ) : (
            <CleanView
              cleanMode={cleanMode}
              dirPath={dirPath}
              selectedFiles={selectedFiles}
              recursive={recursive}
              filter={filter}
              dirSummary={dirSummary}
              fileSummary={fileSummary}
              extensionCounts={extensionCounts}
              cleanup={cleanup}
              busy={{
                dirAnalyze: busy.dirAnalyze,
                fileAnalyze: busy.fileAnalyze,
                cleanup: busy.cleanup
              }}
              dropTarget={dropTarget}
              dropHandlers={dropZoneHandlers}
              onSetCleanMode={setCleanMode}
              onPickDirectory={handlePickDirectory}
              onPickFiles={handlePickFiles}
              onToggleRecursive={() => setRecursive((prev) => !prev)}
              onSetFilter={setFilter}
              onAnalyzeDirectory={handleAnalyzeDirectory}
              onAnalyzeFiles={handleAnalyzeFiles}
              onStartCleanup={handleStartCleanup}
            />
          )}
        </section>
      </main>
      {toast && <Toast toast={toast} />}
    </AppShell>
  );
}
