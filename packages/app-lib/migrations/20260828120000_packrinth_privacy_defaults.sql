-- Packrinth privacy defaults: telemetry off by default; Discord RPC and
-- personalized ads are removed entirely (goal.md §1.1 privacy-first fork).
UPDATE settings SET telemetry = 0;
ALTER TABLE settings DROP COLUMN discord_rpc;
ALTER TABLE settings DROP COLUMN personalized_ads;
