export type EntryLevel = "Info" | "Warning" | "Success" | "Error" | "Muted";

export type ReportEntry = {
  label: string;
  value: string;
  level: EntryLevel;
};

export type SectionNotice = {
  message: string;
  level: EntryLevel;
};

export type ReportSection = {
  title: string;
  entries: ReportEntry[];
  notice?: SectionNotice | null;
};

export type MetadataReport = {
  system: ReportEntry[];
  internal: ReportSection[];
  risks: ReportEntry[];
  errors: string[];
};
