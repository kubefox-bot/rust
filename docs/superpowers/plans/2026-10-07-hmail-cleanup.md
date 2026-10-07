# hMailServer Cleanup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Сделать CLI, который по возрасту писем в БД удаляет файлы и затем соответствующие записи, с безопасным повторным запуском.

**Architecture:** Переносимое Rust-ядро владеет правилами и порядком операций. SQLx/MySQL, файловая система и Windows SCM реализуют его порты; CLI читает конфигурацию и собирает зависимости. Точное вычисление путей заменяет рекурсивный поиск.

**Tech Stack:** Rust 2021, toolchain 1.98.1, существующие семейства SQLx 0.7, Tokio 1, Chrono 0.4, Serde 1, thiserror 1; UUID 1 для проверки GUID; Clap 4 и windows-service 0.8 для новых внешних границ. tempfile 3 — только тесты.

**Spec:** [2026-10-07-hmail-retention-design.md](../specs/2026-10-07-hmail-retention-design.md)

## Global Constraints

- Целевая ОС: Windows Server 2016/1607 x64; исполнение на самом сервере при STOPPED hMailServer.
- Тестовая MySQL: 5.7.21, закреплённый digest образа; mysql --version пользователя подтверждает клиент, версия сервера проверяется SELECT VERSION().
- mise.toml: Rust 1.98.1, задачи fmt/fmt:check/test/lint/check/build/db:up/db:down/test:db; зависимости фиксируются Cargo.lock.
- Файлы удаляются первыми; соответствующие записи БД — после Removed или AlreadyAbsent.
- Карантин, резервное копирование, SQLite-журнал и отдельный resume не нужны.
- Отбор: `messageaccountid > 0`, `messagetype = 2`, `messagecreatetime < cutoff`.
- Сроки: `30d`, `1mo`, `1y` либо `--before`, локальная шкала сервера.
- Очередь, #Public, более новые сообщения, аккаунты и логические IMAP-папки сохраняются.
- После подтверждённого SQL-шагa пустые затронутые GUID/account/domain каталоги удаляются; Data и непустые каталоги сохраняются. Полного прохода по пустым каталогам нет.
- Порция: 1 000, допускается 1–10 000; работники: 2, допускается 1–8; прогресс не чаще раза в 250 мс.
- Без полного обхода дерева, OFFSET, загрузки всех сообщений в RAM и автоматического ALTER/OPTIMIZE.
- Удаляются только конкретные id и их metadata/recipients в одной InnoDB-транзакции.
- Старый Tauri/Vue остаётся отдельным приложением; его обновление — следующий этап.
- Нельзя автоматически читать старые .env, печатать секреты, обращаться к рабочему серверу, публиковать ветку или запускать CI через push.

## Review Focus

- Похожий корень `D:\\Data-other`, drive-relative путь и NTFS alternate stream должны быть отклонены, а не превращены в AlreadyAbsent — Task 1/3.
- Несуществующий или неверный DataFolder не должен удалить все строки БД как отсутствующие файлы — Task 3/5.
- Одна повреждённая запись или повторяющийся basename не должна зациклить cursor либо удалить более новую ссылку — Task 2/4.
- Потерянный ответ COMMIT не должен давать успешный итог; повтор должен корректно обработать фактически committed и rolled-back случаи — Task 2/4.
- Ctrl+C во время удаления должен дождаться начатой порции и завершить допустимый SQL-шаг — Task 4/5.

## Файлы и публичные интерфейсы

Создать Cargo workspace в корне с members `crates/core`, `crates/adapters`,
`crates/cli`; `src-tauri` исключить из workspace. Основной исполняемый пакет —
`hmail-cleaner`. Не менять старые команды Vue/Tauri или их manifests.

