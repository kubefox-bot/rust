# hMail Cleaner MVP: Center — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Построить Rust-центр с одной учётной записью, SQLite с зашифрованными техническими профилями, управлением заданиями и установкой агентов.

**Architecture:** center собирает auth, vault, profiles, jobs и enrollment через явные зависимости; доменные операции отделены от Axum/SQLx/SSH. Browser API и agent transport имеют разные границы доверия. Фоновая работа не имеет постоянного доступа к DEK после выхода.

**Tech Stack:** Rust 1.99.1, Axum + Tokio, SQLx/SQLite, tower-cookies, tower-http CsrfLayer, RustCrypto Argon2id/XChaCha20-Poly1305, russh/SFTP, rustls/ring/reqwest/rcgen.

**Spec:** [Спецификация](../specs/2026-10-17-hmail-control-center-design.md), [общие решения и API](2026-10-17-hmail-control-center-mvp.md). Требуются результаты части A.

## Global Constraints

- Rust + Axum + Tokio; один процесс приложения, без Astro/Bun runtime.
- SQLite + SQLx, весь технический профиль зашифрован до SQL записи; SQLCipher не требуется.
- Один суперадмин и одна RAM-сессия; новый успешный вход заменяет прежнюю.
- Cookie __Host-hmail-session: Secure, HttpOnly, SameSite=Strict, Path=/, без Domain.
- Таймауты 15/30/60/1440 минут; default 15 минут, activity coalescing 30 секунд.
- Argon2id + XChaCha20-Poly1305 и recovery key; полная ротация DEK отложена.
- После рестарта только вход и согласование старых job_id; автоматического повторного clean нет.
- Лимиты запросов/KDF/буферов и схемы ключей — из общего плана.

## Review Focus

- PHC/nonce/метаданные из повреждённой БД не вызывают произвольных allocations/KDF — B1.
- Непрочитанная/повреждённая SQLite не считается новой; два init не создают два admin — B2.
- Старый logout, таймер или поздний login не отзывают/не восстанавливают чужую генерацию — B3, B7.
- Потерянный ответ агента и GET 404 не освобождают резервирование — B5.
- Shell injection, сменившийся host key и частичная установка не раскрывают секреты и не останавливают почту — B6.

---

## B1. Криптографический формат и ограниченный KDF

**Files**

- Create: crates/center/Cargo.toml; crates/center/src/lib.rs.
- Create: crates/center/src/vault/{mod,types,format,kdf,derive,cipher,entropy}.rs.
- Create: crates/center/tests/{vault,kdf_limits}.rs; docs/crypto-format-v1.md.
- Modify: Cargo.toml, Cargo.lock.

**Interfaces**

