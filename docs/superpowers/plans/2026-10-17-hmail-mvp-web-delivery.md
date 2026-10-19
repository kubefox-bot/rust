# hMail Cleaner MVP: Web and Delivery — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Подключить согласованный интерфейс к Rust API и выпустить проверяемую OCI-поставку под Podman + Quadlet с Windows-агентами.

**Architecture:** apps/web — единственное frontend-приложение Bun workspace, Astro собирает статические страницы и клиентские Svelte-острова. Rust обслуживает их вместе с API/SSE. Финальный контейнер содержит release-бинарник, read-only web assets и доверенные пакеты агентов; постоянные данные вынесены в volume.

**Tech Stack:** Bun, Astro static, официальная @astrojs/svelte, Svelte 5, TypeScript, CSS, Rust Axum/tower-http, rootless Podman + Quadlet/systemd, Playwright API под Bun.

**Spec:** [Спецификация](../specs/2026-10-17-hmail-control-center-design.md), [общий план](2026-10-17-hmail-control-center-mvp.md). Требуются agent API части A и browser API части B.

## Global Constraints

- Astro static + Svelte; рабочий контейнер запускает один Rust-процесс приложения.
- Русский UI, светлая рабочая область и графитовый sidebar.
- Sidebar содержит только «Очистка почты»; «История» относится к выбранному серверу в шапке; «Настройки» рядом с SA.
- Управление и прогресс — один основной остров; history/settings загружаются по первому открытию.
- Один SSE на вкладку, bounded journal, server_id/job_id явно передаются в командах.
- Футер: версия/сборка центра и свежесть/версии агентов; раскрытый журнал сохраняет контекст и футер.
- Технические настройки/ключи не попадают в static build, localStorage или журнал.
- Реальные Windows-проверки, контейнерный smoke и измерения фиксируются раздельно.

## Review Focus

- Прямая ссылка на /history или /settings работает из Rust после refresh — C1, C4.
- Вторая вкладка/смена сервера не меняет цель открытого подтверждения — C2.
- Переподключения/навигация/полноэкранный журнал не умножают SSE или DOM — C1, C2.
- Init/HTTPS до входа и пересоздание контейнера не теряют identity/SQLite — C4.
- Обновление со schema migration не обещает rollback старым образом поверх новой БД — C4, C5.

---

## C1. Bun workspace, static shell и сгенерированные API-контракты

**Files**

- Create: package.json, bun.lock; apps/web/{package.json,astro.config.mjs,tsconfig.json}.
- Create: apps/web/src/pages/{index,login,history,settings}.astro; apps/web/src/layouts/Shell.astro.
- Create: apps/web/src/styles/{tokens,global}.css; apps/web/src/config/build.ts.
- Create: apps/web/src/api/{index,types,conts,request,decode,events}.ts.
- Create: apps/web/src/api/generated/{browser.schema.json,browser.types.ts,validators.js}.
- Create: crates/protocol/examples/export_browser_schema.rs; tools/web/{generate-contracts,check-contracts}.ts.
- Create: apps/web/tests/{contracts,events}.test.ts.
- Create: crates/center/src/http/static_files.rs; crates/center/tests/static_files.rs.
- Modify: crates/center/src/{config,app}.rs, crates/center/src/http/routes.rs, crates/center/Cargo.toml.
- Modify: crates/protocol/Cargo.toml, crates/protocol/src/browser/mod.rs, mise.toml, Cargo.lock.

**Interfaces**

- Consumes: browser DTO из protocol; auth/session, servers, jobs/history/settings endpoints B3–B7.
- Produces: requestJson({path, method, body, responseSchema}) с проверенным типом ответа; X-HMail-Request:1 и JSON для изменяющих API, same-origin credentials.
- Produces: subscribe({serverId, onEvent, onResync, onUnauthorized}) -> () => void; один owner SSE в пределах страницы/вкладки.
- Produces: frontend checks mise run web:check, build mise run web:build, воспроизводимая генерация mise run contracts:check.
- Produces: Rust-раздача /, /login, /history, /settings и assets из явно заданного StaticDir; API 404 не превращается в HTML.

- [x] **Step 1: Написать tests контрактов/событий и сетевые assertions shell.**

