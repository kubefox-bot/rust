# Live preview file details implementation plan

> **For agentic workers:** Use superpowers:subagent-driven-development. Existing feature branch and current folder are explicitly authorized. Do not overwrite the preceding uncommitted footer/calendar changes.

**Goal:** Add a Details button available while a preview runs, opening a reusable Svelte file grid with server-side search, sorting and paging, refreshed through existing SSE.

**Architecture:** Selected physical-file metadata is persisted on the agent in its existing SQLite ledger. The center validates session/job/agent ownership and proxies bounded pages. Existing job SSE revisions invalidate the visible page; file rows are never placed in coalescing counters or unbounded event buffers. Existing cleanup confirmation and preview eligibility stay authoritative.

**Tech Stack:** Rust, SQLx/SQLite, Axum, existing mTLS and SSE; static Astro, Svelte 5, stable TanStack Table Svelte adapter, Bun.

## Approved behavior and contracts

- Details is available after a preview is accepted, including while scanning; it opens only on explicit click.
- Grid columns: file/path, byte size, creation time, linked/orphan indicator. File contents are never read for this feature.
- Source is physical files selected by the current preview, including selected orphan files; absent files are not represented as physical files.
- Search is literal case-insensitive path substring. Sort: path, size, created_at; direction asc/desc; ties use stable row ID. Offset is bounded to signed SQLite range; limit defaults to 50, max 100; query length max 512 bytes; stored path max 4096 UTF-8 bytes; dedicated response bound 1 MiB.
- Add PreviewFile {id,path,size_bytes:Counter,created_at:UTC timestamp,orphan:bool}, PreviewFilesQuery {search,sort,direction,offset:u64,limit:u16}, PreviewFilesPage {job_id,rows,total:Counter,revision:Counter,complete:bool} to shared protocol and browser schema registry.
- Agent GET /agent/v1/jobs/{job_id}/files; center GET /api/v1/jobs/{job_id}/files. Both validate query; center checks preview operation, immutable job ownership and response target/limits. Missing support yields an explicit error, not an empty table.
- Agent uses an awaited batch observer at the real selection point, giving database backpressure without dropping file records. Each persisted batch emits an ordinary job snapshot revision; no SSE envelope change is necessary. Existing run entrypoint and CLI output contracts remain compatible.
- Details table keeps only current page. Abort obsolete searches, page loads and closed dialog requests. Coalesce SSE refresh; retry/reload current page on resync, preserve query/sort state; no second SSE connection. Old job/server responses must never populate a new dialog.
- Metadata stays on agent, scoped to job and existing local DB permissions; center does not duplicate file paths in its database. Do not alter retention or delete historical jobs without authorization.
- Existing APIs and ordinary cleanup behavior remain unchanged. Schema migration on the agent must be additive. No CI/CD, push or real mail cleanup.

## Tasks

- [x] A. Root: shared DTOs, validation and generated browser schema; center mTLS client and authenticated job-file proxy; transport/ownership tests.
- [x] B. Agent worker: awaited selected-file observation in core/runner, SQLite metadata batches and deterministic queries, agent endpoint, job SSE notification; meaningful bounded paging/search/sort and actual selection tests.
- [x] C. Web worker: TanStack dependency verified against Svelte 5, reusable lazy details dialog/grid and preview button during running job; server query/SSE invalidation/cancellation tests.
- [x] D. Integrate regenerated contracts, extend synthetic mock agent so Browser can exercise incremental rows, search, sort, paging and reconnect. Run relevant Rust checks, web checks/build, contract reproducibility and manual Browser acceptance.
- [x] E. Independent review of changed boundaries, fix introduced issues, report precise verification and remaining runtime gaps. Do not commit unless requested.

## Ledger

- Follow-up approved: created-from/to (inclusive, browser wall time converted to UTC without dropping fractional seconds), relation All/Linked/Orphan, and reset including path search. Existing date picker is reused with a browser-time caption; its server-time default remains intact. Agent SQL applies every predicate before counting/paging; center forwards and validates filtered rows.
- Filter verification: six agent file-catalog tests and the center mTLS proxy test passed; 53 web tests passed. Browser on https://localhost:54333/?server=mail-one verified 12 total, 3 orphan, 2 after lower date/time bound, 1 after upper bound; reset restored 12, Linked returned 9, and final SSE preserved the filter while marking collection complete. Screenshot: target/local-mvp/browser/44-preview-filters.jpg. Earlier mock centers were preserved.

- Start: user approved TanStack + server pagination/search/sort + SSE; click-to-open Details during preview.
- Ruling: extend the existing preview/job flow rather than introduce a new dashboard or stream; ordinary snapshot revisions invalidate file pages, avoiding an unbounded SSE file feed.
- Ruling: selected physical files only; database-only orphan rows/missing files remain aggregate counters because this is a file grid.
- Baseline: HEAD 301ec56 with authorized uncommitted footer/calendar/status work; preserve it.

- Verification: center/protocol full test suites passed; core/runner/agent suites passed (one existing agent test ignored); web check passed with 51 tests and no Astro/Svelte errors or warnings; web build, contract reproducibility, Rust formatting, Clippy and whitespace checks passed.
- Independent review found and verified fixes for in-flight SSE request starvation and historical uncaptured previews. Historical jobs return explicit HTTP 410; new captured zero-result jobs remain a valid empty result.
- Browser acceptance on isolated trusted HTTPS mock center https://localhost:52979/?server=mail-one: Details available while running; explicit click opens collecting empty list; existing SSE updates it to 12 rows; literal search returns one row; size asc/desc, 10+2 pagination, completion and Escape/reopen passed. Screenshot: target/local-mvp/browser/41-preview-file-details.jpg.
- Browser automation suite is added but was not executed headlessly; acceptance used the Browser UI. Real Windows Server 2016/MySQL/mailbox runtime remains unverified. No actual cleanup, commit, push or deployment performed.
