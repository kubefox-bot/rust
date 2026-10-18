# Решения при реализации MVP

Согласованная архитектура: [спецификация](superpowers/specs/2026-10-17-hmail-control-center-design.md). Ниже сохранены все записи `Ruling:` из журнала реализации в исходном порядке, включая причину и цену пересмотра, где она записана. Английские формулировки сохранены дословно для проверки технических решений. Состояние выполнения и ограничения среды приведены в [приёмке](mvp-acceptance.md).

1. Ruling: Work in the existing checkout on ci/hmail-control-center-mvp — user explicitly selected it — isolation is branch-level, so unrelated user changes must remain untouched.

2. Ruling: The skill task-start extractor only recognizes 'Task N', whereas this approved split plan uses A1/B1/C1 headings — retain the exact source task as the brief and record BASE manually in this ledger — bookkeeping differs, implementation order and task contracts do not.

3. Task A2: Ruling: JobSnapshot.job is absent only for an unknown-ID Cancelled tombstone — cancellation cannot invent profile/agent metadata before a command arrives — cost if wrong: browser and response mapping must retain the nullable tombstone case.

4. Task A2: Ruling: Browser-specific DTO files are added by their B owners instead of creating empty auth/profile/enrollment modules in A2 — no placeholder interfaces are published — cost if wrong: move a file when its owning task begins.

5. Task A2: Ruling: Counter schemas identify uint64-decimal for the later standalone Ajv format validator — string shape alone cannot enforce u64's upper bound — cost if wrong: generated validation would accept an out-of-range count; C1 carries the required BigInt validation.

6. Task A1: Ruling: CLI-specific Observer adapter carries the existing env-file display context — runner remains independent of CLI configuration and preserves existing console/NDJSON output — cost if wrong: adjust only the CLI output adapter.

7. Task A1: Ruling: RunObserver additionally exposes server(version, cutoff) with a no-op default — preserves the existing structured MySQL server event — cost if wrong: remove an optional observer hook.

8. Task A1: Ruling: Move the existing report Mode with Report to runner and re-export it from CLI — preserves wire spelling and existing CLI matching without a dependency from runner to CLI — cost if wrong: local type mapping at the CLI boundary.

9. Task A3: Ruling: ServerTls owns atomic config replacement and connection leases captured before TLS, then guards negotiated plaintext; A4 cancels pending handshakes with that lease — wrapping TCP alone permits buffered TLS plaintext after revocation — cost if wrong: trust-refresh integration must be corrected before agent deployment.

10. Task A3: Ruling: Enrollment rejects existing Unix data directories with group/other permissions, and creates new directories as 0700; Windows ACL enforcement belongs to A5 installation — Unix permission checks are not Windows ACL proof — cost if wrong: native service installation must fail before persisting keys.

11. Task A4: Ruling: Add explicit DatabaseTls Preferred(default)/Required/DisabledLocal to technical profiles and canonical HMAC input before protocol v1 is released — pinned legacy MySQL cannot negotiate the modern client's TLS, and a global silent downgrade is unacceptable; DisabledLocal validates a loopback destination — cost if wrong: revise the profile field before first release; existing CLI URL SSL policy is unchanged.

12. Task A4: Ruling: Add typed MailObservation to the existing runner observer, using an observed MailControl adapter — translated console strings cannot establish Restoring/Running machine state — cost if wrong: adjust the additive observer contract, not CLI output or cleanup safety ordering.

13. Task A4: Ruling: MVP coalesces progress in one watch slot and publishes durable progress every five seconds, with service/phase/terminal changes immediate — this stays below the 4 Hz ceiling and avoids introducing two competing snapshot revisions — cost if wrong: UI progress is coarse; add distinct volatile event sequencing if a faster cadence is required.

14. Task A4: Ruling: Admission and cancellation are owned Tokio tasks; overlapping admission receives Busy through try_lock instead of an unbounded queue — HTTP cancellation must not strand a committed Accepted receipt or a committed cancellation without its worker signal — cost if wrong: a simultaneous duplicate may need a bounded retry while the first receipt is committing.

15. Task A4: Ruling: Keep durable receipt operations in jobs/ and place worker, observer, native mapping and publishing under execution/ — these have independent orchestration and storage responsibilities — cost if wrong: local module paths differ from the plan's illustrative file list, while the public run/protocol contracts remain intact.