- Core: `lib.rs`, `selection.rs`, `paths.rs`, `cleanup/{mod,types,ports}.rs`.
- MySQL: `mysql/{mod,config,schema,select,delete,duplicates}.rs`.
- Файлы: `filesystem/{mod,delete,validation,prune}.rs`.
- Служба: `offline/{mod,windows}.rs`, Windows-код под cfg(windows).
- CLI: `main.rs`, `args.rs`, `commands.rs`, `config/{mod,types}.rs`, `output.rs`.
- Тесты: integration tests соответствующего crate; MySQL-fixture и Compose в
  `crates/adapters/tests/fixtures`; source .7z не требуется на CI.

Core-контракты в `cleanup/types.rs`:

- `Candidate { id: i64, account_id: i32, folder_id: i32, account_address: String, filename: String, created_at: NaiveDateTime }`.
- `Selection { cutoff: NaiveDateTime, upper_id: i64 }`.
- `Snapshot { upper_id: i64 }`.
- `Batch { last_id: i64, candidates: Vec<Candidate>, rejected: Vec<RowFailure> }`; rejected содержит id и причину.
- `FileTarget { id: i64, path: PathBuf }`; пути сформированы core, доверие фактическому filesystem проверяет адаптер.
- `FileOutcome { id: i64, path: PathBuf, result: DeletionResult }`; `DeletionResult::{Removed, AlreadyAbsent, Failed(FileFailure)}`.
- `FileFailure { kind: FileFailureKind, message: String }`; `FileFailureKind::{AccessDenied, InvalidPath, Io}`; `RowFailure { id: i64, reason: String }`.
- `PruneOutcome { removed: u64, failures: Vec<DirectoryFailure> }`; `DirectoryFailure { path: PathBuf, reason: String }`.
- `DeleteResult = Committed | CommitUncertain`; Committed возвращается только после подтверждённого commit.
- `Summary { selected: u64, removed_files: u64, absent_files: u64, deleted_messages: u64, removed_directories: u64, failed: u64 }`; подтверждённые удаления считаются отдельно от файлов.
- `RunOptions { cutoff: NaiveDateTime, batch_size: NonZeroU32, workers: u8, data_directory: PathBuf }`.
- `Cancellation` владеет AtomicBool и предоставляет `request()` и `is_requested()`.
- `Progress` содержит этап, Summary и при ошибке конкретный id/причину.
- `PortResult<T> = Result<T, Box<dyn Error + Send + Sync>>`; CleanupError сохраняет source, этап и достигнутый Summary.

Порты в `cleanup/ports.rs`; операции возвращают `impl Future<Output = PortResult<T>> + Send`:

- `Repository::snapshot(&mut self) -> Snapshot`.
- `Repository::next_batch(&mut self, selection: Selection, cursor: i64, limit: u32) -> Batch`.
- `Repository::delete_batch(&mut self, candidates: &[Candidate]) -> DeleteResult`.
- `Repository::verify_deleted(&mut self, ids: &[i64]) -> ()`.
- `FileStore::delete(&self, files: Vec<FileTarget>, workers: u8) -> Vec<FileOutcome>`.
- `FileStore::prune_empty(&self, parents: Vec<PathBuf>) -> PruneOutcome`.
- `OfflineGuard::ensure_stopped(&self) -> ()`.

Экспорты доступны через публичные модули; запросы SQL и детали workers приватны
внутри владельца. Core не импортирует SQLx, Tauri или windows-service.

---

### Task 1: Период хранения и точный путь

**Files:** Create root Cargo.toml/Cargo.lock, mise.toml, crate manifests;
core lib.rs, selection.rs, paths.rs, cleanup/types.rs, cleanup/ports.rs;
core tests/selection.rs, tests/paths.rs; extend .gitignore только новыми build/tool paths.

**Interfaces:**
- Produces: контракты выше и `Retention::{Days(NonZeroU32), Months(NonZeroU32), Years(NonZeroU32), Before(NaiveDateTime)}`.
- Produces: `cutoff(now: NaiveDateTime, retention: Retention) -> Result<NaiveDateTime, SelectionError>`.
- Produces: `resolve_path(root: &Path, candidate: &Candidate) -> Result<PathBuf, PathError>`.

