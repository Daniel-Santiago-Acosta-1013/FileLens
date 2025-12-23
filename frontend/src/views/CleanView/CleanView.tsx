import type { HTMLAttributes } from "react";
import DropZone from "../../components/molecules/DropZone/DropZone";
import SegmentedControl from "../../components/molecules/SegmentedControl/SegmentedControl";
import Button from "../../components/atoms/Button/Button";
import Toggle from "../../components/atoms/Toggle/Toggle";
import Section from "../../components/layout/Section/Section";
import Sheet from "../../components/layout/Sheet/Sheet";
import type { CleanupState, DirectoryAnalysisSummary } from "../../types/cleanup";
import type { CleanMode, DropTarget, Filter } from "../../types/ui";
import "./CleanView.css";

type CleanViewProps = {
  cleanMode: CleanMode;
  dirPath: string;
  selectedFiles: string[];
  recursive: boolean;
  filter: Filter;
  dirSummary: DirectoryAnalysisSummary | null;
  fileSummary: DirectoryAnalysisSummary | null;
  extensionCounts: [string, number][];
  cleanup: CleanupState;
  busy: {
    dirAnalyze: boolean;
    fileAnalyze: boolean;
    cleanup: boolean;
  };
  dropTarget: DropTarget | null;
  dropHandlers: (target: DropTarget) => HTMLAttributes<HTMLDivElement>;
  onSetCleanMode: (mode: CleanMode) => void;
  onPickDirectory: () => void;
  onPickFiles: () => void;
  onToggleRecursive: () => void;
  onSetFilter: (filter: Filter) => void;
  onAnalyzeDirectory: () => void;
  onAnalyzeFiles: () => void;
  onStartCleanup: () => void;
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

export default function CleanView({
  cleanMode,
  dirPath,
  selectedFiles,
  recursive,
  filter,
  dirSummary,
  fileSummary,
  extensionCounts,
  cleanup,
  busy,
  dropTarget,
  dropHandlers,
  onSetCleanMode,
  onPickDirectory,
  onPickFiles,
  onToggleRecursive,
  onSetFilter,
  onAnalyzeDirectory,
  onAnalyzeFiles,
  onStartCleanup
}: CleanViewProps) {
  const summary = cleanMode === "directory" ? dirSummary : fileSummary;
  const isDirActive = dropTarget === "clean-directory";
  const isFilesActive = dropTarget === "clean-files";

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
            <SegmentedControl
              value={filter}
              options={[
                { id: "all", label: "Todos" },
                { id: "images", label: "Imagenes" },
                { id: "office", label: "Office" }
              ]}
              onChange={onSetFilter}
            />
          </div>
          <div className="section-row">
            <Button variant="secondary" onClick={onAnalyzeDirectory} disabled={busy.dirAnalyze}>
              {busy.dirAnalyze ? "Analizando..." : "Analizar"}
            </Button>
            <Button variant="primary" onClick={onStartCleanup} disabled={busy.cleanup}>
              {busy.cleanup ? "Procesando..." : "Limpiar"}
            </Button>
          </div>
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
          <div className="section-row">
            <SegmentedControl
              value={filter}
              options={[
                { id: "all", label: "Todos" },
                { id: "images", label: "Imagenes" },
                { id: "office", label: "Office" }
              ]}
              onChange={onSetFilter}
            />
          </div>
          {selectedFiles.length > 0 && (
            <div className="file-list">
              {selectedFiles.slice(0, 3).map((file) => (
                <div key={file} className="file-item">
                  {file}
                </div>
              ))}
              {selectedFiles.length > 3 && (
                <div className="file-item muted">+ {selectedFiles.length - 3} mas</div>
              )}
            </div>
          )}
          <div className="section-row">
            <Button variant="secondary" onClick={onAnalyzeFiles} disabled={busy.fileAnalyze}>
              {busy.fileAnalyze ? "Analizando..." : "Analizar"}
            </Button>
            <Button variant="primary" onClick={onStartCleanup} disabled={busy.cleanup}>
              {busy.cleanup ? "Procesando..." : "Limpiar"}
            </Button>
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
          <p className="muted">Ejecuta el analisis para ver el desglose.</p>
        )}
      </Section>

      <Section label="Progreso">
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
          <strong>
            {cleanup.successes} / {cleanup.failures}
          </strong>
        </div>
        {cleanup.lastError && <p className="inline-error">Ultimo error: {cleanup.lastError}</p>}
      </Section>
    </Sheet>
  );
}
