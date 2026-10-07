# hMailServer Cleaner Operations Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Дополнить существующий cleaner полной дочисткой, подтверждёнными счётчиками, env/меню/журналом и настоящей службой Windows с управлением остановкой почты.

**Architecture:** Переносимый core владеет критериями и последовательностью очистки; MySQL, filesystem и Windows реализуют внешние контракты. CLI собирает разовый сценарий, а меню и служба используют этот же сценарий. Работы идут тремя связанными этапами: Tasks 1–4 — очистка, Tasks 5–7 — способы разового запуска, Tasks 8–9 — служба и поставка.

**Tech Stack:** Текущий Rust workspace, Rust 1.98.1, SQLx 0.7/MySQL, Tokio 1, Chrono 0.4, UUID 1, Serde 1, Clap 4, dotenvy 0.15, windows-service 0.8; существующий tempfile для тестов. Добавить только явно необходимые Tokio features time/sync. SQLite/Turso и новые продуктовые библиотеки не требуются.

**Spec:** [Согласованный дизайн](../specs/2026-10-07-hmail-cleaner-operations-design.md), дополняющий [базовый дизайн](../specs/2026-10-07-hmail-retention-design.md).

## Global Constraints

- Ветка `update`, native inline исполнение самим агентом; никаких push/deploy/регистрации на рабочем сервере.
- Windows Server 2016 / 1607 x64; существующая MySQL 5.7.21 без Docker. Docker/Colima используются только для изолированных тестов и cross-build на Mac.
- Сохранить mise Rust 1.98.1, Cargo.lock, MySQL fixture 5.7.21/digest и ручной workflow_dispatch; зависимости менять только по потребности этого плана.
- Возраст связанного письма — `messagecreatetime < cutoff`; даты SQL локальные. Возраст файла без записи — LastWriteTime, сравниваемый с однозначной абсолютной границей.
- Файл → подтверждённое отсутствие → конкретные строки metadata/recipients/messages в одной транзакции → проверка → пустые затронутые каталоги.
- Порция по умолчанию 1000, диапазон 1–10000; работники 2, диапазон 1–8; прогресс не чаще 250 мс.
- До трёх попыток при временной Windows-блокировке/ожидании удаления; паузы 100 и 300 мс. Ошибки прав/пути/I/O не превращать в NotFound и не исправлять изменением ACL.
- `--scan fast|full`, default fast. Служба запускает fast; full выполняется отдельно. Не использовать индекс только старых/доставленных сообщений для определения отсутствия ссылки.
- Полный набор ссылок включает все hm_messages. Предел 1 000 000 уникальных GUID; неполнота/превышение блокируют удаление файлов без записи.
- Поточный обход допустимого account-хранилища, два прохода: предварительная проверка доступности, затем удаление. Data, root queue, #Public, links/reparse points и неизвестные имена защищены.
- Старые metadata/recipients без parent удаляются только в full при STOPPED. Аккаунты и IMAP-папки не удаляются. Нет ALTER/OPTIMIZE, карантина или постоянного файлового каталога.
- CLI > системная среда > явный env-файл > defaults; срок и пути обязательны. Пара периода выбирается целиком из одного источника. Старые .env не читать и не копировать.
- Silence требует локального журнала и несовместим с menu/--json. Журнал NDJSON: 10 MiB на файл, до трёх архивов; секреты не писать.
- Служба hMailCleaner: обязательные HMAIL_RUN_AT=HH:MM и абсолютный HMAIL_LOG_FILE, ежедневный будущий запуск, без немедленной очистки при старте и без очереди пропущенных запусков.
- STOPPED precondition сохраняется; управление остановкой только в service либо явном --manage-service. Restore только если этот проход отправил Stop; Stopped до прохода сохраняется.
- Таймаут Stop 60 секунд, отдельный бюджет restore 60 секунд. Ошибка restore сохраняет исходную ошибку и запрещает следующие проходы службы.
- Ctrl+C/SCM Stop/Shutdown завершают начатую порцию и попытку restore. Принудительный kill/power loss не имеют обещания автоматического восстановления.
- Legacy Tauri/Vue и его конфигурация вне изменений. Не печатать/переносить старые реальные секреты.

## Review Focus

- Удаление запрошено, но stat возвращает AccessDenied: строка БД остаётся и счётчик подтверждённого удаления не увеличивается — Task 1.
- COMMIT прошёл, ответ потерян: исторические остатки не объявляются удалёнными, следующие порции останавливаются, повтор согласует состояние — Tasks 2/4.
- Повреждённая файловая ссылка БД, превышение лимита или недоступная папка: нельзя удалить файл по неполному набору — Tasks 2/3/4.
- Stop принят, но таймаут истёк в StopPending: всё равно выполнить ограниченную попытку restore; ошибка не должна выглядеть успешной очисткой — Task 7.
- Повторяющийся/несуществующий местный час, рестарт службы и долгий проход: не выполнить двойную или немедленную очистку — Task 8.

