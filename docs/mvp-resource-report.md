# Ресурсы MVP

**Статус: ожидается подтверждённый запуск измерений.** Это каркас отчёта по
текущему harness. Значения «ожидается» заполняются из одного успешного
`target/acceptance/resources.json`; численные результаты пока не внесены.
Нулевое значение не заменяет отсутствующее измерение.

Отчёт предназначен для native release-центра на macOS. Его результаты
характеризуют указанную машину и нагрузку. Linux Podman/Quadlet, native
Windows-агент, настоящая почта и длительный SSE-прогон требуют отдельных
измерений. Готовность окружений отражается в [приёмке MVP](mvp-acceptance.md).

## Источники и запуск

Локальные исходные данные сохраняются в ignored-каталоге `target/acceptance`:

- `resources.json` — результат, машина, версии, методика, все samples и
  browser snapshots, размеры артефактов и причины `NOT RUN`.
- `measurement-build.json` — build receipt, созданный после общей сборки
  native центра, release fixture и статического web.
- При переданном release manifest — `target/releases/BUILD/release-manifest.json`
  и проверяемые им файлы доставки. `BUILD` здесь является заполнителем.

```sh
mise run measure:mvp
```

Для явно разрешённой локальной сборки из незакоммиченных исходников:

```sh
HMAIL_RELEASE_ALLOW_DIRTY=1 mise run measure:mvp
```

Необязательный `HMAIL_MVP_RELEASE_MANIFEST` указывает на конкретный manifest
внутри `target/releases`. Без него размер OCI archive и Windows-пакета
получает `NOT RUN`. Измерения выполняются после завершения другой сборочной
нагрузки; нагрузка машины до и после запуска остаётся частью отчёта.

`HMAIL_MVP_MEASURE_SAMPLE_MS` задаёт интервал выборок,
`HMAIL_MVP_MEASURE_WINDOW_MS` — минимальное окно. Настройки по умолчанию в
исходниках: 50 ms и 3000 ms. Фактические значения берутся из
`methodology.requestedSampleIntervalMs` и `methodology.minimumWindowMs`, а
длительность каждого окна — из `native[].summary.durationMs`.

Прямой запуск `tools/acceptance/measure.ts` требует уже подготовленного
`measurement-build.json` с совпадающими исходниками и байтами артефактов.

## Запуск и идентичность сборки

| Поле | Значение | Источник в `resources.json` |
|---|---|---|
| Итог запуска | Ожидается | `verdict` |
| Начало / окончание, UTC | Ожидается | `startedAt`, `finishedAt` |
| Область измерения | Ожидается | `scope` |
| Build центра / web | Ожидается | `versions.centerBuild`, `versions.webBuild` |
| Ревизия артефактов | Ожидается | `source.revision` |
| Fingerprint исходников | Ожидается | `source.fingerprint` |
| Незакоммиченные исходники при сборке | Ожидается | `source.dirty` |
| Профиль / features / default features | Ожидается | `buildProvenance.profile`, `buildProvenance.features`, `buildProvenance.default_features` |
| Manifest доставки, если задан | Ожидается | `deliveryArtifacts.manifest`, `deliveryArtifacts.build` |

`source` и `buildProvenance` относятся к измеренным артефактам. Если те же
исходники позже закоммичены без изменения fingerprint, отчёт сохраняет
исходную ревизию и признак dirty из receipt. Текущий Git HEAD не подменяет
происхождение бинарного файла. Эти же поля продублированы в
`versions.sourceRevision`, `versions.sourceFingerprint`, `versions.sourceDirty`.

Перед атомарной записью `PASS` harness повторно проверяет fingerprint,
содержимое build receipt, реальные байты артефактов, build центра/web и
идентичность переданного release manifest. Очистка fixture завершается до
этой записи. Начало нового запуска инвалидирует прежний `PASS`.
При `FAIL` причина берётся из `reason`; таблицы результатов этого запуска
остаются незаполненными.

## Машина и версии

| Параметр | Значение | Источник |
|---|---|---|
| ОС / версия / архитектура | Ожидается; для этого отчёта ожидается `darwin` | `machine.platform`, `machine.release`, `machine.arch`, `machine.machine` |
| CPU / логические ядра | Ожидается | `machine.cpuModel`, `machine.logicalCpuCount` |
| Оперативная память, bytes | Ожидается | `machine.totalMemoryBytes` |
| Свободная память до / после, bytes | Ожидается | `machine.before.freeMemoryBytes`, `machine.after.freeMemoryBytes` |
| Load average до / после | Ожидается | `machine.before.loadAverage`, `machine.after.loadAverage` |
| Rust / Bun | Ожидается | `versions.rustc`, `versions.bun` |
| Playwright / Chromium | Ожидается | `versions.playwright`, `versions.chromium` |
| Интервал / минимальное окно, ms | Ожидается | `methodology.requestedSampleIntervalMs`, `methodology.minimumWindowMs` |

