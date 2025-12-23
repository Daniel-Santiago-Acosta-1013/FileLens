import type { ReactNode } from "react";
import "./MetaRow.css";

type MetaRowProps = {
  label: string;
  value: ReactNode;
};

export default function MetaRow({ label, value }: MetaRowProps) {
  return (
    <div className="meta-row">
      <span>{label}</span>
      <span className="meta-value">{value}</span>
    </div>
  );
}
