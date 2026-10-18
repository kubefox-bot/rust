# Astra Image CI/CD Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Push в `main` доставляет проверенный контейнер Astra на FCOS автоматически, с graceful ожиданием текущей работы и сохранением данных.

**Architecture:** GitHub Actions собирает одну OCI-поставку, проверяет её и публикует image и подписанный descriptor. Отдельный rootless host updater обнаруживает релиз через исходящий HTTPS и управляет Quadlet, admission gate, offline backup и восстановлением. Zincati координирует reboot через отдельную локальную службу.

**Tech Stack:** Существующие Rust 1.99.1, Bun 1.4.3, Astro/Svelte, SQLx/SQLite; GitHub Actions, GHCR, FCOS, Podman/Quadlet, Cosign/Sigstore.

**Spec:** [Проект решения](../specs/2026-10-18-center-image-delivery-design.md).

**Status:** Пользователь согласовал исполнение в отдельной ветке `ci/astra-image-delivery` и добавил управление интервалами из центра. Локальная реализация разрешена; GitHub setup, публикация и production activation отдельно.

**Уточнение пользователя:** `check_interval_seconds` default60/range30–86400, `graceful_timeout_minutes` default15/range1–1440 задаёт SA в Settings. Host timer30s получает policy через Unix socket, хранит валидную копию и планирует discovery. In-flight update использует snapshot policy. Добавлены protocol/browser updates, API/settings/updates и UI tab; schema1 сохранена.

## Результат исполнения

Локальная реализация задач 1–8 и настроек центра выполнена в ветке
`ci/astra-image-delivery`. Выполнены source tests и независимый review всей
поставки; три замечания устранены с RED → GREEN регрессиями. Один общий commit
фиксирует согласованную поставку вместо отдельных commits промежуточных задач.
Чек-листы ниже сохраняют исходный план; пункты с GitHub publication, native
FCOS, Hyper-V и Windows Server 2016 acceptance остаются NOT RUN. Полные
границы и фактические результаты: [отчёт приёмки](../../center-cicd-acceptance.md).

## Global Constraints

- Trigger: push в `main`; проверки PR не публикуют и не устанавливают релизы.
- Первый target `linux/amd64`; общий build `git-<полный SHA>` для центра, web, агента.
- Только исходящий HTTPS. Не устанавливать runner внутри организации.
- FCOS user `hmail-center` UID/GID 2000; runtime UID/GID 10001, прежний keep-id и `hmail-center-data`.
- Сохранить read-only rootfs, HTTPS health до login, `Pull=never`, digest pin и отсутствие engine socket в центре.
- Discovery 60 секунд, AccuracySec 1 секунда, jitter до 5 секунд. Graceful deadline 15 минут, health deadline 120 секунд.
- Истечение graceful срока откладывает update; не отменяет работу и не разрешает unknown receipt.
- Автоматический restore только до потенциального открытия candidate пользователям; после открытия новые данные не откатываются.
- Schema 1 в первой версии; несовместимые schema/updater/protocol блокируются до остановки центра.
- Автоматические Windows-agent updates, schema migration framework, updater self-update, ручное применение обновления из GUI и golden VM image вне этого плана. Настройки двух интервалов в GUI включены по уточнению пользователя.
- GitHub package по умолчанию private; публикация и настройка credentials — отдельная внешняя операция после review.

## Review Focus

1. Поздно завершившийся старый workflow не перезаписывает более новый канал — Task 3.
2. Concurrent job/enrollment/write после idle-check не попадает между drain и stop — Task 4.
3. Power loss во время backup/volume restore/unit switch не открывает candidate и не удаляет единственную копию данных — Task 6.
4. Смена login/RAM-key, неизвестный agent receipt и недоступный signer/registry не вызывают forced update — Tasks 2, 4, 6.
5. FCOS reboot не обходит maintenance gate и не запускает image с несовместимыми данными — Tasks 6, 7, 8.

## Владение файлами и интерфейсы

Существующие owner APIs переиспользуются: `buildRelease`, `readReleaseManifest`,
`inspectOciArchive`, `deployment`, `center backup`, `center health`.
Расширения release format оформляются отдельным delivery descriptor, существующий
format 1 не переопределяется молча. Новые TS helpers принадлежат `tools/release/`,
процессный запуск переиспользует `tools/lib/process`, не создаётся общий utils.