---

## Ownership and Shared Contracts

- `core/cleanup`: прежний быстрый use case и общие результаты; точные ID/outcome-проверки сохранить.
- `core/maintenance`: новый полный use case и порты дополнительных фаз. Существующий Repository/FileStore не расширять методами, ненужными прежним тестам fast.
- `adapters/mysql/orphans`: SQL исторических остатков; `adapters/mysql/references`: поток ссылок; никакой файловой политики/console I/O.
- `adapters/filesystem`: наблюдения, доступность дерева, ограниченный iterator и операции удаления с проверкой.
- `adapters/offline`: query/stop/start hMailServer; `adapters/service`: Windows SCM host, не расписание и не SQL.
- `cli/app`: композиция/preflight/разовый запуск; `cli/config`: env и typed settings; `cli/menu`, `cli/output`, `cli/service`: отдельные владельцы интерфейса, вывода и расписания.

Task 1 вводит `DeletedRows { messages, metadata, recipients: u64 }`,
`DeleteResult::Committed(DeletedRows)` вместо unit-варианта и
`DeletionResult::Removed { size_bytes: u64 }`. CommitUncertain сохраняет reason.
Summary сохраняет старые JSON-ключи и добавляет `removed_bytes`, `deleted_metadata`,
`deleted_recipients`, `orphan_metadata`, `orphan_recipients`, `scanned_files`,
`selected_orphan_files`, `removed_orphan_files: u64`. Старый removed_files — все
подтверждённо удалённые файлы; removed_orphan_files — его подмножество.

Task 2 определяет в публичном `core::maintenance`:

```rust
enum OrphanTable { Metadata, Recipients }
struct OrphanRow { id: i64, parent_message_id: i64 }
struct OrphanSelection { table: OrphanTable, upper_id: i64 }
struct OrphanBatch { last_id: i64, rows: Vec<OrphanRow> }
type FileReferences = std::collections::HashSet<uuid::Uuid>;
trait FullRepository: Repository {
    fn orphan_upper_id(&mut self, table: OrphanTable) -> impl Future<Output = PortResult<i64>> + Send;
    fn orphan_batch(&mut self, selection: OrphanSelection, cursor: i64, limit: u32) -> impl Future<Output = PortResult<OrphanBatch>> + Send;
    fn delete_orphans(&mut self, table: OrphanTable, rows: &[OrphanRow]) -> impl Future<Output = PortResult<DeleteResult>> + Send;
    fn verify_orphans_deleted(&mut self, table: OrphanTable, ids: &[i64]) -> impl Future<Output = PortResult<()>> + Send;
    fn file_references(&mut self, limit: usize) -> impl Future<Output = PortResult<FileReferences>> + Send;
}
```

Task 3 добавляет `FileObservation { path: PathBuf, guid: Uuid, size_bytes: u64,
modified_at: SystemTime }`, `ScanStats { examined_files: u64 }`,
`ScanBatch { examined_files: u64, files: Vec<FileObservation>, finished: bool }`,
`OrphanFileOutcome { observation: FileObservation, result: DeletionResult }`:

```rust
trait FileScan: Send {
    fn next_batch(&mut self, limit: u32) -> impl Future<Output = PortResult<ScanBatch>> + Send;
    fn close(self) -> impl Future<Output = PortResult<()>> + Send;
}
trait FullFileStore: FileStore {
    type Scan: FileScan;
    fn validate_tree(&self) -> impl Future<Output = PortResult<ScanStats>> + Send;
    fn open_scan(&self) -> impl Future<Output = PortResult<Self::Scan>> + Send;
    fn delete_orphans(&self, files: Vec<FileObservation>, workers: u8) -> impl Future<Output = PortResult<Vec<OrphanFileOutcome>>> + Send;
}
```

`examined_files` считает обычные файлы в допустимой account-структуре, включая
свежие/связанные/неподходящие имена. ScanBatch с finished=false обязан продвинуться
(examined_files > 0 либо не пуст files), иначе ошибка контракта.
References возвращаются только после полного успешного чтения, не частью набора.

---

### Task 1: Подтверждение файлового удаления и точные счётчики