Свободная память и load average описывают состояние всей машины. Они не
являются расходом памяти или CPU центра.

## Native release-центр: RSS и CPU

Измеряется отдельный процесс `target/release/hmail-cleaner`, собранный с
`--release --no-default-features --features center`. Два TLS protocol agents
работают в другом fixture-процессе; их RSS/CPU исключены из счётчиков центра.
Браузер и процесс, запускающий `ps`, также не входят в RSS центра.

| Окно `native[].name` | Нагрузка | RSS min / mean / max, bytes | CPU time, ms / средний CPU, % одного ядра | Samples / длительность, ms |
|---|---|---|---|---|
| `center_idle_locked` | Центр запущен, вход ещё не выполнен | Ожидается | Ожидается | Ожидается |
| `center_login_kdf_and_initial_ui` | Один login KDF и первоначальная загрузка UI | Ожидается | Ожидается | Ожидается |
| `center_two_tls_agents_two_open_ui_tabs` | Два TLS agents и две вкладки с разными серверами | Ожидается | Ожидается | Ожидается |
| `center_fast_preview_protocol_fixture` | Fast preview через UI и синтетический ответ агента | Ожидается | Ожидается | Ожидается |
| `center_full_preview_protocol_fixture` | Full preview через API центра и наблюдение в UI | Ожидается | Ожидается | Ожидается |
| `center_cancel_protocol_fixture` | Запуск preview и отмена через UI | Ожидается | Ожидается | Ожидается |

Для каждой строки используются `summary.rssMinBytes`, `summary.rssMeanBytes`,
`summary.rssMaxBytes`, `summary.cpuElapsedMs`,
`summary.cpuAveragePercentOfOneCore`, `summary.samples`, `summary.durationMs`
у объекта с соответствующим `name`. Все исходные выборки сохранены в его
`raw[]`: `atMs`, `rssBytes`, `cpuMs`.

`ps` сообщает RSS в KiB; harness переводит его в bytes. Для отображения в
MiB bytes делятся на 1 048 576. Максимальный RSS является максимумом
наблюдённых выборок: короткий пик между ними может быть пропущен. Для входа
фиксируется sampled RSS peak всего окна KDF/UI, а не гарантированный
мгновенный пик KDF.

Средний CPU вычисляется как прирост накопленного CPU time процесса,
делённый на фактическую длительность выборок, умноженный на 100. Это процент
одного ядра. `atMs` использует монотонные часы; точность CPU зависит от `ps`.
Фактический интервал включает накладные расходы запуска команды и может
отличаться от запрошенного.

Общий TLS fixture validator возвращает состояние службы `running`.
Поэтому full-окно использует настоящий authenticated API dispatch центра и
UI observer после reload. Оно не подтверждает успешный full preflight
через форму, файловое сканирование или работу SCM. Fast/full/cancel здесь
измеряют нагрузку центра на протокольном стенде.

## Клиент: JavaScript, DOM и chunks

Browser snapshots делаются в свежем Chromium context с доверием только к
SPKI сгенерированного fixture CA. `Runtime.getHeapUsage` возвращает
JavaScript heap без принудительного GC. Эти значения не являются RSS/PSS
всего браузера и не включают полную память browser process tree или GPU.

| Снимок `browser[].name` | JS heap used / total, bytes | DOM elements / nodes | Строки journal / history |
|---|---|---|---|
| `initial_login` | Ожидается | Ожидается | Ожидается |
| `initial_cleanup` | Ожидается | Ожидается | Ожидается |
| `second_cleanup_tab` | Ожидается | Ожидается | Ожидается |
| `fast_preview_completed` | Ожидается | Ожидается | Ожидается |
| `full_preview_completed` | Ожидается | Ожидается | Ожидается |
| `cancelled` | Ожидается | Ожидается | Ожидается |
| `history_first_page` | Ожидается | Ожидается | Ожидается |
| `settings_selected` | Ожидается | Ожидается | Ожидается |

Поля строки: `jsHeapUsedBytes`, `jsHeapTotalBytes`, `domElements`, `domNodes`,
`journalDomRows`, `historyDomRows`. `domNodes` приходит из
`Memory.getDOMCounters`, остальные DOM-счётчики — из selectors страницы.
Короткое наблюдение числа строк не является длительным тестом утечки памяти.