- [x] Создать минимальные manifests пакетов hmail-cleaner-core, hmail-cleaner-adapters,
  hmail-cleaner и local mise pin 1.98.1 для запуска тестов. Пустые entrypoints
  adapters/lib.rs и cli/main.rs нужны для загрузки workspace до Tasks 2/5;
  функциональное поведение в них пока не добавлять. Tauri не входит в workspace.
  Порты использовать без async-trait.
- [x] Написать failing tests `month_clamps_to_february`, `year_clamps_leap_day`, `days_preserve_local_time`, `explicit_before_is_unchanged`.
  Assertions: 2026-03-31 10:00 − 1mo = 2026-02-28 10:00; 2024-02-29 10:00 − 1y = 2023-02-28 10:00; 2026-10-07 12:00 − 30d = 2026-09-07 12:00.
- [x] Написать failing tests `guid_resolves_to_account_bucket`, `legacy_absolute_path_stays_in_root`, `similar_root_is_rejected`, `traversal_stream_and_drive_relative_are_rejected`, `unicode_and_plus_account_are_preserved`.
  Assertions: `info@chem.orp` и `{6C2CD4A9-683F-4EB4-91D7-9E2FD3700000}.eml` дают `root/chem.orp/info/6C/{GUID}.eml`; `../`, ADS, `D:relative.eml` и соседний root возвращают PathError. Проверки Windows-формата должны работать на Mac как чистый разбор.
- [x] Run `mise exec -- cargo test -p hmail-cleaner-core`; ожидается FAIL из-за отсутствующих API/поведения.
- [x] Реализовать retention и resolver, проверяя GUID через UUID. Сравнение root не делать обычным starts_with строки; компоненты и Windows prefix должны проверяться явно. Границы и переполнение дат возвращают ошибку.
- [x] Run тот же test и `mise exec -- cargo fmt --all -- --check`; ожидается PASS.
- [x] Commit `feat: add portable retention and message path rules`.

### Task 2: Выборка MySQL и удаление точных id

**Files:** Modify scaffold adapters lib.rs; create mysql/mod.rs, config.rs, schema.rs, select.rs, delete.rs,
duplicates.rs; tests/mysql.rs, fixtures/schema.sql, fixtures/mysql.compose.yml,
fixtures/mod.rs. Fixture воспроизводит соответствующие таблицы из предоставленной DDL.

**Interfaces:**
- Consumes: Repository, Candidate, Selection, Snapshot, Batch, DeleteResult.
- Produces: `MySqlRepository::connect(config: &DatabaseConfig) -> PortResult<Self>`; DatabaseConfig валидирован и не раскрывает URL в Debug.
- Produces: `mysql::DatabaseConfig::parse(url: &str) -> Result<Self, DatabaseConfigError>` в mysql/config.rs; приватные MySqlConnectOptions, безопасные Display/Debug. CLI вызывает этот валидатор, адаптер не читает env и не импортирует CLI.
- Implements: Repository; владеет выделенным SQLx MySqlConnection и advisory lock. Клиент не строится внутри core.

- [x] Создать MySQL 5.7 fixture на loopback port 13306 (на ARM Mac platform linux/amd64).
  Тестовый URL читается только конфигурацией fixture из HMAIL_TEST_DATABASE_URL,
  без fallback к DATABASE_URL. Разрешены только loopback host и отдельная
  уникальная тестовая БД, созданная самим fixture; старые БД не переиспользуются.
  Для запуска: `docker compose -p hmail-cleanup-tests -f crates/adapters/tests/fixtures/mysql.compose.yml up -d --wait`; test URL указывает только этот fixture, выключение удаляет только его собственные ресурсы.
