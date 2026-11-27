import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { FormEvent } from 'react';
import { selectProfileLocked } from '../application';
import type { FileConflict, Profile } from '../domain';
import { isVanilla, moveItem, toggleMod } from '../domain';
import { ConfirmAction } from './ConfirmAction';
import { Icon } from './Icon';
import { useI18n } from './i18n';
import { useLauncher } from './LauncherContext';
import { stagger } from './motion';

type Direction = 'up' | 'down';

/**
 * Conflicts of the unsaved draft order. Each order change asks the back end again; a response
 * that arrives after a newer request is dropped. Fewer than two mods cannot conflict, so nothing
 * is asked.
 */
function ConflictsPanel({ order, nameOf }: { order: string[]; nameOf: (id: string) => string }) {
  const { t } = useI18n();
  const { controller } = useLauncher();
  const [conflicts, setConflicts] = useState<FileConflict[]>([]);
  // A string key so the effect reruns on a real order change, not on every render.
  const orderKey = JSON.stringify(order);

  // The previous result stays on screen while a new one loads, so the panel does not flicker.
  const shown = order.length < 2 ? [] : conflicts;

  useEffect(() => {
    const ids = JSON.parse(orderKey) as string[];
    if (ids.length < 2) return;
    let current = true;
    void controller.previewConflicts(ids).then((result) => {
      if (current) setConflicts(result);
    });
    return () => {
      current = false;
    };
  }, [controller, orderKey]);

  return (
    <section aria-labelledby="conflicts-title" className="conflicts">
      <div className="conflicts-head">
        <h4 id="conflicts-title" className="section-title">
          {t.profiles.conflictsTitle}
        </h4>
        <p role="status" aria-live="polite" className={shown.length === 0 ? 'badge badge-off' : 'badge badge-warning'}>
          {t.profiles.conflictsSummary(shown.length)}
        </p>
      </div>
      {shown.length === 0 ? (
        <div className="empty-state empty-state-inline">
          <span className="empty-icon" aria-hidden="true">
            <Icon name="check" />
          </span>
          <p className="hint">{t.profiles.conflictsEmpty}</p>
        </div>
      ) : (
        <>
          <p className="hint">{t.profiles.conflictsHint}</p>
          <ul className="conflict-list">
            {shown.map((conflict, index) => {
              const pathId = `conflict-path-${index}`;
              return (
                <li key={conflict.path} className="conflict-item">
                  <span id={pathId} className="conflict-path" title={conflict.path}>
                    {conflict.path}
                  </span>
                  <ol className="conflict-mods" aria-labelledby={pathId}>
                    {conflict.mod_ids.map((id) => (
                      <li
                        key={id}
                        className={id === conflict.winner ? 'conflict-mod conflict-mod-winner' : 'conflict-mod'}
                      >
                        <span className="conflict-mod-name">{nameOf(id)}</span>
                        {id === conflict.winner && (
                          <>
                            {' '}
                            <span className="badge badge-info">{t.profiles.conflictWinner}</span>
                          </>
                        )}
                      </li>
                    ))}
                  </ol>
                </li>
              );
            })}
          </ul>
        </>
      )}
    </section>
  );
}

