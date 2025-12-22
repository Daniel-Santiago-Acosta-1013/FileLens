import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type ViewId = "analyze" | "clean";

type EntryLevel = "Info" | "Warning" | "Success" | "Error" | "Muted";

type ReportEntry = {
  label: string;
  value: string;
  level: EntryLevel;
};

type SectionNotice = {
  message: string;
  level: EntryLevel;
};

type ReportSection = {
  title: string;
  entries: ReportEntry[];
  notice?: SectionNotice | null;
};

type MetadataReport = {
  system: ReportEntry[];
  internal: ReportSection[];
  risks: ReportEntry[];
  errors: string[];
};

type DirectoryAnalysisSummary = {
  total_files: number;
  images_count: number;
  office_count: number;
  extension_counts: [string, number][];
  image_extensions: string[];
  office_extensions: string[];
};

type CleanupProgress =
  | { type: "started"; total: number }
  | { type: "processing"; index: number; total: number; path: string }
  | { type: "success"; path: string }
  | { type: "failure"; path: string; error: string }
  | { type: "finished"; successes: number; failures: number };

type ToastKind = "info" | "success" | "warning" | "error";

type Toast = {
  kind: ToastKind;
  message: string;
};

type CleanupState = {
  running: boolean;
  total: number;
  index: number;
  successes: number;
  failures: number;
  current: string;
  lastError: string;
  finished: boolean;
};

type Filter = "all" | "images" | "office";

type OfficeField = "author" | "title" | "subject" | "company";

const NAV_ITEMS: { id: ViewId; label: string }[] = [
  { id: "analyze", label: "Analisis" },
  { id: "clean", label: "Limpieza" }
];

const CLEANUP_EMPTY: CleanupState = {
  running: false,
  total: 0,
  index: 0,
  successes: 0,
  failures: 0,
  current: "",
  lastError: "",
  finished: false
};

const SYSTEM_ALLOWLIST = new Set([
  "Tipo",
  "Tamaño",
  "Tipo MIME",
  "Hash SHA-256",
  "Última modificación",
  "Fecha de creación"
]);

const OFFICE_FIELD_LABELS: Record<OfficeField, string> = {
  author: "Creador",
  title: "Título",
  subject: "Asunto",
  company: "Empresa"
};

function getEntry(report: MetadataReport | null, label: string) {
  if (!report) return null;
  return report.system.find((entry) => entry.label === label) ?? null;
}

function extractSystem(report: MetadataReport | null) {
  if (!report) return [] as ReportEntry[];
  return report.system.filter((entry) => SYSTEM_ALLOWLIST.has(entry.label));
}

function extractOfficeValue(report: MetadataReport | null, label: string) {
  if (!report) return "";
  for (const section of report.internal) {
    for (const entry of section.entries) {
      if (entry.label === label) return entry.value;
    }
  }
  return "";
}

