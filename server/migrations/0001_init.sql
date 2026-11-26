CREATE TABLE users (
    id           TEXT PRIMARY KEY NOT NULL,
    discord_id   TEXT NOT NULL UNIQUE,
    username     TEXT NOT NULL,
    global_name  TEXT,
    avatar_hash  TEXT,
    created_at   INTEGER NOT NULL,
    updated_at   INTEGER NOT NULL
);

CREATE TABLE sessions (
    id          TEXT PRIMARY KEY NOT NULL,
    user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash  TEXT NOT NULL UNIQUE,
    created_at  INTEGER NOT NULL,
    expires_at  INTEGER NOT NULL,
    revoked_at  INTEGER
);
CREATE INDEX sessions_user_id ON sessions(user_id);

CREATE TABLE oauth_flows (
    state            TEXT PRIMARY KEY NOT NULL,
    discord_verifier TEXT NOT NULL,
    redirect         TEXT NOT NULL,
    app_state        TEXT NOT NULL,
    app_challenge    TEXT NOT NULL,
    expires_at       INTEGER NOT NULL
);

CREATE TABLE one_time_codes (
    code_hash      TEXT PRIMARY KEY NOT NULL,
    user_id        TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    app_challenge  TEXT NOT NULL,
    expires_at     INTEGER NOT NULL
);
