import type { ReactNode } from "react";
import "./Section.css";

type SectionProps = {
  label: string;
  children: ReactNode;
};

export default function Section({ label, children }: SectionProps) {
  return (
    <div className="section">
      <span className="label">{label}</span>
      {children}
    </div>
  );
}
