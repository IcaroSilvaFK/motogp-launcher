CREATE TABLE comments (
    id          TEXT PRIMARY KEY NOT NULL,
    mod_id      TEXT NOT NULL,
    user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    body        TEXT NOT NULL,
    created_at  INTEGER NOT NULL,
    edited_at   INTEGER
);
-- Keyset pagination: oldest first within a mod.
CREATE INDEX comments_mod_created ON comments(mod_id, created_at, id);
