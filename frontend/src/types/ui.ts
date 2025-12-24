export type ViewId = "analyze" | "clean" | "logs";

export type ToastKind = "info" | "success" | "warning" | "error";

export type Toast = {
  kind: ToastKind;
  message: string;
};

export type LogLevel = "info" | "success" | "warning" | "error";

export type LogEntry = {
  id: string;
  time: string;
  level: LogLevel;
  context: string;
  message: string;
  detail: string;
};

export type OfficeField = "author" | "title" | "subject" | "company";

export type DropTarget = "analyze-file" | "clean-directory" | "clean-files";

export type CleanMode = "directory" | "files";

export type ExportFormat = "json" | "txt" | "xlsx" | "pdf";

export type NavItem = {
  id: ViewId;
  label: string;
};
