# hMail Cleaner MVP: Agent — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Получить нативный агент, который безопасно выполняет только явные команды центра и переиспользует существующую очистку.

**Architecture:** Существующий разовый сценарий выделяется в runner; protocol задаёт wire-контракты, agent владеет квитанциями и выполнением. Переносимый mTLS-адаптер общий для центра и агента; Windows SCM остаётся платформенной границей.

**Tech Stack:** Rust 1.99.1, Tokio, Axum, SQLx 0.7.4/MySQL/SQLite, существующие core/adapters, rustls/ring/rcgen, windows-service.

**Spec:** [Спецификация](../specs/2026-10-17-hmail-control-center-design.md), [общие контракты и порядок](2026-10-17-hmail-control-center-mvp.md).

## Global Constraints

- Rust 1.99.1; Windows Server 2016, x86_64-pc-windows-msvc со статическим MSVC runtime.
- Только ручная очистка. Прежний планировщик и HMAIL_RUN_AT удаляются.
- CreationTime < cutoff для существующих файлов; дата БД для отсутствующих.
- Полный preview требует уже остановленной службы; управление Stop/Restore только при явном clean/manage.
- Подтверждение отсутствия файла предшествует SQL-delete; restore предшествует release SQL lock.
- Принятое задание не зависит от браузера; рестарт агента не возобновляет удаление.
- Версии, лимиты и поля DTO берутся из общего плана; технический профиль в журналах не сохраняется.

## Review Focus

- Ошибка restore/close не теряет исходную ошибку и partial counters — A1.
- Запоздалый POST после неизвестного cancel не запускает работу — A2.
- Доверенный CA с чужим agent_id/center_id не даёт доступа — A3.
- ENOSPC после Accepted и до terminal commit не превращается в Completed — A2, A4.
- Обновление SCM при активной очистке не прерывает restore и не оставляет старый scheduler — A5.

---

## A1. Общий разовый запуск и удаление расписания

**Files**

- Create: crates/runner/Cargo.toml; crates/runner/src/{lib,types,observer,run,lifecycle,report}.rs.
- Create: crates/runner/tests/lifecycle.rs.
- Modify: Cargo.toml, Cargo.lock, mise.toml, tools/build-exe/settings.rs.
- Modify: crates/adapters/Cargo.toml, crates/cli/Cargo.toml.
- Modify: crates/cli/src/{main,args,commands}.rs, app/mod.rs, config/{mod,types,tests,source_tests}.rs.
- Delete: crates/cli/src/service/{mod,runner,schedule}.rs.
- Modify: crates/cli/tests/{help,config,launch}.rs, docs/cleanup-cli.md.

**Interfaces**

- Consumes: recursive::run(Dependencies, &RunOptions, ScanOptions, &Cancellation, P, Action); MailControl; MySqlRepository; LocalFiles; WindowsOffline.
- Produces: runner::RunRequest с database, options: RunOptions, scan, action, service_policy, hmail_ini, service_name.
- Produces: runner::RunObserver: begin(&RunRequest), progress(Progress), action(&str), mail_state(&str), failed() -> bool; Send + Sync.
- Produces: runner::run_once(&RunRequest, &impl RunObserver, &Cancellation) -> RunResult.
- Produces: прежние Report/status/summary/exit_code и RunResult::{Finished,Fatal}; ServicePolicy::{RequireStopped,Manage}.

- [x] **Step 1: Закрепить регрессии.** Перенести meaningful lifecycle-тест из CLI в runner/tests/lifecycle.rs; расширить restore failure и close failure. В CLI tests/launch.rs проверить удалённый режим:

~~~rust
// database_lock_is_held_until_restore_finishes
assert_eq!(trace, ["acquire lock", "verify access", "stop", "stopped", "work", "restore", "release lock"]);
// restore_failure_preserves_cleanup_failure
assert!(result.is_fatal());
assert!(report.error.as_ref().unwrap().contains("fixture cleanup failure"));
assert!(report.error.as_ref().unwrap().contains("fixture restore failure"));
// legacy_service_run_is_rejected_without_work
assert_eq!(exit_code, 2);
assert_eq!(runner_calls, 0);
~~~