~~~ts
// counters_remain_exact
expect(decoded.summary.removed_bytes).toBe("9007199254740993");
// malformed_event_requires_resync
expect(resyncRequests).toBe(1);
// subscriptions_share_one_transport
expect(openEventSources).toBe(1);
// release_last_subscriber_closes_transport
expect(openEventSourcesAfterDispose).toBe(0);
// changed_stream_epoch_requires_fresh_snapshot
expect(resyncRequestsAfterEpochChange).toBe(1);
~~~

  Schema test отклоняет неизвестный phase, invalid UUID/counter overflow; generated output не содержит AgentCredentials/private-key DTO. На странице очистки до открытия history/settings число их API-запросов и загрузок island chunks равно нулю.
- [x] **Step 2: Проверить RED.** bun test --cwd apps/web tests/contracts.test.ts tests/events.test.ts после создания минимального test entry. Ожидается FAIL до client implementation.
- [x] **Step 3: Собрать shell и generation.** Root package private с workspaces=["apps/web"]; Cargo/Bun lockfiles отдельные. Закрепить Bun 1.4.3 и последние стабильные совместимые версии Astro/Svelte/integration на момент выполнения в lockfile, без canary и floating latest в build. Проверить registry engines/peerDependencies. Astro output=static, без adapter; четыре prerendered страницы. Выбрать обычные MPA-переходы без автоматического prefetch: отдельная страница загружает свой остров при открытии. Основная очистка client:load; history/settings не импортируются её entry.

  JSON Schema генерируется из allowlist Rust browser DTO (schemars), TypeScript — json-schema-to-typescript, validators — Ajv standalone. Для формата uint64-decimal зарегистрировать проверку канонической десятичной строки и диапазона через BigInt; одной проверки string/pattern недостаточно. У Bun нет встроенного генератора этих контрактов; npm-инструменты запускаются Bun и не остаются в runtime-контейнере. Проверить генерацию без Node в PATH. Не дублировать DTO вручную. Изменения типов/fixtures должны попадать в тот же коммит.

  Оформление — CSS variables и локальные SVG, без отдельного UI-kit пакета.
  Конфигурацию читает только config owner; в браузер экспортируются лишь
  публичные build значения; API prefix — owner-local constant.
  Rust ServeDir обслуживает точные static routes, MIME/cache/security headers
  и не открывает /data через traversal. Отдельной SPA fallback на /api нет.
  Реальное состояние сессии загружается
  из API, placeholder SA не означает авторизацию.
- [x] **Step 4: Проверить GREEN.** bun install --frozen-lockfile; mise run contracts:check; mise run web:check; mise run web:build; cargo test -p hmail-cleaner-center --test static_files --locked. Эти команды создать в mise/package.json здесь; первичное разрешение зависимостей выполняется перед frozen install и фиксируется lockfile. web:check выполняет Astro/Svelte/TS checks и Bun tests; bundle не содержит fixture-secret sentinels или Node server output. Передавать серверные данные при сборке не требуется.
- [x] **Step 5: Локальный коммит.** git commit -m "feat: build static Astro shell and typed browser api".

## C2. Очистка, подтверждение, прогресс и раскрываемый журнал

**Files**

- Create: apps/web/src/features/cleanup/{index,types,conts,model,api}.ts.
- Create: apps/web/src/features/cleanup/{CleanupIsland,ServerHeader,RunForm,Confirmation,ProgressPanel,Journal}.svelte.
- Create: apps/web/src/shell/{navigation,footer,auth}.ts.
- Create: apps/web/src/features/auth/{index,types,api}.ts; apps/web/src/features/auth/LoginIsland.svelte.
- Create: apps/web/tests/cleanup.test.ts; apps/web/tests/browser/fixture.ts, apps/web/tests/browser/cleanup.test.ts.
- Create: crates/center/examples/browser_fixture.rs; apps/web/tests/browser/config.ts.
- Modify: apps/web/src/pages/{index,login}.astro, apps/web/src/layouts/Shell.astro, apps/web/package.json.

**Interfaces**