**Files:** Modify `crates/core/src/cleanup/{types,file_results,workflow,mod}.rs`;
`crates/adapters/src/filesystem/{delete,validation}.rs` и
`crates/adapters/src/mysql/delete.rs`. Create
`crates/adapters/src/filesystem/removal.rs` для одной операции и
`crates/adapters/src/filesystem/retry.rs` для классификации/повторов.
Test `crates/core/tests/cleanup.rs`, `crates/adapters/tests/{filesystem,mysql}.rs`.

**Interfaces:** Produces результаты/Summary из Shared Contracts.
`removal::remove(path: &Path) -> DeletionResult` удаляет и подтверждает один файл.
Проверка observation появится в Task 3 как отдельная операция orphan owner,
без optional placeholder-параметра и без пустого scaffold.
`retry` владеет только попытками/задержками и принимает операцию через closure;
классификатор Windows проверяет реальные OS error codes, не текст ошибки.

- [ ] Написать RED tests на финальный результат и счётчики:
  `removed_file_is_verified_and_counted_by_size` (7 bytes → files=1, bytes=7),
  `absent_file_contributes_no_bytes`, `verification_error_preserves_message`,
  `temporary_lock_has_three_attempts`, `permanent_denial_has_one_attempt`.
  Системные варианты тестировать на Windows реальным locked handle;
  последовательности ошибок/задержки — в тестах чистого retry-механизма.
  Ключевые assertions этих отдельных сценариев:
  ```rust
  assert_eq!((summary.removed_files, summary.removed_bytes), (1, 7));
  assert_eq!(absent_summary.removed_bytes, 0);
  assert_eq!(failed_verification_summary.deleted_messages, 0);
  assert_eq!(temporary_attempts, 3);
  assert_eq!(retry_delays_ms, [100, 300]);
  assert_eq!(denied_attempts, 1);
  ```
- [ ] RED: `mise exec -- cargo test --workspace --locked`; новые переносимые
  проверки должны упасть по недостающему поведению, не по неверной fixture.
- [ ] Реализовать подтверждение отсутствия и размеры; обновить только затронутые
  варианты и exhaustiveness. MySQL delete возвращает rows_affected по трём
  таблицам; core увеличивает SQL-счётчики после verify, не сразу после COMMIT.
- [ ] GREEN: `mise run check`; затем `mise run test:db` на owned fixture.
  Проверить ошибки/потерю COMMIT: bytes за подтверждённые файлы остаются,
  SQL-счётчики не увеличиваются без подтверждения и verification.
- [ ] Commit `feat: verify file removal and report confirmed cleanup counts`.

### Task 2: Порционная дочистка SQL и полный набор ссылок

**Files:** Create `crates/core/src/maintenance/{mod,ports,types,limits}.rs`;
`crates/adapters/src/mysql/orphans/{mod,selection,deletion}.rs` и
`crates/adapters/src/mysql/references.rs`. Modify `crates/core/src/lib.rs`,
`crates/core/src/paths/mod.rs`, `crates/adapters/src/mysql/{mod,schema}.rs`.
Test `crates/core/tests/paths.rs`, `crates/adapters/tests/mysql.rs`;
не менять присланную schema ради прохождения теста.

**Interfaces:** Produces FullRepository и типы из Shared Contracts;
`MAX_REFERENCES: usize = 1_000_000` в maintenance/limits;
`paths::message_guid(filename: &str) -> Result<Uuid, PathError>` валидирует
braced GUID + eml basename для обычной/абсолютной формы. Существующий resolver
переиспользует принадлежащую paths проверку GUID.

- [ ] RED tests `historical_orphans_are_deleted_and_live_rows_preserved`,
  `orphan_cursor_advances_by_primary_id`, `orphan_parent_reappearance_is_retained`,
  `orphan_commit_uncertain_stops_counting`, `all_message_states_protect_guids`,
  `malformed_reference_rejects_complete_set`, `reference_limit_is_enforced`.
  Для лимита параметр limit позволяет тест с 2 IDs: 2 разных GUID проходят,
  третий даёт Err; повтор одного GUID не расходует дополнительный элемент.
  В fixture создать по одной живой и исторической строке каждой таблицы;
  remaining counts получают SELECT из настоящей тестовой БД:
  ```rust
  assert_eq!((remaining_metadata, remaining_recipients), (1, 1));
  assert_eq!((confirmed_metadata, confirmed_recipients), (1, 1));
  assert!(matches!(uncertain, DeleteResult::CommitUncertain { .. }));
  assert!(malformed_references.is_err());
  assert!(over_limit_references.is_err());
  assert_eq!(two_references.len(), 2);
  ```
- [ ] RED: portable `cargo test --workspace --locked` через mise; SQL tests —
  `mise run test:db`. Зафиксировать ожидаемый RED каждой новой SQL-проверки.
