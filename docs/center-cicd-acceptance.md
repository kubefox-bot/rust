# Приёмка автоматической доставки Astra

Дата: 2026-10-18. Это отчёт о проверенных границах текущей реализации. Запись
сценария или workflow не означает его выполнение на GitHub или целевой ВМ.
Процедуры эксплуатации описаны в [runbook](center-cicd.md).

## Итог локального исполнения

- `mise run check` после исправлений review: **361 passed / 0 failed / 32 ignored / 0 warnings**, exit 0. Включает formatting, workspace tests, Clippy и standalone build-tool tests. Ignored требуют отдельных MySQL/Windows окружений.
- `mise run release:test`: **40 passed / 0 failed**, strict TypeScript PASS.
- `mise run web:check`: **27 passed / 0 failed**, Astro/Svelte/TypeScript без diagnostics.
- `mise run contracts:check`: browser contracts воспроизводимы.
- Update settings: **6 backend + 5 browser tests PASS**.
- Actionlint обеих новых workflows PASS; pinned Butane/Ignition validation PASS.
- Реальная локальная проверка официального Cosign bundle PASS; tamper и чужая identity отвергнуты. Это не доказательство подписи Astra workflow.
- F1: потерянный ответ при открытии старой версии — RED → GREEN; данные после открытия не восстанавливаются повторно.
- F2: pull съел запас диска — RED → GREEN; maintenance не начинается.
- F3: cached generated unit при boot — RED → GREEN; выбор прежнего Quadlet предшествует reload и startup. Проверка использует порт с моделью generator cache, а не native systemd.

Полная сборка новой OCI-поставки, Linux updater package и rollout на FCOS в
этом исполнении не запускались. Внешние workflow/публикация/production не
активированы. Исходники механизма готовы к выделенной target-приёмке.

## Центр: локальные проверки

| Проверка | Результат | Доказательство |
|---|---|---|
| Новый admission закрывается после begin; существующий writer завершает работу | PASS | `crates/center/tests/maintenance.rs` |
| Durable marker блокирует admission после открытия store заново | PASS | `durable_marker_survives_restart_and_corruption_fails_closed` |
| Повреждённый marker не даёт открыть store | PASS | Тот же тест |
| Неизвестный remote job slot не становится ready | PASS | `uncertain_durable_remote_slot_never_reports_ready` |
| Между наблюдением slot и seal нет admission gap | PASS | `maintenance::gate::tests::admission_revision_closes_slot_observation_gap` |
| Profile/identity domain entrypoints закрыты, reconcile разрешён во время drain | PASS | `domain_admission_rejects_new_work_but_reconcile_remains_available_while_draining` |
| Browser login и reconcile доходят до auth во время drain, новые mutations отклоняются | PASS | `browser_login_and_reconciliation_remain_possible_only_before_ready_seal` |
| После ready новые continuation mutations закрыты | PASS | Gate и browser tests |
| JSON protocol version, размер frame и владелец UUID проверяются | PASS | `maintenance::socket::tests::local_protocol_is_bounded_versioned_and_operation_owned` |
| Snapshot включает policy и исключает transient maintenance marker | PASS | `offline_backup_retains_update_policy_and_excludes_transient_marker` |
| Все файлы backup имеют mode 0600 на Unix | PASS | Тот же тест: RED 0644 → GREEN 0600 |
| Marker symlink отклоняется без изменения цели | PASS | `symlink_marker_is_rejected_without_reading_or_changing_its_target` |
| Чужой UID peer получает только безопасный unauthorized | PASS | `maintenance::socket::tests::untrusted_uid_receives_only_safe_unauthorized_response` |
| Vault UUID читается существующим SQL owner; повреждение не скрывается | PASS | `maintenance::gate::tests::local_snapshot_reads_vault_identity_from_owned_pool_and_rejects_corruption` |

Тесты выполняются на локальной macOS с Unix socket. Они не доказывают SELinux
mapping socket внутри rootless контейнера на FCOS. Финальные targeted проверки maintenance: 7 integration tests и 4 unit tests —
PASS. Они включают marker symlink, чужой UID, vault identity, admission race и
backup modes. Полный suite центра перед последним расширением vault identity:
103 passed, 0 failed, 0 ignored; warnings отсутствуют.

## Остальные уровни

