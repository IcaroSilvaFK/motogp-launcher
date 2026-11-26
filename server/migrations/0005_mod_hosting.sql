-- Mod hosting: mods, every uploaded version (history), resumable upload sessions and screenshots.

CREATE TABLE mods (
    id             TEXT PRIMARY KEY NOT NULL,
    owner_user_id  TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at     INTEGER NOT NULL
);
CREATE INDEX mods_owner ON mods(owner_user_id);

CREATE TABLE mod_versions (
    id                   TEXT PRIMARY KEY NOT NULL,
    mod_id               TEXT NOT NULL REFERENCES mods(id) ON DELETE CASCADE,
    version              TEXT NOT NULL,
    title                TEXT NOT NULL,
    description          TEXT NOT NULL,
    long_description     TEXT,
    category             TEXT NOT NULL,
    game                 TEXT NOT NULL,
    file_name            TEXT NOT NULL,
    size                 INTEGER NOT NULL,
    sha256               TEXT NOT NULL,
    storage_key          TEXT NOT NULL,
    status               TEXT NOT NULL CHECK (status IN ('pending', 'approved', 'rejected')),
    rejection_reason     TEXT,
    ownership_confirmed  INTEGER NOT NULL DEFAULT 1,
    uploaded_at          INTEGER NOT NULL,
    reviewed_at          INTEGER,
    reviewed_by          TEXT REFERENCES users(id) ON DELETE SET NULL,
    UNIQUE (mod_id, version)
);
CREATE INDEX mod_versions_status ON mod_versions(status);

CREATE TABLE mod_uploads (
    id                TEXT PRIMARY KEY NOT NULL,
    user_id           TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    mod_id            TEXT NOT NULL,
    version           TEXT NOT NULL,
    title             TEXT NOT NULL,
    description       TEXT NOT NULL,
    long_description  TEXT,
    category          TEXT NOT NULL,
    game              TEXT NOT NULL,
    file_name         TEXT NOT NULL,
    size              INTEGER NOT NULL,
    client_sha256     TEXT,
    received          INTEGER NOT NULL DEFAULT 0,
    created_at        INTEGER NOT NULL,
    expires_at        INTEGER NOT NULL
);
CREATE INDEX mod_uploads_user ON mod_uploads(user_id);
CREATE INDEX mod_uploads_expires ON mod_uploads(expires_at);

-- Every started upload, kept after the session is gone (per-day limit).
CREATE TABLE mod_upload_starts (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    started_at  INTEGER NOT NULL
);
CREATE INDEX mod_upload_starts_user ON mod_upload_starts(user_id, started_at);

-- A screenshot belongs to an upload session first, then to the version created from it.
CREATE TABLE mod_screenshots (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    upload_id     TEXT REFERENCES mod_uploads(id) ON DELETE CASCADE,
    version_id    TEXT REFERENCES mod_versions(id) ON DELETE CASCADE,
    idx           INTEGER NOT NULL CHECK (idx >= 0 AND idx < 8),
    storage_key   TEXT NOT NULL,
    content_type  TEXT NOT NULL,
    UNIQUE (upload_id, idx),
    UNIQUE (version_id, idx)
);
