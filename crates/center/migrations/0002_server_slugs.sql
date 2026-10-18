ALTER TABLE servers ADD COLUMN slug TEXT;
CREATE UNIQUE INDEX servers_slug ON servers(slug);
PRAGMA user_version=2;
