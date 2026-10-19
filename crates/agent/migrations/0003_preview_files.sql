-- Only new preview admission opts into capture; historical receipts stay unavailable.
CREATE TABLE preview_catalogs (
    job_id TEXT PRIMARY KEY NOT NULL REFERENCES jobs(job_id)
);
CREATE TABLE preview_files (
    job_id TEXT NOT NULL REFERENCES jobs(job_id),
    id TEXT NOT NULL,
    path TEXT NOT NULL,
    search_path TEXT NOT NULL,
    size_bytes TEXT NOT NULL,
    created_at TEXT NOT NULL,
    orphan INTEGER NOT NULL CHECK (orphan IN (0, 1)),
    PRIMARY KEY (job_id, id)
);
CREATE INDEX preview_files_path ON preview_files(job_id, path, id);
CREATE INDEX preview_files_created ON preview_files(job_id, created_at, id);
