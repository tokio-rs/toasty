CREATE TABLE "svc_api_keys" (
    "id" BLOB NOT NULL,
    "tenant_id" BLOB NOT NULL,
    "token" TEXT NOT NULL,
    PRIMARY KEY ("id")
);
-- #[toasty::breakpoint]
CREATE INDEX "index_svc_api_keys_by_tenant_id" ON "svc_api_keys" ("tenant_id");
-- #[toasty::breakpoint]
CREATE UNIQUE INDEX "index_svc_api_keys_by_token" ON "svc_api_keys" ("token");
-- #[toasty::breakpoint]
CREATE TABLE "svc_tenants" (
    "id" BLOB NOT NULL,
    "slug" TEXT NOT NULL,
    "name" TEXT NOT NULL,
    PRIMARY KEY ("id")
);
-- #[toasty::breakpoint]
CREATE UNIQUE INDEX "index_svc_tenants_by_slug" ON "svc_tenants" ("slug");
