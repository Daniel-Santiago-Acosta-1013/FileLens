import type { HTMLAttributes } from "react";
import DropZone from "../../components/molecules/DropZone/DropZone";
import Note from "../../components/molecules/Note/Note";
import MetaRow from "../../components/molecules/MetaRow/MetaRow";
import Button from "../../components/atoms/Button/Button";
import Toggle from "../../components/atoms/Toggle/Toggle";
import Section from "../../components/layout/Section/Section";
import Sheet from "../../components/layout/Sheet/Sheet";
import { OFFICE_FIELD_LABELS } from "../../constants";
import { getEntry } from "../../utils/metadata";
import type { MetadataReport, ReportEntry } from "../../types/metadata";
import type { OfficeField } from "../../types/ui";
import "./AnalyzeView.css";

type AnalyzeViewProps = {
  filePath: string;
  includeHash: boolean;
  report: MetadataReport | null;
  reportError: string;
  systemEntries: ReportEntry[];
  typeEntry: ReportEntry | null;
  mimeEntry: ReportEntry | null;
  isOffice: boolean;
  officeValues: Record<OfficeField, string>;
  busy: {
    analyze: boolean;
    remove: boolean;
    edit: boolean;
  };
  dropActive: boolean;
  dropHandlers: HTMLAttributes<HTMLDivElement>;
  onPickFile: () => void;
  onToggleHash: () => void;
  onAnalyze: () => void;
  onRemoveMetadata: () => void;
  onEditField: (field: OfficeField) => void;
  onOfficeValueChange: (field: OfficeField, value: string) => void;
};

const fileIcon = (
  <svg viewBox="0 0 24 24" role="img">
    <path
      d="M7.5 18.5h9a4 4 0 0 0 .5-7.97 5.5 5.5 0 0 0-10.92 1.3A3.5 3.5 0 0 0 7.5 18.5z"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
    <path
      d="M12 14.75V9.5m0 0 2.25 2.25M12 9.5 9.75 11.75"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
  </svg>
);

export default function AnalyzeView({
  filePath,
  includeHash,
  report,
  reportError,
  systemEntries,
  typeEntry,
  mimeEntry,
  isOffice,
  officeValues,
  busy,
  dropActive,
  dropHandlers,
  onPickFile,
  onToggleHash,
  onAnalyze,
  onRemoveMetadata,
  onEditField,
  onOfficeValueChange
}: AnalyzeViewProps) {
  const extension = filePath.split(".").pop()?.toUpperCase() || "-";
  const sizeEntry = getEntry(report, "Tamaño");

  return (
    <Sheet>
      <Section label="Archivo">
        <DropZone
          title="Arrastra y suelta un archivo"
          subtitle="o usa Explorar para seleccionarlo"
          path={filePath || "Ningun archivo seleccionado"}
          actionLabel="Explorar"
          icon={fileIcon}
          active={dropActive}
          handlers={dropHandlers}
          onAction={onPickFile}
        />
        <div className="section-row">
          <Toggle
            label="Calcular hashes (MD5 + SHA-256)"
            checked={includeHash}
            onChange={onToggleHash}
          />
          <Button variant="primary" onClick={onAnalyze} disabled={busy.analyze}>
            {busy.analyze ? "Analizando..." : "Analizar"}
          </Button>
        </div>
        {reportError && <p className="inline-error">{reportError}</p>}
      </Section>

      <Section label="Tipo detectado">
        <div className="meta-inline">
          <span>{typeEntry?.value || "Archivo"}</span>
          <span>{extension}</span>
          <span className="muted">{mimeEntry?.value || "MIME no disponible"}</span>
          <span className="muted">{sizeEntry?.value || ""}</span>
        </div>
      </Section>

      <Section label="Metadata encontrada">
        {report ? (
          <div className="meta-list">
            {systemEntries.map((entry, index) => (
              <MetaRow key={`${entry.label}-${index}`} label={entry.label} value={entry.value} />
            ))}
            {report.internal.map((section) => (
              <div key={section.title} className="meta-group">
                <div className="section-title">{section.title}</div>
                {section.entries.map((entry, index) => (
                  <MetaRow key={`${section.title}-${index}`} label={entry.label} value={entry.value} />
                ))}
                {section.notice && <Note tone={section.notice.level}>{section.notice.message}</Note>}
              </div>
            ))}
            {report.risks.length > 0 && (
              <div className="meta-group">
                <div className="section-title">Riesgos</div>
                {report.risks.map((entry, index) => (
                  <MetaRow key={`risk-${index}`} label={entry.label} value={entry.value} />
                ))}
              </div>
            )}
          </div>
        ) : (
          <p className="muted">Ejecuta el analisis para ver los resultados.</p>
        )}
      </Section>

      <Section label="Acciones sobre metadata">
        <div className="section-row">
          <Button variant="danger" onClick={onRemoveMetadata} disabled={busy.remove || !report}>
            {busy.remove ? "Eliminando..." : "Eliminar metadata"}
          </Button>
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
                    onChange={(event) => onOfficeValueChange(fieldKey as OfficeField, event.target.value)}
                    placeholder="(vacio)"
                  />
                </label>
                <Button
                  variant="secondary"
                  onClick={() => onEditField(fieldKey as OfficeField)}
                  disabled={busy.edit || !report}
                >
                  Guardar
                </Button>
              </div>
            ))}
          </div>
        )}
      </Section>
    </Sheet>
  );
}
