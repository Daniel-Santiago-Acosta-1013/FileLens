import Section from "../../components/layout/Section/Section";
import Sheet from "../../components/layout/Sheet/Sheet";
import LogPanel from "../../components/organisms/LogPanel/LogPanel";
import type { LogEntry } from "../../types/ui";
import "./LogsView.css";

type LogsViewProps = {
  logs: LogEntry[];
};

export default function LogsView({ logs }: LogsViewProps) {
  return (
    <div className="logs-view">
      <Sheet>
        <Section label="Logs">
          <LogPanel entries={logs} />
        </Section>
      </Sheet>
    </div>
  );
}
