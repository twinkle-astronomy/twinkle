-- This file should undo anything in `up.sql`
-- down.sql
-- Drop settings table first to maintain foreign key integrity
DROP TABLE IF EXISTS settings;

-- Then drop telescope_configs table
DROP TABLE IF EXISTS telescope_configs;