16. Task A5: Ruling: Do not modify .github/workflows/cleanup-cli.yml despite A5's illustrative file list — the user's explicit instruction excludes CI/CD — cost if wrong: agent delivery remains a local/manual build until CI/CD is separately authorized.

17. Task A5: Ruling: Agent runtime takes an OS file lock before crash recovery; installer freezes admission in the existing SQLite writer transaction before stopping/replacing a service — a parallel process must not label another process's running job Interrupted or admit work between the idle check and replacement — cost if wrong: installation remains frozen after failure and must be repaired/retried; it never starts cleanup to recover.

18. Task A5: Ruling: Use the installed Windows PowerShell/.NET ACL API only during provisioning to replace the directory DACL atomically with SYSTEM/Administrators rights — icacls grants alone retain unrelated explicit ACEs; this adds no resident runtime — cost if wrong: native Windows ACL verification must reject provisioning before private-key creation.

19. Task A5: Ruling: Center feature wiring waits for its actual B-owned crate instead of an empty feature placeholder — agent-only and default local CLI are already isolated — cost if wrong: one additional Cargo feature edit in B.

20. Task A5: Ruling: Provision only dedicated flat local Windows directories named hMailCleanerAgent, resetting existing file DACLs too — replacing a directory DACL alone can retain explicit permissions on old private-key files, and a generic target parent could be a shared system directory — cost if wrong: custom/nested directory layouts require an explicit later provisioning contract.

21. Task A5: Ruling: Keep one verified previous EXE and atomic protected installation metadata; mismatched/incomplete filesystem state fails closed instead of guessing a rollback — SCM, files and SQLite cannot commit atomically together — cost if wrong: an interrupted update can require operator reconciliation before retry.

22. Task A5: Ruling: Accept vendor LNK4099 debug-symbol warnings from cargo-xwin's static Microsoft CRT, without suppressing linker warning severity — release linkage succeeds, missing private PDB files affect debug symbols — cost if wrong: vendor crash-frame diagnostics are limited; native Server 2016 acceptance is still required.

23. Task A5: Ruling: Agent health readiness is a one-way per-process projection (revision 1 ready, 2 unavailable/draining) — recovery requires restart, so constant revision 1 would conceal state change — cost if wrong: future in-place recovery must introduce a general health revision counter.

24. Task B1: Ruling: open_record additionally requires the expected EnvelopeHeader supplied by its owner — an intact encrypted envelope swapped between rows still authenticates its own header; binding must be checked against the requested record identity — cost if wrong: one explicit context argument at new, unreleased callers; B4 must construct it from the requested profile identity.

25. Task B1: Ruling: The center-plan Goal's 'encrypted SQLite' means SQLite containing encrypted technical records, not whole-file SQLCipher — this is explicit in its Global Constraints and the approved specification — cost if wrong: documentation wording changes only.

26. Task B2: Ruling: This first center schema has no earlier supported schema to migrate; all unknown versions (including uninitialized version 0 files) fail closed. Future supported migrations must snapshot before changes — no automatic repair of arbitrary existing SQLite — cost if wrong: an operator must move an invalid/foreign database explicitly instead of init overwriting it.

27. Task B2: Ruling: B2's temporary run skeleton exposes only loopback /healthz; B3 replaces it with HTTPS before browser auth/API exists — avoid pretending unfinished TLS/auth are deployed — cost if wrong: B2 alone is not a usable deployed center.

28. Task B3: Ruling: Logout invalidates the server token without Set-Cookie deletion — an old logout response must not erase a newer login's browser cookie — cost if wrong: an invalid opaque cookie remains until replaced by login or browser close, and grants no access.

29. Task B3: Ruling: Login admissions carry monotonically ordered attempt tickets in addition to account CAS revision — a slow earlier success must not replace a later successful session — cost if wrong: an older successful browser request receives an auth rejection and needs a fresh login.

30. Task B3: Ruling: Initial HTTPS certificate/key and public CA certificate are create-new bootstrap files, committed before the account transaction completes; partial bootstrap persistence fails closed and is not overwritten by init — filesystem and SQLite cannot commit atomically — cost if wrong: interrupted first initialization requires explicit local reconciliation; no existing identity is silently replaced.

