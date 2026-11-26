-- Moderation: hidden comments, bans, comment reports, audit log, admin OAuth flows.

ALTER TABLE comments ADD COLUMN hidden_at INTEGER;
ALTER TABLE comments ADD COLUMN hidden_by TEXT REFERENCES users(id) ON DELETE SET NULL;
ALTER TABLE comments ADD COLUMN hidden_reason TEXT;

-- One row per banned user. Unbanning deletes the row (history lives in the audit log).
CREATE TABLE user_bans (
    user_id    TEXT PRIMARY KEY NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    banned_by  TEXT REFERENCES users(id) ON DELETE SET NULL,
    reason     TEXT NOT NULL,
    banned_at  INTEGER NOT NULL
);

CREATE TABLE comment_reports (
    id           TEXT PRIMARY KEY NOT NULL,
    comment_id   TEXT NOT NULL REFERENCES comments(id) ON DELETE CASCADE,
    reporter_id  TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    reason       TEXT NOT NULL,
    details      TEXT,
    created_at   INTEGER NOT NULL,
    resolved_at  INTEGER,
    resolved_by  TEXT,
    resolution   TEXT, -- hidden | dismissed
    UNIQUE (comment_id, reporter_id)
);
CREATE INDEX comment_reports_open ON comment_reports(created_at) WHERE resolved_at IS NULL;

CREATE TABLE moderation_actions (
    id          TEXT PRIMARY KEY NOT NULL,
    admin_id    TEXT REFERENCES users(id) ON DELETE SET NULL,
    action      TEXT NOT NULL, -- hide_comment | restore_comment | ban_user | unban_user | dismiss_report | approve_version | reject_version
    target      TEXT NOT NULL, -- comment id, user id or mod@version
    reason      TEXT,
    created_at  INTEGER NOT NULL
);
CREATE INDEX moderation_actions_created ON moderation_actions(created_at);

-- app = the launcher's sign-in; admin = the /admin web login.
ALTER TABLE oauth_flows ADD COLUMN purpose TEXT NOT NULL DEFAULT 'app';
