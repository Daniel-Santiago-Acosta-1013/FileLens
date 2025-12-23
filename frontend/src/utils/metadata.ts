import { OFFICE_FIELD_LABELS, SYSTEM_ALLOWLIST } from "../constants";
import type { MetadataReport, ReportEntry } from "../types/metadata";

export const getEntry = (report: MetadataReport | null, label: string) => {
  if (!report) return null;
  return report.system.find((entry) => entry.label === label) ?? null;
};

export const extractSystem = (report: MetadataReport | null) => {
  if (!report) return [] as ReportEntry[];
  return report.system.filter((entry) => SYSTEM_ALLOWLIST.has(entry.label));
};

export const extractOfficeValue = (report: MetadataReport | null, label: string) => {
  if (!report) return "";
  for (const section of report.internal) {
    for (const entry of section.entries) {
      if (entry.label === label) return entry.value;
    }
  }
  return "";
};

export const buildOfficeValues = (report: MetadataReport | null) => ({
  author: extractOfficeValue(report, OFFICE_FIELD_LABELS.author),
  title: extractOfficeValue(report, OFFICE_FIELD_LABELS.title),
  subject: extractOfficeValue(report, OFFICE_FIELD_LABELS.subject),
  company: extractOfficeValue(report, OFFICE_FIELD_LABELS.company)
});