Новый `crates/updater` — Linux-only binary с командами `check`, `apply`, `resume`,
`status`, `prepare-start`, `coordinate-reboot`. Публичных Rust library APIs для других crates нет.
Внутри независимые capabilities: `config`, `delivery`, `discovery`, `journal`,
`maintenance`, `podman`, `backup`, `apply`, `reboot`. Каждый владеет своими errors
и constants; transport не читает env в обход config. Domain decisions тестируются
с портами конкретных внешних границ; composition в `main.rs`.

## Task 1: CI для согласованной поставки

**Files:** Create `.github/workflows/center-ci.yml`; modify `mise.toml`, `tools/release/build-agent.ts`, `tools/release/config.ts`, `tools/release/types.ts`, `tools/release/conts.ts`; create `tools/release/tests/agent-input.test.ts`.

**Interfaces:** `buildRelease(config)` сохраняется. Добавить validated option `agentPackageDirectory?: string` через `HMAIL_RELEASE_AGENT_PACKAGE`: supplied пакет проверяется существующим `validateAgentPackage`, копируется и не пересобирается. Без option прежняя локальная сборка сохраняется. Reusable CI отдаёт `build`, полный `source_sha`, artifact identity и проверенную release directory.

- [ ] Написать Bun tests: supplied пакет и common build принимаются; чужой build/неправильный hash/PE target/необычный файл отвергаются; fallback вызывает существующий builder ровно один раз.
- [ ] Запустить `mise exec -- bun test tools/release/tests/agent-input.test.ts`; зафиксировать ожидаемый FAIL отсутствующего контракта.
- [ ] Реализовать option и reuse. На Windows runner собрать агент существующей командой; Linux release job скачивает именно artifact того же workflow/SHA, проверяет и встраивает его, не строит Windows binary второй раз.
- [ ] Добавить PR/main reusable CI: Rust check, contracts/web check+build, isolated MySQL integration, Windows build/SCM fixture, Linux delivery unit tests и OCI lifecycle. Existing CLI workflow сохранить. `bun install --frozen-lockfile`; Actions pins полные SHA; default contents read. Не считать Windows-2022 доказательством Server-2016 acceptance.
- [ ] Добавить команды `release:test` для существующих TS/unit проверок и `delivery:test` для будущих updater/center tests, чтобы локальный запуск не требовал реального registry. Наличие команды `test:delivery` с реальным manifest сохранить.
- [ ] Запустить release tests + TypeScript check + `mise run check`, `mise run contracts:check`, `mise run web:check`; получить OCI `test:delivery` report для собранной поставки. Внешний workflow run пока NOT RUN.
- [ ] Commit `ci: verify a consistent center and agent release` только при выбранном исполнении плана.

## Task 2: Подписанный delivery descriptor и проверка доверия

**Files:** Create `tools/release/delivery-descriptor.ts`, `tools/release/delivery-descriptor/conts.ts`, `tools/release/tests/descriptor.test.ts`, `deploy/delivery.schema.json`, `deploy/tool-pins.json`; create `crates/updater/Cargo.toml`, `crates/updater/src/{main,config,delivery}.rs`, `crates/updater/tests/descriptor.rs`; modify `Cargo.toml`, `Cargo.lock`.

**Interfaces:** TS `createDeliveryDescriptor(input: DeliveryDescriptorInput): DeliveryDescriptor`; `DeliveryDescriptorInput` = validated existing manifest + full SHA + channel/main + sequence + compatible updater range + schema 1 + manifest hash. Rust `verify_descriptor(bytes: &[u8], bundle: &Path, policy: &TrustPolicy) -> Result<VerifiedDelivery, VerifyError>`; `VerifiedDelivery` непрозрачен за пределами owner и создаётся только после signature+semantic checks. JSON keys определены schema из spec, TS/Rust fixtures согласованы.