- [x] Написать ignored integration tests:
  `selects_only_old_delivered_accounts_across_folders`,
  `keyset_cursor_advances_over_rejected_rows`,
  `legacy_and_bare_duplicate_names_are_rejected`,
  `deletes_only_requested_ids_and_their_dependents`,
  `empty_delete_is_a_noop`, `failed_child_delete_rolls_back_parent`,
  `second_connection_cannot_acquire_cleanup_lock`.
  В fixture есть old/new/boundary, Inbox/Sent, queue, public, missing account/folder,
  metadata и recipients как с совпадающим, так и с посторонним id.
- [x] Запустить fixture и `mise exec -- cargo test -p hmail-cleaner-adapters --test mysql -- --ignored --test-threads=1`.
  При отсутствующем fixture URL тест должен FAIL с ясной причиной, не PASS со skip.
- [x] Реализовать schema preflight требуемых колонок/InnoDB, snapshot upper_id,
  справочник account/folder и один запрос дубликатов по нормализованному basename.
  next_batch читает диапазон primary key, строгий cutoff и LIMIT; rejected rows
  учитываются в last_id и результате.
- [x] Реализовать delete_batch: SELECT FOR UPDATE и сверка id/account/folder/name/date,
  DELETE metadata/recipients/parent по bind-id в одной транзакции, проверка
  rows_affected, commit. Пустой набор возвращает Committed без DELETE.
  Ошибка commit возвращает CommitUncertain, не утверждение о rollback.
- [x] Реализовать verify_deleted точными id для трёх таблиц. Advisory lock
  берётся на том же выделенном соединении, его жизненный цикл освобождает lock.
- [x] Повторить MySQL tests; ожидается PASS. Проверить EXPLAIN выборки в fixture:
  диапазон по primary key без OFFSET, сортировки по дате и зависимых запросов на каждое письмо.
- [x] Commit `feat: delete exact message batches transactionally in MySQL`.

### Task 3: Файловые работники и offline preflight

**Files:** Create filesystem/mod.rs, delete.rs, validation.rs, prune.rs;
offline/mod.rs, windows.rs; tests/filesystem.rs, tests/offline.rs.

**Interfaces:**
- Consumes: FileStore, OfflineGuard, FileTarget, FileOutcome.
- Produces: `LocalFiles::new(root: PathBuf) -> PortResult<Self>`, implements FileStore.
- Produces: `WindowsOffline::new(service_name: String) -> Self`, implements OfflineGuard.
- Produces: `verify_data_folder(root: &Path, ini_path: &Path) -> PortResult<()>`.
  Отсутствующая служба/INI/корень — preflight failure.

- [x] Написать failing tests `removes_only_exact_targets`, `missing_target_is_already_absent`,
  `permission_error_is_failure`, `missing_root_is_not_missing_message`,
  `symlink_chain_is_rejected`, `prunes_empty_parents_but_preserves_root`,
  `preserves_nonempty_and_hidden_entries`, `prune_has_no_global_tree_walk`,
  `workers_never_exceed_configured_limit`, `ini_root_mismatch_blocks_changes`.
  Failure injection приватна тестовому filesystem boundary; production CLI не
  имеет флага отключения проверок. На Windows добавить тест reparse attributes.
- [x] Run `mise exec -- cargo test -p hmail-cleaner-adapters --test filesystem --test offline`;
  ожидается FAIL для отсутствующих адаптеров.
- [x] Реализовать проверку root и компонентов без рекурсивного read_dir.
  Группировать цели по родительскому GUID-каталогу и запускать не больше workers
  блокирующих операций; внутри группы последовательный цикл std::fs::remove_file.
  Проверку стабильных родительских каталогов можно переиспользовать внутри группы.
