import { useEffect } from 'react';
import { formatSize, useI18n } from './i18n';
import { useLauncher } from './LauncherContext';

export function LibraryScreen() {
  const { t, locale } = useI18n();
  const { controller, state } = useLauncher();

  // Drop listener: released exactly once, even if the subscription resolves after unmount.
  useEffect(() => {
    let released = false;
    let release: (() => void) | null = null;
    void controller.subscribeDrop().then((unsubscribe) => {
      if (released) unsubscribe();
      else release = unsubscribe;
    });
    return () => {
      released = true;
      release?.();
      release = null;
    };
  }, [controller]);

  return (
    <section aria-labelledby="library-title">
      <h2 id="library-title">{t.library.title}</h2>
      <button type="button" onClick={() => void controller.importFromDialog()}>
        {t.library.import}
      </button>
      <p className="hint">{t.library.dropHint}</p>
      {state.mods.length === 0 ? (
        <p>{t.library.empty}</p>
      ) : (
        <table>
          <thead>
            <tr>
              <th scope="col">{t.library.title}</th>
              <th scope="col">{t.library.size}</th>
              <th scope="col">{t.library.fileCount}</th>
              <th scope="col">
                <span className="visually-hidden">{t.library.remove}</span>
              </th>
            </tr>
          </thead>
          <tbody>
            {state.mods.map((mod) => (
              <tr key={mod.id}>
                <th scope="row">{mod.name}</th>
                <td>{formatSize(mod.size_bytes, locale)}</td>
                <td>{t.library.files(mod.files.length)}</td>
                <td>
                  <button type="button" onClick={() => void controller.removeMod(mod.id)}>
                    {t.library.removeNamed(mod.name)}
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}
