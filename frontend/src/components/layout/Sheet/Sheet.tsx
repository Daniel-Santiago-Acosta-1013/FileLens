import type { ReactNode } from "react";
import "./Sheet.css";

type SheetProps = {
  children: ReactNode;
};

export default function Sheet({ children }: SheetProps) {
  return <div className="sheet">{children}</div>;
}
