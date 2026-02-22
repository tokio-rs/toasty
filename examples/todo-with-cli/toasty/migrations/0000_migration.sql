CREATE TABLE "users" (
    "id" TEXT NOT NULL,
    "name" TEXT NOT NULL,
    "email" TEXT NOT NULL,
    PRIMARY KEY ("id")
);
-- #[toasty::breakpoint]
CREATE UNIQUE INDEX "index_users_by_id" ON "users" ("id");
-- #[toasty::breakpoint]
CREATE UNIQUE INDEX "index_users_by_email" ON "users" ("email");
-- #[toasty::breakpoint]
CREATE TABLE "todos" (
    "id" TEXT NOT NULL,
    "user_id" TEXT NOT NULL,
    "title" TEXT NOT NULL,
    "completed" BOOLEAN NOT NULL,
    PRIMARY KEY ("id")
);
-- #[toasty::breakpoint]
CREATE UNIQUE INDEX "index_todos_by_id" ON "todos" ("id");
-- #[toasty::breakpoint]
CREATE INDEX "index_todos_by_user_id" ON "todos" ("user_id");
-- #[toasty::breakpoint]
CREATE INDEX "index_todos_by_title" ON "todos" ("title");