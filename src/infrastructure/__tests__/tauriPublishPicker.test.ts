import { describe, expect, it, vi } from 'vitest';
import { createTauriPublishPicker } from '../tauriPublishPicker';

describe('Given the publish picker', () => {
  it('When picking the mod file Then it asks for one .zip/.7z/.pak', async () => {
    const open = vi.fn().mockResolvedValue('C:\\m\\a.zip');
    expect(await createTauriPublishPicker(open).pickModFile()).toBe('C:\\m\\a.zip');
    expect(open).toHaveBeenCalledWith({
      multiple: false,
      directory: false,
      filters: [{ name: 'Mods', extensions: ['zip', '7z', 'pak'] }],
    });
  });

  it('When the mod file dialog is cancelled Then it is null', async () => {
    expect(await createTauriPublishPicker(vi.fn().mockResolvedValue(null)).pickModFile()).toBeNull();
  });

  it('When picking screenshots Then it asks for images and keeps at most 8', async () => {
    const many = Array.from({ length: 10 }, (_, i) => `${i}.png`);
    const open = vi.fn().mockResolvedValue(many);
    const picked = await createTauriPublishPicker(open).pickScreenshots();
    expect(picked).toEqual(many.slice(0, 8));
    expect(open).toHaveBeenCalledWith({
      multiple: true,
      directory: false,
      filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp'] }],
    });
  });

  it('When screenshots are cancelled or a single string comes back Then null or a one-item list', async () => {
    expect(await createTauriPublishPicker(vi.fn().mockResolvedValue(null)).pickScreenshots()).toBeNull();
    expect(await createTauriPublishPicker(vi.fn().mockResolvedValue('a.png')).pickScreenshots()).toEqual(['a.png']);
  });
});