| Уровень | Результат |
|---|---|
| Полный локальный workspace после review | PASS: 361 passed / 0 failed / 32 ignored; targeted maintenance — 11 passed |
| `cargo clippy -p hmail-cleaner-center --all-targets --locked` | PASS; warnings отсутствуют |
| Локальные ссылки runbook и acceptance | PASS |
| Политика обновлений: authenticated API, CSRF, bounds, persistence | PASS: 6 backend + 5 browser tests; не является native delivery |
| Descriptor/publishing/updater/reboot source tests | PASS в release:test и финальном workspace suite |
| Реальный GitHub Actions run | NOT RUN |
| Публикация OCI в GHCR и keyless signature реальным workflow | NOT RUN |
| Изолированная FCOS: automatic A → B → bad C, rollback, power/reboot faults | NOT RUN |
| SELinux + rootless Podman + UID peer mapping на FCOS | NOT RUN |
| Реальное обновление ОС через Zincati/FleetLock | NOT RUN |
| Hyper-V x86_64 организации | NOT RUN |
| Windows Server 2016 agents/mail end-to-end после новой доставки | NOT RUN |
| Production activation / первый rollout | NOT RUN |

Native evidence прежнего bootstrap находится в
[отчёте FCOS IaC](fcos-iac-acceptance.md). Та проверка не включала нынешнюю
автоматическую доставку приложения и не заменяет её приёмку.

## Что записать при native приёмке

Для каждой выделенной тестовой ВМ: ОС/архитектура, boot ID, версии Podman/Quadlet,
SELinux, фактические image/config digests, build и release sequence. Секреты и
исходные credentials в отчёт не добавляются.

Обязательны: быстрые последовательные релизы; active job и unknown receipt;
отключённый registry; неверная подпись; несовместимая schema/updater; недостаток
места; неуспешный candidate; остановка процесса и reboot на границах journal;
потеря ответа finish после Exposing; подтверждение отсутствия повторного dispatch.

Перед открытием новой версии сверяются vault/CA/HTTPS identity и история. После
Exposing автоматическое восстановление прежних данных должно быть запрещено.
Для ОС отдельно проверяется отказ reboot при работе и восстановление grant после
смены boot ID. Фиксируются время CI/download/drain/switch, peak disk и RSS updater,
coordinator и центра; выделенные ресурсы ВМ не выдаются за measured minimum.

## Решения и ограничения исполнения

| Решение | Причина и цена |
|---|---|
| Новая ветка в текущем checkout | Запрос пользователя; отдельного worktree и изоляции параллельных checkout нет |
| Два параметра центра: 60 секунд и 15 минут по умолчанию | Host refresh каждые 30 секунд; discovery округляется до ближайшего tick, текущая transaction сохраняет snapshot политики |
| Параллельная работа по отдельным владельцам файлов | Ранее разрешена пользователем; интеграция общих контрактов проверяется общим suite |
| Rust updater на хосте, Bun только в tooling | Не добавляет JS runtime в FCOS; bootstrap updater устанавливается отдельно |
| Offline restore в существующий named volume | Podman не предоставляет атомарную подмену имени volume; восстановление занимает время и требует native crash acceptance |
| Recovery Draining только снимает свой gate | Не останавливает ещё выполняющуюся работу; retry может ждать reconcile |
| Host updater/recover без NoNewPrivileges | Rootless UID mapping использует setuid helpers; права host account остаются обычными. Контейнерные ограничения сохранены. Узкий локальный повтор после первоначального auto-review rejection одобрен |
| Удалён только disposable incremental cache | Освобождён диск для проверок; следующая сборка теряет incremental ускорение |
| Кэш verified metadata с owner-only доступом | Уменьшает повторы скачиваний и Cosign; изменение asset ID вызывает refetch, повреждение блокирует обновление |
| Локальный acceptance harness отдельно от target acceptance | Source tests не подтверждают работу FCOS/Hyper-V/Windows и production |
| GitHub OIDC, private GHCR и credentials не проверены на реальном релизе | Внешние операции не разрешены; требуется выделенная приёмка и включение публикации |
| FCOS SELinux/UID/systemd/power loss/rollback не проверены на новой ВМ | Порты тестируют решения и порядок; production readiness не подтверждена |
| Zincati и Hyper-V остаются NOT RUN | Требуется фактический reboot на тестовом target |
| Windows Server 2016 mail end-to-end остаётся NOT RUN | Эта поставка не обновляет агенты автоматически; target compatibility проверяется отдельно |
| Validators проверены reproducibility/contracts тестами | Review охватывает diff и затронутые consumers; каждый generated body и весь прежний MVP повторно не пересматриваются |
| RSS, задержки и устойчивость к потере ВМ не измерены | Нельзя заявлять minimum ресурсов; same-disk snapshot не заменяет внешнюю резервную копию |
| Один согласованный implementation commit | Общие контракты проверены вместе; bisect имеет более крупный шаг |

Независимый review рассмотрел исходный tree
`8aec8f76c7ee0ca4252e65f73e3a095f02edd397` от base `0d320b9`.
Три замечания приняты и исправлены одним проходом; каждый regression сначала
упал, затем прошёл. Повторный review не запускался. Deferred minors отсутствуют.