function ProfileEditor({ profile }: { profile: Profile }) {
  const { t } = useI18n();
  const { controller, state } = useLauncher();
  const [name, setName] = useState(profile.name);
  const [draft, setDraft] = useState<string[]>(profile.mod_ids);
  const modsById = new Map(state.mods.map((mod) => [mod.id, mod]));
  const ordered = draft.filter((id) => modsById.has(id));
  const nameOf = (id: string) => modsById.get(id)?.name ?? id;

  // Keyboard focus follows the moved mod: its button in the same direction, or the other one
  // when the move reached an end of the list (that button is then disabled).
  const moveButtons = useRef(new Map<string, HTMLButtonElement>());
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const [moved, setMoved] = useState<{ id: string; direction: Direction } | null>(null);
  useLayoutEffect(() => {
    if (moved === null) return;
    const same = moveButtons.current.get(`${moved.id}:${moved.direction}`);
    const other = moveButtons.current.get(`${moved.id}:${moved.direction === 'up' ? 'down' : 'up'}`);
    (same && !same.disabled ? same : other)?.focus();
  }, [moved]);

  const move = (id: string, index: number, direction: Direction) => {
    setDraft(moveItem(ordered, index, direction === 'up' ? -1 : 1));
    setMoved({ id, direction });
  };

  const bindMoveButton = (id: string, direction: Direction) => (element: HTMLButtonElement | null) => {
    const key = `${id}:${direction}`;
    if (element) moveButtons.current.set(key, element);
    else moveButtons.current.delete(key);
  };

  // aria-disabled (not disabled) keeps the actions focusable so the reason stays reachable.
  const locked = selectProfileLocked(state);
  const lockProps = {
    'aria-disabled': locked || undefined,
    'aria-describedby': locked ? 'profile-editor-lock-hint' : undefined,
    title: locked ? t.profiles.locked : undefined,
  };

  const rename = (event: FormEvent) => {
    event.preventDefault();
    if (!locked) void controller.renameProfile(profile.id, name);
  };

  return (
    <section aria-label={profile.name} className="card editor">
      <div className="card-head">
        <h3>{profile.name}</h3>
        {!isVanilla(profile) && (
          <button
            id="delete-profile"
            type="button"
            className="btn btn-danger btn-sm"
            aria-label={t.profiles.deleteNamed(profile.name)}
            {...lockProps}
            onClick={() => {
              if (!locked) setConfirmingDelete(true);
            }}
          >
            {t.profiles.delete}
          </button>
        )}
      </div>

      {confirmingDelete && !locked && !isVanilla(profile) && (
        <ConfirmAction
          prompt={t.profiles.confirmDelete(profile.name)}
          confirmLabel={t.profiles.delete}
          cancelLabel={t.profiles.cancel}
          returnFocusId="delete-profile"
          onConfirm={() => {
            setConfirmingDelete(false);
            void controller.deleteProfile(profile.id);
          }}
          onCancel={() => setConfirmingDelete(false)}
        />
      )}

      {locked && !isVanilla(profile) && (
        <p id="profile-editor-lock-hint" className="hint">
          {t.profiles.locked}
        </p>
      )}

      {isVanilla(profile) ? (
        <p className="hint">{t.profiles.vanillaLocked}</p>
      ) : (
        <form className="form-row" onSubmit={rename}>
          <div className="form-field">
            <label htmlFor="rename-profile">{t.profiles.renameLabel}</label>
            <input
              id="rename-profile"
              className="input"
              value={name}
              onChange={(event) => setName(event.target.value)}
            />
          </div>
          <button type="submit" className="btn" {...lockProps}>
            {t.profiles.rename}
          </button>
        </form>
      )}

      {!isVanilla(profile) && (
        <>
          <div className="editor-columns">
            <div className="editor-column">
              <h4 className="section-title">{t.profiles.mods}</h4>
              {state.mods.length === 0 ? (
                <p className="hint">{t.profiles.noMods}</p>
              ) : (
                <ul className="plain check-list">
                  {state.mods.map((mod) => (
                    <li key={mod.id}>
                      <label className="check-row">
                        <input
                          type="checkbox"
                          checked={draft.includes(mod.id)}
                          onChange={() => setDraft((current) => toggleMod(current, mod.id))}
                        />
                        <span>{mod.name}</span>
                      </label>
                    </li>
                  ))}
                </ul>
              )}
            </div>

            {ordered.length > 0 && (
              <div className="editor-column">
                <h4 id="load-order-title" className="section-title">
                  {t.profiles.orderTitle}
                </h4>
                <p className="hint">{t.profiles.orderHint}</p>
                <ol className="order-list" aria-labelledby="load-order-title">
                  {ordered.map((id, index) => {
                    const modName = nameOf(id);
                    return (
                      <li key={id}>
                        {/* The list already tells assistive tech the position; the number is visual. */}
                        <span className="order-pos num" aria-hidden="true">
                          {index + 1}
                        </span>
                        <span className="order-name" title={modName}>
                          {modName}
                        </span>
                        <button
                          ref={bindMoveButton(id, 'up')}
                          type="button"
                          className="btn btn-icon btn-sm"
                          disabled={index === 0}
                          aria-label={t.profiles.moveUp(modName)}
                          onClick={() => move(id, index, 'up')}
                        >
                          <Icon name="up" />
                        </button>
                        <button
                          ref={bindMoveButton(id, 'down')}
                          type="button"
                          className="btn btn-icon btn-sm"
                          disabled={index === ordered.length - 1}
                          aria-label={t.profiles.moveDown(modName)}
                          onClick={() => move(id, index, 'down')}
                        >
                          <Icon name="down" />
                        </button>
                      </li>
                    );
                  })}
                </ol>
              </div>
            )}
          </div>

          <ConflictsPanel order={ordered} nameOf={nameOf} />

          <div className="card-footer">
            <button
              type="button"
              className="btn btn-accent-outline"
              {...lockProps}
              onClick={() => {
                if (!locked) void controller.setProfileMods(profile.id, ordered);
              }}
            >
              {t.profiles.save}
            </button>
          </div>
        </>
      )}
    </section>
  );
}