| JavaScript | Имена / encoded body / decoded body / transfer, bytes | Источник |
|---|---|---|
| Первая загрузка login | Ожидается | `browser[]` с `name=initial_login`: `scripts[]` |
| Первая загрузка cleanup | Ожидается | `browser[]` с `name=initial_cleanup`: `scripts[]` |
| Дополнительные chunks истории | Ожидается | `lazyPages.historyAdditionalScripts`; соответствующие `scripts[]` снимка `history_first_page` |
| Дополнительные chunks настроек | Ожидается | `lazyPages.settingsAdditionalScripts`; соответствующие `scripts[]` снимка `settings_selected` |
| Преждевременные запросы history/settings | Ожидается | `lazyPages.prematureDataRequests`, `workload.initialRequests` |

Каждый `scripts[]` содержит `path`, `encodedBodyBytes`, `decodedBodyBytes`,
`transferBytes`. Суммы считаются отдельно для каждого снимка по его списку
scripts. Это browser-reported `PerformanceResourceTiming`: body до/после
декодирования и transfer с response headers. При cache hit transfer может
быть нулевым. `staticAssets.jsBytes` описывает все JS-файлы сборки, поэтому
initial JS берётся именно из browser snapshot.

Контакты и команды синтетических агентов записаны в
`workload.counters.stats[]`: `id`, `puts`, `cancels`, `subscriptions`,
`health_calls`. Они подтверждают нагрузку protocol fixtures, а не количество
реальных операций над почтой.

## Размеры и контрольные суммы артефактов

| Артефакт | Размер, bytes | SHA-256 / идентичность | Источник |
|---|---|---|---|
| Native release center | Ожидается | Ожидается | `build.artifact.name`, `.size`, `.sha256` |
| Отдельный release fixture | Ожидается | Ожидается | `build.fixtureArtifact.name`, `.size`, `.sha256` |
| Всё статическое web-дерево | Ожидается | Перечень файлов ожидается | `staticAssets.totalBytes`, `buildProvenance.artifacts[]` для `apps/web/dist/` |
| Все JS-файлы web-дерева | Ожидается | Пофайловые SHA-256 ожидаются | `staticAssets.jsBytes`, `buildProvenance.artifacts[]` |
| OCI archive | Ожидается либо `NOT RUN` | Ожидается | `deliveryArtifacts.artifacts[]` выбранного archive |
| Windows EXE и пакет | Ожидается либо `NOT RUN` | Ожидается | `deliveryArtifacts.artifacts[]` выбранных файлов |

Пофайловые данные receipt: `buildProvenance.artifacts[].name`, `.size`,
`.sha256`. Список размеров статического дерева дополнительно находится в
`staticAssets.files[].path`, `.bytes`. Доставка содержит такие же
`name`, `size`, `sha256` для каждого файла в `deliveryArtifacts.artifacts[]`,
а также immutable image identity в `deliveryArtifacts.image`.

Размер OCI archive — размер файла доставки. Он не измеряет распакованное
хранилище контейнера, RSS контейнера или cgroup memory. Размер Windows EXE
не подтверждает расход памяти Windows-службы. При отсутствии manifest
причина берётся из `deliveryArtifacts.reason` вместе с
`deliveryArtifacts.verdict`.

## Отдельные непроверенные области

Текущий harness оставляет следующие проверки со статусом `NOT RUN`.
После запуска их точные причины переносятся из `notRun[].reason` по `name`.

| `notRun[].name` | Статус | Что требуется отдельно |
|---|---|---|
| `linux_rootless_podman_quadlet_container_rss_cpu` | NOT RUN | Release под rootless Podman и systemd/Quadlet на Linux; замеры реального контейнера |
| `windows_server_2016_agent_idle_cleanup_scm_rss_cpu` | NOT RUN | Native Windows Server 2016 fixture с SCM, MySQL и синтетической файловой системой |
| `real_mail_fast_full_cancel_throughput` | NOT RUN | Настоящие FS/SQL/SCM операции на выделенных синтетических данных |
| `thirty_minute_slow_sse_resource_run` | NOT RUN | Полный 30-минутный прогон с медленным потребителем SSE |
| `browser_total_process_memory` | NOT RUN | Отдельное измерение памяти всего дерева процессов браузера |

`PASS` локального resource run не меняет эти статусы. До появления чисел и
сопоставимой методики этот отчёт не задаёт бюджет RSS/CPU и не утверждает
преимущество по ресурсам перед другим runtime.

## Основания методики

- [Состав машинного отчёта](../tools/acceptance/measure.ts).
- [Проверка build receipt и fingerprint](../tools/acceptance/measurement/build-receipt.ts),
  [граница атомарной записи PASS](../tools/acceptance/measurement/report.ts).
- [Окна нагрузки](../tools/acceptance/measurement/workload.ts),
  [сэмплирование процесса](../tools/acceptance/measurement/process.ts),
  [вычисление RSS/CPU](../tools/acceptance/measurement/counters.ts).
- [Browser snapshots и lazy chunks](../tools/acceptance/measurement/browser.ts),
  [проверка файлов доставки](../tools/acceptance/measurement/release-artifacts.ts).