- [x] **Step 2: Проверить RED.** Сначала сохранить baseline: mise run check; затем новые тесты: cargo test -p hmail-cleaner --test launch --locked. Ожидается FAIL для legacy service, а отсутствие runner package — диагностируемый RED при первом запуске cargo test -p hmail-cleaner-runner --test lifecycle.
- [x] **Step 3: Выделить сценарий.** Перенести текущую композицию и Report без смены порядка/exit codes; CLI Output реализует RunObserver. CLI config преобразует свой Config в RunRequest и остаётся единственным владельцем env загрузки. Agent не импортирует cli/config. Сохранить no-args menu, preview/clean, NDJSON и существующие проверки. Удалить расписание, ServiceSettings и чтение HMAIL_RUN_AT; прежний service run возвращает явную ошибку. Не менять ядро для удобства HTTP.
- [x] **Step 4: Проверить GREEN и сборочную границу.** Запустить cargo test -p hmail-cleaner-runner --locked, mise run check, mise run test:db. Закрепить 1.99.1 во всех трёх текущих владельцах версии; features SQLx mysql/runtime-tokio-rustls/chrono перенести в adapters. Ожидается PASS без изменения CLI результатов; MySQL fixture запускается только отдельной командой.
- [x] **Step 5: Локальный коммит.** git commit -m "refactor: share manual cleanup execution and remove scheduling" после добавления только файлов A1.

## A2. Протокол и долговечные квитанции

**Files**

- Create: crates/protocol/Cargo.toml; crates/protocol/src/{lib,ids,jobs,profiles,events,errors,limits,canonical}.rs.
- Create: crates/protocol/tests/{wire,canonical}.rs; crates/protocol/tests/fixtures/{job-v1,snapshot-v1}.json.
- Create: crates/protocol/src/browser/{mod,auth,profiles,jobs,enrollment,settings}.rs; DTO дополняются задачей-владельцем соответствующего HTTP API в части B.
- Create: crates/agent/Cargo.toml; crates/agent/src/lib.rs; crates/agent/src/jobs/{mod,types,repository,recovery}.rs.
- Create: crates/agent/migrations/0001_jobs.sql; crates/agent/tests/receipts.rs.
- Modify: Cargo.toml, Cargo.lock.

**Interfaces**

- Consumes: общие JobCommand/ResolvedCutoff/JobSnapshot/EventEnvelope и v1 из основного плана.
- Produces: protocol::validate_job(JobCommand) -> Result<ValidatedJob, ProtocolError>.
- Produces: protocol::canonical_job(&ValidatedJob) -> Vec<u8>; фиксированные поля и length-prefixed значения, порядок документирован в tests/fixtures.
- Produces: agent::jobs::JobRepository::open(&Path) -> Result<Self, JobStoreError>.
- Produces: accept(&self, &ValidatedJob, RequestFingerprint) -> Result<AcceptOutcome, JobStoreError>; AcceptOutcome::{New,Existing,Conflict,Busy,Cancelled}.
- Produces: cancel(&self, JobId) -> Result<JobSnapshot, JobStoreError>; lookup(&self, JobId) -> Result<Option<JobSnapshot>, JobStoreError>; recover_interrupted(&self) -> Result<(), JobStoreError>.

- [x] **Step 1: Написать тесты протокола и SQLite/tempdir.**

~~~rust
// same_id_same_command_has_one_receipt; same_id_changed_profile_is_conflict
assert_eq!(receipts_count, 1);
assert_eq!(changed_command_result, AcceptOutcome::Conflict);
// cancel_unknown_blocks_late_post
assert_eq!(cancelled.phase, JobPhase::Cancelled);
assert_eq!(late_accept, AcceptOutcome::Cancelled);
assert_eq!(runner_calls, 0);
// restart_interrupts_without_resume
assert_eq!(after_restart.phase, JobPhase::Interrupted);
assert_eq!(runner_calls_after_restart, 0);
// receipt_commit_failure_never_starts_worker
assert_eq!(runner_calls_after_disk_error, 0);
// wire_counters_are_lossless
assert_eq!(json["summary"]["removed_bytes"], "9007199254740993");
~~~

  Дополнительно: concurrent distinct IDs → один New/один Busy; ошибка commit → runner_calls=0; corrupted DB → Err, а не None; Windows-путь остаётся исходной строкой на Linux. Неизвестная protocol_version отклоняется до преобразования MailProfile.