- [ ] Реализовать fixed table descriptors, watermark/keyset SELECT NOT EXISTS,
  транзакционные DELETE только ID с повторной проверкой parent и verify exact IDs.
  Изменившиеся условия не считать удалением; сохранять original/rollback/close errors.
  В references читать hm_messages порциями по primary ID, а не fetch_all всей таблицы.
- [ ] GREEN: `mise run check` и `mise run test:db`. Реальный EXPLAIN подтверждает
  primary range для дочерней таблицы и lookup parent по PK; без OFFSET/filesort.
  После неопределённого COMMIT повтор должен удалить только оставшиеся строки.
- [ ] Commit `feat: reconcile historical database orphans in bounded batches`.

### Task 3: Поточный account-обход и удаление файловых остатков

**Files:** Extend `crates/core/src/maintenance/{ports,types}.rs`;
create `crates/adapters/src/filesystem/scan/{mod,layout,session}.rs` и
`crates/adapters/src/filesystem/orphans.rs`; modify
`crates/adapters/src/filesystem/{mod,removal}.rs`. Test `crates/adapters/tests/filesystem.rs`.

**Interfaces:** Produces FullFileStore/FileScan и наблюдения из Shared Contracts;
`orphans::remove_observed(observation: &FileObservation) -> DeletionResult`
повторно проверяет наблюдение и использует removal из Task 1.
LocalFiles::Scan владеет iterator и ресурсами до `close(self)`.
Layout проверяет Data/domain/account/XX/GUID по правилам paths;
не собирает все дочерние paths в Vec и не следует junction/symlink.

- [ ] RED tests `scan_yields_bounded_batches`, `scan_counts_fresh_and_referenced_files`,
  `root_queue_and_public_are_excluded`, `bucket_mismatch_is_preserved`,
  `changed_observation_is_not_deleted`, `inaccessible_tree_fails_precheck`,
  `scan_resources_are_closed_on_cancellation`. Native Windows проверить junction
  и deny-access, не подменять PermissionDenied на portable root-тесте.
  Для batch limit=2 и настоящих файлов в private temp root:
  ```rust
  assert!(batch.files.len() <= 2);
  assert!(protected_public_path.exists());
  assert!(protected_queue_path.exists());
  assert!(changed_path.exists());
  assert!(matches!(changed_outcome.result, DeletionResult::Failed(_)));
  assert!(inaccessible_precheck.is_err());
  ```
- [ ] RED: `mise exec -- cargo test -p hmail-cleaner-adapters --test filesystem --locked`.
  Assertions: batch files.len <= limit; чужие/свежие файлы сохранены; результат
  changed observation — Failed и файл существует; precheck error → delete calls=0.
- [ ] Реализовать предварительный read-only проход и owned blocking session
  второго прохода с порционным next_batch. Согласовать close с текущим
  blocking шагом; не оставлять detached worker после возврата.
  Перед orphan removal повторно сверить размер/mtime, затем использовать Task 1.
- [ ] GREEN: `mise run check`; Windows test binaries cross-compile и выполнить
  native проверки на доступном Windows-стенде отдельно от Mac-результата.
- [ ] Commit `feat: scan account storage and remove verified file orphans`.

### Task 4: Полный use case, preview и согласование результатов

**Files:** Create `crates/core/src/maintenance/{workflow,database,files}.rs`;
modify `crates/core/src/maintenance/mod.rs`, `crates/core/src/cleanup/progress.rs`.
Test `crates/core/tests/maintenance.rs`. Modify `crates/cli/src/args.rs`,
`crates/cli/src/config/{types,mod}.rs`, `crates/cli/src/{commands,main}.rs` лишь
для подключения --scan/full (env/menu далее); update `docs/cleanup-cli.md`.

**Interfaces:** `enum ScanMode { Fast, Full }` и
`enum ScanOptions { Fast, Full { file_cutoff: SystemTime } }`;
`maintenance::clean(deps: Dependencies<'_,R,F,G>, options: &RunOptions,
scan: ScanOptions, cancellation: &Cancellation, progress: P) -> Result<Summary,CleanupError>`;
`maintenance::preview(deps: Dependencies<'_,R,F,G>, options: &RunOptions,
scan: ScanOptions, cancellation: &Cancellation, progress: P) -> Result<Summary,CleanupError>`;
оба async fn. R: FullRepository, F: FullFileStore, G: OfflineGuard;
P: Fn(Progress) + Send + Sync. Full preview использует OfflineGuard.
Fast делегирует прежнему use case, а не копирует его тело. Расширить Stage/Failure
для SQL orphan table/row и orphan file/path. Config переводит local cutoff в
SystemTime только для Full, rejecting неоднозначное/несуществующее время.

