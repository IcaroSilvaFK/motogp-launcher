-- One star per (user, mod). Mod ids are catalog slugs the server does not validate against a catalog.
CREATE TABLE mod_stars (
    user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    mod_id      TEXT NOT NULL,
    created_at  INTEGER NOT NULL DEFAULT (unixepoch()),
    PRIMARY KEY (user_id, mod_id)
);
CREATE INDEX mod_stars_mod_id ON mod_stars(mod_id);
