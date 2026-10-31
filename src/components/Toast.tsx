import {
  createContext,
  useCallback,
  useContext,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { IconAlert, IconCheckCircle, IconInfo, IconX } from "./icons";

type ToastTone = "info" | "success" | "error";

interface ToastItem {
  id: number;
  tone: ToastTone;
  message: string;
  detail?: string;
}

interface ToastContextValue {
  push: (tone: ToastTone, message: string, detail?: string) => void;
}

const ToastContext = createContext<ToastContextValue | null>(null);

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<ToastItem[]>([]);
  const nextId = useRef(1);

  const remove = useCallback((id: number) => {
    setToasts((items) => items.filter((item) => item.id !== id));
  }, []);

  const push = useCallback(
    (tone: ToastTone, message: string, detail?: string) => {
      const id = nextId.current++;
      setToasts((items) => [...items.slice(-4), { id, tone, message, detail }]);
      window.setTimeout(() => remove(id), tone === "error" ? 9000 : 5000);
    },
    [remove],
  );

  return (
    <ToastContext.Provider value={{ push }}>
      {children}
      <div className="toast-stack" aria-live="polite">
        {toasts.map((toast) => (
          <div key={toast.id} className={`toast toast-${toast.tone}`}>
            <span className="toast-icon">
              {toast.tone === "success" ? (
                <IconCheckCircle size={18} />
              ) : toast.tone === "error" ? (
                <IconAlert size={18} />
              ) : (
                <IconInfo size={18} />
              )}
            </span>
            <div className="toast-copy">
              <strong>{toast.message}</strong>
              {toast.detail ? <span>{toast.detail}</span> : null}
            </div>
            <button
              className="icon-button icon-button-sm"
              onClick={() => remove(toast.id)}
              aria-label="Dismiss"
            >
              <IconX size={14} />
            </button>
          </div>
        ))}
      </div>
    </ToastContext.Provider>
  );
}

export function useToast(): ToastContextValue {
  const context = useContext(ToastContext);
  if (!context) {
    throw new Error("useToast must be used inside ToastProvider");
  }
  return context;
}
