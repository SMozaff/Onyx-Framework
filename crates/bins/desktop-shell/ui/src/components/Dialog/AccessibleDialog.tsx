import { useEffect, useId, useRef, type ReactNode, type RefObject } from "react";

interface AccessibleDialogProps {
  open: boolean;
  title: string;
  description?: string;
  initialFocusRef?: RefObject<HTMLElement | null>;
  busy?: boolean;
  closeOnEscape?: boolean;
  onRequestClose: () => void;
  children: ReactNode;
}

function focusableElements(root: HTMLElement): HTMLElement[] {
  return Array.from(root.querySelectorAll<HTMLElement>('button:not([disabled]), [href], textarea:not([disabled]), input:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])'));
}

export default function AccessibleDialog({
  open,
  title,
  description,
  initialFocusRef,
  busy = false,
  closeOnEscape = true,
  onRequestClose,
  children,
}: AccessibleDialogProps) {
  const dialogRef = useRef<HTMLElement>(null);
  const previousFocusRef = useRef<HTMLElement | null>(null);
  const titleId = useId();
  const descriptionId = useId();

  useEffect(() => {
    if (!open) return;
    previousFocusRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const focusInitial = () => {
      const target = initialFocusRef?.current ?? focusableElements(dialogRef.current!)[0];
      target?.focus();
    };
    focusInitial();

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && closeOnEscape && !busy) {
        event.preventDefault();
        onRequestClose();
        return;
      }
      if (event.key !== "Tab" || !dialogRef.current) return;
      const elements = focusableElements(dialogRef.current);
      if (!elements.length) return;
      const first = elements[0];
      const last = elements[elements.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      previousFocusRef.current?.focus();
    };
  }, [open, busy, closeOnEscape, initialFocusRef, onRequestClose]);

  if (!open) return null;
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4" role="presentation">
      <section ref={dialogRef} className="w-full max-w-md rounded-lg border border-onyx-border bg-onyx-surface p-5" role="dialog" aria-modal="true" aria-labelledby={titleId} aria-describedby={description ? descriptionId : undefined}>
        <h2 id={titleId} className="text-base font-semibold text-onyx-text">{title}</h2>
        {description && <p id={descriptionId} className="mt-1 text-sm text-onyx-text-dim">{description}</p>}
        {children}
      </section>
    </div>
  );
}
