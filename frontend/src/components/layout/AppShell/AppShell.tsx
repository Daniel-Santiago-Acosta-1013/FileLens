import type { ReactNode } from "react";
import "./AppShell.css";

type AppShellProps = {
  children: ReactNode;
};

export default function AppShell({ children }: AppShellProps) {
  return <div className="app-shell">{children}</div>;
}
