ALTER TABLE coordination ADD COLUMN maintenance INTEGER NOT NULL DEFAULT 0 CHECK (maintenance IN (0, 1));
