import { useMemo, useState } from "react";
import type { HTMLAttributes } from "react";
import DropZone from "../../components/molecules/DropZone/DropZone";
import SegmentedControl from "../../components/molecules/SegmentedControl/SegmentedControl";
import Button from "../../components/atoms/Button/Button";
import Toggle from "../../components/atoms/Toggle/Toggle";
import Section from "../../components/layout/Section/Section";
import Sheet from "../../components/layout/Sheet/Sheet";
import MetaRow from "../../components/molecules/MetaRow/MetaRow";
import Note from "../../components/molecules/Note/Note";
import type { CleanFileItem, DirectoryAnalysisSummary } from "../../types/cleanup";
import type { CleanMode, DropTarget } from "../../types/ui";
import { extractSystem } from "../../utils/metadata";
import "./CleanView.css";

type CleanViewProps = {
  cleanMode: CleanMode;
  dirPath: string;
  selectedFiles: string[];
  recursive: boolean;
  dirSummary: DirectoryAnalysisSummary | null;
  fileSummary: DirectoryAnalysisSummary | null;
  extensionCounts: [string, number][];
  cleanupRunning: boolean;
  dirItems: CleanFileItem[];
  fileItems: CleanFileItem[];
  dropTarget: DropTarget | null;
  dropHandlers: (target: DropTarget) => HTMLAttributes<HTMLDivElement>;
  onSetCleanMode: (mode: CleanMode) => void;
  onPickDirectory: () => void;
  onPickFiles: () => void;
  onToggleRecursive: () => void;
  onCleanItem: (path: string) => void;
  onCleanAll: () => void;
};

const folderIcon = (
  <svg viewBox="0 0 24 24" role="img">
    <path
      d="M4.5 7.75h5.2l1.6 1.7H19a1.5 1.5 0 0 1 1.5 1.5v6.3A1.5 1.5 0 0 1 19 18.75H5A1.5 1.5 0 0 1 3.5 17.25V9.25a1.5 1.5 0 0 1 1-1.5z"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
    <path
      d="M12 14.5v-3m0 0 1.6 1.6M12 11.5 10.4 13.1"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
  </svg>
);

const filesIcon = (
  <svg viewBox="0 0 24 24" role="img">
    <path
      d="M6.5 4.75h7.3l3.7 3.7v10.8A1.5 1.5 0 0 1 16 20.75H6.5A1.5 1.5 0 0 1 5 19.25V6.25a1.5 1.5 0 0 1 1.5-1.5z"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
    <path
      d="M13.5 4.75v3.5h3.5"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
    <path
      d="M12 14.5v-3m0 0 1.6 1.6M12 11.5 10.4 13.1"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
  </svg>
);

const getStatusMeta = (item: CleanFileItem) => {
  if (item.cleanupStatus === "cleaning") {
    return { label: "Limpiando", tone: "status--active" };
  }
  if (item.cleanupStatus === "success") {
    return { label: "Limpio", tone: "status--success" };
  }
  if (item.cleanupStatus === "error") {
    return { label: "Error", tone: "status--error" };
  }
  if (item.cleanupStatus === "queued") {
    return { label: "Preparando", tone: "status--muted" };
  }
  if (item.analysisStatus === "analyzing") {
    return { label: "Analizando", tone: "status--active" };
  }
  if (item.analysisStatus === "queued") {
    return { label: "Preparando", tone: "status--muted" };
  }
  if (item.analysisStatus === "error") {
    return { label: "Error", tone: "status--error" };
  }
  return { label: "Listo", tone: "status--ready" };
};