31. Task B4: Ruling: Agent clients have zero idle pooled connections and disable redirects/proxy discovery — each manual control call checks the current certificate serial plus CA/role/UUID; active requests select against session and trust revocation — cost if wrong: extra TLS handshakes versus retaining one idle connection, to be measured in C5. This is below the one-idle-connection ceiling and prevents idle sockets retaining old trust.

32. Task B4: Ruling: Initial enrollment creates an explicitly unconfigured encrypted profile version; the first mail configuration requires Replace(password), then later edits may Keep — agents can be installed before MySQL/path setup, and no job can use an unconfigured version — cost if wrong: a separate configuration step is visible before cleanup becomes available.

33. Task B4: Ruling: SessionLease generation/account revision become private with read-only accessors before B5/B7 consume them — callers must not rewrite key-generation metadata while retaining an unrelated private owner — cost if wrong: update unreleased call sites; no cookie/session behavior change.

34. B4 complete: profile/identity/health, real mTLS renewal and pinning, session/storage/init/backup regressions PASS; full mise check, real MySQL core+agent regressions and Windows MSVC release cross-build PASS. Native Server 2016 runtime NOT RUN. Ruling: refuse changing permissions on an existing shared nonempty data directory; avoids claiming unrelated contents. New guard RED then GREEN, newer-schema fixture now acquires dedicated directory before writing synthetic DB. No external deployment.

35. Task B5: BASE f59e155. RED: dispatch test could not import JobService/SubmitRequest; subsequent GREEN real TLS fixtures prove uncertain delivery/global reservation, no restart PUT, changed-preview rejection, restored-service gate, logout after accept and fresh-login reconciliation. Bounded event/history tests and real upstream SSE observer PASS. Ruling: record intended cleanup ServicePolicy in preview intent while sending RequireStopped to agent — preview cannot Manage per A2, yet clean must bind explicitly confirmed policy — cost if wrong: unreleased browser contract revision. Ruling: persist initial known mail state in agent receipt, preserving across Interrupted, and compare fresh authenticated service observation before releasing Clean/Manage slot — final Stopped alone cannot distinguish initial Stopped from failed restoration — cost if wrong: old receipts lacking initial state require operator investigation, never automatic cleanup. Ruling: public job DTO is SubmitRequest/JobView/JobEvent, uses a shared typed error for submit/reconcile/cancel and the existing protocol PublicJob; no extra forwarding aliases. Agent and browser SSE use bounded 4KiB frames; reconnect is lookup only with 5s retry, not scheduled cleanup. Full check currently running.

36. Task 10 / B5: complete. mise run check PASS without warnings (87.16s, b5-check.log). Focused observer/SSE/reconciliation plus all-features/all-targets center Clippy PASS (b5-final.log). Receipt regression 11 PASS incl initial service state across restart. MySQL execution was not changed; its B4 real fixture receipts remain valid. Full suite exposed one B4 directory-guard fixture order mismatch: csrf test now acquires the dedicated empty directory before writing center.json, matching production CLI; no permissions relaxation. Ruling: initial intent uses conservative Unknown for both reserved-before-send and uncertain delivery, rather than exposing an unproven Pending distinction — every such state requires lookup/cancel, never implicit retry. Public JobView/JobEvent replace a new browser use of the earlier generic EventEnvelope; C1 exports the concrete browser contracts. Cost: unreleased DTO naming differs from plan examples, semantics retained.

37. Ruling: Extend native installer receipt with optional operation_id and expose authenticated installation status without executable paths — health version alone cannot prove completion of a particular uncertain update, especially reinstalling the same version — cost if wrong: unreleased agent protocol/CLI extension; old receipts remain readable but cannot prove new enrollment completion. Center requires exact operation UUID, package SHA, healthy/idle and version before releasing provisioning reservation. A post-response disconnect never authorizes another install.

38. Ruling: one durable provisioning reservation shares the same first SQLite writer reservation with job admission — installs and cleanup cannot both be admitted as the global heavy operation — cost if wrong: unknown installation outcomes retain the global slot for investigation. Checkpoints contain only IDs, public host fingerprint/build/hash/phase; OS credentials are ephemeral. Before installation begins, failures release as Failed; after possible SCM effects, Unknown retains the slot. Restart marks interrupted provisioning Unknown without SSH or decryption.

