CREATE TABLE coordination (id INTEGER PRIMARY KEY CHECK (id = 1), revision INTEGER NOT NULL);
INSERT INTO coordination (id, revision) VALUES (1, 0);
CREATE TABLE jobs (
    job_id TEXT PRIMARY KEY NOT NULL,
    fingerprint BLOB,
    phase TEXT NOT NULL CHECK (phase IN ('accepted','running','cancelling','restoring','completed','partial','failed','cancelled','interrupted')),
    snapshot TEXT NOT NULL
);
CREATE UNIQUE INDEX one_active_job ON jobs ((1))
    WHERE phase IN ('accepted','running','cancelling','restoring');