- Consumes: requestJson/subscribe; resolve/preview/submit/cancel; persisted JobView и generation-aware auth.
- Produces: CleanupIsland; reducer transition(state: CleanupState, event: CleanupEvent) -> CleanupState.
- Produces: ConfirmedRun с serverId, profileVersion, previewJobId, resolvedCutoff, scan, servicePolicy; после открытия подтверждения цель immutable.
- Produces: login/logout UI; footer health reader и единственный event transport; test:browser script на Bun + Playwright API.
- Produces: browser_fixture использует public center initialize/run и test-only fixtures B5, реальные production assets и ephemeral данные; в рабочем бинарнике такого режима нет.

- [x] **Step 1: Написать state/browser tests.**

~~~ts
// changing_parameters_invalidates_preview
expect(next.preview).toBeNull();
// two_tabs_cannot_retarget_confirmation
expect(submitted.server_id).toBe(serverFromFirstTab);
// cancelling_is_not_terminal_until_restore
expect(screenStatus).toBe("Восстановление службы");
// unknown_total_has_no_fake_percent
expect(progressPercent).toBeNull();
// expand_and_escape_preserve_job_and_subscription
expect(jobIdAfterEscape).toBe(jobIdBeforeExpand);
expect(activeSseConnections).toBe(1);
~~~

  Browser assertions: full preview при Running показывает требование остановленной службы без Stop request; managed clean требует отдельного подтверждения; logout не отправляет cancel; stale events другого job/server не подменяют экран. Неизвестный dispatch не показывает кнопку слепого повторного clean.
  Browser Back/BFCache после logout не показывает прежний технический профиль:
  pagehide закрывает подписку/скрывает авторизованный UI, pageshow повторно
  проверяет сессию до открытия данных. HTTPS browser fixture использует
  собственное явно настроенное доверие; общий ignoreHTTPSErrors не подменяет
  проверку TLS. Ручной импорт CA в обычный браузер проверяется отдельно в C5.
- [x] **Step 2: Проверить RED.** bun test --cwd apps/web tests/cleanup.test.ts; bun run --cwd apps/web test:browser cleanup. До компонентов/fixture ожидается FAIL.
- [x] **Step 3: Реализовать компоненты.** Главный остров владеет выбором сервера, формой, подтверждением и прогрессом; отдельные маленькие компоненты получают typed props. Server truth не дублируется глобальным UI store. Форма показывает server-local cutoff из ответа агента; до preview server/profile/параметры валидируются backend.

  Раскрытие журнала — локальный layout-state: скрыть sidebar/header/form,
  оставить server context/progress/footer, не размонтировать job owner.
  Escape и «Свернуть» возвращают вид/фокус; окно 1000 записей сохраняется.
  Результаты selected/removed/free disk не смешивать; null verification
  показывать как незавершённую сверку. Ошибки API не заменять пустыми данными.
  При смене страницы подписка освобождается; новая получает снимок.
- [x] **Step 4: Проверить GREEN.** mise run web:check; bun run --cwd apps/web test:browser cleanup. Проверить ширины 1440/1024/736/375/320, keyboard/focus/Escape, reconnect и два tabs. Тесты должны пройти с production assets, отдаваемыми Rust fixture, без Astro dev-server.
- [x] **Step 5: Локальный коммит.** git commit -m "feat: control cleanup with confirmed targets and live progress".

## C3. История, настройки, агенты и учётная запись

**Files**

- Create: apps/web/src/features/history/{index,types,api}.ts; HistoryIsland.svelte, HistoryTable.svelte в том же owner.
- Create: apps/web/src/features/settings/{index,types,conts,api}.ts.
- Create: apps/web/src/features/settings/{SettingsIsland,Agents,Enrollment,Profile,Account,Recovery}.svelte.
- Create: apps/web/tests/browser/{history,settings,enrollment,account}.test.ts.
- Modify: apps/web/src/pages/{history,settings}.astro, apps/web/src/shell/{navigation,footer}.ts.

**Interfaces**

- Consumes: history cursor API, profile patch Keep/Replace, host-key challenge/install, agent health/update, password/recovery/session settings.
- Produces: HistoryIsland({serverId}); SettingsIsland с выбранным AgentId/ProfileId; публичные entrypoints index.ts только с необходимыми экспортами.
- Produces: реальные загрузки history/settings только при переходе на их страницы; данные выбранного сервера сохраняются в URL как несекретный UUID.

- [x] **Step 1: Написать browser assertions.**

