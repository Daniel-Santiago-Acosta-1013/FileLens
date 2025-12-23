import type { CleanupState } from "./types/cleanup";
import type { NavItem, OfficeField } from "./types/ui";

export const NAV_ITEMS: NavItem[] = [
  { id: "analyze", label: "Analisis" },
  { id: "clean", label: "Limpieza" }
];

export const CLEANUP_EMPTY: CleanupState = {
  running: false,
  total: 0,
  index: 0,
  successes: 0,
  failures: 0,
  current: "",
  lastError: "",
  finished: false
};

export const SYSTEM_ALLOWLIST = new Set([
  "Nombre",
  "Tipo",
  "Tipo de archivo",
  "Extensión del tipo de archivo",
  "Tamaño",
  "Tamaño (bytes)",
  "Categoría",
  "Encabezado (hex)",
  "Tipo MIME",
  "Hash MD5",
  "Hash SHA-256",
  "Última modificación",
  "Fecha de creación"
]);

export const OFFICE_FIELD_LABELS: Record<OfficeField, string> = {
  author: "Creador",
  title: "Título",
  subject: "Asunto",
  company: "Empresa"
};
