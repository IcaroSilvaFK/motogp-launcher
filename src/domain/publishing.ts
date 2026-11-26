// Publishing a mod to the community server (#130). The backend streams the file, holds the session
// token and talks to the server; the UI only sees these shapes. The form rules mirror the server's.

export const PUBLISH_FILE_EXTENSIONS = ['zip', '7z', 'pak'] as const;
export const SCREENSHOT_EXTENSIONS = ['png', 'jpg', 'jpeg', 'webp'] as const;
export const MAX_SCREENSHOTS = 8;
export const MAX_MOD_ID_LENGTH = 64;
export const MAX_TITLE_LENGTH = 120;
export const MAX_DESCRIPTION_LENGTH = 1000;
export const MAX_LONG_DESCRIPTION_LENGTH = 20000;
export const MAX_VERSION_LENGTH = 32;
/** Mod ids that collide with server routes. */
export const RESERVED_MOD_IDS = ['mine', 'uploads', 'stats'] as const;

export type VersionStatus = 'pending' | 'approved' | 'rejected';
export type PublishPhase = 'hashing' | 'uploading' | 'finishing' | 'done';

export interface PublishProgress {
  sent: number;
  total: number;
  phase: PublishPhase;
}

/** What the UI edits. `file` is an absolute path (or null before one is picked). */
export interface PublishForm {
  modId: string;
  title: string;
  description: string;
  longDescription: string;
  category: string;
  game: string;
  version: string;
  file: string | null;
  screenshots: string[];
  ownershipConfirmed: boolean;
}

/** What `community_publish_mod` takes. */
export interface PublishRequest {
  modId: string;
  title: string;
  description: string;
  longDescription?: string;
  category: string;
  game: string;
  version: string;
  file: string;
  screenshots: string[];
  ownershipConfirmed: boolean;
}

export interface ModVersion {
  modId: string;
  version: string;
  title: string;
  description: string;
  longDescription?: string;
  category: string;
  game: string;
  fileName: string;
  size: number;
  sha256: string;
  status: VersionStatus;
  rejectionReason?: string;
  screenshotCount: number;
  uploadedAt: string;
  reviewedAt?: string;
}

export interface MyMod {
  id: string;
  versions: ModVersion[];
}

export type PublishField = keyof PublishForm;

/** Stable keys for i18n (`publish.validation.<key>`). */
export const PUBLISH_FORM_ERROR_KEYS = [
  'modIdRequired',
  'modIdInvalid',
  'modIdTooLong',
  'modIdReserved',
  'titleRequired',
  'titleTooLong',
  'descriptionRequired',
  'descriptionTooLong',
  'longDescriptionTooLong',
  'categoryRequired',
  'gameRequired',
  'versionRequired',
  'versionInvalid',
  'fileRequired',
  'fileUnsupported',
  'screenshotsTooMany',
  'screenshotUnsupported',
  'ownershipRequired',
] as const;

export type PublishFormErrorKey = (typeof PUBLISH_FORM_ERROR_KEYS)[number];

export type PublishFormErrors = Partial<Record<PublishField, PublishFormErrorKey>>;

const SLUG = /^[a-z0-9]+(-[a-z0-9]+)*$/;
const VERSION = /^[A-Za-z0-9._+-]+$/;

function extensionOf(path: string): string {
  const name = path.split(/[\\/]/).pop() ?? '';
  const dot = name.lastIndexOf('.');
  return dot < 0 ? '' : name.slice(dot + 1).toLowerCase();
}

function isReserved(slug: string): boolean {
  return (RESERVED_MOD_IDS as readonly string[]).includes(slug);
}

/** Validates the form; the result holds at most one key per field and is empty when valid. */
export function validatePublishForm(form: PublishForm): PublishFormErrors {
  const errors: PublishFormErrors = {};
  const modId = form.modId.trim();
  if (modId === '') errors.modId = 'modIdRequired';
  else if (modId.length > MAX_MOD_ID_LENGTH) errors.modId = 'modIdTooLong';
  else if (!SLUG.test(modId)) errors.modId = 'modIdInvalid';
  else if (isReserved(modId)) errors.modId = 'modIdReserved';

  const title = form.title.trim();
  if (title === '') errors.title = 'titleRequired';
  else if (title.length > MAX_TITLE_LENGTH) errors.title = 'titleTooLong';

  const description = form.description.trim();
  if (description === '') errors.description = 'descriptionRequired';
  else if (description.length > MAX_DESCRIPTION_LENGTH) errors.description = 'descriptionTooLong';

  if (form.longDescription.trim().length > MAX_LONG_DESCRIPTION_LENGTH) {
    errors.longDescription = 'longDescriptionTooLong';
  }
  if (form.category.trim() === '') errors.category = 'categoryRequired';
  if (form.game.trim() === '') errors.game = 'gameRequired';

  const version = form.version.trim();
  if (version === '') errors.version = 'versionRequired';
  else if (version.length > MAX_VERSION_LENGTH || !VERSION.test(version)) errors.version = 'versionInvalid';

  if (form.file === null || form.file.trim() === '') errors.file = 'fileRequired';
  else if (!(PUBLISH_FILE_EXTENSIONS as readonly string[]).includes(extensionOf(form.file))) {
    errors.file = 'fileUnsupported';
  }

  if (form.screenshots.length > MAX_SCREENSHOTS) errors.screenshots = 'screenshotsTooMany';
  else if (form.screenshots.some((p) => !(SCREENSHOT_EXTENSIONS as readonly string[]).includes(extensionOf(p)))) {
    errors.screenshots = 'screenshotUnsupported';
  }

  if (!form.ownershipConfirmed) errors.ownershipConfirmed = 'ownershipRequired';
  return errors;
}