- [x] **Step 2: Проверить RED.** cargo test -p hmail-cleaner-protocol --test wire; cargo test -p hmail-cleaner-agent --test receipts. Ожидается отсутствие новых API/провал assertions.
- [x] **Step 3: Реализовать контракт и квитанции.** Секреты в ValidatedJob ограничены и не выводятся через Debug. Agent DB хранит только fingerprint, публичные параметры, фазы/итоги; ключ HMAC квитанций — отдельный файл с правами ОС. SQLite transaction + unique active constraint завершаются до New. Cancel неизвестного ID сохраняет tombstone. На boot отмечать старые active как Interrupted; не исполнять их. Различать DTO и прежние core-типы, не менять сериализацию CLI Summary.
- [x] **Step 4: Проверить GREEN.** cargo test -p hmail-cleaner-protocol --locked; cargo test -p hmail-cleaner-agent --test receipts --locked. Проверить fixture с лишними/неверными полями, изменением mail profile под прежним version, u64 overflow и несовместимым протоколом. Ожидается PASS.
- [x] **Step 5: Локальный коммит.** git commit -m "feat: persist idempotent agent job receipts".

## A3. mTLS и проверка идентичности до передачи профиля

**Files**

- Create: crates/adapters/src/mtls/{mod,types,identity,client,server}.rs.
- Modify: crates/adapters/src/lib.rs, crates/adapters/Cargo.toml.
- Create: crates/adapters/tests/mtls.rs.
- Create: crates/agent/src/enrollment/{mod,keys,csr}.rs.
- Create: crates/agent/tests/enrollment.rs.
- Modify: crates/agent/Cargo.toml, Cargo.lock.

**Interfaces**

- Consumes: PeerIdentity::{Center(Uuid),Agent(Uuid)}, TLS limits/policy основного плана.
- Produces: mtls::client_config(&ClientIdentity, &TrustPolicy, expected: PeerIdentity, &RevokedSerials) -> Result<rustls::ClientConfig, TlsError>.
- Produces: mtls::server_config(&ServerIdentity, &TrustPolicy, expected: PeerIdentity, &RevokedSerials) -> Result<rustls::ServerConfig, TlsError>.
- Produces: agent::enrollment::prepare(&AgentBootstrap) -> Result<AgentCsr, EnrollmentError>; локальный ключ/CSR, без старта очистки.
- Adapters features: mail (сохраняет прежний default), mtls (новый optional); center подключает adapters только с mtls, agent — mail + mtls. Компиляция mail-модулей, MySQL и Windows/SCM dependencies gated, а не просто неиспользуемая. Sqlite agent ledger подключается самим agent crate.

- [x] **Step 1: Написать TLS fixtures и отрицательные тесты.**

~~~rust
// trusted_ca_wrong_node_rejected_before_request_body
assert!(handshake_result.is_err());
assert_eq!(received_profile_bytes, 0);
// missing_expired_revoked_wrong_eku_certificates_rejected
assert_eq!(accepted_requests, 0);
// valid_center_and_expected_agent
assert_eq!(authenticated_peer, PeerIdentity::Agent(agent_id));
// agent_private_key_never_exported
assert!(!csr_output.contains("PRIVATE KEY"));
~~~

- [x] **Step 2: Проверить RED.** cargo test -p hmail-cleaner-adapters --test mtls; cargo test -p hmail-cleaner-agent --test enrollment. До добавления реализаций ожидается FAIL.
- [x] **Step 3: Реализовать shared adapter.** Использовать выбранные версии §8.4 и ring без default crypto-provider. Обёртки verifier делегируют проверку цепочки/подписей WebPKI и дополнительно проверяют SAN, EKU, UUID, serial denylist. Идентичность проверяется в handshake, а не после POST. CSR подпись проверяется rcgen; выдаваемые extensions задаёт центр. Agent private key создаётся в защищённом каталоге. Реализовать атомарную замену TLS config и разрыв старых соединений при отзыве.
- [x] **Step 4: Проверить GREEN.** cargo test -p hmail-cleaner-adapters --features mtls --test mtls --locked; cargo test -p hmail-cleaner-agent --test enrollment --locked. Проверить ca/hostname/identity по отдельности, истечение срока и refresh verifier. Ожидается PASS; native Server 2016 пока NOT RUN.
- [x] **Step 5: Локальный коммит.** git commit -m "feat: authenticate center and agent identities with mtls".