39. Ruling: UpdateRequest explicitly includes fresh SSH target/fingerprint/credentials — the plan omitted these despite prohibiting default credential retention — cost if wrong: administrator enters OS credentials for each manual update. Fixed bootstrap uses only UTF16 encoded static PowerShell source and bounded stdin JSON; no mail profile/SQL credentials enter shell text. Local package format is trusted-catalog/<build>/{manifest.json,hmail-cleaner-agent.exe}; future C4 image owns read-only /opt/hmail/agent-packages.

40. Ruling: RecoverIdentity is a separate explicit UpdateMode, not a fallback after TLS failure — main plan requires SSH recovery for expired trust while normal update must preserve identity — cost if wrong: extra wizard action in C3; failed recovery after maintenance retains Unknown slot. Agent prepare-recovery performs only durable idle maintenance, never Stop/Start mail. Existing native installer ends maintenance; old private key and receipt DB are preserved.

41. Ruling: Add bounded GET /enrollment recent checkpoints (50) so browser restart can find Unknown operations even if registration failed before a server profile existed — known operation IDs alone cannot provide that UI recovery — cost: one bounded lazy settings query and up to 50 local receipt reads. Constructor, checkpoint failure, restart and recent-read owners are separate modules.

42. Task B7: Ruling: recovery candidate retains encrypted account material and a SHA-256 fingerprint of the random canonical recovery key, not its plaintext or DEK — enough for confirmation and limits secret lifetime — cost if wrong: replace candidate representation, no disk migration.

43. Task B7: Ruling: revoke current session immediately before the short CAS transaction under credential barrier; rollback may require another login — spec explicitly permits session termination on rollback, and no old lease survives successful commit — cost if wrong: extra login after storage failure.

44. Task B7: Ruling: AppState authority made read-only externally with with_authority composition and authority() accessor — prevents mismatched account/session owners, fixture setters migrated — cost if wrong: unreleased Rust embedding callers adjust one builder call.

45. Task C1: Ruling: latest compatible TypeScript6.0.3 rather than latest7.0.2 — Astro/Svelte tooling peer range stops at6; Bun remains the actual CLI runtime — cost if wrong: repin compiler/check tools.

46. Task C1: Ruling: browser allowlist includes SSH private-key input contract required by approved B6/C3, excludes agent private identity, CA key, MailProfile and agent job payloads — SSH credentials must be representable at the browser boundary, never persisted — cost if wrong: remove unsupported browser key-auth form.

47. Task C1: Ruling: generated validator JS is type-represented by generated .d.ts, excluded as a root TS source and emitted with code.lines for readable diagnostics — Astro checking minified generated implementation produced 230 internal hints/102MB failed log; no application warning/error severity suppressed — cost if wrong: fix generator declaration contract; generated behavior remains tested.

48. Task C1: Ruling: use `bun run --cwd apps/web <script>` — `bun --cwd apps/web run <script>` printed help with zero exit instead of executing under1.4.3; read actual output and rejected this as verification — cost if wrong: adjust CLI arguments, no app behavior.

49. Task C1: Ruling: C1 creates a minimal real CleanupIsland (C2 will extend it) and installs Playwright one task early — main client:load and actual browser network assertions are C1 requirements; avoids a placeholder claim with no Svelte compilation — cost if wrong: move this coherent setup into C2 commit only.

50. Task C1: Ruling: split unit checks from browser checks; test:browser builds assets first — web:check must work on a clean checkout without ignored dist, C2 also needs a separate native-fixture browser lifecycle — cost if wrong: adjust script composition; no check severity removed.

51. Task C2: Ruling: add a narrow authenticated /servers/{id}/cleanup-context response (profile version/configured, latest durable job and global busy), not a history/settings read on landing — existing ServerView omits current version and a fresh tab cannot discover an accepted job ID; context is necessary to reconcile safely and satisfy deferred history/settings loading — cost if wrong: adjust this unreleased browser endpoint and regenerate contracts; no persisted schema change.

52. C2: Ruling: add Bun-compatible Prettier3.9.10 + Svelte4.1.1/Astro1.1.1 formatter plugins only as dev tools — source components need readable formatting; verified peer ranges and local execution, no lint suppression or new CI gate — cost if wrong: adjust formatter pins; shipped runtime unaffected.