~~~ts
// history_is_scoped_and_paginated
expect(request.server_id).toBe(selectedServer);
expect(renderedHistoryRows).toBeLessThanOrEqual(50);
// unchanged_password_uses_keep
expect(profilePatch.password.kind).toBe("keep");
// host_key_confirmation_precedes_credentials
expect(credentialRequestsBeforeTrustConfirmation).toBe(0);
// installed_is_not_ready_without_valid_mail_profile
expect(agentLabel).toBe("Требуется настройка");
// recovery_key_is_not_persisted_in_browser
expect(persistedBrowserValues).not.toContain(recoveryKey);
~~~

  Проверить loss of install response → attention/reconcile; error ≠ empty history; session timeout options 15/30/60/1440; credential commit → обычный login; одна вкладка выходит после нового входа из другой browser context.
- [x] **Step 2: Проверить RED.** bun run --cwd apps/web test:browser history settings enrollment account. Ожидается FAIL до реализации соответствующих страниц.
- [x] **Step 3: Реализовать разделы.** В sidebar один модуль, history в серверной шапке, settings рядом с SA. Настройки включают agents/add/update, технический профиль с явной целью и account/password/recovery. Сохранённые passwords/private keys не запрашивать на чтение; форму изменений не писать в localStorage. Recovery candidate показывается один раз и подтверждается до commit. В футере различать online/stale/offline, показывать observed_at и версии без фонового Data scan. Не добавлять роли, scheduler, кнопку lock или будущие пустые модули.
- [x] **Step 4: Проверить GREEN.** mise run web:check; bun run --cwd apps/web test:browser history settings enrollment account. Проверить начальную lazy загрузку и повторные открытия без накопления listeners/SSE; backend и browser validation обязательны, snapshots сами по себе не достаточны.
- [x] **Step 5: Локальный коммит.** git commit -m "feat: manage server history agents and account settings".

## C4. Podman + Quadlet, persistent data, обновление и воспроизводимая поставка

**Files**

- Create: deploy/Containerfile, deploy/quadlet/hmail-center.container, deploy/quadlet/hmail-center-data.volume, deploy/center.example.json, .containerignore.
- Modify: crates/center/src/http/{static_files,health}.rs, crates/center/tests/static_files.rs.
- Create: tools/release/{config,build,manifest,verify}.ts; tools/release/tests/delivery.test.ts.
- Modify: crates/center/src/http/routes.rs, crates/cli/src/args.rs, mise.toml, docs/center-deployment.md.
- Modify: crates/cli/build.rs; передать одну проверенную build/version/Git identity в binary, frontend и release manifest.

**Interfaces**

- Consumes: Rust center feature build, apps/web/dist, agent package/manifest A5.
- Produces: mise run release:local → локальный versioned image + manifests, без push.
- Produces: mise run test:delivery → lifecycle контейнера на disposable volume.
- Produces: rootless Quadlet → hmail-center.service с постоянным data volume; автозапуск через пользовательский systemd manager с linger.
- Produces: проверяемая последовательность обновления образа по digest на Linux-хосте; контейнер центра не имеет доступа к Podman API/socket.
- Produces: center backup и center health; health через HTTPS работает без login/DEK и не открывает DB вторым владельцем.
- Produces: GET /healthz, статические /, /login, /history, /settings и hashed assets из того же Axum server.

- [x] **Step 1: Написать routing/delivery tests.**

~~~ts
// production_image_does_not_require_js_runtime
expect(runtimeProcesses).toEqual(["hmail-cleaner"]);
// recreate_preserves_vault_and_identity
expect(after.vault_id).toBe(before.vault_id);
expect(after.agent_trust).toEqual(before.agent_trust);
// restart_requires_login_but_agent_keeps_running
expect(centerSession).toBeNull();
expect(agentRunnerCalls).toBe(1);
// rollback_restores_matching_backup
expect(restored.schema_version).toBe(previous.schema_version);
// rootless_unit_starts_without_interactive_login
expect(afterUserLogout.centerHealthy).toBe(true);
// selected_digest_and_same_data_volume_are_used_for_init_and_run
expect(running.imageDigest).toBe(selected.imageDigest);
expect(running.dataVolume).toBe(initialization.dataVolume);
// scheduled_auto_update_is_not_enabled_by_default
expect(deployment.autoUpdateEnabled).toBe(false);
~~~

  Rust static tests: direct /history refresh → HTML 200; API 404 не возвращает HTML; traversal не открывает /data; correct MIME/cache/security headers. Init на пустом named volume, health до login, bind-mounted ownership failure дают различимые исходы.