- [x] Каждый target возвращает FileOutcome; NotFound после проверки root/пути —
  AlreadyAbsent, остальные ошибки — Failed. Дождаться всех начатых работников,
  включая ошибку join. В отдельном prune_empty обработать только переданные
  родительские пути: deduplicate, remove_dir снизу вверх до границы root;
  DirectoryNotEmpty останавливает подъём, NotFound допускает проверку родителя,
  остальные ошибки попадают в PruneOutcome. Не использовать recursive remove_dir_all.
- [x] Реализовать сверку DataFolder и Windows SCM STOPPED через windows-service
  под cfg(windows). Production clean на других ОС возвращает UnsupportedPlatform;
  тестовый core получает fake OfflineGuard, не production bypass.
- [x] Повторить filesystem/offline tests и fmt; ожидается PASS.
- [x] Commit `feat: add bounded exact-path deletion and Windows service checks`.

### Task 4: Сценарий очистки и повторного запуска

**Files:** Create core cleanup/mod.rs, tests/cleanup.rs;
расширить adapters tests/mysql.rs проверкой потери подтверждения.

**Interfaces:**
- Consumes: все три порта и RunOptions.
- Produces: `Dependencies<'a, R, F, G> { repository: &'a mut R, files: &'a F, guard: &'a G }`.
- Produces: `clean(dependencies: Dependencies<'_, R, F, G>, options: &RunOptions, cancellation: &Cancellation, progress: P) -> Result<Summary, CleanupError>`;
  async, R: Repository, F: FileStore, G: OfflineGuard, P: Fn(Progress) + Send + Sync.
- Produces: `preview(repository: &mut R, options: &RunOptions, progress: P) -> Result<Summary, CleanupError>`;
  async, без удаления, с тем же отбором и resolver.

- [x] Написать failing tests `file_step_precedes_database_delete`,
  `removed_and_absent_ids_are_deleted_but_failed_ids_are_preserved`,
  `repeated_run_cleans_rows_after_previous_sql_failure`,
  `uncertain_commit_stops_new_batches`, `rejected_rows_advance_cursor`,
  `cancel_finishes_started_batch`, `preview_has_no_delete_calls`,
  `prunes_only_after_confirmed_database_deletion`, `failed_database_step_does_not_prune`.
  Fake-порты записывают порядок эффектов и фактические наборы id.
- [x] Run `mise exec -- cargo test -p hmail-cleaner-core --test cleanup`; ожидается FAIL.
- [x] Реализовать один явный цикл: STOPPED → snapshot → next_batch → STOPPED →
  resolve targets → file results → точные допустимые candidates → delete_batch →
  verify_deleted → prune_empty для допустимых путей → следующий cursor.
  Ошибки prune учитываются отдельно от уже подтверждённого SQL-результата.
  Не создавать интерпретатор действий.
- [x] Сводка считает только подтверждённые удаления. Row/path/file failures
  публикуются как Progress и дают partial result; SQL/preflight/unknown-commit
  останавливают новые порции, сохраняя достигнутую сводку.
  Cancellation проверяется на границе порций, без принудительного process exit.
- [x] Дополнить MySQL fixture сценарием dropped-ack: выполнить настоящий commit
  внутри тестового boundary, заменить подтверждение на CommitUncertain,
  затем повторить сценарий и проверить отсутствие повторной потери/чужих удалений.
  Второй вариант — rollback после file step, повтор удаляет оставшиеся строки.
- [x] Повторить core cleanup и MySQL tests; ожидается PASS. Проверить отсутствие
  полной коллекции всех сообщений/результатов: Summary — счётчики, ошибки идут
  в Progress, текущие candidate/id collections ограничены порцией.
- [x] Commit `feat: add repeatable file-first cleanup workflow`.

### Task 5: CLI, сборка и проверка рабочего сценария

**Files:** Modify scaffold cli main.rs; create args.rs, commands.rs, config/mod.rs,
config/types.rs, output.rs, config/tests.rs; args/commands unit tests под cfg(test)
в соответствующих владельцах; tests/help.rs для read-only проверки бинарника.
Create examples/cleaner.env.example, docs/cleanup-cli.md,
.github/workflows/cleanup-cli.yml. Существующий UI не изменять.

