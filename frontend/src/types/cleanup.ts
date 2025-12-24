export type DirectoryAnalysisSummary = {
  total_files: number;
  images_count: number;
  office_count: number;
  extension_counts: [string, number][];
  image_extensions: string[];
  office_extensions: string[];
};

export type CleanupProgress =
  | { type: "started"; total: number }
  | { type: "processing"; index: number; total: number; path: string }
  | { type: "success"; path: string }
  | { type: "failure"; path: string; error: string }
  | { type: "finished"; successes: number; failures: number };

export type CleanupState = {
  running: boolean;
  total: number;
  index: number;
  successes: number;
  failures: number;
  current: string;
  lastError: string;
  finished: boolean;
};

export type CleanAnalysisStatus = "queued" | "analyzing" | "ready" | "error";

export type CleanCleanupStatus = "idle" | "queued" | "cleaning" | "success" | "error";

export type CleanFileItem = {
  path: string;
  name: string;
  analysisStatus: CleanAnalysisStatus;
  analysisError: string;
  report: import("./metadata").MetadataReport | null;
  cleanupStatus: CleanCleanupStatus;
  cleanupError: string;
};