/** Suggests a mod id from a title ("Pista Épica 2025" -> "pista-epica-2025"); empty when nothing usable remains. */
export function suggestSlug(title: string): string {
  const slug = title
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, MAX_MOD_ID_LENGTH)
    .replace(/-+$/g, '');
  return isReserved(slug) ? '' : slug;
}

/** Trims the form into the IPC request. Call only for a form that passed `validatePublishForm`. */
export function toPublishRequest(form: PublishForm): PublishRequest {
  const request: PublishRequest = {
    modId: form.modId.trim(),
    title: form.title.trim(),
    description: form.description.trim(),
    category: form.category.trim(),
    game: form.game.trim(),
    version: form.version.trim(),
    file: form.file ?? '',
    screenshots: [...form.screenshots],
    ownershipConfirmed: form.ownershipConfirmed,
  };
  const long = form.longDescription.trim();
  if (long !== '') request.longDescription = long;
  return request;
}

export const PUBLISH_ERROR_CODES = [
  'not_signed_in',
  'session_expired',
  'offline',
  'file_too_large',
  'quota_exceeded',
  'rate_limited',
  'version_exists',
  'mod_id_taken',
  'unsupported_file',
  'checksum_mismatch',
  'cancelled',
  'invalid_request',
  'server_error',
  'forbidden',
] as const;

export type PublishErrorCode = (typeof PUBLISH_ERROR_CODES)[number];

/** Rejection of every `ModPublisher` call. `message` is the backend's English text (for logs, not the UI). */
export class PublishError extends Error {
  readonly code: PublishErrorCode;

  constructor(code: PublishErrorCode, message = `publish failed: ${code}`) {
    super(message);
    this.name = 'PublishError';
    this.code = code;
  }
}

/** Maps a rejected IPC error (`{ code, message }`) to a typed error; anything unknown is `server_error`. */
export function toPublishError(error: unknown): PublishError {
  if (error instanceof PublishError) return error;
  if (typeof error !== 'object' || error === null) return new PublishError('server_error');
  const { code, message } = error as { code?: unknown; message?: unknown };
  const known = (PUBLISH_ERROR_CODES as readonly unknown[]).includes(code);
  return new PublishError(
    known ? (code as PublishErrorCode) : 'server_error',
    typeof message === 'string' ? message : undefined,
  );
}

const STATUSES: readonly unknown[] = ['pending', 'approved', 'rejected'];
const VERSION_STRINGS = [
  'modId',
  'version',
  'title',
  'description',
  'category',
  'game',
  'fileName',
  'sha256',
  'uploadedAt',
] as const;

/** Validates one untrusted version; `null` when it is malformed. */
export function parseModVersion(value: unknown): ModVersion | null {
  if (typeof value !== 'object' || value === null) return null;
  const v = value as Record<string, unknown>;
  if (VERSION_STRINGS.some((k) => typeof v[k] !== 'string')) return null;
  if (typeof v.size !== 'number' || !Number.isFinite(v.size)) return null;
  if (typeof v.screenshotCount !== 'number' || !Number.isFinite(v.screenshotCount)) return null;
  if (!STATUSES.includes(v.status)) return null;
  const version: ModVersion = {
    modId: v.modId as string,
    version: v.version as string,
    title: v.title as string,
    description: v.description as string,
    category: v.category as string,
    game: v.game as string,
    fileName: v.fileName as string,
    size: v.size,
    sha256: v.sha256 as string,
    status: v.status as VersionStatus,
    screenshotCount: v.screenshotCount,
    uploadedAt: v.uploadedAt as string,
  };
  if (typeof v.longDescription === 'string') version.longDescription = v.longDescription;
  if (typeof v.rejectionReason === 'string') version.rejectionReason = v.rejectionReason;
  if (typeof v.reviewedAt === 'string') version.reviewedAt = v.reviewedAt;
  return version;
}

/** Validates the `community_my_mods` answer; malformed entries are dropped, a non-array is `null`. */
export function parseMyMods(value: unknown): MyMod[] | null {
  if (!Array.isArray(value)) return null;
  const mods: MyMod[] = [];
  for (const item of value) {
    if (typeof item !== 'object' || item === null) continue;
    const { id, versions } = item as { id?: unknown; versions?: unknown };
    if (typeof id !== 'string' || id === '' || !Array.isArray(versions)) continue;
    mods.push({ id, versions: versions.map(parseModVersion).filter((x): x is ModVersion => x !== null) });
  }
  return mods;
}

const PHASES: readonly unknown[] = ['hashing', 'uploading', 'finishing', 'done'];

/** Validates a progress event payload; `null` when malformed. */
export function parsePublishProgress(value: unknown): PublishProgress | null {
  if (typeof value !== 'object' || value === null) return null;
  const v = value as Record<string, unknown>;
  if (typeof v.sent !== 'number' || typeof v.total !== 'number' || !PHASES.includes(v.phase)) return null;
  return { sent: v.sent, total: v.total, phase: v.phase as PublishPhase };
}
