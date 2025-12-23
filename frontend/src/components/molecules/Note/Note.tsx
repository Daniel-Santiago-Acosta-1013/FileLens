import type { ReactNode } from "react";
import type { EntryLevel } from "../../../types/metadata";
import "./Note.css";

type NoteProps = {
  tone?: EntryLevel;
  children: ReactNode;
};

const TONE_CLASS: Record<EntryLevel, string> = {
  Info: "note--info",
  Success: "note--success",
  Warning: "note--warning",
  Error: "note--error",
  Muted: "note--muted"
};

export default function Note({ tone = "Info", children }: NoteProps) {
  return <div className={`note ${TONE_CLASS[tone]}`}>{children}</div>;
}
