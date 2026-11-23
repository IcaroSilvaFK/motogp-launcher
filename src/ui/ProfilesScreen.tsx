import { useState } from 'react';
import type { FormEvent } from 'react';
import type { Profile } from '../domain';
import { isVanilla, moveItem, toggleMod } from '../domain';
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
    <section aria-label={profile.name}>
      <h3>{profile.name}</h3>

      {isVanilla(profile) ? (
        <p>{t.profiles.vanillaLocked}</p>
      ) : (
        <>
          <form onSubmit={rename}>
            <label htmlFor="rename-profile">{t.profiles.renameLabel}</label>
            <input id="rename-profile" value={name} onChange={(event) => setName(event.target.value)} />
            <button type="submit">{t.profiles.rename}</button>
          </form>
          <button type="button" onClick={() => void controller.deleteProfile(profile.id)}>
            {t.profiles.deleteNamed(profile.name)}
          </button>
        </>
      )}

      {!isVanilla(profile) && (
        <>
          <h4>{t.profiles.mods}</h4>
          {state.mods.length === 0 ? (
            <p>{t.profiles.noMods}</p>
          ) : (
            <ul className="plain">
              {state.mods.map((mod) => (
                <li key={mod.id}>
                  <label>
                    <input
                      type="checkbox"
                      checked={draft.includes(mod.id)}
                      onChange={() => setDraft((current) => toggleMod(current, mod.id))}
                    />{' '}
                    {mod.name}
                  </label>
                </li>
              ))}
            </ul>
          )}

          {ordered.length > 0 && (
            <>
              <h4>{t.profiles.orderTitle}</h4>
              <ol>
                {ordered.map((id, index) => {
                  const modName = modsById.get(id)?.name ?? id;
                  return (
                    <li key={id}>
                      <span>{modName}</span>{' '}
                      <button
                        type="button"
                        disabled={index === 0}
                        aria-label={t.profiles.moveUp(modName)}
                        onClick={() => setDraft(moveItem(ordered, index, -1))}
                      >
                        ↑
                      </button>
                      <button
                        type="button"
                        disabled={index === ordered.length - 1}
                        aria-label={t.profiles.moveDown(modName)}
                        onClick={() => setDraft(moveItem(ordered, index, 1))}
                      >
                        ↓
                      </button>
                    </li>
                  );
                })}
              </ol>
            </>
          )}

          <button type="button" onClick={() => void controller.setProfileMods(profile.id, ordered)}>
            {t.profiles.save}
          </button>
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

  const create = (event: FormEvent) => {
    event.preventDefault();
    void controller.createProfile(newName);
    setNewName('');
  };

  return (
    <section aria-labelledby="profiles-title">
      <h2 id="profiles-title">{t.profiles.title}</h2>

      <form onSubmit={create}>
        <label htmlFor="new-profile">{t.profiles.nameLabel}</label>
        <input id="new-profile" value={newName} onChange={(event) => setNewName(event.target.value)} />
        <button type="submit">{t.profiles.create}</button>
      </form>

      <ul className="plain">
        {state.profiles.map((profile) => (
          <li key={profile.id}>
            <button type="button" aria-pressed={profile.id === selectedId} onClick={() => setSelectedId(profile.id)}>
              {profile.name}
            </button>
          </li>
        ))}
      </ul>

      {/* Remounts (resets the draft) whenever the stored profile changes. */}
      {selected !== null && (
        <ProfileEditor key={`${selected.id}|${selected.name}|${selected.mod_ids.join(',')}`} profile={selected} />
      )}
    </section>
  );
}