## A4. Выполнение задания, прогресс и корректная отмена

**Files**

- Create: crates/agent/src/{app,config}.rs.
- Create: crates/agent/src/http/{mod,routes,jobs,resolve,health}.rs.
- Create: crates/agent/src/jobs/{execute,observer,events,redaction}.rs.
- Create: crates/agent/src/jobs/mapping.rs.
- Create: crates/agent/tests/{execution,reconnect,cutoff}.rs; crates/agent/tests/fixtures/mod.rs.
- Modify: crates/agent/src/jobs/repository.rs, crates/agent/Cargo.toml.
- Modify: crates/adapters/src/mysql/config.rs, mise.toml.

**Interfaces**

- Consumes: runner::run_once; JobRepository; TLS configs; protocol validated types.
- Produces: agent::run(AgentConfig, Arc<Cancellation>) -> Result<(), AgentError>.
- Produces: POST /agent/v1/resolve → ResolvedCutoff; PUT /agent/v1/jobs/{job_id} → JobSnapshot; GET /agent/v1/jobs/{job_id}; PUT /agent/v1/jobs/{job_id}/cancel.
- Produces: GET /agent/v1/health и GET /agent/v1/jobs/{job_id}/events; все рабочие endpoints требуют mTLS.
- Produces: POST /agent/v1/profile/validate без Stop/scan/delete; DatabaseConfig::from_parts(&DatabaseParts) -> Result<DatabaseConfig, DatabaseConfigError>, переиспользующий проверку существующего parse.
- Produces: execute(&ValidatedJob, &impl RunObserver, &Cancellation) -> RunResult; DTO → native RunRequest преобразует агент.

- [ ] **Step 1: Написать execution/reconnect/cutoff tests с fake runner и clock.**

~~~rust
// accepted_before_execution
assert_eq!(trace[0], "receipt committed");
assert_eq!(trace[1], "runner started");
// cancel_waits_for_restore
assert_eq!(during_restore.phase, JobPhase::Restoring);
assert!(!during_restore.is_terminal());
// logout_or_network_disconnect_does_not_cancel_job
assert_eq!(runner_calls, 1);
assert_eq!(finished.phase, JobPhase::Completed);
// progress_sink_failure_is_not_success
assert_ne!(result.phase, JobPhase::Completed);
// terminal_commit_failure_never_publishes_success
assert_eq!(completed_events_after_disk_error, 0);
~~~

  Cutoff tests: календарный 1mo на конце месяца; DST nonexistent/ambiguous → ошибка; local cutoff для MySQL и UTC file_cutoff согласованы. Full preview при Running не вызывает Stop; managed clean восстанавливает исходное состояние; неправильный agent_id не открывает MySQL. ENOSPC terminal commit → нет события Completed.
- [ ] **Step 2: Проверить RED.** cargo test -p hmail-cleaner-agent --test execution; cargo test -p hmail-cleaner-agent --test cutoff. Ожидается FAIL до API/worker.
- [ ] **Step 3: Реализовать worker и transport.** Work начинается только после durable receipt; worker принадлежит агенту, не request task. Не держать DB/session mutex во время runner/сети. Важные итоги сохранять до публикации, counters coalesce; bounded observer не тормозит cleanup из-за медленного SSE. При ошибке собственного обязательного журнала запросить безопасную отмену и сохранить failure. PublicEvent строить по allowlist, не сериализовать сырой Failure/path/error. Потеря диска не даёт фиктивного terminal success. DatabaseConfig::from_parts создаёт MySqlConnectOptions без ручной интерполяции URI и использует ту же валидацию, что parse; старый API/SSL-политика сохраняются.
- [ ] **Step 4: Проверить GREEN.** cargo test -p hmail-cleaner-agent --locked; mise run test:db; mise run test:agent-db. Последнюю команду добавить здесь для ignored MySQL-кейсов tests/execution.rs, с существующим disposable MySQL и HMAIL_TEST_DATABASE_URL из test fixture owner. Повторно использовать canonical schema.sql из adapters/tests/fixtures как явно общий тестовый ресурс, не импортировать внутренний Rust fixture-код. Проверить реальные MySQL/файлы: preview ничего не удаляет, повтор ID вызывает runner один раз, cancel завершает безопасный пакет/restore, null remaining_old_files сохраняется. Ожидается PASS на изолированных данных.
- [ ] **Step 5: Локальный коммит.** git commit -m "feat: execute manual agent jobs with bounded progress".

