# Автоматическая доставка Astra контейнерными образами

Статус: решение и исполнение согласованы пользователем; реализация в ветке `ci/astra-image-delivery`. Публикация и target activation требуют отдельной приёмки.

## Цель и согласованные требования

После push в `main` пользователь больше не заходит на FCOS ради обновления
Astra. GitHub Actions проверяет исходники, собирает одну поставку и публикует
её. ВМ сама обнаруживает успешный релиз через исходящий HTTPS, загружает образ
и заменяет контейнер после безопасного завершения текущей работы.

Основа остаётся FCOS + rootless Podman + Quadlet, Rust + static Astro/Svelte,
SQLite и Windows-агенты. Hyper-V настраивает оператор. Входящие подключения из
интернета, runner внутри организации и доступ центра к engine socket не нужны.

Пользователь выбрал автоматические проверку и установку, с graceful-периодом
для текущей работы. Push является запросом на доставку, но не отменяет проверки:
ошибка CI, недоступность сети и занятость агента оставляют текущую версию.

## Существующая основа и граница изменения

На момент проектирования HEAD — `0d320b9`, ветка — `ci/hmail-control-center-mvp`,
локальный remote default — `origin/main`. Это снимок, не утверждение о текущем
состоянии GitHub. Существуют:

- `tools/release/build.ts`: OCI archive, единый build, manifest, агент и Quadlet;
- `tools/release/verify.ts`: проверка архива и lifecycle;
- `deploy/quadlet/hmail-center.container`: rootless, `Pull=never`, digest,
  read-only rootfs, healthcheck, named volume;
- `center backup` и HTTPS `center health --expect-build`;
- `iac/fcos/`: подготовка служебного пользователя и периодические обновления ОС;
- `.github/workflows/cleanup-cli.yml`: ручные portable/MySQL/Windows проверки.

Сейчас автоматической доставки нет. Этот документ расширяет текущий ручной
процесс; изменения рабочих инструкций вступают в силу только после реализации.

## Выбор подхода

1. **Отдельный updater на FCOS — выбранное предложение.** Работает независимо
   от контейнера, умеет дождаться работы, сделать offline backup и восстановить
   предыдущую версию. Небольшой дополнительный компонент, зато явный lifecycle.
2. `podman auto-update`: меньше собственного кода, но рестарт systemd unit
   сам по себе не обеспечивает drain, согласованный backup и rollback SQLite.
3. Runner/SSH push из GitHub: требует доступа к сети организации или runner с
   такими правами; для данной сети исключаем.

## CI и публикация

PR запускает проверки без прав публикации. Push в `main` запускает проверки и
сборку. Новые workflows: `center-ci.yml` и `center-delivery.yml`. Существующий
ручной CLI workflow сохраняется. Общие проверки оформляются reusable workflow,
чтобы публикация зависела от проверок того же SHA, а не от другого зелёного run.

Первый target — `linux/amd64`; build идентификатор — `git-<полный SHA>`.
Все компоненты поставки получают один build. Используем существующий Containerfile,
lockfiles, версии mise и проверку source identity. Артефакт строится один раз,
проверяется и затем публикуется без пересборки.

Предлагаемый registry — приватный `ghcr.io/kubefox-bot/astra-center`.
Публичность пакета не меняем. CI публикует с `GITHUB_TOKEN`; FCOS получает
отдельные read-only credentials. Для GHCR private pull требуется подходящий
PAT classic с `read:packages`; GitHub Releases в private repo потребует доступа
на чтение releases/contents. Это конфигурация установки, не пароль суперадмина.
Храним credentials в закрытом host-файле, вне Ignition/Git/контейнера/журналов.

После smoke проверяем неизменность OCI manifest digest при push в registry.
Если транспорт меняет manifest, публикация останавливается: нельзя подписать
новый digest без повторной проверки фактически опубликованного образа.
Версионный image сохраняется по digest; `latest` не управляет установкой.

Публикация создаёт release с именем `center-<полный SHA>` и включает существующий
manifest, `delivery.json`, Sigstore bundle, проверенный OCI archive и Quadlet.
Release становится доступным для установки только после финальной подписи и
публикации целого набора. Частичный draft игнорируется. OCI archive остаётся
резервным путём для первоначальной/offline установки.