- [ ] Добавить тесты двух языков: byte mutation, неверные issuer/workflow/SHA, неподписанный manifest, wrong product/platform/repository, lower/equal sequence с другим digest, несовместимые updater/schema/protocol. Сеть и stderr verifier не превращать в valid/no-update.
- [ ] Выполнить тесты до реализации, подтвердить FAIL; Cargo workspace поправить вместе с минимальным crate scaffold, не оставлять сломанный member между commits.
- [ ] Реализовать строгий формат и bounded input, в spec зафиксировать точные limits metadata/config/скачиваний. Reuse existing validators и locked HTTP/hash dependencies. Проверить подходящие Bun/Rust Sigstore APIs; использовать зрелый Cosign CLI вместо самодельного verifier, поскольку у проекта нет готовой реализации trust bundle validation. Bun runtime на FCOS не добавлять.
- [ ] Закрепить конкретный Cosign binary+trust root/checksums и версии Actions в `deploy/tool-pins.json` по текущим официальным releases; проверить подписи bootstrap-tools. На host проверять OIDC main identity из spec, signature, log inclusion/timestamp и trusted root. Никаких insecure flags и доверия к любому GitHub signer.
- [ ] Проверить tampered/valid bundle на локальных fixtures; CI identity acceptance выполнять на выделенном тестовом publish только после разрешения. Документировать обновление trust root без автоматического доверия сетевому файлу.
- [ ] Запустить `mise exec -- bun test tools/release/tests/descriptor.test.ts` и `mise exec -- cargo test -p astra-updater --test descriptor --locked`; green fixtures не считать внешним signing evidence.
- [ ] Commit `feat(delivery): verify signed release descriptors`.

## Task 3: Публикация проверенного image и promotion канала

**Files:** Create `.github/workflows/center-delivery.yml`, `tools/release/publish.ts`, `tools/release/publishing/{config,image,release,promote}.ts`, `tools/release/tests/publish.test.ts`; modify `tools/release/conts.ts` source input list для `.github/workflows` и новых deploy contracts.

**Interfaces:** `publishRelease(input: PublishInput): Promise<PublishedRelease>` consumes Task 1 artifacts и Task 2 descriptor. `PublishedRelease` фиксирует source SHA, sequence, image digest, release id. `promoteRelease(input: PromotionInput): Promise<"promoted" | "superseded">`; проверяет current HEAD и сериализует финальную публикацию. CLI args не включают secret values.

- [ ] Написать tests: отказ gate не публикует; rerun не пересобирает; digest changed on push блокирует publication; missing asset остаётся draft; старый run завершился позже нового → superseded; equal sequence/different digest → conflict.
- [ ] Выполнить `mise exec -- bun test tools/release/tests/publish.test.ts`; ожидать FAIL.
- [ ] Реализовать private GHCR push существующего image с сохранением digest; сравнить remote manifest/config/layers с проверенным archive. Публикация не выполняет Containerfile повторно.
- [ ] `center-delivery.yml` trigger push main: вызвать reusable CI, потреблять ровно его артефакты, подписать descriptor через GitHub OIDC, загрузить manifest/bundle/Quadlet/archive в draft release `center-<SHA>`. Package write, contents write и id-token write выдавать только owning jobs.
- [ ] Сериализовать promotion, проверить HEAD main перед release publish. Если устарел — не публиковать канал. Discovery позднее фильтрует только completed non-draft main releases и выбирает max signed sequence; API latest/release creation time не определяют порядок версий.
- [ ] Внедрить dependency cache для lockfile/platform/tool versions; до измерений не обещать длительность сборки. Retention не удаляет assets текущей/предыдущей установленной версии автоматически.
- [ ] Запустить Bun tests/TS check и workflow structural validation. Составить список required repository settings/permissions; на этом шаге не менять настройки без разрешения. Отдельно после разрешения: настоящая публикация в test package, pull и сверка digest.
- [ ] Commit `ci: publish verified center images from main`.

## Task 4: Graceful admission gate в центре

**Files:** Create `crates/center/src/maintenance/{mod,gate,marker,transport}.rs`, `crates/center/tests/maintenance.rs`; modify `crates/center/src/{lib,app}.rs`, `crates/center/src/app/state.rs`, `crates/center/src/jobs/repository.rs`, `crates/center/src/enrollment/repository.rs`, владельцев profile/account/settings mutations, `deploy/quadlet/hmail-center.container`, `tools/release/deployment.ts`.

**Interfaces:** `MaintenanceGate::begin(operation: Uuid) -> Result<DrainStatus, MaintenanceError>`, `status(operation)`, `finish(operation)`, `abort(operation)`; обычные mutations получают admission permit на весь свой сценарий, не только HTTP handler. `DrainStatus = Draining { active_jobs, active_enrollments, active_writers } | Ready`. Unix socket принимает только эти versioned операции, ограничивает peer/размер/timeout. Marker долговечен и связан с operation id; других updater-запросов не исполняет.