53. C2: Ruling: frontend consumes local preview after a dispatch, while backend remains the authoritative exact-parameter admission check — avoids accidental repeated clean from the same screen — cost if wrong: operator runs another preview before another clean.

54. C2: Ruling: frontend session guard owns pagehide/pageshow redaction and closes SSE, footer polling has its own bounded lifetime and reports null freshness on error; passive polling never calls activity — protects cached pages without retaining secrets or extending idle sessions — cost if wrong: an extra reload after connectivity loss.

55. C2: Ruling: browser test runner accepts explicit suite names and builds static assets plus native fixture before tests — avoids accidental inclusion of unrelated unit files from Bun positional filter behavior — cost if wrong: adapt only local runner registry; CI/CD unchanged.

56. Task C3: Ruling: plan snippet password.action corrected to password.kind — actual approved protocol uses tagged PasswordPatch {kind:keep|replace}; no wire-contract rename — cost if wrong: one assertion correction.

57. Task C3: Ruling: user explicitly switched remaining work to parallel subagents; independent C3 history, account, agents owners dispatched in shared branch, root owns profile/composition/fixtures. Overrides inline and skill no-parallel-implementers preference with explicit disjoint file ownership; no worker commits or duplicate build writers — cost if wrong: integration rework, verified together before commit.

58. Task C4 Ruling: deploy example is center.example.json, not staleplan .toml — actualCenterConfig owns strictJSON; no secondconfigformat — costifwrong: filename/documentation only.

59. Task C4 Ruling: health uses direct local TCP + ordinary rustls CA/ServerName verifier +Hyper HTTP1 — reqwest DNSoverride skipsIP literals and LAN publicIPorigin mustworkinsidecontainer atlistenport — costifwrong: adjustboundedhealthtransport, noTLSbypass.

60. Task C4 Ruling: permanent SessionAuthority closed admission onshutdown — revocationaloneletpendingKDFadmitduringdrain; REDraceprovedgap — costifwrong: processmustrestarttoadmitagain, correctforshutdown.

61. Task C4 Ruling: share source-revision validation via source-included crates/cli/build_support.rs and EXE source.rs honors explicitrevision beforeGit — existingcontainerhelperoverrode dirtyfingerprint anddivergedfromreleaseidentity — costifwrong: localmetadata/buildhelperrework, existingpackerreused.

62. Task C5 Ruling: declare existingPlaywright1.64.1 directlyatroot foractualmeasurementconsumer — deeprelativepackageimportlostplaywright-coretypes; avoidbareforwardingwrapper/newversion — costifwrong: devdependencyscopeonly; no browserdownloadorGC.

63. Task C4 Ruling: native Podman health uses raw JSON argv without CMD while OCI/inspect Test retains CMD — primary Podman5.6.2 parser treats CMD-prefixed JSON as a string whereas6.1.3 fixes it; rawargv works in both — cost if wrong: native health fails, so actual Podman acceptance remains separate.

64. Task C5 Ruling: use one shared POSIX owned-process-group termination mechanism for check runner and resource fixture — both consumers require bounded TERM/KILL and independent cleanup; Windows group semantics explicitly unsupported — cost if wrong: retain cleanup failure and task-owned resources for inspection instead of claiming PASS.

65. Task C5 Ruling: require pre-measurement artifact/source receipt and validate inside atomic PASS publication after cleanup — current Git HEAD alone cannot identify a dirty-built native binary or detect source mutation — cost if wrong: measurements are rejected and must be rebuilt, no performance numbers silently attributed to another source.

66. Task C4 Ruling: webbuild uses native BUILDPLATFORM, Rust/runtime keep selectedtarget; stagedContainerfile ARGdefaults are rendered fromvalidatedbaseimages soleowner — actualx64Bun underARMQEMU lacksAVX andcrashes139, whileweboutputisarchitectureindependent; avoidsNodefallback/duplicatepins — cost if wrong: crossbuild fails, checkedviaactualnativewebandPodman5.6.2primaryparser beforefullrelease.

67. Task C5 Ruling: wake existing same-generation jobfollower withcoalescedNotify onresume, retain5ssteadycadence — actualtrace terminalSSEarrived4482ms afterfinish with0agentwatchers andUIcompleted4832ms, leaving168ms of5000msassertion; backendREDprovedidlefollowerdidnotstartwatch within1s — cost if wrong: extraearlyreadonlypoll, targetedrevocation/singleowner regressionsrequired.

