-- Download counts: one row per (mod, hashed install id, UTC day).
-- `install_hash` is SHA-256 (hex) of the install id; the raw id is never stored.
-- `day` is the number of days since 1970-01-01 UTC.
CREATE TABLE mod_downloads (
    mod_id       TEXT    NOT NULL,
    install_hash TEXT    NOT NULL,
    day          INTEGER NOT NULL,
    PRIMARY KEY (mod_id, install_hash, day)
) WITHOUT ROWID;

CREATE INDEX idx_mod_downloads_mod_day ON mod_downloads (mod_id, day);