- [ ] Написать tests гонки begin/drain против job reserve, agent enrollment/update, profile/account/settings writers; mutation либо успевает завершиться до Ready, либо получает Maintenance. Повтор intent не dispatch повторно.
- [ ] Добавить tests: unknown slot ≠ idle; session expiry не снимает gate; restart с marker закрывает mutations до recovery; health/read-only остаются доступны; login/cancel/reconcile разрешены только как явно определённые control flows и включены в writer accounting. Другой peer/operation id не снимает gate.
- [ ] Выполнить `mise exec -- cargo test -p hmail-cleaner-center --test maintenance --locked`; подтвердить FAIL новых сценариев.
- [ ] Реализовать owner gate + durable file marker вне schema 1, startup before routes и согласованный порядок locks с SQLite slot reservation. Middleware сам по себе не является гарантией. Audit каждого domain write entrypoint, не только POST routes.
- [ ] Смонтировать только отдельный socket directory, с SELinux labels и keep-id ownership; host credentials/engine socket не монтировать. Обновить release Quadlet contract tests; unit Start/health не должен требовать SA login.
- [ ] Проверить новые тесты + existing races, dispatch, ssh_install, agent_update, credential_changes, profiles, settings_activity; запустить полный `mise run check` после фиксации API.
- [ ] Commit `feat(center): drain operations before image replacement`.

## Task 5: Автоматическое обнаружение и staging на FCOS

**Files:** Create `crates/updater/src/discovery/{mod,github}.rs`, `crates/updater/src/{config,journal,maintenance,podman}.rs`, `crates/updater/tests/{discovery,staging}.rs`; create `deploy/updater/{config.example.json,astra-updater.service,astra-updater.timer}`.

**Interfaces:** `discover(config: &Config, accepted: &AcceptedRelease) -> Result<Option<VerifiedDelivery>, DiscoveryError>`; `stage(delivery: &VerifiedDelivery, host: &Host) -> Result<StagedRelease, StageError>`. `None` означает подтверждённое отсутствие более нового релиза; сеть/401/rate-limit/parse/signature — errors/status, не None. Commands `check` только проверяет, `apply` делает check+stage+transaction; timer вызывает `apply`.

- [ ] Написать tests pagination, draft/partial release, out-of-order list, rate limit/backoff, 401, redirect вне allowlist, truncated/oversized download, wrong architecture, недостаток диска, concurrent tick и last-good retention. Несколько pending релизов выбирают последний verified sequence; staged target не меняется в середине apply.
- [ ] Выполнить `mise exec -- cargo test -p astra-updater --test discovery --test staging --locked`; подтвердить FAIL.
- [ ] Реализовать config boundary и credential loader `0600`, запрет symlink/foreign owner, bounded streaming, timeouts, redaction, строгий HTTPS и registry allowlist. Reuse existing dependencies, не вводить JS/Node runtime на FCOS.
- [ ] Реализовать Podman subprocess adapter с fixed argv/env/authfile, timeout/cancellation и cleanup только своих staging resources. Credentials никогда не попадают в argv/journal; explicit authfile не использует случайный пользовательский login.
- [ ] Timer 60 секунд + jitter до 5; одна operation lease. Ошибки discovery не останавливают центр. Status содержит desired/installed build, phase и безопасный error code.
- [ ] Проверить под native rootless Podman: скачивание не меняет работающий image, registry auth failure оставляет старый контейнер. Запустить unit tests и `cargo clippy -p astra-updater --all-targets --locked`.
- [ ] Commit `feat(updater): discover and stage verified releases automatically`.

## Task 6: Транзакционная замена image и восстановление

**Files:** Create `crates/updater/src/apply/{mod,state,run,recover}.rs`, `crates/updater/src/backup/{mod,create,restore}.rs`, `crates/updater/tests/{apply,recovery}.rs`; modify `crates/updater/src/{main,journal}.rs`, `deploy/updater/astra-updater.service`; create `deploy/updater/astra-updater-recover.service`; extend `deploy/quadlet/hmail-center.container` ordering.