68. Task C5 Ruling: refreshhistoryonterminalphaseevenwhileglobal slotheld — actualcompleted+slot_held=true occurswhenmailserviceverificationUnknown andoldobserverneverrefreshes; actualbrowserREDprovedstaleRunning — cost if wrong: onecoalescedhistoryreadperterminaltransition; summary/attentionstateandglobaladmissionremainauthoritative.

69. Task C5 Ruling: retry only zero-signal EPERM as uncertain until existing grace deadline; gone requires ESRCH, TERM/KILL errors propagate — actual100orphanstress4EPERM allsameUID/ownedPGID thenESRCH, consistentXNUzombie filtering; fixed100/100inclrealEPERMretry — cost if wrong: boundedfailureandretainedprivatefiles, neverfalsecleanupsuccess. Scopedreviewapproved.

70. Task C5 Ruling: persistent unknown-delivery fault hides both GET and initial SSE receipt reads until explicit release — raw trace proved onePUT, persistent GEThiding but authoritative Running SSE9ms afterPOST; newNotify safely exposed acceptedreceipt — costifwrong: fixturecoverageonly; productionNotify/reconcile remainunchanged. Cleanup7/7PASS65assertions48.30s, scopedreviewapproved. Added explicitfaultrelease discoverssamejobIDwithonePUT and unknownheld→cancel proof; rootfullharnesspending.

71. Task C4 Ruling: Rust compilation uses BUILDPLATFORM with a closed GNU cross-target mapping and validates the actual final ELF e_machine — release2's emulated linker crashed, and image architecture labels alone cannot prove the executable target — cost if wrong: fail-closed build/OCI inspection; no changed runtime architecture. Actual native AArch64 proc-macro + GNU-linked x86_64 diagnostic PASS; GCC/binutils/cross-sysroot direct inputs pinned, transitive APT resolution recorded but not snapshot-pinned or claimed byte reproducible. Scoped14tests76assertions/TS/fmtPASS; fresh release3 and harness3 now running independently. Source FROZEN; measurement waits for heavy builds/tests to finish.

72. Task C5 Ruling: initialize each check log with explicit truncation and0600 before FileSink/child, returnFAILreceipt for initialization I/O failure — actualBun1.4.3 FileSink retainedoldtail and Bun.write emptystring modeoption created0644 — costifwrong: failclosed logging, no unchecked child; compatible fsPromises retains Bunruntime/noNodeprocess. REDoldtail+EISDIR, focused9/9PASS39assertions, TS/fmtPASS; scopedreviewapproved. Fullharness4 firstfixtureguards14/14PASS70assertions; release4/rusttests inprogress, no extra sourceedits.

73. C4lifecycle diagnosis: Docker29containerd ImageInspect.Id is the OCI manifest digest856a..., not configdigest616d...; actual --pull=never wrongIDexit125/noimage, manifestID --version exactbuildPASS and sanitizedinitprobe prompt4/4exit0/OOMfalse. Ownprobecontainer/volume removed. Ruling: inspect localtag and fail-closed validate actualimmutableId equals manifestdigest orconfigdigest, exactLinuxarch/build/source labels, then runbyverifiedId with --pull=never — supportsactualcontainerd/classicstorewithoutblindpull — costifwrong: diagnosticpreflightfailure, no initresourceallocation. Accountfixinprogress; sourceeditswerehelduntilnativeprobeended.

74. C5 actualUIbugconfirmed: restoredlocalcutoff2026-09-18T02:55:44.049364 becomesemptyrequired datetime-local input, thirdpreview sendszeroAPIcalls/PUTs. Ruling: retainexactcanonicalcutoffinstate; derive nativefielddisplayatmillisecondprecisionwithstepany and showfullboundary; actualoperatorfieldedit replacescanonical, omittedzerosecondsnormalizedatinputboundary — HTMLsupports≤3fractiondigits, protocolRustsupports≤9; silentroundingchangescleanupmeaning — costifwrong: focusedUI/inputadapterrework, nobackend/schemachange. Resourceworkerowns scopedcleanupUI+reload/submit/cancelprecisionregression; no finalmetricsyet.
