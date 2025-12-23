import type { NavItem, ViewId } from "../../../types/ui";
import "./Sidebar.css";

type SidebarProps = {
  items: NavItem[];
  active: ViewId;
  onSelect: (id: ViewId) => void;
  running: boolean;
};

export default function Sidebar({ items, active, onSelect, running }: SidebarProps) {
  return (
    <aside className="sidebar">
      <div className="brand">
        <div className="brand-mark">FL</div>
        <div>
          <strong>FileLens</strong>
          <span>Desktop</span>
        </div>
      </div>
      <nav className="nav">
        {items.map((item) => (
          <button
            key={item.id}
            className={`nav-btn ${active === item.id ? "active" : ""}`}
            onClick={() => onSelect(item.id)}
          >
            {item.label}
          </button>
        ))}
      </nav>
      <div className="sidebar-footer">
        <span className="status-dot" />
        <span>{running ? "Procesando" : "Listo"}</span>
      </div>
    </aside>
  );
}
