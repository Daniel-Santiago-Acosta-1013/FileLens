import type { LogEntry } from "../../../types/ui";
import "./LogPanel.css";

type LogPanelProps = {
  entries: LogEntry[];
};

const LEVEL_LABELS: Record<LogEntry["level"], string> = {
  info: "INFO",
  success: "OK",
  warning: "WARN",
  error: "ERROR"
};

export default function LogPanel({ entries }: LogPanelProps) {
  if (!entries.length) {
    return <p className="muted">Sin logs todavia.</p>;
  }

  return (
    <div className="log-panel">
      {entries.map((entry) => (
        <div key={entry.id} className={`log-entry log-entry--${entry.level}`}>
          <div className="log-entry__meta">
            <span className={`log-entry__level log-entry__level--${entry.level}`}>
              {LEVEL_LABELS[entry.level]}
            </span>
            <span>{entry.time}</span>
            {entry.context && <span className="log-entry__context">{entry.context}</span>}
          </div>
          <strong className="log-entry__message">{entry.message}</strong>
          {entry.detail && <pre className="log-entry__detail">{entry.detail}</pre>}
        </div>
      ))}
    </div>
  );
}