Последний шаг promotion сериализован и повторно проверяет HEAD `main`.
Устаревший run не продвигается. Два push подряд: уже опубликованная версия может
успеть установиться, затем придёт следующая; если обе доступны до переключения,
updater выбирает более новую. Во время применения выбранный target неизменяем.
Нельзя обещать, что промежуточная версия никогда не запустится.

Все Actions закреплены полным commit SHA, permissions минимальны для job;
PR-код не получает signing/publish permissions. Изменения workflow и host updater
входят в проверяемую поставку исходников. Cache служит ускорению, не источником
доверенного готового release. Установка FCOS не зависит от GitHub Actions artifacts
с ограниченным сроком хранения: используются Releases и registry.

## Контракт доверия

`delivery.json` format 1 содержит product `astra-center`, channel `main`, полный
source SHA, build, монотонный sequence из `github.run_number` выделенного workflow,
image reference/digest/config digest, platform, SHA-256 release manifest, диапазон
совместимых версий updater и точный schema version. Повторный run того же номера
не может опубликовать другой digest; конфликт блокируется. Удаление/переименование
workflow, обнуляющее счётчик, требует отдельной миграции канала.

Cosign keyless подписывает точные байты `delivery.json`. Host проверяет bundle,
issuer `https://token.actions.githubusercontent.com` и точную identity
`https://github.com/kubefox-bot/rust/.github/workflows/center-delivery.yml@refs/heads/main`.
Также проверяются signed workflow/source claims и совпадение с подписанным SHA.
Существующий manifest хранит 12-символьный revision: сравнивается с префиксом
полного SHA, а label image и descriptor дополнительно несут полный SHA.
Доверенный Sigstore root и версия Cosign закрепляются при bootstrap; сетевой
сбой не разрешает пропуск проверки. Ротация trust root — отдельная управляемая
операция. Никаких signing private keys на FCOS.

После проверки подписи разрешены только настроенные registry/repository,
платформа, product/channel и более новый sequence. JSON с изменённым digest,
неизвестным signer или downgrade отвергается. GitHub API помогает обнаружить
release; доверие к артефакту определяется подписью. Свежая установка требует
доступа к API текущего канала; cached metadata не доказывает отсутствие новых
версий. Сбой обнаружения отражается в status, а текущий центр продолжает работать.

Cosign — предложение готового специализированного verifier вместо собственной
реализации Sigstore. Перед закреплением версии проверяются official checksum,
подпись и Linux/FCOS совместимость; конкретные pins записываются в lockfile
инструментов при реализации, не выдумываются в плане.

## Host updater

Отдельный Rust binary `astra-updater` в `crates/updater/`, rootless пользователь
`hmail-center`. Bun остаётся инструментом CI и разработки: на FCOS его сейчас
нет. Нативный Rust позволяет переиспользовать locked зависимости проекта и
ограничиться одним процессом на время проверки/применения. Это конкретная причина
выбора Rust вместо добавления Bun runtime. Cosign/Podman вызываются фиксированным
argv без shell и исполняются только на время операции.

Установка один раз добавляет updater, trust, конфигурацию и systemd user units.
Timer обновляет политику каждые 30 секунд, `AccuracySec=1s`, случайная задержка до 5 секунд.
Интервал проверки релизов задаётся в центре: `check_interval_seconds`,
по умолчанию 60, диапазон 30–86400. Ожидание работы: `graceful_timeout_minutes`,
по умолчанию 15, диапазон 1–1440. Только SA может менять эти поля через Settings;
credentials/trust/repository не доступны для изменения браузеру. Центр сохраняет
политику в `/data/update-policy.json`, updater забирает через Unix socket и
атомарно сохраняет последнюю валидную копию на host. Нет центра — действует
последняя копия; повреждённая host-копия не заменяется дефолтами молча.
Новая политика применяется к следующей транзакции. Discovery может округляться
вверх до очередного timer tick; нет обещания строго периодических 60 секунд.
Применение автоматическое, дополнительного согласования релиза нет.
Цель обнаружения — примерно минута после успешной публикации при доступной сети;
сборка CI, скачивание и graceful ожидание в эту минуту не входят.