export function ProfilesScreen() {
  const { t } = useI18n();
  const { controller, state } = useLauncher();
  const [newName, setNewName] = useState('');
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const selected = state.profiles.find((profile) => profile.id === selectedId) ?? null;
  const active = state.profiles.find((profile) => profile.id === state.config.active_profile) ?? null;
  const modIds = new Set(state.mods.map((mod) => mod.id));

  const locked = selectProfileLocked(state);

  const create = (event: FormEvent) => {
    event.preventDefault();
    if (locked) return;
    void controller.createProfile(newName);
    setNewName('');
  };

  return (
    <section aria-labelledby="profiles-title" className="page">
      <div className="page-head">
        <div className="page-heading">
          <h2 id="profiles-title">{t.profiles.title}</h2>
          <p className="page-subtitle">{t.profiles.summary(state.profiles.length, active?.name ?? null)}</p>
        </div>
      </div>

      <div className="split">
        <div className="split-rail">
          <div className="panel">
            <h3 id="profiles-list-title" className="rail-title">
              {t.profiles.listTitle}
            </h3>
            <ul className="plain profile-list" aria-labelledby="profiles-list-title">
              {state.profiles.map((profile, index) => {
                const count = profile.mod_ids.filter((id) => modIds.has(id)).length;
                return (
                  <li key={profile.id} className="profile-item stagger-item" style={stagger(index)}>
                    <button
                      type="button"
                      className="profile-select"
                      aria-pressed={profile.id === selectedId}
                      onClick={() => setSelectedId(profile.id)}
                    >
                      {profile.name}
                    </button>
                    <span className="profile-meta">
                      <span className="num">{t.profiles.modCount(count)}</span>
                      {profile.id === state.config.active_profile && (
                        <span className="badge badge-on">{t.profiles.active}</span>
                      )}
                    </span>
                  </li>
                );
              })}
            </ul>
          </div>

          <form className="panel" onSubmit={create}>
            <h3 className="rail-title">{t.profiles.createTitle}</h3>
            <p className="hint">{t.profiles.createHint}</p>
            <div className="form-field">
              <label htmlFor="new-profile">{t.profiles.nameLabel}</label>
              <input
                id="new-profile"
                className="input"
                value={newName}
                onChange={(event) => setNewName(event.target.value)}
              />
            </div>
            {locked && (
              <p id="profile-create-lock-hint" className="hint">
                {t.profiles.locked}
              </p>
            )}
            <button
              type="submit"
              className="btn"
              aria-disabled={locked || undefined}
              aria-describedby={locked ? 'profile-create-lock-hint' : undefined}
              title={locked ? t.profiles.locked : undefined}
            >
              <Icon name="plus" />
              {t.profiles.create}
            </button>
          </form>
        </div>

        <div className="split-main">
          {/* Remounts (resets the draft) whenever the stored profile changes. */}
          {selected === null ? (
            <div className="empty-state">
              <span className="empty-icon" aria-hidden="true">
                <Icon name="profiles" />
              </span>
              <p>{t.profiles.selectHint}</p>
            </div>
          ) : (
            <ProfileEditor key={`${selected.id}|${selected.name}|${selected.mod_ids.join(',')}`} profile={selected} />
          )}
        </div>
      </div>
    </section>
  );
}
