---
paths:
  - "src/db/**/*"
  - "src/scanner/**/*"
---

- rusqlite only, no ORM. Every value goes through bound parameters (`?1`, `params!`) — never `format!` user data into SQL. (`format!("%{query}%")` for a LIKE pattern is fine only when the pattern itself is bound.)
- All queries must handle null/empty results explicitly (`Option`, `query_row` -> `QueryReturnedNoRows`)
- Schema lives in `run_migrations`; changes are additive only (`ALTER TABLE ... ADD COLUMN`, `CREATE TABLE IF NOT EXISTS`) — never drop or rename columns in one step
- No N+1 queries — use joins or batch statements
- Index foreign keys
- Tests use `tempfile` or an in-memory DB, never the user's real library