- [x] **Step 2: Проверить RED.** cargo test -p hmail-cleaner-center --test static_files; bun test tools/release/tests/delivery.test.ts. Контейнерные проверки запускать только после готовности Linux fixture с rootless Podman, systemd и cgroup v2; генерацию/запуск Quadlet проверять именно в ней.
- [x] **Step 3: Реализовать поставку.** Multi-stage Bun frontend build → Rust 1.99.1 build → минимальный Linux runtime. На этапе выполнения проверить и закрепить digest базовых образов; final stage не копирует Bun/Node/compiler. Статика — read-only файлы внутри образа, не отдельный процесс; StaticDir задаёт configuration owner. Пользователь контейнера non-root, порт 8443; data volume /data с SQLite/bootstrap/config; graceful shutdown.

  Подготовка Linux-ВМ: служебный пользователь, rootless Podman, cgroup v2,
  корректные subuid/subgid и пользовательский systemd manager с linger.
  Quadlet-файлы .container и .volume помещаются в пользовательский search path;
  постоянный том получает фиксированное имя hmail-center-data. В .container
  задать Image по digest и [Install] WantedBy=default.target. После daemon-reload
  управлять сгенерированной hmail-center.service через systemctl --user;
  не выполнять enable для временно сгенерированной service.
  Первый запуск документировать как interactive podman run --rm для center init
  с тем же образом, named volume и отображением UID/GID, затем запуск Quadlet-службы.
  Пользователь переносит публичный CA
  в доверие своего браузера; private key никуда не экспортируется.
  Обновление на Linux-хосте: скачать и проверить выбранный immutable image
  по digest → остановить Quadlet-службу центра → согласованный backup
  SQLite/bootstrap/config → изменить Image → daemon-reload/start → migrations
  → health/version до login → обычный login/reconcile. Backup выполняется
  командой из прежнего образа после остановки центра, без второго владельца БД.
  Старый image нельзя запустить поверх несовместимой новой схемы:
  rollback восстанавливает соответствующий backup. Accepted agent job
  продолжается; restart центра не повторяет clean.

  В контейнер не монтировать Podman API/socket. Собственный updater-контейнер
  не создавать. AutoUpdate=registry и штатный podman-auto-update.timer по
  умолчанию не включать: они не заменяют backup/migration workflow, а pinned
  digest не предназначен для слежения за изменяемым тегом. Получение образов
  требует только исходящего HTTPS; входящие подключения из интернета для
  доставки не нужны. Проверить права постоянного volume у rootless UID/GID
  при init, run, backup и restore, а также отсутствие секретов в unit-файлах.

  Release-инструменты на Bun используют typed config, существующий EXE
  build helper и проверенные manifest/hash; не создают второй упаковщик
  Windows с тем же контрактом. CI/CD вынесен на отдельный этап после MVP:
  workflows сборки/публикации/доставки сейчас не создавать. Локальные команды
  проверки и сборки сохраняются для будущего переиспользования в CI/CD;
  установка и обновление центра документируются как ручные операции.
  В доступных изолированных средах Linux проверяет center/web/container;
  Windows — agent/SCM; macOS/Windows center smoke сохраняют переносимость
  отдельно от OCI-сборки. Образ центра собирать Podman из deploy/Containerfile;
  существующий EXE build helper переиспользовать без попутной миграции его
  контейнерного инструментария. Feature isolation проверяется по артефактам.
- [x] **Step 4: Проверить GREEN.** mise run check; mise run web:check; mise run web:build; mise run release:local; mise run test:delivery. Проверить rootless Quadlet startup после logout/reboot, генерацию systemd unit, recreate/backup/restore/upgrade с active fixture job, Linux amd64 и прямые маршруты из Rust. Отдельно подтвердить health до разблокировки, сохранность named volume, старый digest + matching DB backup при rollback и отсутствие второго запуска clean. Ожидается PASS; ни push, ни изменение рабочих ВМ не выполнять.
- [x] **Step 5: Локальный коммит.** git commit -m "build: deliver the Rust center with Podman and Quadlet".

