import { describe, expect, it, vi } from 'vitest';
import { createTauriPicker } from '../tauriPicker';

describe('Given the Tauri picker', () => {
  it('When picking mods Then dialog_filters_supported_mod_extensions', async () => {
    const open = vi.fn().mockResolvedValue(['a.pak', 'b.zip']);
    const result = await createTauriPicker(open).pickModFiles();
    expect(result).toEqual(['a.pak', 'b.zip']);
    expect(open).toHaveBeenCalledWith(
      expect.objectContaining({
        multiple: true,
        directory: false,
        filters: [{ name: 'Mods', extensions: ['pak', 'utoc', 'ucas', 'zip'] }],
      }),
    );
  });

  it('When picking the game directory Then it asks for a single directory', async () => {
    const open = vi.fn().mockResolvedValue('C:/Game');
    expect(await createTauriPicker(open).pickGameDirectory()).toBe('C:/Game');
    expect(open).toHaveBeenCalledWith(expect.objectContaining({ directory: true, multiple: false }));
  });

  it('When the dialog is cancelled Then cancelled_dialog_returns_null', async () => {
    const open = vi.fn().mockResolvedValue(null);
    const picker = createTauriPicker(open);
    expect(await picker.pickModFiles()).toBeNull();
    expect(await picker.pickGameDirectory()).toBeNull();
  });

  it('When the dialog answers with one string Then single_selection_is_wrapped', async () => {
    const open = vi.fn().mockResolvedValue('a.pak');
    expect(await createTauriPicker(open).pickModFiles()).toEqual(['a.pak']);
  });
});
