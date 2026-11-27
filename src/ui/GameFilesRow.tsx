import { useId } from 'react';
import { selectRestoreBlock } from '../application';
import { Icon } from './Icon';
import { useI18n } from './i18n';
import { useLauncher } from './LauncherContext';

/**
 * Settings > Storage, "Game files" row (#150): says that original game files are never changed, how
 * many mod files are deployed right now, and offers to remove them all. While it cannot run the
 * button stays focusable with `aria-disabled` and an attached reason (same pattern as InstallButton).
 */
export function GameFilesRow() {
  const { t } = useI18n();
  const { controller, state } = useLauncher();
  const reasonId = useId();
  const text = t.storage.gameFiles;
  const block = selectRestoreBlock(state);
  const reason = block === null ? null : text.blocked[block];
  const disabled = reason !== null || state.busy;
  const blockProps =
    reason === null ? {} : { 'aria-disabled': true as const, 'aria-describedby': reasonId, title: reason };

  return (
    <div className="setting-row">
      <div className="setting-text">
        <p className="setting-label">{text.label}</p>
        <p className="hint">{text.hint}</p>
        <p className="hint num">{text.status(state.deployedFiles)}</p>
        {reason !== null && (
          <p id={reasonId} className="hint">
            {reason}
          </p>
        )}
      </div>
      <div className="setting-control">
        <button
          type="button"
          className="btn btn-sm"
          {...blockProps}
          aria-busy={state.busy}
          onClick={() => {
            if (!disabled) void controller.restoreGameFiles();
          }}
        >
          <Icon name="refresh" />
          {state.busy ? text.restoring : text.restore}
        </button>
      </div>
    </div>
  );
}
