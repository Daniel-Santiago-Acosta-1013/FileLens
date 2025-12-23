import type { Toast as ToastType } from "../../../types/ui";
import "./Toast.css";

type ToastProps = {
  toast: ToastType;
};

export default function Toast({ toast }: ToastProps) {
  return (
    <div className={`toast toast--${toast.kind}`}>
      <span>{toast.message}</span>
    </div>
  );
}