function toneClass(level: EntryLevel) {
  switch (level) {
    case "Success":
      return "note note--success";
    case "Warning":
      return "note note--warning";
    case "Error":
      return "note note--error";
    case "Muted":
      return "note note--muted";
    default:
      return "note note--info";
  }
}

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

  const [cleanMode, setCleanMode] = useState<"directory" | "files">("directory");
  const [dirPath, setDirPath] = useState("");
  const [selectedFiles, setSelectedFiles] = useState<string[]>([]);
  const [recursive, setRecursive] = useState(false);
  const [filter, setFilter] = useState<Filter>("all");
  const [dirSummary, setDirSummary] = useState<DirectoryAnalysisSummary | null>(null);
  const [fileSummary, setFileSummary] = useState<DirectoryAnalysisSummary | null>(null);
  const [cleanup, setCleanup] = useState<CleanupState>(CLEANUP_EMPTY);

  const [toast, setToast] = useState<Toast | null>(null);
  const toastTimer = useRef<number | null>(null);

  const [busy, setBusy] = useState({
    analyze: false,
    dirAnalyze: false,
    fileAnalyze: false,
    cleanup: false,
    remove: false,
    edit: false
  });

  const systemEntries = useMemo(() => extractSystem(report), [report]);
  const mimeEntry = useMemo(() => getEntry(report, "Tipo MIME"), [report]);
  const typeEntry = useMemo(() => getEntry(report, "Tipo"), [report]);

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
    setOfficeValues({
      author: extractOfficeValue(report, OFFICE_FIELD_LABELS.author),
      title: extractOfficeValue(report, OFFICE_FIELD_LABELS.title),
      subject: extractOfficeValue(report, OFFICE_FIELD_LABELS.subject),
      company: extractOfficeValue(report, OFFICE_FIELD_LABELS.company)
    });
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

  const handlePickFile = async () => {
    const selected = await invoke<string | null>("pick_file");
    if (selected) {
      setFilePath(selected);
      setReport(null);
      setReportError("");
      showToast("info", "Archivo seleccionado");
    }
  };

  const handlePickDirectory = async () => {
    const selected = await invoke<string | null>("pick_directory");
    if (selected) {
      setDirPath(selected);
      setDirSummary(null);
      setCleanup(CLEANUP_EMPTY);
      showToast("info", "Directorio seleccionado");
    }
  };

  const handlePickFiles = async () => {
    const selected = await invoke<string[] | null>("pick_files");
    if (selected && selected.length) {
      setSelectedFiles(selected);
      setFileSummary(null);
      setCleanup(CLEANUP_EMPTY);
      showToast("info", `${selected.length} archivos seleccionados`);
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
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-mark">FL</div>
          <div>
            <strong>FileLens</strong>
            <span>Desktop</span>
          </div>
        </div>
        <nav className="nav">
          {NAV_ITEMS.map((item) => (
            <button
              key={item.id}
              className={`nav-btn ${view === item.id ? "active" : ""}`}
              onClick={() => setView(item.id)}
            >
              {item.label}
            </button>
          ))}
        </nav>
        <div className="sidebar-footer">
          <span className="status-dot" />
          <span>{cleanup.running ? "Procesando" : "Listo"}</span>
        </div>
      </aside>

      <main className="main">
        <header className="topbar">
          <div>
            <h1>{NAV_ITEMS.find((item) => item.id === view)?.label}</h1>
            <p>Flujo principal</p>
          </div>
        </header>

        <section className="content">
          {view === "analyze" && (
            <div className="sheet">
              <div className="section">
                <div className="section-row">
                  <div>
                    <span className="label">Archivo</span>
                    <div className="path-box">{filePath || "Ningun archivo seleccionado"}</div>
                  </div>
                  <button className="secondary" onClick={handlePickFile}>Explorar</button>
                </div>
                <div className="section-row">
                  <label className="toggle">
                    <input
                      type="checkbox"
                      checked={includeHash}
                      onChange={() => setIncludeHash((prev) => !prev)}
                    />
                    <span>Calcular hash SHA-256</span>
                  </label>
                  <button className="primary" onClick={handleAnalyze} disabled={busy.analyze}>
                    {busy.analyze ? "Analizando..." : "Analizar"}
                  </button>
                </div>
                {reportError && <p className="inline-error">{reportError}</p>}
              </div>

              <div className="section">
                <span className="label">Tipo detectado</span>
                <div className="meta-inline">
                  <span>{typeEntry?.value || "Archivo"}</span>
                  <span>{filePath.split(".").pop()?.toUpperCase() || "-"}</span>
                  <span className="muted">{mimeEntry?.value || "MIME no disponible"}</span>
                  <span className="muted">{getEntry(report, "Tamaño")?.value || ""}</span>
                </div>
              </div>

              <div className="section">
                <span className="label">Metadata encontrada</span>
                {report ? (
                  <div className="meta-list">
                    {systemEntries.map((entry, index) => (
                      <div key={`${entry.label}-${index}`} className="meta-row">
                        <span>{entry.label}</span>
                        <span className="meta-value">{entry.value}</span>
                      </div>
                    ))}
                    {report.internal.map((section) => (
                      <div key={section.title} className="meta-group">
                        <div className="section-title">{section.title}</div>
                        {section.entries.map((entry, index) => (
                          <div key={`${section.title}-${index}`} className="meta-row">
                            <span>{entry.label}</span>
                            <span className="meta-value">{entry.value}</span>
                          </div>
                        ))}
                        {section.notice && (
                          <div className={toneClass(section.notice.level)}>
                            {section.notice.message}
                          </div>
                        )}
                      </div>
                    ))}
                    {report.risks.length > 0 && (
                      <div className="meta-group">
                        <div className="section-title">Riesgos</div>
                        {report.risks.map((entry, index) => (
                          <div key={`risk-${index}`} className="meta-row">
                            <span>{entry.label}</span>
                            <span className="meta-value">{entry.value}</span>
                          </div>
                        ))}
                      </div>
                    )}
                  </div>
                ) : (
                  <p className="muted">Ejecuta el analisis para ver los resultados.</p>
                )}
              </div>

              <div className="section">
                <span className="label">Acciones sobre metadata</span>
                <div className="section-row">
                  <button className="danger" onClick={handleRemoveMetadata} disabled={busy.remove || !report}>
                    {busy.remove ? "Eliminando..." : "Eliminar metadata"}
                  </button>
                  {!isOffice && <span className="muted">Edicion disponible solo para Office.</span>}
                </div>
                {isOffice && (
                  <div className="edit-grid">
                    {Object.entries(OFFICE_FIELD_LABELS).map(([fieldKey, label]) => (
                      <div key={fieldKey} className="edit-row">
                        <label className="field">
                          <span>{label}</span>
                          <input
                            value={officeValues[fieldKey as OfficeField]}
                            onChange={(event) =>
                              setOfficeValues((prev) => ({
                                ...prev,
                                [fieldKey]: event.target.value
                              }))
                            }
                            placeholder="(vacio)"
                          />
                        </label>
                        <button
                          className="secondary"
                          onClick={() => handleEditField(fieldKey as OfficeField)}
                          disabled={busy.edit || !report}
                        >
                          Guardar
                        </button>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </div>
          )}

          {view === "clean" && (
            <div className="sheet">
              <div className="section">
                <span className="label">Modo</span>
                <div className="segmented">
                  <button
                    className={cleanMode === "directory" ? "active" : ""}
                    onClick={() => setCleanMode("directory")}
                  >
                    Directorio
                  </button>
                  <button
                    className={cleanMode === "files" ? "active" : ""}
                    onClick={() => setCleanMode("files")}
                  >
                    Archivos
                  </button>
                </div>
              </div>

              {cleanMode === "directory" ? (
                <div className="section">
                  <div className="section-row">
                    <div>
                      <span className="label">Directorio</span>
                      <div className="path-box">{dirPath || "Ningun directorio seleccionado"}</div>
                    </div>
                    <button className="secondary" onClick={handlePickDirectory}>Explorar</button>
                  </div>
                  <div className="section-row">
                    <label className="toggle">
                      <input
                        type="checkbox"
                        checked={recursive}
                        onChange={() => setRecursive((prev) => !prev)}
                      />
                      <span>Incluir subdirectorios</span>
                    </label>
                    <div className="segmented">
                      <button
                        className={filter === "all" ? "active" : ""}
                        onClick={() => setFilter("all")}
                      >
                        Todos
                      </button>
                      <button
                        className={filter === "images" ? "active" : ""}
                        onClick={() => setFilter("images")}
                      >
                        Imagenes
                      </button>
                      <button
                        className={filter === "office" ? "active" : ""}
                        onClick={() => setFilter("office")}
                      >
                        Office
                      </button>
                    </div>
                  </div>
                  <div className="section-row">
                    <button className="secondary" onClick={handleAnalyzeDirectory} disabled={busy.dirAnalyze}>
                      {busy.dirAnalyze ? "Analizando..." : "Analizar"}
                    </button>
                    <button className="primary" onClick={handleStartCleanup} disabled={busy.cleanup}>
                      {busy.cleanup ? "Procesando..." : "Limpiar"}
                    </button>
                  </div>
                </div>
              ) : (
                <div className="section">
                  <div className="section-row">
                    <div>
                      <span className="label">Archivos</span>
                      <div className="path-box">
                        {selectedFiles.length ? `${selectedFiles.length} archivos seleccionados` : "Ningun archivo seleccionado"}
                      </div>
                    </div>
                    <button className="secondary" onClick={handlePickFiles}>Explorar</button>
                  </div>
                  <div className="section-row">
                    <div className="segmented">
                      <button
                        className={filter === "all" ? "active" : ""}
                        onClick={() => setFilter("all")}
                      >
                        Todos
                      </button>
                      <button
                        className={filter === "images" ? "active" : ""}
                        onClick={() => setFilter("images")}
                      >
                        Imagenes
                      </button>
                      <button
                        className={filter === "office" ? "active" : ""}
                        onClick={() => setFilter("office")}
                      >
                        Office
                      </button>
                    </div>
                  </div>
                  {selectedFiles.length > 0 && (
                    <div className="file-list">
                      {selectedFiles.slice(0, 3).map((file) => (
                        <div key={file} className="file-item">{file}</div>
                      ))}
                      {selectedFiles.length > 3 && (
                        <div className="file-item muted">+ {selectedFiles.length - 3} mas</div>
                      )}
                    </div>
                  )}
                  <div className="section-row">
                    <button className="secondary" onClick={handleAnalyzeFiles} disabled={busy.fileAnalyze}>
                      {busy.fileAnalyze ? "Analizando..." : "Analizar"}
                    </button>
                    <button className="primary" onClick={handleStartCleanup} disabled={busy.cleanup}>
                      {busy.cleanup ? "Procesando..." : "Limpiar"}
                    </button>
                  </div>
                </div>
              )}

              <div className="section">
                <span className="label">Desglose</span>
                {(cleanMode === "directory" ? dirSummary : fileSummary) ? (
                  <div className="summary">
                    <div className="summary-row">
                      <span>Total</span>
                      <strong>{(cleanMode === "directory" ? dirSummary : fileSummary)?.total_files}</strong>
                    </div>
                    <div className="summary-row">
                      <span>Imagenes</span>
                      <strong>{(cleanMode === "directory" ? dirSummary : fileSummary)?.images_count}</strong>
                    </div>
                    <div className="summary-row">
                      <span>Office</span>
                      <strong>{(cleanMode === "directory" ? dirSummary : fileSummary)?.office_count}</strong>
                    </div>
                    <div className="summary-section">
                      <span className="label">Extensiones principales</span>
                      {extensionCounts.map(([ext, count]) => (
                        <div key={ext} className="summary-row">
                          <span>{ext}</span>
                          <strong>{count}</strong>
                        </div>
                      ))}
                    </div>
                  </div>
                ) : (
                  <p className="muted">Ejecuta el analisis para ver el desglose.</p>
                )}
              </div>

              <div className="section">
                <span className="label">Progreso</span>
                <div className="progress">
                  <div
                    className="progress-bar"
                    style={{
                      width: cleanup.total ? `${Math.round((cleanup.index / cleanup.total) * 100)}%` : "0%"
                    }}
                  />
                </div>
                <div className="summary-row">
                  <span>Actual</span>
                  <span className="mono">{cleanup.current || "-"}</span>
                </div>
                <div className="summary-row">
                  <span>OK / ERR</span>
                  <strong>{cleanup.successes} / {cleanup.failures}</strong>
                </div>
                {cleanup.lastError && <p className="inline-error">Ultimo error: {cleanup.lastError}</p>}
              </div>
            </div>
          )}
        </section>
      </main>

      {toast && (
        <div className={`toast toast--${toast.kind}`}>
          <span>{toast.message}</span>
        </div>
      )}
    </div>
  );
}