**Interfaces:**
- Consumes: cutoff/resolver, clean/preview, адаптеры и Cancellation.
- Produces: `load_config(args: &Args) -> Result<Config, ConfigError>`; единственный
  production-владелец env/env-file. Подключение БД и рабочий каталог передаются явно.
- Produces: команды `hmail-cleaner preview` и `hmail-cleaner clean` с
  `--older-than` либо `--before`, `--data-dir`, `--hmail-ini`,
  `--service-name`, `--batch-size`, `--workers`, `--env-file`, `--json`.
  DATABASE_URL — system env или явно выбранный env-file; credentials не в CLI args.
- Produces: exit 0 complete, 1 partial/backend/uncertain, 2 invalid config/preflight,
  130 cancelled. JSON выдаёт типизированные события и итог, без credentials.

- [x] Написать failing tests `retention_options_are_exclusive`, `zero_retention_is_rejected`,
  `defaults_are_1000_and_2`, `workers_outside_1_to_8_are_rejected`,
  `batch_size_outside_1_to_10000_is_rejected`,
  `legacy_env_is_not_auto_loaded`, `explicit_env_file_is_supported`,
  `diagnostics_do_not_contain_secret_url`,
  `preview_is_read_only`, `partial_cleanup_has_nonzero_exit`.
  Commands тестируются с fake-портами; raw env setup только в config tests.
- [x] Run `mise exec -- cargo test -p hmail-cleaner`; ожидается FAIL.
- [x] Реализовать Args через Clap, конфигурацию через одного владельца;
  clocks/cutoff берутся один раз из server-local времени. Группировать вывод
  progress на интервале 250 мс; конкретные ошибки и финальный результат не терять.
  Ctrl+C только запрашивает Cancellation.
- [x] Создать пустой безопасный env example и руководство двух команд с точной
  границей и объяснением повторного запуска. Пример не копирует tracked .env.
- [x] Настроить Windows MSVC static CRT только в CLI build-команде через scoped
  RUSTFLAGS, не через общую .cargo/config.toml, влияющую на старый Tauri. Подготовить CI:
  macOS core tests, Linux MySQL 5.7 fixture, Windows native build/tests + exe artifact.
  Никаких secrets для working hMailServer и никакого автозапуска workflow через push.
- [x] Run fmt, `mise exec -- cargo test --workspace`,
  `mise exec -- cargo clippy --workspace --all-targets`,
  отдельные ignored MySQL tests и `mise exec -- cargo build --release -p hmail-cleaner`.
  Читать warnings; не добавлять deny/suppress lint ради зелёного результата.
- [x] Если Mac cross-build доступен, проверить CLI через cargo-xwin с
  x86_64-pc-windows-msvc. Target/LLVM/SDK устанавливать только локально для задачи,
  не менять глобальную конфигурацию mise. Если недоступен — оставить готовый CI
  и явно указать отсутствие Windows binary proof; публикация требует отдельного запроса.
- [x] Зафиксировать реальные результаты: Mac/core, MySQL, Windows build и
  Windows Server 2016 runtime — отдельные пункты. Без доступа к Windows серверу
  не утверждать, что реальная очистка или проверка на нём выполнена.
- [x] Commit `feat: expose cleanup through a standalone CLI`.

## Проверка плана и способ исполнения

Все требования спецификации покрываются Tasks 1–5. SQLite, карантин, бэкапы,
перенос окна и blanket dependency upgrades исключены. Проверки Review Focus
включены в соответствующие тесты; зависимости и публичные имена согласованы.

Рекомендован Native: один последовательный сценарий с общими контрактами,
который я реализую в этой сессии по задачам. Subagent-driven остаётся альтернативой
с отдельным implementer/reviewer на каждую задачу.
Исполнение начинается после проверки этого плана и выбора пользователем способа.