Доступная проверка: immutable Linux amd64 release и Docker/Colima lifecycle пройдены. Native rootless Podman/Quadlet, logout/reboot и работа отдельного Windows-агента остаются NOT RUN; см. итоговую приёмку.

## C5. Полная приёмка, сбои и ресурсный отчёт

**Files**

- Create: tools/acceptance/{config,run,measure}.ts; tools/acceptance/tests/scenarios.test.ts.
- Create: docs/mvp-acceptance.md; docs/mvp-resource-report.md.
- Modify: mise.toml, docs/agent-installation.md, docs/center-deployment.md, docs/recovery.md.

**Interfaces**

- Consumes: локальный release image, оба пакета, fixtures, явно переданные тестовые endpoints/paths.
- Produces: mise run test:mvp — две fixture identities/agent targets; execution report PASS/FAIL/NOT RUN по каждой среде.
- Produces: mise run measure:mvp — release RSS/CPU/container/JS/DOM показатели с описанием машины, версий, нагрузки и времени измерения.

- [x] **Step 1: Закрепить сценарии приёмки.** Harness должен отвергать target без fixture marker, имя рабочей почтовой службы и пути вне выделенного synthetic root. Для каждого сценария зафиксировать assertions:

~~~ts
// two_servers_and_two_tabs
expect(cleanedServerIds).toEqual([confirmedServerId]);
// lost_response_and_restart
expect(executionsForJobId).toBe(1);
// disk_error_and_agent_crash
expect(displayedSuccess).toBe(false);
// thirty_minute_slow_consumer_run
expect(maxJournalDomRows).toBeLessThanOrEqual(1000);
expect(maxQueuedEvents).toBeLessThanOrEqual(256);
// manual_only
expect(cleanRequestsAfterIdleOrProcessRestart).toBe(0);
~~~

  Отдельные scenarios: full preview/fast preview, unmanaged/managed clean, restore failure, cancel, invalid/expired/revoked cert, credential races/recover, container upgrade/rollback. Верифицировать фактические файлы/SQL и состояние SCM, а не только UI text.
- [x] **Step 2: Проверить RED harness.** bun test tools/acceptance/tests/scenarios.test.ts проверяет отказ для non-fixture target и detection намеренно подставленного duplicate execution/oversized queue. Не запускать разрушительные кейсы против доступных по умолчанию серверов.
- [x] **Step 3: Подготовить и выполнить изолированную приёмку.** Linux с rootless Podman + Quadlet/systemd + два synthetic Windows Server 2016 targets с реальными FS attributes/SCM и MySQL fixture. Если среды нет, завершить доступные проверки, а native test оставить NOT RUN с точной причиной; не выдавать windows-2022/cross-build за Server 2016. Отдельно проверять фактическую совместимость заранее установленного OpenSSH.
- [x] **Step 4: Снять измерения и проверить GREEN.** mise run test:mvp; mise run measure:mvp. Измерить отдельно центр idle, login peak, два connected agents, открытый UI, fast/full/cancel, длительный slow SSE; агент idle/cleanup; image/EXE size; initial JS, lazy chunks, DOM и browser memory. Хранить воспроизводимые исходные значения и вывод, без выдуманного бюджета RSS или обещанного выигрыша над Bun. Все доступные сценарии PASS; недоступные не скрываются.
- [x] **Step 5: Локальный коммит.** git commit -m "test: verify MVP delivery recovery and resource limits".

Доступная приёмка и измерения завершены для исходников `b09d0d04237f`,
поставка `0.1.0+b09d0d04237f.1792284226`. Результаты и исходные значения:
[приёмка](../../mvp-acceptance.md), [ресурсы](../../mvp-resource-report.md).
Native Windows Server 2016, rootless Podman/Quadlet, реальная почтовая нагрузка
и длительный slow SSE остаются NOT RUN; локальные результаты их не заменяют.

## Готовность MVP

В отчёте раздельно отмечены source/tests, Podman/Quadlet на Linux, SSH/SCM на Server 2016,
browser и resource measurements. После этого подготовлены конкретные
артефакты и эксплуатационные команды для отдельного решения о публикации
и рабочем развёртывании. Рабочая почта не является тестовыми данными.