**Interfaces:** `apply(staged: StagedRelease, host: &Host) -> Result<ApplyOutcome, ApplyError>`, `resume(host: &Host) -> Result<RecoveryOutcome, RecoveryError>`. Journal `format=1` owns operation, previous/candidate identity, backup identity, phase. States: Staged → Draining → Stopped → BackedUp → Starting → Verified → Exposing → Committed; Restore/Failed/Quarantined явно отдельные outcomes. `Exposing` fsync записывается ДО снятия gate: crash после него запрещает automatic restore, даже если окончательный commit ещё не записан.

- [ ] Написать fault injection tests на каждой границе side effect/journal fsync: stop, backup create, unit rename, start, health, gate release, volume rename/restore. Повтор `resume` идемпотентен; единственный matching backup и previous data не удаляются.
- [ ] Добавить tests: graceful 15m истёк → old keeps running/gate снят; lost reply при finish → нет rollback; candidate unhealthy → preserve failed data и restore matching backup; new schema blocked before stop; старый image не открывает candidate DB; quarantine не retry каждый tick.
- [ ] Выполнить `mise exec -- cargo test -p astra-updater --test apply --test recovery --locked`; ожидать FAIL.
- [ ] Реализовать транзакцию с offline `center backup` старым image, прежними userns/volume settings, верификацией backup, atomic unit replacement+fsync, health 120s и runtime image/build/TLS/vault identity checks. Gate остаётся при старте candidate.
- [ ] Реализовать restore в чистый named volume, сохраняя предыдущие данные до подтверждения. Проверить права и отсутствие второго SQLite owner. Recovery unit вызывает только `prepare-start`: проверяет/восстанавливает согласованность файлов, marker и unit, завершается успешно до center start и никогда не вызывает start центра из его зависимости. Обычный timer затем вызывает `resume` для health/commit. Center unit явно Requires/After recovery unit; его ошибка блокирует запуск; crash после Exposing продолжает валидацию/open path либо остаётся в maintenance, не откатывает данные.
- [ ] Ограничить staging/backup/image cleanup только известными transaction resources. Сохранять current/previous, active/latest-success backup и unresolved failed state; отказ по disk quota не вызывает prune.
- [ ] Native FCOS A→B→bad-C: доказать автоматическое обновление, health build, сохранение CA/vault/history, потерю старой RAM session, rollback до Exposing и отсутствие restore после него; reboot в промежуточных phases. Отчёт с actual digests и boot ids.
- [ ] Commit `feat(updater): replace images with recoverable offline backups`.

## Task 7: Bootstrap и координация обновлений FCOS

**Files:** Create `tools/release/updater-package.ts`, `tools/release/tests/updater-package.test.ts`, `deploy/updater/astra-reboot-coordinator.service`, `crates/updater/src/reboot/{mod,fleet_lock,policy}.rs`, `crates/updater/tests/reboot.rs`; modify `iac/fcos/config.bu`, `iac/fcos/files/zincati.toml`, `iac/fcos/README.md`, `iac/README.md`, `docs/center-deployment.md`.

**Interfaces:** Host command `coordinate-reboot` обслуживает loopback fleet-lock endpoint только для Zincati; не предоставляет произвольный exec/container management. `RebootPolicy::request(node, now) -> Grant | Deferred` учитывает окно Sun03:00–04:00UTC, app admission и updater lock. Повтор request для того же node идемпотентен; unknown service или journal означает Deferred. Используется официальный protocol contract, endpoint/port задаются в typed config.

- [ ] Написать tests busy/unknown/offline center, apply active, duplicate fleet-lock calls, outside window, ready gate survives до reboot, reboot отменён и grant корректно отозван, startup recovery после нового boot id. Unreachable coordinator не разрешает reboot.
- [ ] Выполнить `mise exec -- cargo test -p astra-updater --test reboot --locked`; ожидать FAIL.
- [ ] Реализовать bounded local coordinator и persistent grant/recovery, выбирая fleet-lock стратегию Zincati. Перенести прежнее periodic window в policy coordinator, чтобы не заявлять две одновременные стратегии. Права root Zincati и rootless socket явно проверить; не делать общий localhost admin API.
- [ ] Собирать отдельный host updater package в CI с exact versions/checksums, Cosign/trust; bootstrap устанавливает его один раз из проверенного локального пакета. Не добавлять secret в Ignition. Установка existing VM не повторяет Ignition: отдельная идемпотентная host install procedure, backup и совместимая версия центра обязательны.
- [ ] Прогнать Butane strict + ignition-validate; проверить resulting config и plaintext secret absence. В native FCOS доказать user units/linger после logout/reboot, socket SELinux mapping, blocked reboot при работе и разрешённый idle reboot. Измерить RSS coordinator отдельно.
- [ ] Обновить инструкции: initial installation, private registry credentials, upgrade от legacy image, pending host-version mismatch, pause delivery локальным disable timer для аварийного обслуживания, disk failure recovery. Обычные релизы не требуют SSH.
- [ ] Commit `feat(iac): bootstrap automatic delivery and coordinate OS reboots`.