- [ ] RED tests `fast_never_opens_scan`, `full_orders_sql_then_references_then_files`,
  `failed_reference_load_prevents_orphan_file_removal`, `failed_tree_precheck_prevents_removal`,
  `new_queue_and_public_references_protect_files`, `full_preview_never_deletes`,
  `commit_uncertain_prevents_later_phases`, `cancel_finishes_current_orphan_batch`,
  `scan_close_failure_preserves_primary_error`, `unexpected_orphan_outcome_blocks_pruning`.
  Наблюдать сохранённые данные fake портов и trace фаз, не только число вызовов:
  ```rust
  assert_eq!((full.orphan_metadata, full.orphan_recipients), (1, 1));
  assert_eq!((full.selected_orphan_files, full.removed_orphan_files), (1, 1));
  assert_eq!(preview_summary.removed_files, 0);
  assert!(protected_fresh_file.exists());
  assert!(failed_scan_candidate.exists());
  assert!(!trace_after_uncertain.contains(&"references"));
  ```
- [ ] RED: `mise exec -- cargo test -p hmail-cleaner-core --test maintenance --locked`;
  CLI full date tests pin ambiguous/absent timezone conversion with injected zone.
- [ ] Реализовать явный порядок фаз; outcome-path/count проверять до зачёта
  и prune. Сохранять байтовые/файловые итоги предыдущих фаз и дополнительные
  SQL counts; ошибка записи/scan не превращается в пустую успешную коллекцию.
  Preview использует текущие ссылки, без симуляции будущего удаления.
- [ ] GREEN: `mise run check` + `mise run test:db`; end-to-end owned fixture:
  old/live message, missing EML, historical metadata/recipient, old unreferenced
  file, fresh file. Assert all surviving rows/files and exact counters.
- [ ] Commit `feat: expose full cleanup and read-only orphan previews`.

### Task 5: Типизированная env-конфигурация с прежними CLI overrides

**Files:** Modify `crates/cli/src/args.rs`, `crates/cli/src/args/retention.rs`,
`crates/cli/src/config/{mod,types,tests}.rs`; create
`crates/cli/src/config/{sources,selection}.rs`. Modify `examples/cleaner.env.example`,
`docs/cleanup-cli.md`. Read existing config first; dotenvy остаётся loader.

**Interfaces:** `load_settings(args: &Args) -> Result<Settings, ConfigError>`;
`ServicePolicy::{RequireStopped, Manage}` и
`RunIntent { mode: Mode, scan: ScanMode, service_policy: ServicePolicy }`;
`Settings::for_run(&self, intent: RunIntent, now: DateTime<Local>) -> Result<Config,ConfigError>`.
Settings хранит validated DatabaseConfig, paths, Retention, scan, batch, workers,
service_name и log_file; Config содержит прежний RunOptions + ScanOptions +
явную ServicePolicy. `Settings::with_retention(&self, retention: Retention) -> Settings`
создаёт настройки текущей menu-сессии без изменения исходного файла.
`config::parse_age(&str)` и `parse_before(&str)` — crate-public entrypoints
прежних валидаторов для Clap и menu; error types остаются безопасными строками.
Имена env и parsing живут только в config.
Команда/--env-file/--silence/--json остаются аргументами запуска, не скрытым env clean.

- [ ] RED tests `env_supplies_paths_age_and_limits`, `cli_overrides_system_and_file`,
  `system_overrides_file`, `cli_age_replaces_env_before`, `same_source_retention_pair_rejected`,
  `missing_age_or_path_is_error`, `invalid_workers_or_batch_is_error`,
  `duplicate_recognized_env_key_is_error`, `legacy_env_not_loaded`,
  `diagnostics_never_contain_url_secret`. Проверять источники через injected
  read-only map, без глобальной параллельной мутации process environment.
  Override fixture: file workers=2/batch=1000/BEFORE=2020-01-01,
  system workers=3, CLI workers=4/batch=500/OLDER_THAN=30d:
  ```rust
  assert_eq!(config.options.workers, 4);
  assert_eq!(config.options.batch_size.get(), 500);
  assert_eq!(config.options.cutoff, now.naive_local() - chrono::Duration::days(30));
  assert!(!format!("{error:?} {error}").contains("secret-marker"));
  assert!(same_source_pair.is_err());
  ```
- [ ] RED: `mise exec -- cargo test -p hmail-cleaner --locked`.
- [ ] Реализовать optional raw Clap values и один owner defaults/validators;
  reuse Retention parse и диапазоны. Обновить пример одиночными кавычками для
  Windows paths. Непустые обязательные строки валидируются, пустое значение
  не выбирает опасный default. System env читается один раз владельцем config.
