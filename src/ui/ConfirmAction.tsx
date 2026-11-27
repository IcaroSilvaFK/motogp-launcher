import { useEffect, useId, useRef } from 'react';
import type { KeyboardEvent } from 'react';

/**
 * Inline confirmation for a destructive action (#145), in the style of the comment delete confirm.
 * Focus lands on Cancel (the safe choice), Esc cancels, and when it closes focus returns to the
 * element named by `returnFocusId` (the button that opened it).
 */
export function ConfirmAction({
  prompt,
  confirmLabel,
  cancelLabel,
  returnFocusId,
  onConfirm,
  onCancel,
}: {
  prompt: string;
  confirmLabel: string;
  cancelLabel: string;
  returnFocusId: string;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const promptId = useId();
  const cancelRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    cancelRef.current?.focus();
    return () => {
      // The trigger is re-rendered in the same commit, so focus it once that commit is done.
      queueMicrotask(() => document.getElementById(returnFocusId)?.focus());
    };
  }, [returnFocusId]);

  const onKeyDown = (event: KeyboardEvent<HTMLElement>) => {
    if (event.key !== 'Escape') return;
    event.preventDefault();
    event.stopPropagation();
    onCancel();
  };

  return (
    <div role="group" aria-labelledby={promptId} className="inline-confirm" onKeyDown={onKeyDown}>
      <p id={promptId} className="inline-confirm-text">
        {prompt}
      </p>
      <div className="inline-confirm-actions">
        <button type="button" className="btn btn-sm btn-danger" onClick={onConfirm}>
          {confirmLabel}
        </button>
        <button ref={cancelRef} type="button" className="btn btn-sm btn-ghost" onClick={onCancel}>
          {cancelLabel}
        </button>
      </div>
    </div>
  );
}