- Produces: SecretText = Zeroizing<String>, Dek = Zeroizing<[u8; 32]>; EnvelopeHeader, EncryptedRecord, PreparedVault, VaultError.
- Produces: initialize(&SecretText, &dyn Entropy) -> Result<PreparedVault, VaultError>.
- Produces: open_password(&AccountMaterial, &SecretText) -> Result<Dek, VaultError>; open_recovery(&AccountMaterial, &RecoveryKey) -> Result<Dek, VaultError>.
- Produces: seal_record(&Dek, &EnvelopeHeader, &[u8]) -> Result<EncryptedRecord, VaultError>; open_record(&Dek, expected: &EnvelopeHeader, &EncryptedRecord) -> Result<Zeroizing<Vec<u8>>, VaultError>. Expected context задаёт владелец записи, чтобы целиком переставленный envelope не принимался за другой профиль.
- Produces: encode_aad(&EnvelopeHeader) -> Result<Vec<u8>, VaultError>.
- Produces: async KdfGate::try_run<T>(&self, job: impl FnOnce() -> Result<T, VaultError> + Send + 'static) -> Result<T, KdfError>, T: Send + 'static; один spawn_blocking без очереди.

- [x] **Step 1: Написать vault/kdf tests и фиксированный тестовый вектор формата.**

~~~rust
// password_and_recovery_open_same_dek
assert_eq!(password_dek.as_ref(), recovery_dek.as_ref());
// saved_verifier_is_not_an_unlock_key
assert!(open_with_verifier_result.is_err());
// swapped_record_or_changed_aad_fails
assert!(swapped_profile_result.is_err());
// unsupported_kdf_rejected_before_allocation
assert_eq!(arena_allocations, 0);
// cancelled_http_request_does_not_release_kdf_early
assert_eq!(max_simultaneous_kdfs, 1);
~~~

  Тесты неверного пароля/recovery/tag, перепутанного назначения ключей, oversized envelope, сбоя CSPRNG, новых nonce при повторном seal. Контрольная KDF-арена очищается на success/error; не обещать очистку чужих копий.
- [x] **Step 2: Проверить RED.** cargo test -p hmail-cleaner-center --test vault; cargo test -p hmail-cleaner-center --test kdf_limits. Ожидается FAIL до реализации.
- [x] **Step 3: Реализовать формат v1.** Выбранные библиотеки §5.8, одна 32 MiB арена, две разные соли/последовательные KDF. В docs/crypto-format-v1.md зафиксировать порядок/длины big-endian полей из общего плана и тестовый hex-вектор. HKDF назначения разделены; все метаданные обёртки аутентифицированы. Не использовать JSON как AAD. У каждой обёртки собственное поколение; версия учётной записи для CAS не заставляет менять профильные ciphertext. KDF permit принадлежит closure до её фактического завершения.
- [x] **Step 4: Проверить GREEN.** cargo test -p hmail-cleaner-center --test vault --locked; cargo test -p hmail-cleaner-center --test kdf_limits --locked. Ожидается PASS; cargo tree -p hmail-cleaner-center -e features не включает Rayon/AWS-LC случайно.
- [x] **Step 5: Локальный коммит.** git commit -m "feat: encrypt center records with password and recovery wraps".

## B2. SQLite, каталог данных и локальный init

**Files**

- Create: crates/center/src/storage/{mod,directory,connection,migrations,backup}.rs.
- Create: crates/center/migrations/0001_center.sql.
- Create: crates/center/src/account/{mod,types,repository,initialize}.rs; crates/center/src/config.rs.
- Create: crates/center/tests/{initialization,storage,backup}.rs.
- Modify: crates/cli/src/{main,args}.rs, crates/cli/Cargo.toml, mise.toml.

**Interfaces**

- Consumes: PreparedVault, EncryptedRecord; Rust file locking для исключительного доступа к каталогу.
- Produces: CenterConfig::load(&CenterArgs) -> Result<CenterConfig, ConfigError>; единственный владелец raw env/конфигурации центра.
- Produces: DataDirectory::acquire(&Path) -> Result<DataDirectoryLease, StorageError>.
- Produces: CenterStore::open(&DataDirectoryLease) -> Result<CenterStore, StorageError>; backup(&self, &Path) -> Result<BackupManifest, StorageError>.
- Produces: initialize(&CenterStore, &InitInput, &mut impl SecretPrompt) -> Result<InitReceipt, InitError>.
- Produces: команды hmail-cleaner center init/run с --data-dir; run пока поднимает только проверяемое приложение без UI.

- [x] **Step 1: Написать init/storage/backup tests.**

~~~rust
// concurrent_init_has_one_account_and_no_partial_wraps
assert_eq!(accounts_count, 1);
assert_eq!(active_password_wraps, 1);
assert_eq!(active_recovery_wraps, 1);
// recovery_confirmation_precedes_commit
assert_eq!(accounts_before_confirmation, 0);
// corrupt_existing_database_is_not_initialized_over
assert!(init_result.is_err());
assert_eq!(original_database_bytes, after_bytes);
// plaintext_never_reaches_sqlite_wal_or_backup
assert!(!all_persisted_bytes.windows(sentinel.len()).any(|w| w == sentinel));
~~~

  Sentinels задаются для всех MySQL полей, Data/INI, службы и agent private key. Backup восстанавливается в другом tempdir с тем же vault identity и обоими способами unlock. Повторный init не меняет версии/ключи.
- [x] **Step 2: Проверить RED.** cargo test -p hmail-cleaner-center --test initialization; cargo test -p hmail-cleaner-center --test backup. Ожидается FAIL до storage/init.
- [x] **Step 3: Реализовать SQLite/init.** Схема: singleton account/settings/identity, encrypted_records, servers, immutable profile_versions, jobs, trust/enrollment metadata. WAL/FULL/FK/busy_timeout/пулы — из общего плана. Все команды одного каталога используют один OS lock. Не копировать живой .db без WAL: backup через согласованный SQLite snapshot (VACUUM INTO на отдельном соединении), включает версию/identity/bootstrap manifest. До миграции существующей схемы создаётся backup; неизвестная новая версия блокирует запуск. Скрытый ввод rpassword, recovery confirmation до короткой транзакции. После init нет RAM-сессии/DEK рабочего центра.
- [x] **Step 4: Проверить GREEN.** cargo test -p hmail-cleaner-center --test initialization --locked; cargo test -p hmail-cleaner-center --test storage --locked; cargo test -p hmail-cleaner-center --test backup --locked. cargo build -p hmail-cleaner --no-default-features --features center --locked. Проверить отсутствие mail/mysql/crossterm в feature-графе center-поставки; CLI secret-параметры не существуют.
- [x] **Step 5: Локальный коммит.** git commit -m "feat: initialize a portable encrypted center database".

## B3. HTTPS-вход, одна сессия, CSRF и бездействие

**Files**

- Create: crates/center/src/auth/{mod,types,session,admission,activity,limits,http}.rs.
- Create: crates/center/src/http/{mod,routes,errors,csrf}.rs; crates/center/src/app.rs.
- Create: crates/center/src/http/health.rs.
- Create: crates/center/src/identity/{mod,bootstrap,repository}.rs.
- Create: crates/center/tests/{sessions,csrf,login_limits}.rs.
- Modify: crates/center/src/account/repository.rs, crates/center/Cargo.toml, Cargo.lock.
- Modify: crates/protocol/src/browser/{auth,settings}.rs.

**Interfaces**

- Consumes: CenterStore, vault open_password, KdfGate, bootstrap TLS и версии account.
- Produces: SessionAuthority::admit(account_revision, Dek) -> Result<SessionGrant, AuthError>; authenticate(token) -> Result<SessionLease, AuthError>.
- Produces: revoke(generation) -> bool; touch(generation, monotonic_now) -> Result<(), AuthError>; серверный idle timer с injectable Clock.
- Produces: POST /api/v1/auth/login, /logout, /activity; GET /api/v1/auth/session; GET/PATCH /api/v1/settings/session.
- Produces: center::run(CenterConfig, Arc<Cancellation>) -> Result<(), CenterError>.
- Produces: GET /healthz без секрета/пути БД; ожидание login не делает исправный центр unhealthy.

- [x] **Step 1: Написать auth/CSRF tests.**

~~~rust
// successful_login_replaces_old_session; failed_login_preserves_it
assert!(authority.authenticate(old_token).is_err());
assert!(authority.authenticate(new_token).is_ok());
// stale_logout_and_old_timer_do_not_revoke_new_generation
assert!(!authority.revoke(old_generation));
assert!(authority.authenticate(new_token).is_ok());
// sse_and_background_reads_do_not_extend_idle
assert!(session_after_fifteen_minutes.is_none());
// missing_custom_header_or_foreign_origin_has_no_effect
assert_eq!(protected_effects, 0);
~~~

  Проверить 15/30/60/1440, изменение timeout от последней активности, idle без HTTP, рестарт без сессии, отозванный SSE. Cookie tokens malformed/oversized отклоняются до allocation. Без Fetch Metadata сохраняется X-HMail-Request:1 + JSON + origin/Host policy. GET/HEAD/OPTIONS ничего не меняют; account lockout навсегда не возникает.
- [x] **Step 2: Проверить RED.** cargo test -p hmail-cleaner-center --test sessions; cargo test -p hmail-cleaner-center --test csrf; cargo test -p hmail-cleaner-center --test login_limits.
- [x] **Step 3: Реализовать допуск.** Tower-cookies/CsrfLayer и атрибуты cookie строго из §5.7. DEK имеет владельца поколения отдельно от сессии. Общий credential barrier сериализует финальный admission и commit B7; KDF/сеть идут вне него. Token bucket ограничен одним состоянием, KDF — try acquire без очереди. На revoke закрыть browser SSE и generation-bound agent clients/streams; принятый agent job не отменять. TLS bootstrap работает до входа. Защищённые API-ответы используют Cache-Control:no-store. Открытая регистрация и состояние UI lock не создаются.
- [x] **Step 4: Проверить GREEN.** cargo test -p hmail-cleaner-center --test sessions --locked; cargo test -p hmail-cleaner-center --test csrf --locked; cargo test -p hmail-cleaner-center --test login_limits --locked. HTTPS fixture проверяет атрибуты cookie и Host/origin. Не заменять эти проверки HTTP dev-mode.
- [x] **Step 5: Локальный коммит.** git commit -m "feat: authenticate one center session with csrf and idle expiry".

## B4. Версии профилей и доверие конкретным агентам

**Files**

- Create: crates/center/src/profiles/{mod,types,repository,read,save,validate,http}.rs.
- Create: crates/center/src/agents/{mod,types,client,health,trust}.rs.
- Create: crates/center/src/identity/{issue,renew,revoke}.rs.
- Create: crates/center/tests/{profiles,identity,health}.rs.
- Modify: crates/protocol/src/{profiles,errors}.rs, crates/center/src/http/routes.rs.
- Modify: crates/protocol/src/browser/profiles.rs.

**Interfaces**

- Consumes: EncryptedRecord, SessionLease, immutable profile versions, mTLS adapter.
- Produces: ProfileService::read(ProfileId, &SessionLease) -> Result<EditableProfile, ProfileError>.
- Produces: save(ProfileId, expected_version: u64, ProfilePatch, &SessionLease) -> Result<ProfileVersion, ProfileError>.
- Produces: AgentClient::resolve(&ResolveRequest) -> Result<ResolvedCutoff, AgentError>; validate_profile(&MailProfile) -> Result<ValidationReport, AgentError>.
- Produces: GET /api/v1/servers; GET/PATCH /api/v1/servers/{id}/profile; POST /api/v1/servers/{id}/validate; POST /api/v1/servers/{id}/resolve.
- Produces: GET /api/v1/agents; POST /api/v1/agents/{id}/renew и /revoke; health содержит observed_at и свежесть, а не только saved status.

- [ ] **Step 1: Написать profile/identity tests.**

~~~rust
// edit_creates_immutable_version
assert_eq!(active_job.profile_version, old_version);
assert_eq!(current_profile.version, old_version + 1);
// secret_keep_is_explicit_and_read_never_returns_password
assert!(editable.password.is_none());
assert_eq!(saved_password, previous_password);
// linux_center_preserves_windows_paths
assert_eq!(agent_request.data_dir, r"C:\Mail Data\Данные");
// wrong_agent_never_receives_mail_profile
assert_eq!(received_profile_bytes, 0);
~~~

  Проверить отсутствие секретов в JSON/logs/ошибках, expected_version conflict, повреждение ciphertext, Loopback MySQL относится к агенту. Невалидные Data/INI/права — отдельная диагностика без полного scan/Stop.
- [ ] **Step 2: Проверить RED.** cargo test -p hmail-cleaner-center --test profiles; cargo test -p hmail-cleaner-center --test identity.
- [ ] **Step 3: Реализовать profiles/trust.** Защищённый профиль включает MailProfile и AgentCredentials, но агент получает только MailProfile. Password patch — Keep или Replace(value); строка-маска не является паролем. Профильные версии immutable; старые сохраняются для job reconciliation. Проверка Windows-путей/INI принадлежит агенту. Центр выпускает CA/leaf по политике основного плана, проверяет CSR signature, pin identity в handshake. Выпуск, продление и отзыв требуют текущего допуска; после отзыва пересоздаются verifier/пулы.
- [ ] **Step 4: Проверить GREEN.** cargo test -p hmail-cleaner-center --test profiles --locked; cargo test -p hmail-cleaner-center --test identity --locked; cargo test -p hmail-cleaner-center --test health --locked. Ожидается PASS для двух независимых fixtures и truthful stale/offline state.
- [ ] **Step 5: Локальный коммит.** git commit -m "feat: manage encrypted server profiles and agent trust".

## B5. Команды, история, согласование и SSE

**Files**

- Create: crates/center/src/jobs/{mod,types,repository,submit,reconcile,cancel,events,http}.rs.
- Create: crates/center/src/history/{mod,query,http}.rs.
- Create: crates/center/tests/{dispatch,reconciliation,event_limits,history}.rs.
- Create: crates/center/tests/fixtures/{mod,agents}.rs — намеренный общий test-only entrypoint для backend tests и browser_fixture в C2.
- Modify: crates/center/src/agents/client.rs, crates/center/src/http/routes.rs, crates/protocol/src/{jobs,events}.rs.
- Modify: crates/protocol/src/browser/jobs.rs.

**Interfaces**

- Consumes: AgentClient, SessionLease, ProfileVersion, JobCommand/JobSnapshot.
- Produces: submit(&SessionLease, SubmitRequest) -> Result<JobView, SubmitError>; SubmitRequest фиксирует server_id, profile_version, preview_job_id для clean и подтверждённые параметры.
- Produces: reconcile(JobId, &SessionLease) -> Result<JobView, ReconcileError>; cancel(JobId, &SessionLease) -> Result<JobView, CancelError>.
- Produces: POST /api/v1/servers/{id}/jobs; GET /api/v1/jobs/{id}; POST /api/v1/jobs/{id}/cancel; POST /api/v1/jobs/{id}/reconcile.
- Produces: GET /api/v1/servers/{id}/history?cursor=...&limit=...; GET /api/v1/events?server_id=...; browser snapshot + события с revision.

- [ ] **Step 1: Написать dispatch/reconnect tests с двумя fake agents и SQLite.**

~~~rust
// lost_accept_response_does_not_duplicate_cleanup
assert_eq!(job.dispatch, DispatchState::Unknown);
assert_eq!(clean_requests_for_new_ids, 0);
// one_404_does_not_release_slot_before_late_post_is_resolved
assert!(next_heavy_job_result.is_busy());
// center_restart_reconciles_only
assert_eq!(put_job_calls_after_restart, 0);
// target_is_per_request_not_global_selection
assert_eq!(agent_a.job.server_id, server_a);
assert_eq!(agent_b.received_commands, 0);
// persisted_terminal_survives_missed_sse
assert_eq!(history_result.phase, JobPhase::Completed);
// publisher_restart_does_not_hide_interrupted_snapshot
assert_ne!(new_snapshot.stream_epoch, old_snapshot.stream_epoch);
assert_eq!(new_snapshot.phase, JobPhase::Interrupted);
~~~

  Проверить изменённый cutoff/scan/service_policy против preview, отрицательный/неуспешный preview, версии профиля; login revoke во время submit; медленный subscriber/overflow → bounded queue + resync; counter events не продлевают сессию. History сохраняет null remaining_old_files и различает selected/removed/free disk.
- [ ] **Step 2: Проверить RED.** cargo test -p hmail-cleaner-center --test dispatch; cargo test -p hmail-cleaner-center --test reconciliation; cargo test -p hmail-cleaner-center --test event_limits.
- [ ] **Step 3: Реализовать последовательность.** До сетевого submit сохранить intent и резервирование в SQLite. Стабильный job_id не заменять при timeout; повтор browser submit с тем же ID возвращает тот же запуск. После рестарта/входа только lookup. GET 404 оставляет unknown; явный cancel использует долговечную tombstone A2. Слот освобождается после согласованного terminal и наблюдаемого состояния агента/службы, не по обрыву сети. Сохранять terminal result до SSE. Переподключение/lag требует снимок; raw core errors не публиковать. Клиент/подписки привязать к поколению сессии, освобождать на logout.
- [ ] **Step 4: Проверить GREEN.** cargo test -p hmail-cleaner-center --test dispatch --locked; cargo test -p hmail-cleaner-center --test reconciliation --locked; cargo test -p hmail-cleaner-center --test event_limits --locked; cargo test -p hmail-cleaner-center --test history --locked. Проверить два tabs/server IDs, выход при принятом задании и agent restart → Interrupted без resubmit.
- [ ] **Step 5: Локальный коммит.** git commit -m "feat: orchestrate manual jobs with durable reconciliation and sse".

## B6. SSH-установка и ручное обновление агента

**Files**

- Create: crates/center/src/enrollment/{mod,types,probe,ssh,transfer,install,reconcile,update,http}.rs.
- Create: crates/center/tests/{ssh_install,ssh_failures,agent_update}.rs.
- Create: crates/center/tests/fixtures/ssh/mod.rs; docs/center-deployment.md.
- Modify: crates/center/src/http/routes.rs, crates/center/src/agents/trust.rs, crates/center/Cargo.toml.
- Modify: crates/protocol/src/browser/enrollment.rs.

**Interfaces**

- Consumes: trusted agent manifest A5, local native installer, certificate issuer B4.
- Produces: probe(SshTarget) -> Result<HostKeyChallenge, EnrollmentError> без передачи credential.
- Produces: install(&SessionLease, InstallRequest) -> Result<EnrollmentView, EnrollmentError>; InstallRequest содержит подтверждённый fingerprint, credentials, ожидаемый agent_id и package version.
- Produces: update(&SessionLease, AgentId, PackageVersion) -> Result<EnrollmentView, EnrollmentError>.
- Produces: POST /api/v1/enrollment/probe, /install; GET /api/v1/enrollment/{id}; POST /api/v1/agents/{id}/update.

- [ ] **Step 1: Написать SSH/install tests с изолированным SSH fixture.**

~~~rust
// host_key_changed_blocks_auth_and_transfer
assert_eq!(auth_attempts, 0);
assert_eq!(uploaded_bytes, 0);
// install_has_no_mail_side_effects
assert_eq!(mail_stop_calls, 0);
assert_eq!(cleanup_calls, 0);
// package_hash_mismatch_never_installs_service
assert_eq!(service_install_calls, 0);
// busy_agent_update_is_refused
assert_eq!(replaced_executables, 0);
~~~

  Проверить quoting для пробелов/кириллицы и входа с shell metacharacters; revoked session; partial upload, disconnect после создания SCM и потерянный финальный ответ. Никакой secret не появляется в persisted checkpoints/logs.
- [ ] **Step 2: Проверить RED.** cargo test -p hmail-cleaner-center --test ssh_install; cargo test -p hmail-cleaner-center --test agent_update.
- [ ] **Step 3: Реализовать bounded SSH workflow.** Russh + SFTP, потоковый пакет, один install, проверка manifest и SHA-256 на ВМ. Удалённая команда использует фиксированный bootstrap и валидированные аргументы/входной JSON, без подстановки MySQL/путей почты в shell. Agent создаёт key/CSR, центр выпускает leaf, агент запускается idle, затем проверяется mTLS и отдельно профиль. Password/key ОС не сохранять по умолчанию; на logout прекращать новые привилегированные шаги и очищать секреты. Сохранять безопасный checkpoint; частичный исход показывать как unknown/requires-attention, не выполнять слепую повторную регистрацию. Обновление только явное, idle, с сохранением identity/data и предыдущего EXE; unattended updates не добавлять.
- [ ] **Step 4: Проверить GREEN.** cargo test -p hmail-cleaner-center --test ssh_install --locked; cargo test -p hmail-cleaner-center --test ssh_failures --locked; cargo test -p hmail-cleaner-center --test agent_update --locked. Затем native Server 2016 fixture по инструкции, с явным NOT RUN если среды нет.
- [ ] **Step 5: Локальный коммит.** git commit -m "feat: install and update idle agents over verified ssh".

## B7. Смена пароля, recovery key и локальное восстановление

**Files**

- Create: crates/center/src/account/{change_password,recovery,restore,http}.rs.
- Create: crates/center/tests/{credential_changes,recovery,races}.rs.
- Modify: crates/center/src/account/repository.rs, crates/center/src/auth/admission.rs, crates/center/src/http/routes.rs, crates/cli/src/args.rs.
- Modify: crates/protocol/src/browser/auth.rs.
- Create: docs/recovery.md.

**Interfaces**

- Consumes: vault wraps, account_revision, credential barrier B3, exclusive DataDirectoryLease B2.
- Produces: change_password(&SessionLease, ChangePasswordInput) -> Result<(), AccountError>.
- Produces: prepare_recovery(&SessionLease, &SecretText) -> Result<RecoveryCandidate, AccountError>; confirm_recovery(&SessionLease, CandidateId, &RecoveryKey) -> Result<(), AccountError>.
- Produces: restore(&CenterStore, &RecoveryKey, &mut impl SecretPrompt) -> Result<RecoveryReceipt, AccountError>; команда center recover при остановленном центре.
- Produces: POST /api/v1/account/password; POST /api/v1/account/recovery/prepare; POST /api/v1/account/recovery/confirm.

- [ ] **Step 1: Написать credential/recovery race tests.**

~~~rust
// password_change_rewraps_only
assert_eq!(profile_ciphertexts_before, profile_ciphertexts_after);
assert!(old_password_result.is_err());
assert!(existing_recovery_result.is_ok());
// recovery_candidate_is_not_active_before_confirmation
assert!(old_recovery_result.is_ok());
assert!(candidate_recovery_result.is_err());
// stale_login_cannot_admit_after_credential_commit
assert!(stale_admission.is_err());
// rollback_keeps_previous_unlock_methods
assert!(previous_password_result.is_ok());
assert!(previous_recovery_result.is_ok());
~~~

  Проверить candidate TTL 10 минут, logout/restart cancellation, duplicate confirm, потерю ответа после commit, corrupt wrap, recovery с запущенным центром → отказ без изменения БД. Локальный recover заменяет password+recovery, сохраняет DEK и требует обычного входа.
- [ ] **Step 2: Проверить RED.** cargo test -p hmail-cleaner-center --test credential_changes; cargo test -p hmail-cleaner-center --test races; cargo test -p hmail-cleaner-center --test recovery.
- [ ] **Step 3: Реализовать атомарную смену.** Кандидат хранит только необходимый временный secret, связан с session/account revision; выдаётся один раз, подтверждается до commit. Все KDF идут последовательно через общий gate. Короткая транзакция CAS фиксирует verifier/wraps/revision, credential barrier защищает final admission/отзыв. После успешной смены отозвать сессию и очистить её DEK; profiles не перешифровывать. Не заявлять отзыв старых backup или DEK. Recover имеет тот же OS lock и скрытый ввод, что init.
- [ ] **Step 4: Проверить GREEN.** cargo test -p hmail-cleaner-center --test credential_changes --locked; cargo test -p hmail-cleaner-center --test recovery --locked; cargo test -p hmail-cleaner-center --test races --locked. Проверить БД/WAL/backup на sentinels и отсутствие recovery/password в argv/logs. Ожидается PASS.
- [ ] **Step 5: Локальный коммит.** git commit -m "feat: change and recover center credentials atomically".

## Готовность части B

Полный backend сценарий выполняется с двумя fixture-агентами, без UI и без
рабочей почты. Ошибки/сбои отличаются от отсутствия данных. Профили, auth,
job admission, SSH и crypto имеют отдельные владельцы; Axum routes только
разбирают/авторизуют запрос и вызывают соответствующую операцию.
