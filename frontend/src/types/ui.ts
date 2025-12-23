export type ViewId = "analyze" | "clean";

export type ToastKind = "info" | "success" | "warning" | "error";

export type Toast = {
  kind: ToastKind;
  message: string;
};

export type Filter = "all" | "images" | "office";

export type OfficeField = "author" | "title" | "subject" | "company";

export type DropTarget = "analyze-file" | "clean-directory" | "clean-files";

export type CleanMode = "directory" | "files";

export type NavItem = {
  id: ViewId;
  label: string;
};