Host paths:
- `~/.local/lib/astra-updater/` — закреплённые binary/verifier/trust;
- `~/.config/astra-updater/config.json` — typed config, mode `0600`;
- `~/.config/astra-updater/credentials.json` — read-only network credentials, `0600`;
- `~/.local/state/astra-updater/` — журнал транзакции и status, `0700`;
- `~/.local/share/astra-updater/` — staging и backup, `0700`.

Центр не видит host credentials, executable или engine socket. Для управления
maintenance используется отдельный Unix socket в явно смонтированном каталоге
`/run/user/2000/astra-maintenance/`, доступный только соответствующему host UID.
Транспорт проверяет peer credentials и ограничивает размер/время запроса.
Новый публичный HTTP endpoint управления хостом не создаётся.

Одновременно работает одна транзакция обновления. systemd не дублирует unit;
дополнительно updater держит process lock, сохраняет phase через atomic replace
с fsync. После reboot `prepare-start` проверяет согласованность диска, unit и marker
перед стартом центра; эта команда не вызывает start зависимой службы. После
старта обычный `resume` завершает health/recovery без циклического ожидания units.
Неизвестное состояние трактуется как остановка для восстановления, не как успех.

Сам updater не обновляется из контейнера автоматически в первой версии. Релиз,
требующий другой версии updater, остаётся pending с явным статусом. Это
ограничение первой версии; ротация host runtime требует отдельного плана.

## Graceful и последовательность применения

1. Discover/verify: проверить descriptor, manifest и подпись. Download/pull до
   остановки центра, сверь digest/config/platform/build; загрузка не продлевает
   пользовательскую RAM-сессию. Сбой сохраняет работающую версию.
2. Preflight: проверить место под image, backup и сохранение старых данных;
   предыдущие image и unit доступны. Бюджет операции и оценка места явные.
3. Drain: центр атомарно закрывает admission новых мутаций и сохраняет maintenance
   marker. Проверяются обе durable slots: cleanup и provisioning/update агента.
   Уже принятая работа, её cancellation/reconcile и необходимые подтверждения
   продолжаются. Unknown/unreachable receipt считается занятостью.
4. Graceful ожидание — по настройке центра, по умолчанию до 15 минут на попытку. Это максимум ожидания updater,
   не таймаут очистки. По истечении — не убивать работу, снять admission gate,
   отложить обновление. Новая попытка на очередном tick. Потеря RAM-ключа и
   невозможность reconcile могут потребовать обычного входа SA: не сохраняем
   пароль/DEK ради обещания полной автоматизации в аварийной ситуации.
5. Когда durable slots и уже начатые mutations пусты — остановить центр,
   подтвердить освобождение data lease. Старым image выполнить `center backup`.
   Проверить matching schema/build/vault/config/TLS и завершённость backup.
6. Сохранить предыдущий unit и транзакцию, атомарно установить unit с новым
   digest/build. `Pull=never` сохраняется: нужный image уже скачан. Start под
   maintenance gate, поэтому новые пользовательские операции ещё запрещены.
7. В течение 120 секунд дождаться HTTPS health до login с ожидаемым build,
   проверить runtime image identity, schema и vault/TLS identity без разглашения
   секретов. Перед снятием gate записать и fsync phase `Exposing`: с этого момента
   автоматический restore запрещён, поскольку доступ уже мог быть открыт. Затем
   снять gate и зафиксировать committed sequence/build.
8. При health failure до записи `Exposing`: остановить candidate, сохранить его данные
   отдельно, восстановить matching backup в чистый volume с прежним фиксированным
   именем и UID mapping, вернуть unit/digest, проверить health. Старый image
   никогда не запускается поверх потенциально изменённой candidate БД.
9. Начиная с `Exposing` автоматический restore запрещён: новые операции могли быть
   приняты. Поздняя ошибка получает статус failed/needs attention без отката данных.

