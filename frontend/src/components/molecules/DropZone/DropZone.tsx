import type { HTMLAttributes, ReactNode } from "react";
import Button from "../../atoms/Button/Button";
import "./DropZone.css";

type DropZoneProps = {
  title: string;
  subtitle: string;
  path: string;
  actionLabel: string;
  icon: ReactNode;
  active?: boolean;
  onAction: () => void;
  handlers?: HTMLAttributes<HTMLDivElement>;
};

export default function DropZone({
  title,
  subtitle,
  path,
  actionLabel,
  icon,
  active,
  onAction,
  handlers
}: DropZoneProps) {
  const { className: handlerClassName = "", ...rest } = handlers ?? {};
  const classes = `drop-zone ${active ? "drop-zone--active" : ""} ${handlerClassName}`.trim();

  return (
    <div className={classes} {...rest}>
      <div className="drop-zone__content">
        <div className="drop-zone__icon" aria-hidden="true">
          {icon}
        </div>
        <div className="drop-zone__text">
          <strong>{title}</strong>
          <span>{subtitle}</span>
          <div className="drop-zone__path">{path}</div>
        </div>
      </div>
      <div className="drop-zone__actions">
        <Button
          variant="secondary"
          onClick={(event) => {
            event.stopPropagation();
            onAction();
          }}
        >
          {actionLabel}
        </Button>
      </div>
    </div>
  );
}
