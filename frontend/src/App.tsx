import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type ViewId = "home" | "analyze" | "clean" | "report" | "edit";

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

const NAV_ITEMS: { id: ViewId; label: string; hint: string }[] = [
  { id: "home", label: "Inicio", hint: "Resumen general" },
  { id: "analyze", label: "Analisis", hint: "Archivo individual" },
  { id: "clean", label: "Limpieza", hint: "Directorios" },
  { id: "report", label: "Reporte", hint: "Detalles" },
  { id: "edit", label: "Edicion", hint: "Metadata" }
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

function levelClass(level: EntryLevel) {
  switch (level) {
    case "Success":
      return "pill success";
    case "Warning":
      return "pill warning";
    case "Error":
      return "pill error";
    case "Muted":
      return "pill muted";
    default:
      return "pill info";
  }
}

function toneClass(level: EntryLevel) {
  switch (level) {
    case "Success":
      return "success";
    case "Warning":
      return "warning";
    case "Error":
      return "error";
    case "Muted":
      return "muted";
    default:
      return "info";
  }
}

export default function App() {
  const [view, setView] = useState<ViewId>("home");
  const [filePath, setFilePath] = useState("");
  const [includeHash, setIncludeHash] = useState(true);
  const [report, setReport] = useState<MetadataReport | null>(null);
  const [reportError, setReportError] = useState("");
  const [fileMatches, setFileMatches] = useState<string[]>([]);
  const [dirMatches, setDirMatches] = useState<string[]>([]);

  const [dirPath, setDirPath] = useState("");
  const [recursive, setRecursive] = useState(false);
  const [filter, setFilter] = useState<Filter>("all");
  const [dirSummary, setDirSummary] = useState<DirectoryAnalysisSummary | null>(null);
  const [cleanup, setCleanup] = useState<CleanupState>(CLEANUP_EMPTY);

  const [editField, setEditField] = useState<OfficeField>("author");
  const [editValue, setEditValue] = useState("");

  const [toast, setToast] = useState<Toast | null>(null);
  const toastTimer = useRef<number | null>(null);

  const [busy, setBusy] = useState({
    analyze: false,
    searchFile: false,
    searchDir: false,
    dirAnalyze: false,
    cleanup: false,
    remove: false,
    edit: false
  });

  const extensionCounts = useMemo(() => {
    if (!dirSummary) return [] as [string, number][];
    return dirSummary.extension_counts.slice(0, 10);
  }, [dirSummary]);

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
      if (stop) {
        stop();
      }
    };
  }, []);

  const showToast = (kind: ToastKind, message: string) => {
    if (toastTimer.current) {
      window.clearTimeout(toastTimer.current);
    }
    setToast({ kind, message });
    toastTimer.current = window.setTimeout(() => setToast(null), 4200);
  };

  const handleAnalyze = async () => {
    if (!filePath.trim()) {
      showToast("warning", "Ingresa una ruta de archivo");
      return;
    }
    setBusy((prev) => ({ ...prev, analyze: true }));
    setReportError("");
    try {
      const result = await invoke<MetadataReport>("analyze_file", {
        path: filePath,
        include_hash: includeHash
      });
      setReport(result);
      setView("report");
      showToast("success", "Analisis completado");
    } catch (error) {
      setReportError(String(error));
      showToast("error", "No se pudo analizar el archivo");
    } finally {
      setBusy((prev) => ({ ...prev, analyze: false }));
    }
  };

  const handleSearchFiles = async () => {
    if (!filePath.trim()) {
      showToast("warning", "Ingresa un nombre para buscar");
      return;
    }
    setBusy((prev) => ({ ...prev, searchFile: true }));
    try {
      const matches = await invoke<string[]>("search_files", { query: filePath });
      setFileMatches(matches);
      if (!matches.length) {
        showToast("warning", "No se encontraron coincidencias");
      }
    } catch (error) {
      showToast("error", `Busqueda fallida: ${error}`);
    } finally {
      setBusy((prev) => ({ ...prev, searchFile: false }));
    }
  };

  const handleAnalyzeDirectory = async () => {
    if (!dirPath.trim()) {
      showToast("warning", "Ingresa la ruta de un directorio");
      return;
    }
    setBusy((prev) => ({ ...prev, dirAnalyze: true }));
    try {
      const summary = await invoke<DirectoryAnalysisSummary>("analyze_directory", {
        path: dirPath,
        recursive
      });
      setDirSummary(summary);
      showToast("success", "Analisis de directorio completado");
    } catch (error) {
      showToast("error", `No se pudo analizar: ${error}`);
    } finally {
      setBusy((prev) => ({ ...prev, dirAnalyze: false }));
    }
  };

  const handleSearchDirectories = async () => {
    if (!dirPath.trim()) {
      showToast("warning", "Ingresa un nombre para buscar");
      return;
    }
    setBusy((prev) => ({ ...prev, searchDir: true }));
    try {
      const matches = await invoke<string[]>("search_directories", { query: dirPath });
      setDirMatches(matches);
      if (!matches.length) {
        showToast("warning", "No se encontraron directorios");
      }
    } catch (error) {
      showToast("error", `Busqueda fallida: ${error}`);
    } finally {
      setBusy((prev) => ({ ...prev, searchDir: false }));
    }
  };

  const handleStartCleanup = async () => {
    if (!dirPath.trim()) {
      showToast("warning", "Ingresa la ruta de un directorio");
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
  };

  const handleRemoveMetadata = async () => {
    if (!filePath.trim()) {
      showToast("warning", "Ingresa la ruta de un archivo");
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

  const handleEditMetadata = async () => {
    if (!filePath.trim()) {
      showToast("warning", "Ingresa la ruta de un archivo");
      return;
    }
    if (!editValue.trim()) {
      showToast("warning", "Ingresa un nuevo valor");
      return;
    }
    setBusy((prev) => ({ ...prev, edit: true }));
    try {
      await invoke("edit_office_metadata", {
        path: filePath,
        field: editField,
        value: editValue
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
      <aside className="side">
        <div className="brand">
          <span className="brand-mark">FL</span>
          <div>
            <h1>FileLens</h1>
            <p>Studio UI</p>
          </div>
        </div>
        <nav className="nav">
          {NAV_ITEMS.map((item) => (
            <button
              key={item.id}
              className={`nav-btn ${view === item.id ? "active" : ""}`}
              onClick={() => setView(item.id)}
            >
              <span>{item.label}</span>
              <small>{item.hint}</small>
            </button>
          ))}
        </nav>
        <div className="side-card">
          <h3>Estado rapido</h3>
          <p>Reporte: {report ? "listo" : "sin datos"}</p>
          <p>Limpieza: {cleanup.running ? "en progreso" : "detenida"}</p>
          <p>Directorio: {dirSummary ? `${dirSummary.total_files} archivos` : "sin analisis"}</p>
        </div>
      </aside>

      <main className="main">
        <header className="hero">
          <div>
            <p className="kicker">FileLens Desktop</p>
            <h2>Control total de metadata, sin perder detalle</h2>
            <p className="subtitle">
              Analiza archivos, limpia directorios y edita propiedades sensibles con una vista clara y moderna.
            </p>
          </div>
          <div className="hero-stats">
            <div>
              <span className="stat-label">Modo</span>
              <strong>{view.toUpperCase()}</strong>
            </div>
            <div>
              <span className="stat-label">Hash</span>
              <strong>{includeHash ? "ON" : "OFF"}</strong>
            </div>
          </div>
        </header>

        <section className="content">
          {view === "home" && (
            <div className="grid">
              <div className="card tall">
                <h3>Flujo sugerido</h3>
                <ol className="steps">
                  <li>Define la ruta del archivo o directorio.</li>
                  <li>Ejecuta el analisis para obtener contexto.</li>
                  <li>Revisa el reporte o limpia en bloque.</li>
                  <li>Si es Office, ajusta campos clave.</li>
                </ol>
                <div className="cta-row">
                  <button className="primary" onClick={() => setView("analyze")}>Analizar archivo</button>
                  <button className="ghost" onClick={() => setView("clean")}>Limpieza masiva</button>
                </div>
              </div>
              <div className="card">
                <h3>Checklist rapido</h3>
                <ul className="checklist">
                  <li>Soporte para imagenes y Office</li>
                  <li>Reporte con riesgos destacados</li>
                  <li>Limpieza con progreso detallado</li>
                  <li>Edicion puntual de campos Office</li>
                </ul>
              </div>
              <div className="card">
                <h3>Ultimo archivo</h3>
                <p>{filePath ? filePath : "Aun no has definido una ruta"}</p>
                <button className="ghost" onClick={() => setView("report")}>Ver reporte</button>
              </div>
            </div>
          )}

          {view === "analyze" && (
            <div className="grid two">
              <div className="card">
                <h3>Analisis de archivo</h3>
                <label className="field">
                  <span>Ruta del archivo</span>
                  <input
                    value={filePath}
                    onChange={(event) => setFilePath(event.target.value)}
                    placeholder="/Users/tu/archivo.pdf"
                  />
                </label>
                <div className="row">
                  <label className="toggle">
                    <input
                      type="checkbox"
                      checked={includeHash}
                      onChange={() => setIncludeHash((prev) => !prev)}
                    />
                    <span>Calcular hash SHA-256</span>
                  </label>
                  <button className="ghost" onClick={handleSearchFiles} disabled={busy.searchFile}>
                    Buscar coincidencias
                  </button>
                </div>
                <div className="cta-row">
                  <button className="primary" onClick={handleAnalyze} disabled={busy.analyze}>
                    {busy.analyze ? "Analizando..." : "Analizar"}
                  </button>
                  <button className="ghost" onClick={() => setView("report")}>
                    Ir al reporte
                  </button>
                </div>
                {reportError && <p className="inline-error">{reportError}</p>}
              </div>
              <div className="card">
                <h3>Coincidencias recientes</h3>
                {fileMatches.length === 0 && <p>Sin resultados aun.</p>}
                <div className="match-list">
                  {fileMatches.map((match) => (
                    <button
                      key={match}
                      className="match"
                      onClick={() => {
                        setFilePath(match);
                        showToast("info", "Ruta seleccionada");
                      }}
                    >
                      {match}
                    </button>
                  ))}
                </div>
              </div>
            </div>
          )}

          {view === "clean" && (
            <div className="grid two">
              <div className="card">
                <h3>Limpieza de directorio</h3>
                <label className="field">
                  <span>Ruta del directorio</span>
                  <input
                    value={dirPath}
                    onChange={(event) => setDirPath(event.target.value)}
                    placeholder="/Users/tu/Documentos"
                  />
                </label>
                <div className="row">
                  <label className="toggle">
                    <input
                      type="checkbox"
                      checked={recursive}
                      onChange={() => setRecursive((prev) => !prev)}
                    />
                    <span>Incluir subdirectorios</span>
                  </label>
                  <button className="ghost" onClick={handleSearchDirectories} disabled={busy.searchDir}>
                    Buscar directorios
                  </button>
                </div>
                <div className="row">
                  <div className="pill-group">
                    <button
                      className={filter === "all" ? "pill active" : "pill"}
                      onClick={() => setFilter("all")}
                    >
                      Todos
                    </button>
                    <button
                      className={filter === "images" ? "pill active" : "pill"}
                      onClick={() => setFilter("images")}
                    >
                      Imagenes
                    </button>
                    <button
                      className={filter === "office" ? "pill active" : "pill"}
                      onClick={() => setFilter("office")}
                    >
                      Office
                    </button>
                  </div>
                </div>
                <div className="cta-row">
                  <button className="primary" onClick={handleAnalyzeDirectory} disabled={busy.dirAnalyze}>
                    {busy.dirAnalyze ? "Analizando..." : "Analizar directorio"}
                  </button>
                  <button className="danger" onClick={handleStartCleanup} disabled={busy.cleanup}>
                    {busy.cleanup ? "En proceso..." : "Iniciar limpieza"}
                  </button>
                </div>
              </div>

              <div className="card">
                <h3>Resumen</h3>
                {dirSummary ? (
                  <div className="summary">
                    <div className="summary-row">
                      <span>Total</span>
                      <strong>{dirSummary.total_files}</strong>
                    </div>
                    <div className="summary-row">
                      <span>Imagenes</span>
                      <strong>{dirSummary.images_count}</strong>
                    </div>
                    <div className="summary-row">
                      <span>Office</span>
                      <strong>{dirSummary.office_count}</strong>
                    </div>
                    <div className="summary-section">
                      <h4>Extensiones principales</h4>
                      {extensionCounts.map(([ext, count]) => (
                        <div key={ext} className="summary-row">
                          <span>{ext}</span>
                          <strong>{count}</strong>
                        </div>
                      ))}
                    </div>
                  </div>
                ) : (
                  <p>Ejecuta el analisis para ver conteos.</p>
                )}
              </div>

              <div className="card wide">
                <h3>Progreso de limpieza</h3>
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
                  <strong>{cleanup.current || "-"}</strong>
                </div>
                <div className="summary-row">
                  <span>OK / ERR</span>
                  <strong>{cleanup.successes} / {cleanup.failures}</strong>
                </div>
                {cleanup.lastError && <p className="inline-error">Ultimo error: {cleanup.lastError}</p>}
                {dirMatches.length > 0 && (
                  <div className="match-list">
                    {dirMatches.map((match) => (
                      <button
                        key={match}
                        className="match"
                        onClick={() => {
                          setDirPath(match);
                          showToast("info", "Directorio seleccionado");
                        }}
                      >
                        {match}
                      </button>
                    ))}
                  </div>
                )}
              </div>
            </div>
          )}

          {view === "report" && (
            <div className="grid two">
              <div className="card">
                <h3>Reporte de metadata</h3>
                {!report && <p>No hay reporte cargado. Ejecuta un analisis.</p>}
                {report && (
                  <div className="report-section">
                    <h4>Sistema</h4>
                    {report.system.map((entry, index) => (
                      <div key={`${entry.label}-${index}`} className="report-row">
                        <span>{entry.label}</span>
                        <span className={levelClass(entry.level)}>{entry.value}</span>
                      </div>
                    ))}
                  </div>
                )}
              </div>
              <div className="card">
                <h3>Metadata interna</h3>
                {report?.internal?.length ? (
                  report.internal.map((section) => (
                    <div key={section.title} className="report-section">
                      <h4>{section.title}</h4>
                      {section.entries.map((entry, index) => (
                        <div key={`${section.title}-${index}`} className="report-row">
                          <span>{entry.label}</span>
                          <span className={levelClass(entry.level)}>{entry.value}</span>
                        </div>
                      ))}
                      {section.notice && (
                        <div className={`notice ${toneClass(section.notice.level)}`}>
                          {section.notice.message}
                        </div>
                      )}
                    </div>
                  ))
                ) : (
                  <p>Sin metadata interna para mostrar.</p>
                )}
              </div>

              {report && (
                <>
                  <div className="card">
                    <h3>Riesgos</h3>
                    {report.risks.length ? (
                      report.risks.map((entry, index) => (
                        <div key={`risk-${index}`} className="report-row">
                          <span>{entry.label}</span>
                          <span className={levelClass(entry.level)}>{entry.value}</span>
                        </div>
                      ))
                    ) : (
                      <p>Sin riesgos detectados.</p>
                    )}
                  </div>
                  <div className="card">
                    <h3>Errores</h3>
                    {report.errors.length ? (
                      <ul className="error-list">
                        {report.errors.map((error, index) => (
                          <li key={`error-${index}`}>{error}</li>
                        ))}
                      </ul>
                    ) : (
                      <p>Sin errores reportados.</p>
                    )}
                  </div>
                </>
              )}
            </div>
          )}

          {view === "edit" && (
            <div className="grid two">
              <div className="card">
                <h3>Eliminar metadata</h3>
                <p>Compatible con imagenes y documentos Office.</p>
                <label className="field">
                  <span>Ruta del archivo</span>
                  <input
                    value={filePath}
                    onChange={(event) => setFilePath(event.target.value)}
                    placeholder="/Users/tu/archivo.docx"
                  />
                </label>
                <button className="danger" onClick={handleRemoveMetadata} disabled={busy.remove}>
                  {busy.remove ? "Eliminando..." : "Eliminar metadata"}
                </button>
              </div>

              <div className="card">
                <h3>Editar metadata Office</h3>
                <p>Solo para archivos .docx, .xlsx, .pptx.</p>
                <label className="field">
                  <span>Campo</span>
                  <select value={editField} onChange={(event) => setEditField(event.target.value as OfficeField)}>
                    <option value="author">Autor</option>
                    <option value="title">Titulo</option>
                    <option value="subject">Asunto</option>
                    <option value="company">Empresa</option>
                  </select>
                </label>
                <label className="field">
                  <span>Nuevo valor</span>
                  <input
                    value={editValue}
                    onChange={(event) => setEditValue(event.target.value)}
                    placeholder="Nuevo valor"
                  />
                </label>
                <button className="primary" onClick={handleEditMetadata} disabled={busy.edit}>
                  {busy.edit ? "Actualizando..." : "Actualizar"}
                </button>
              </div>
            </div>
          )}
        </section>
      </main>

      {toast && (
        <div className={`toast ${toast.kind}`}>
          <span>{toast.message}</span>
        </div>
      )}
    </div>
  );
}