Admission gate не является простым HTTP middleware: нужна синхронизация с
доменными admission points и всеми начавшимися изменениями данных. Создание gate,
выдача read/write permits и проверка slots имеют единый порядок, который не
допускает race «проверили idle → приняли новую job». Marker находится вне SQLite,
проверяется при startup, до открытия mutation routes; первая версия не меняет
schema version 1 только ради delivery. Read-only history/health доступны;
login и reconcile допускаются как явно перечисленные control операции с учётом
того, что они тоже могут писать operational state. Перед backup останавливаются
все writers вместе с процессом.

Все release первой версии требуют schema 1. Автоматические schema-changing
релизы блокируются ещё на preflight; механизм миграций не придумывается внутри
updater. Failure-restore всё равно используется, поскольку данные candidate могут
меняться даже без schema migration. Retry не повторяет установленный bad digest:
он quarantined до нового релиза или явного операторского решения.

Храним текущий и предыдущий подтверждённый image, активный backup и последний
успешный backup, failed candidate state до разрешения инцидента. Нехватка места
останавливает обновление; запрещён общий prune. Backup на той же ВМ обеспечивает
откат обновления, но не восстановление после потери диска/ВМ.

## ОС и агенты

Zincati обновляет ОС отдельно; существующее окно Sunday 03:00–04:00 UTC само по
себе не знает о cleanup. Перед production нужна координация reboot с приложением
и updater: локальный fleet-lock coordinator на loopback, вне контейнера центра,
отказывает при active/unknown работе и транзакции delivery, удерживает drain gate
до перезагрузки. Пока координатор недоступен, reboot update не разрешается.
Служба coordinator работает постоянно; фактические ресурсы измеряются при приёмке.
Просто добавить `Before=zincati.service` или краткий inhibitor недостаточно.
Сохраняем периодическое окно в политике coordinator, поскольку Zincati использует
одну выбранную стратегию. Штатный restart/crash/reboot recovery gates проверяются.
План не обещает защиту от принудительного выключения Hyper-V.

Центр продолжает поставляться с доверенным пакетом агента. Обновление центра
не обновляет Windows-агенты автоматически; существующий ручной onboarding/update
остаётся. Диапазон protocol compatibility проверяется до применения центра.
Совместимость с реальным Server 2016 и native SCM подтверждается отдельно от
Windows-2022 CI.

## Приёмка и границы

Две сборки A/B: push → CI → registry/release → auto discover → drain → backup →
replace → health → commit. Дополнительно bad signature/digest, registry outage,
disk full, активная/unknown job, параллельный push, падение updater на каждой phase,
неуспех candidate и reboot в каждой точке переключения. После reboot нет двойного
владельца SQLite, слепого повтора job и выдачи candidate пользователям до commit.

Первый bootstrap старой поставки выполняется один раз администратором; marker
recovery ещё не умеет старый binary, поэтому initial rollout требует выделенного
окна и pre-install backup. После этого совместимые релизы доставляются автоматически.

В предыдущем запросе созданы документы; пользователь затем разрешил реализацию и новую ветку. Настройки GitHub, secrets, публикации,
production FCOS и Windows-агенты не изменяются. Перед первой реальной публикацией
проверяются exact repository/package/credentials и согласуется внешний запуск.

## Источники

- [Podman auto-update и границы rollback](https://docs.podman.io/en/stable/markdown/podman-auto-update.1.html)
- [GHCR, permissions и authentication](https://docs.github.com/en/packages/working-with-a-github-packages-registry/working-with-the-container-registry)
- [Cosign signing bundles](https://docs.sigstore.dev/cosign/signing/signing_with_blobs/)
- [Cosign verification](https://docs.sigstore.dev/cosign/verifying/verify/)
- [GitHub Actions security](https://docs.github.com/en/actions/reference/security/secure-use)
- [Zincati strategies и fleet-lock](https://coreos.github.io/zincati/usage/updates-strategy/)

## Уточнения по результатам локального review

Открытие прежней версии после rollback или ошибки backup имеет отдельные
устойчивые фазы `RollbackExposing` / `PreviousExposing`. Их записывают до снятия
gate. После них recovery не заменяет данные повторно. Недостаток диска после
pull проверяется до начала maintenance. Подготовка при boot выполняет
`systemctl --user daemon-reload` после восстановления прежнего Quadlet, до
старта зависимого центра. Native FCOS acceptance для этих границ остаётся
отдельной проверкой.