- [ ] GREEN: `mise run check`; env-only preview configuration reaches valid
  Config, не обращаясь к working DB. for_run через месяц пересчитывает age cutoff.
- [ ] Commit `feat: support validated cleanup settings from explicit env files`.

### Task 6: Общий вывод, локальный журнал, silence и текстовое меню

**Files:** Move `crates/cli/src/output.rs` в
`crates/cli/src/output/{mod,event,console,journal}.rs`; create
`crates/cli/src/menu/{mod,input,period}.rs`, `crates/cli/src/app/mod.rs`.
Modify `crates/cli/src/{args,main,commands}.rs`, `docs/cleanup-cli.md`.
Test CLI-local output/menu modules and subprocesses in `crates/cli/tests/launch.rs`.

**Interfaces:** `Output::open(policy: OutputPolicy, log_file: Option<&Path>,
cancel: Arc<Cancellation>) -> io::Result<Output>`;
`OutputPolicy::{Text, Json, Silent}`; server/progress/finish/failed сохраняются.
`menu::next_action(reader: &mut impl BufRead, writer: &mut impl Write,
settings: &Settings) -> Result<MenuAction, MenuError>`;
MenuAction::{Preview(ScanMode), Clean{scan: ScanMode, service_policy: ServicePolicy},
SetRetention(Retention), Exit}. Config строится for_run в момент подтверждённого
запуска, не вычислять старую дату в menu.
`app::run_once(config: &Config, output: &Output, cancel: &Cancellation) -> Report`
извлекается из main::run и остаётся единственным owning composition point.

- [ ] RED tests `silent_requires_log`, `silent_rejects_json_and_menu`,
  `journal_writes_result_without_url_secrets`, `rotation_keeps_three_archives`,
  `write_failure_requests_cancellation`, `menu_reprompts_invalid_choice`,
  `menu_eof_exits_without_cleanup`, `no_terminal_no_implicit_menu`,
  `period_changes_session_without_rewriting_env`, `menu_clean_requires_confirmation`.
  Assertions на captured I/O, bytes private env и реальные rotated файлы:
  ```rust
  assert!(silent_stdout.is_empty());
  assert_eq!(archive_count, 3);
  assert!(log_text.contains("\"type\":\"result\""));
  assert!(!log_text.contains("secret-marker"));
  assert!(cancel.is_requested());
  assert_eq!(env_after_menu, env_before_menu);
  assert!(matches!(eof_action, MenuAction::Exit));
  ```
- [ ] RED: `mise exec -- cargo test -p hmail-cleaner --locked`.
- [ ] Реализовать typed actions и BufRead/Write меню без нового TUI framework;
  лог открывается до app/preflight. Ротация 10 MiB/3 архива без раскрытия секретов;
  failed output останавливает следующую порцию и сохраняет nonzero exit.
  До открытия sink ошибки CLI/config доступны на stderr с кодом 2; внутри
  службы ошибка запуска отражается в SCM, не требует консоли.
- [ ] GREEN: `mise run check`; subprocess tests используют private temp env,
  fake DB URL и не запускают clean против живых данных. Assert exit и отсутствие
  нежелательных prompts/stdout, итоговый NDJSON и сохранённый env-файл.
- [ ] Commit `feat: add an interactive CLI and silent rotating cleanup logs`.

### Task 7: Управляемая остановка hMailServer и гарантированная попытка restore

**Files:** Create `crates/core/src/maintenance/mail_service.rs` для владения ресурсом;
`crates/adapters/src/offline/control.rs`; move `crates/adapters/src/offline/windows.rs`
в `crates/adapters/src/offline/windows/{mod,query,control,wait}.rs`.
Сохранить ensure_stopped, modify `crates/adapters/src/offline/mod.rs`,
`crates/cli/src/app/mod.rs`, `crates/cli/src/config/types.rs`, menu entrypoint.
Test `crates/core/tests/mail_service.rs`, `crates/adapters/tests/offline.rs`, CLI app tests.

**Interfaces:** `MailState::{Running,Stopped,Transitioning,Unsupported}`;
MailControl операции state/stop/wait_for_stopped/restore_running возвращают
RPIT futures Send с PortResult. state возвращает MailState; остальные — ().
wait_for_stopped и restore_running принимают `timeout: Duration`; native
restore_running владеет общим monotonic бюджетом ожидания StopPending и Start.
Core `with_stopped<C,F,Fut,T>(control: &C, cancellation: &Cancellation, work: F)
-> ManagedResult<T>` — async fn, где C: MailControl, F: FnOnce() -> Fut,
Fut: Future<Output=PortResult<T>>. `ManagedResult<T> { result: PortResult<T>,
restore_error: Option<Box<dyn Error + Send + Sync>> }` сохраняет оба исхода.
STOP_TIMEOUT и RESTORE_TIMEOUT — отдельные константы по 60 секунд в mail_service.
Fake control проверяет запрошенные бюджеты; native backend владеет polling clock.
App `run_once` теперь возвращает `RunResult::{Finished(Report), Fatal(Report)}`:
restore/output failure — Fatal, остальные завершённые исходы — Finished.
CLI выдаёт Report и exit_code из обоих вариантов; service использует вариант
для решения, разрешён ли следующий проход.

