import type { MyMod, ModVersion, PublishProgress, PublishRequest } from '../../domain';
import type { Unsubscribe } from '../ports';

/**
 * Publishing mods to the community server (#130). The backend streams the file, holds the session
 * token (Windows Credential Manager) and resumes interrupted uploads; the UI never sees either.
 * Every method that talks to the server rejects with a `PublishError` carrying a typed code.
 */
export interface ModPublisher {
  /** Uploads the mod; only one publish runs at a time. Resolves with the stored version (`pending`). */
  publish(request: PublishRequest): Promise<ModVersion>;
  /** Asks the running publish to stop; it then rejects with `cancelled`. No-op when none runs. */
  cancel(): Promise<void>;
  /** The signed-in creator's mods with every version and its review status. */
  myMods(): Promise<MyMod[]>;
  /** Progress of the running publish. Returns right away; call the result to stop listening. */
  onProgress(callback: (progress: PublishProgress) => void): Unsubscribe;
}

/** Native dialogs for the publish form; both resolve `null` when the user cancels. */
export interface PublishFilePicker {
  /** One mod file (.zip, .7z or .pak): an absolute path. */
  pickModFile(): Promise<string | null>;
  /** Up to 8 images (.png, .jpg, .jpeg, .webp): absolute paths. */
  pickScreenshots(): Promise<string[] | null>;
}