export default function CleanView({
  cleanMode,
  dirPath,
  selectedFiles,
  recursive,
  dirSummary,
  fileSummary,
  extensionCounts,
  cleanupRunning,
  dirItems,
  fileItems,
  dropTarget,
  dropHandlers,
  onSetCleanMode,
  onPickDirectory,
  onPickFiles,
  onToggleRecursive,
  onCleanItem,
  onCleanAll
}: CleanViewProps) {
  const summary = cleanMode === "directory" ? dirSummary : fileSummary;
  const items = cleanMode === "directory" ? dirItems : fileItems;
  const isDirActive = dropTarget === "clean-directory";
  const isFilesActive = dropTarget === "clean-files";
  const [detailsPath, setDetailsPath] = useState<string | null>(null);

  const detailsItem = useMemo(() => {
    if (!detailsPath) return null;
    return items.find((item) => item.path === detailsPath) ?? null;
  }, [detailsPath, items]);

  const detailsReport = detailsItem?.report ?? null;
  const detailsSystem = useMemo(() => extractSystem(detailsReport), [detailsReport]);

  const analyzingCount = items.filter((item) => item.analysisStatus === "analyzing").length;
  const queuedCount = items.filter((item) => item.analysisStatus === "queued").length;
  const analysisPending = analyzingCount + queuedCount > 0;
  const canCleanAll =
    items.length > 1 &&
    items.some((item) => item.analysisStatus === "ready") &&
    !analysisPending &&
    !cleanupRunning;
  const filteredOutCount = Math.max(0, selectedFiles.length - fileItems.length);

  const closeDetails = () => {
    setDetailsPath(null);
  };

  return (
    <Sheet>
      <Section label="Modo">
        <SegmentedControl
          value={cleanMode}
          options={[
            { id: "directory", label: "Directorio" },
            { id: "files", label: "Archivos" }
          ]}
          onChange={onSetCleanMode}
        />
      </Section>

      {cleanMode === "directory" ? (
        <Section label="Directorio">
          <DropZone
            title="Arrastra y suelta un directorio"
            subtitle="o usa Explorar para seleccionarlo"
            path={dirPath || "Ningun directorio seleccionado"}
            actionLabel="Explorar"
            icon={folderIcon}
            active={isDirActive}
            handlers={dropHandlers("clean-directory")}
            onAction={onPickDirectory}
          />
          <div className="section-row">
            <Toggle
              label="Incluir subdirectorios"
              checked={recursive}
              onChange={onToggleRecursive}
            />
          </div>
          {items.length > 0 ? (
            <div className="clean-toolbar">
              <div className="clean-count">
                <strong>{items.length}</strong>
                <span>archivos cargados</span>
                {analysisPending && (
                  <span className="muted">
                    Analizando {analyzingCount + queuedCount} archivo
                    {analyzingCount + queuedCount === 1 ? "" : "s"}...
                  </span>
                )}
              </div>
              {items.length > 1 && (
                <Button variant="primary" onClick={onCleanAll} disabled={!canCleanAll}>
                  {cleanupRunning ? "Limpiando..." : "Limpiar todos"}
                </Button>
              )}
            </div>
          ) : (
            <p className="muted">Carga un directorio para iniciar el analisis automatico.</p>
          )}
        </Section>
      ) : (
        <Section label="Archivos">
          <DropZone
            title="Arrastra y suelta archivos"
            subtitle="o usa Explorar para seleccionarlos"
            path={
              selectedFiles.length
                ? `${selectedFiles.length} archivos seleccionados`
                : "Ningun archivo seleccionado"
            }
            actionLabel="Explorar"
            icon={filesIcon}
            active={isFilesActive}
            handlers={dropHandlers("clean-files")}
            onAction={onPickFiles}
          />
          {items.length > 0 ? (
            <div className="clean-toolbar">
              <div className="clean-count">
                <strong>{items.length}</strong>
                <span>archivos cargados</span>
                {filteredOutCount > 0 && (
                  <span className="muted">{filteredOutCount} omitidos por tipo no compatible</span>
                )}
                {analysisPending && (
                  <span className="muted">
                    Analizando {analyzingCount + queuedCount} archivo
                    {analyzingCount + queuedCount === 1 ? "" : "s"}...
                  </span>
                )}
              </div>
              {items.length > 1 && (
                <Button variant="primary" onClick={onCleanAll} disabled={!canCleanAll}>
                  {cleanupRunning ? "Limpiando..." : "Limpiar todos"}
                </Button>
              )}
            </div>
          ) : (
            <p className="muted">Carga archivos para iniciar el analisis automatico.</p>
          )}
        </Section>
      )}

      {items.length > 0 && (
        <Section label="Archivos en limpieza">
          <div className="clean-grid">
            {items.map((item) => {
              const status = getStatusMeta(item);
              const isLoading =
                item.analysisStatus === "analyzing" || item.cleanupStatus === "cleaning";
              const canOpenDetails = item.analysisStatus === "ready";
              const canClean =
                item.analysisStatus === "ready" &&
                item.cleanupStatus === "idle" &&
                !cleanupRunning;
              const cleanLabel =
                item.cleanupStatus === "success"
                  ? "Limpio"
                  : item.cleanupStatus === "cleaning"
                    ? "Limpiando..."
                    : "Limpiar";

              return (
                <article
                  key={item.path}
                  className={`clean-card ${isLoading ? "is-loading" : ""}`.trim()}
                >
                  <div className="clean-card__header">
                    <div className="clean-card__title">
                      <strong>{item.name}</strong>
                      <span className="muted">{item.path}</span>
                    </div>
                    <span className={`status-pill ${status.tone}`}>{status.label}</span>
                  </div>
                  <div className="clean-card__actions">
                    <Button
                      variant="secondary"
                      onClick={() => {
                        setDetailsPath(item.path);
                      }}
                      disabled={!canOpenDetails}
                    >
                      Detalles
                    </Button>
                    <Button variant="primary" onClick={() => onCleanItem(item.path)} disabled={!canClean}>
                      {cleanLabel}
                    </Button>
                  </div>
                  {item.analysisStatus === "error" && (
                    <p className="inline-error">
                      No se pudo analizar{item.analysisError ? `: ${item.analysisError}` : ""}
                    </p>
                  )}
                  {item.cleanupStatus === "error" && (
                    <p className="inline-error">
                      Error de limpieza{item.cleanupError ? `: ${item.cleanupError}` : ""}
                    </p>
                  )}
                </article>
              );
            })}
          </div>
        </Section>
      )}

      <Section label="Desglose">
        {summary ? (
          <div className="summary">
            <div className="summary-row">
              <span>Total</span>
              <strong>{summary.total_files}</strong>
            </div>
            <div className="summary-row">
              <span>Imagenes</span>
              <strong>{summary.images_count}</strong>
            </div>
            <div className="summary-row">
              <span>Office</span>
              <strong>{summary.office_count}</strong>
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
          <p className="muted">Carga archivos para ver el desglose.</p>
        )}
      </Section>

      {detailsItem && detailsReport && (
        <div className="clean-modal-overlay" onClick={closeDetails}>
          <div className="clean-modal" onClick={(event) => event.stopPropagation()}>
            <div className="clean-modal__header">
              <div className="clean-modal__title">
                <span className="label">Detalles</span>
                <strong>{detailsItem.name}</strong>
                <span className="muted">{detailsItem.path}</span>
              </div>
              <Button variant="secondary" onClick={closeDetails}>
                Cerrar
              </Button>
            </div>
            <div className="clean-modal__body">
              <div className="meta-list">
                {detailsSystem.map((entry, index) => (
                  <MetaRow key={`${entry.label}-${index}`} label={entry.label} value={entry.value} />
                ))}
                {detailsReport.internal.map((section) => (
                  <div key={section.title} className="meta-group">
                    <div className="section-title">{section.title}</div>
                    {section.entries.map((entry, index) => (
                      <MetaRow
                        key={`${section.title}-${index}`}
                        label={entry.label}
                        value={entry.value}
                      />
                    ))}
                    {section.notice && <Note tone={section.notice.level}>{section.notice.message}</Note>}
                  </div>
                ))}
                {detailsReport.risks.length > 0 && (
                  <div className="meta-group">
                    <div className="section-title">Riesgos</div>
                    {detailsReport.risks.map((entry, index) => (
                      <MetaRow key={`risk-${index}`} label={entry.label} value={entry.value} />
                    ))}
                  </div>
                )}
                {detailsReport.errors.length > 0 && (
                  <div className="meta-group">
                    <div className="section-title">Errores</div>
                    {detailsReport.errors.map((error, index) => (
                      <Note key={`error-${index}`} tone="Error">
                        {error}
                      </Note>
                    ))}
                  </div>
                )}
              </div>
            </div>
          </div>
        </div>
      )}
    </Sheet>
  );
}