- [ ] RED tests `preflight_failure_never_stops_mail`, `stopped_before_run_is_not_started`,
  `running_mail_is_restored_after_success_error_and_cancel`, `stop_rejected_keeps_running`,
  `accepted_stop_timeout_still_attempts_restore`, `restore_timeout_preserves_cleanup_error`,
  `database_lock_is_held_until_restore_finishes`.
  Event assertions: acquire lock/verify access precede stop; work only after Stopped;
  restore before release. Использовать fake MailControl/clock, не рабочую службу.
  Отдельные сценарии проверяют результат и оба независимых исхода:
  ```rust
  assert_eq!(initially_stopped_final_state, MailState::Stopped);
  assert_eq!(restored_final_state, MailState::Running);
  assert!(failed_cleanup_and_restore.result.is_err());
  assert!(failed_cleanup_and_restore.restore_error.is_some());
  assert_eq!(stop_budget.as_secs(), 60);
  assert_eq!(restore_budget.as_secs(), 60);
  ```
- [ ] RED: `mise exec -- cargo test --workspace --locked`.
- [ ] Реализовать two-budget workflow: Stop<=60 s, restore<=60 s, учитывая
  StopPending. При отправленном Stop сохранять обязанность restore даже при
  отмене/таймауте. App выполняет конфигурацию, журнал, INI/Data/schema/rights/lock
  перед остановкой; общий cleanup и native STOPPED guard остаются.
  Ошибка restore даёт failed result; запускавший сервисный цикл получает fatal outcome.
- [ ] GREEN: `mise run check` и native test query/stop/start только специально
  созданной тестовой службы в отдельном Windows-стенде. Mac fake-control PASS
  не выдавать за проверку SCM Server 2016.
- [ ] Commit `feat: restore mail service state after managed cleanup`.

### Task 8: Ежедневное расписание и настоящий хост Windows SCM

**Files:** Create `crates/cli/src/service/{mod,schedule,runner}.rs` и
`crates/adapters/src/service/{mod,windows,handler,status}.rs`; modify
`crates/cli/src/{args,main}.rs`, `crates/cli/src/config/{types,mod}.rs`,
`crates/adapters/src/lib.rs`, workspace/adapter manifests for Tokio features.
Test CLI-local schedule/runner modules и `crates/adapters/tests/service.rs` (Windows).
Update `.github/workflows/cleanup-cli.yml` Windows job.

**Interfaces:** `schedule::next_run(now: DateTime<Tz>, at: NaiveTime,
last_local_date: Option<NaiveDate>) -> Result<DateTime<Tz>, ScheduleError>`;
Tz: chrono::TimeZone, reusable injectable zone for DST tests.
Config добавляет `ServiceSettings { cleanup: Settings, run_at: NaiveTime }` и
`load_service_settings(args: &Args) -> Result<ServiceSettings,ConfigError>`:
читает HMAIL_RUN_AT строго HH:MM, требует absolute log_file, fast scan и
mail service_name, отличное от hMailCleaner без учёта ASCII-регистра.
`WaitResult::{Reached,Cancelled}` и `ServiceClock::now() -> DateTime<Local>`;
`ServiceClock::wait_until(deadline: DateTime<Local>, stop: &Cancellation)`
возвращает RPIT future Send с PortResult<WaitResult>.
`ScheduledJob::execute(&mut self, intent: RunIntent, stop: &Cancellation)`
возвращает RPIT future Send с RunResult; production job владеет Settings/Output
и использует общий app::run_once. `service::run<C,J>(settings: &ServiceSettings,
clock: &C, job: &mut J, stop: Arc<Cancellation>) -> PortResult<()>` — async fn,
C: ServiceClock, J: ScheduledJob. Хост `service::host(name: &str, job:
Box<dyn Fn(Arc<Cancellation>) -> PortResult<()> + Send + Sync>) -> PortResult<()>`
экспортируется adapter entrypoint, сохраняет callback на время SCM dispatcher.
Settings/локальный sink подготавливаются до host без подключения к MySQL;
длинный Tokio service runner выполняется на owned runtime в потоке ServiceMain.
Регистрация/учётная запись остаются стандартными Windows-командами оператора.

