# Core API Mirror (Server -> Mint)

This directory is a **copy-first migration mirror** for moving server core API code toward `mint`.

Current status:
- Files are copied from `server/src/api/*` and `server/src/server/*`.
- The mirror is **not wired into compilation/runtime** yet.
- Purpose is to keep a safe snapshot for incremental migration and diff review.

Next migration step (recommended):
1. Convert one endpoint group at a time from axum handler style to `ApiService` style.
2. Register migrated routes from `mint::api` behind a temporary switch.
3. Remove duplicated endpoint implementation from `server` after parity checks.

Scope copied now:
- Core routes: block/transaction/create/submit/fee/scan transfer.
- Core helpers: action json codec, query macros, rendering and block loader helpers.
