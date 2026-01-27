ALTER TABLE "todos" DROP COLUMN "title";
-- #[toasty::breakpoint]
ALTER TABLE "todos" ADD COLUMN "title2" TEXT NOT NULL;
-- #[toasty::breakpoint]
DROP INDEX "index_todos_by_title";
-- #[toasty::breakpoint]
CREATE INDEX "index_todos_by_title2" ON "todos" ("title2");