- [ ] RED tests `startup_waits_for_future_slot`, `missed_days_are_not_replayed`,
  `long_job_does_not_enqueue_jobs`, `repeated_hour_runs_once`,
  `nonexistent_hour_is_skipped`, `stop_interrupts_wait`,
  `stop_during_job_finishes_batch_and_restore`, `fatal_restore_failure_ends_loop`,
  `service_rejects_full_scan_and_missing_schedule`, `service_cannot_stop_itself`,
  `each_run_recomputes_retention`.
  DST tests используют собственный deterministic TimeZone fixture, не меняют TZ системы.
  Для completed-date/idle/fatal сценариев:
  ```rust
  assert!(next > now);
  assert_ne!(next.date_naive(), completed_local_date);
  assert_eq!(jobs_before_future_slot, 0);
  assert_eq!(jobs_in_repeated_hour, 1);
  assert_eq!(jobs_after_fatal_restore, 0);
  assert!(self_target_settings.is_err());
  ```
- [ ] RED: `mise exec -- cargo test -p hmail-cleaner --locked`;
  SCM-specific tests RED на native тестовой Windows-службе.
- [ ] Реализовать daily future-slot calculation; выбирать один occurrence
  ambiguous local time и не догонять прошлое. Runner ждёт без DB connection,
  при обычном partial пишет результат и ждёт следующего дня; при restore/log
  fatal error завершает host. Service always fast и manage-service, без prompts.
- [ ] Реализовать dispatcher, StartPending/Running/StopPending/Stopped,
  Stop/Shutdown handler и progress checkpoints во время graceful stopping.
  Job/worker принадлежит host и join до Stopped; unsafe FFI boilerplate
  доверить windows-service macro, без ручной эмуляции SCM.
- [ ] GREEN: `mise run check`; Windows release/test cross-build; native SCM tests
  подтверждают actual start/status/stop и ошибку callback. Отдельно сохранить
  статус Server 2016 проверки как verified/unverified, не смешивать с cross-build.
- [ ] Commit `feat: host scheduled cleanup as a Windows service`.

### Task 9: Проверка всей поставки и операторская инструкция

**Files:** Modify docs/cleanup-cli.md, docs/cleanup-review.md,
examples/cleaner.env.example, .github/workflows/cleanup-cli.yml только для новых
native checks; mise.toml только если реальный check требует задачу.
Обновить ignored dist/cli/EXE, README.txt, env example, build-info.json и SHA256.
Не включать EXE или credentials в Git.

**Interfaces:** Существующие mise tasks/check/build/test:db сохраняются.
CLI help показывает старые и новые launch modes; old required values остаются
валидным способом запуска. Операторская инструкция содержит dry-run/full,
env-only/menu/silence/service, ожидание schedule и восстановление после ошибок.

- [ ] Написать матрицу acceptance для Task 4 fixture и всех launch modes:
  plain/JSON/silent preview; direct clean refusing running/non-Windows;
  managed clean с fake и native test-mail service; полная очистка/repeat;
  service registration в disposable Windows-стенде через documented команды.
- [ ] Выполнить `mise run check`, `mise run test:db`, `mise run build`;
  native Windows `mise run build:windows` и meaningful Windows tests.
  Если Server 2016 или native host недоступны, сохранить эту границу как
  неподтверждённую, а не объявить её PASS. Не запускать подготовленный workflow
  через публикацию без пользовательской авторизации.
- [ ] Выполнить один свежий whole-branch review по workflow выбранного native
  исполнения, исправить introduced defects с RED/GREEN и повторить затронутые
  проверки. Не заказывать отдельный fresh reviewer для каждого Task.
- [ ] Пересобрать EXE после последних code fixes; provenance включает commit,
  pinned toolchain, фактические checks/warnings и runtime gaps. Windows text
  package UTF-8 BOM/CRLF, env sample ASCII/CRLF; проверять docs paths/links.
- [ ] Снять только owned disposable fixture/build containers и остановить
  owned Colima profile, не prune чужие Docker-данные. Commit операторских
  документов `docs: explain full cleanup and scheduled Windows operation`.

---

## Coverage and Handoff

Spec sections: SQL orphan cleanup → Task 2; file scan/protection/RAM → Tasks 2–4;
verification/retry/counters → Tasks 1/4; env/menu/silence/log → Tasks 5/6;
mail-state ownership → Task 7; daily SCM service → Task 8; complete delivery → Task 9.
Native inline execution method сохраняется. До реализации пользователь читает
этот план и подтверждает, что он отражает согласованный дизайн.
