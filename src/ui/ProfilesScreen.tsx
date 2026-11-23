import { useState } from 'react';
import type { FormEvent } from 'react';
import type { Profile } from '../domain';
import { isVanilla, moveItem, toggleMod } from '../domain';
import { Icon } from './Icon';
import { useI18n } from './i18n';
import { useLauncher } from './LauncherContext';

function ProfileEditor({ profile }: { profile: Profile }) {
  const { t } = useI18n();
  const { controller, state } = useLauncher();
  const [name, setName] = useState(profile.name);
  const [draft, setDraft] = useState<string[]>(profile.mod_ids);
  const modsById = new Map(state.mods.map((mod) => [mod.id, mod]));
  const ordered = draft.filter((id) => modsById.has(id));

  const rename = (event: FormEvent) => {
    event.preventDefault();
    void controller.renameProfile(profile.id, name);
  };

  return (
    <section aria-label={profile.name} className="card editor">
      <div className="card-head">
        <h3>{profile.name}</h3>
        {!isVanilla(profile) && (
          <button
            type="button"
            className="btn btn-danger btn-sm"
            aria-label={t.profiles.deleteNamed(profile.name)}
            onClick={() => void controller.deleteProfile(profile.id)}
          >
            {t.profiles.delete}
          </button>
        )}
      </div>

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
          <button type="submit" className="btn">
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
                <h4 className="section-title">{t.profiles.orderTitle}</h4>
                <ol className="order-list">
                  {ordered.map((id, index) => {
                    const modName = modsById.get(id)?.name ?? id;
                    return (
                      <li key={id}>
                        <span className="order-name">{modName}</span>
                        <button
                          type="button"
                          className="btn btn-icon btn-sm"
                          disabled={index === 0}
                          aria-label={t.profiles.moveUp(modName)}
                          onClick={() => setDraft(moveItem(ordered, index, -1))}
                        >
                          <Icon name="up" />
                        </button>
                        <button
                          type="button"
                          className="btn btn-icon btn-sm"
                          disabled={index === ordered.length - 1}
                          aria-label={t.profiles.moveDown(modName)}
                          onClick={() => setDraft(moveItem(ordered, index, 1))}
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

          <div className="card-footer">
            <button
              type="button"
              className="btn btn-accent-outline"
              onClick={() => void controller.setProfileMods(profile.id, ordered)}
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

  const create = (event: FormEvent) => {
    event.preventDefault();
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
              {state.profiles.map((profile) => {
                const count = profile.mod_ids.filter((id) => modIds.has(id)).length;
                return (
                  <li key={profile.id} className="profile-item">
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
            <button type="submit" className="btn">
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