## Task 8: Сквозная приёмка CI/CD и эксплуатационный runbook

**Files:** Create `tools/acceptance/delivery/{run,scenarios,report}.ts`, `docs/center-cicd.md`, `docs/center-cicd-acceptance.md`; modify `mise.toml`, `README.md`; extend `tools/acceptance/tsconfig.json` по необходимости.

**Interfaces:** `mise run test:updates` consumes explicit disposable FCOS target и test delivery source, пишет `target/acceptance/updates/<UUID>/report.json`. Harness требует явных target identity/ownership и никогда не использует production named volumes по умолчанию. Secrets не входят в report.

- [ ] Написать assertions сценариев: push A, быстрый push B, active cleanup, unknown receipt, offline registry, wrong signature, disk pressure, crash/reboot, unhealthy candidate. Assert image digest/build, durable operation id без повторного dispatch, data/vault/CA identity, maintenance phases и absence of leaked secrets.
- [ ] Реализовать harness с существующими acceptance/lib/process механизмами; добавить test:updates. Resource cleanup ограничить выделенной fixture VM/volumes и сохранить отчёт до удаления.
- [ ] Выполнить локальные `mise run check`, `mise run contracts:check`, `mise run web:check`, release/updater TS+Rust tests и delivery lifecycle. Зафиксировать version/pins; warnings разобрать отдельно.
- [ ] После разрешения внешнего теста: GitHub test publish → native FCOS test auto update → actual reboot coordinator; затем отдельно acceptance на целевом Hyper-V x86_64. Письменно разделить source PASS, CI PASS, native FCOS PASS, Hyper-V PASS/NOT RUN и Windows-2016 agent PASS/NOT RUN.
- [ ] В runbook указать egress endpoints GitHub/API/assets/GHCR/CDN и FCOS update sources, credentials renewal, signature trust rotation, backup off-VM, manual recovery, канал/version, retained images и отсутствие guaranteed zero downtime. Измерить CI/build/download/drain/switch durations, disk peak и RSS; не выдавать allocations за measured minimum.
- [ ] Выполнить один итоговый review реализации, исправить подтверждённые дефекты, повторить затронутые проверки. Перед production activation получить разрешение на конкретный registry/repository/VM target и первый rollout.
- [ ] Commit `test(delivery): verify automatic image rollout and recovery`.

## Порядок и итоговые критерии

Task 1 → Task 2 → Task 3; Task 4 → Task 5 → Task 6; Task 7 → Task 8.
Task 1–3 можно подготовить до host update, но production promotion не активируется,
пока Tasks 4–8 не прошли на тестовой площадке. Task 7 является обязательным для
production graceful гарантии при автоматических обновлениях FCOS.

Готовность: обычный push в main после green CI сам обновляет idle центр; active
операция завершается без повторного запуска; плохой/частичный релиз не заменяет
рабочий; pre-exposure failure восстанавливается; reboot/crash сохраняет данные и
не обходит admission. Пользовательские ключи не требуются для discovery/backup/
health; обычный login после restart разблокирует vault согласно прежнему решению.

## Self-review

- Существующая offline поставка, ручной agent update и единый build сохранены.
- Пользовательское уточнение учтено: нет обязательной команды установки на каждый релиз.
- Каждый Review Focus связан с конкретными negative/race/fault tests.
- Admission, host transaction и OS reboot имеют отдельных owners и явные interfaces.
- Нет обещания побайтовой воспроизводимости APT, мгновенного update, zero downtime,
  rollback после новых операций или полной автоматизации при неизвестном agent state.
- Начальная установка updater и последующая ротация host tooling различаются.
- Публикация/permissions/production activation являются явно выделенными внешними шагами.