## A5. Windows-служба и пакет агента

**Files**

- Create: crates/agent/src/service/{mod,install,migration}.rs; crates/agent/tests/service.rs.
- Modify: crates/cli/src/{main,args}.rs, crates/cli/Cargo.toml, crates/adapters/src/service/mod.rs при необходимом расширении существующего host.
- Modify: tools/build-exe/{main,settings,container,artifact}.rs, tools/build-exe.sh, tools/build-exe.ps1.
- Create: tools/release/agent-manifest.ts, docs/agent-installation.md.
- Modify: .github/workflows/cleanup-cli.yml, mise.toml; Create: crates/agent/examples/service_fixture.rs.

**Interfaces**

- Consumes: agent::run, enrollment::prepare, существующий adapters::service::host(name, ServiceJob).
- Produces: команды agent enroll/run/service/install-service; SCM name hMailCleanerAgent.
- Produces: service::install(InstallServiceInput) -> Result<InstallationReceipt, InstallError>; InstallError::Busy блокирует замену при active/restore.
- Produces: агентский пакет dist/agent/windows-x64 с EXE, manifest.json, SHA-256; manifest содержит product, build, target, protocol_min=1, protocol_max=1, sha256.
- Build helper принимает --component cli|agent; существующий вызов без аргумента сохраняет прежний CLI артефакт и поведение при неудачной сборке.

- [ ] **Step 1: Написать миграционные тесты и SCM fixture.**

~~~rust
// service_start_waits_for_manual_command
assert_eq!(runner_calls, 0);
// update_is_refused_while_job_or_restore_is_active
assert!(matches!(update_result, Err(InstallError::Busy)));
assert_eq!(replaced_executables, 0);
// unrelated_service_name_collision_is_rejected
assert_eq!(modified_services, 0);
// legacy_scheduler_is_stopped_before_agent_activation
assert!(trace.iter().position(|x| x == "legacy stopped").unwrap()
    < trace.iter().position(|x| x == "agent enabled").unwrap());
~~~

- [ ] **Step 2: Проверить RED.** cargo test -p hmail-cleaner-agent --test service; mise run test:build после добавления agent packaging assertions. Native fixture пометить ignored с требованием отдельной Windows-среды.
- [ ] **Step 3: Реализовать ожидание команд в SCM.** Переиспользовать host и кооперативную Cancellation. Service STOP отменяет текущую работу и ждёт безопасного завершения. При миграции проверять принадлежность старой службы hMailCleaner, дождаться её корректной остановки/restore и отключить расписание до активации агента; неизвестная служба или ошибка миграции блокируют установку. Не переиспользовать или перезаписывать её наугад. Пакет не содержит env, базы или секретов; SHA-256 сверяется с доверенным manifest центра.
- [ ] **Step 4: Проверить GREEN.** cargo test -p hmail-cleaner-agent --locked; mise run test:build; mise run build:agent. Новую mise build:agent добавить здесь. На Windows собрать service_fixture, выполнить cargo test -p hmail-cleaner-agent --test service -- --ignored --test-threads=1 в выделенной среде. Проверить startup/no command/no deletion и STOP/restore. Отдельно выполнить на Server 2016 и отметить реальный результат.
- [ ] **Step 5: Локальный коммит.** git commit -m "feat: package a manual Windows cleanup agent".

## Готовность части A

Агент и CLI проверены по отдельности; переносимые тесты не выдаются за native
Windows-приёмку. Перед частью B доступны protocol v1, TLS adapter, agent API
и fixture-пакет. Реальная удалённая установка не выполняется без доступной
целевой среды и разрешения на изменение этой среды.
