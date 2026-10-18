# Развёртывание центра и агентов

Центр предназначен для Linux-машины; первоначальная целевая среда — ВМ
на Hyper-V. OCI-образ содержит Rust-процесс
и собранную статику Astro/Svelte. Этот документ описывает ручную установку
и резервный процесс обновления. Автоматическая доставка, её первоначальная
настройка и границы приёмки описаны в [CI/CD runbook](center-cicd.md).

Первоначальная подготовка Fedora CoreOS на вручную созданной машине:
[переносимая конфигурация IaC](../iac/fcos/README.md). Она готовит ОС;
проверка поставки, интерактивный init и запуск приложения описаны ниже.

## Локальная сборка поставки

Проверьте исходники и соберите согласованную поставку:

```sh
mise run check
mise run web:check
mise run release:local
```

Команда `bun tools/release/build.ts`, вызываемая `release:local`, по умолчанию
использует Podman и `linux/amd64`. Она проверяет immutable references из
`deploy/base-images.json`, повторно использует `tools/build-exe.sh --component agent`,
собирает Linux-центр и создаёт новый каталог `target/releases/<build>/`:

```text
center.oci.tar
release-manifest.json
agent-packages/<build>/manifest.json
agent-packages/<build>/hmail-cleaner-agent.exe
agent-packages/<build>/hmail-cleaner-agent.exe.sha256
quadlet/hmail-center.container
quadlet/hmail-center-data.volume
```

Одна `HMAIL_BUILD_VERSION` передаётся Rust-центру, frontend, агенту и manifest.
Manifest фиксирует Git revision, SHA-256 исходников, платформу, base images,
OCI manifest/config digests и хеши файлов поставки. Контекст сборки создаётся
из явного списка исходников; `.env`, `.git`, `.superpowers`, зависимости и
локальные build outputs в daemon не передаются. Docker получает тот же ignore
policy через временный `.dockerignore`. Изменение исходников во время сборки
блокирует финальный manifest. Публикации образа и загрузки в registry нет.

Без явного разрешения dirty source блокируется. Для локальной проверки
незакоммиченного состояния задайте `HMAIL_RELEASE_ALLOW_DIRTY=1`: revision
получит суффикс `-dirty.<fingerprint>`. Это состояние остаётся явным в manifest.
`HMAIL_RELEASE_OUTPUT` меняет родительский каталог, `HMAIL_RELEASE_IMAGE` —
repository локального image reference, `HMAIL_RELEASE_PLATFORM=linux/arm64` —
целевую платформу. Новый release-каталог не перезаписывает прежнюю поставку.

Для OCI smoke на macOS доступен отдельный путь через Docker/Colima:

```sh
HMAIL_RELEASE_ENGINE=docker HMAIL_RELEASE_DOCKER_CONTEXT=colima \
  HMAIL_RELEASE_ALLOW_DIRTY=1 mise run release:local
HMAIL_RELEASE_ENGINE=docker HMAIL_RELEASE_DOCKER_CONTEXT=colima \
  HMAIL_RELEASE_MANIFEST=/absolute/path/to/release-manifest.json mise run test:delivery
```

Docker build создаёт собственный временный builder с закреплённым BuildKit
image и удаляет только его. Текущий context и выбранный пользователем builder
не меняются. Web и Rust build stages работают на `BUILDPLATFORM` самого билдера.
Выбранная `linux/amd64` или `linux/arm64` задаёт GNU Rust target и linker из
`tools/release/conts.ts`; proc-macro и build scripts исполняются нативно.
При cross-build внутри Bookworm build image устанавливаются закреплённые
версии GNU compiler, binutils и glibc sysroot 2.36, затем `rustup target add`
добавляет stdlib выбранного target. При совпадении host/target используется
native toolchain закреплённого Rust image. Host-машина не изменяется.

ARG defaults base images подставляются во временный Containerfile из
проверенного `deploy/base-images.json`. Build log содержит фактические версии
пакетов и APT sources. Транзитивные зависимости APT не закреплены отдельным
snapshot: это не обещание побайтово воспроизводимой сборки. На ARM-хосте
исполнение amd64 runtime при smoke-проверке по-прежнему требует эмуляции.
Docker-проверка OCI не подтверждает Podman, Quadlet, rootless UID mapping,
запуск после logout/reboot или работу Windows SCM.

Runtime image содержит Rust binary, статические файлы и доверенный пакет
агента. Bun, Node.js, компилятор и shell остаются в build stages. Проверка OCI
сверяет хеши metadata/layers, платформу, фактический ELF64 little-endian
`e_machine` центра (62 для amd64, 183 для arm64), UID и entrypoint, build labels и
отсутствие runtime JS/compiler/shell files. Container работает от UID:GID
`10001:10001`, слушает `8443`, имеет read-only rootfs и записывает состояние
только в `/data`. Из контейнера нет доступа к Podman/Docker API/socket.

## Подготовка Linux-ВМ для rootless Podman

На Linux-ВМ нужны Podman с Quadlet, cgroup v2, служебный пользователь,
непересекающиеся диапазоны `/etc/subuid` и `/etc/subgid`, доступный
`systemd --user` manager и linger. Установка Podman и изменение системных
пользователей выполняются администратором этой ВМ по её правилам.

Проверьте от служебного пользователя:

```sh
podman info
systemctl --user is-system-running
loginctl show-user "$(id -un)" -p Linger
```

В `podman info` должны быть rootless и cgroup v2. Если linger отключён,
администратор включает его для выбранного служебного пользователя.
Не подменяйте rootless проверку запуском через `sudo podman`.

Перенесите проверенную поставку на ВМ, проверьте SHA-256 файлов по manifest,
загрузите OCI archive и сверьте digest выбранного образа:

```sh
release_dir=/absolute/path/to/release
podman load --input "$release_dir/center.oci.tar"
selected_image='значение image.reference из release-manifest.json'
podman image inspect "$selected_image"
```

Используйте exact reference с `@sha256:…`. Исходник
`deploy/quadlet/hmail-center.container` является шаблоном: устанавливается
**сгенерированный** файл `quadlet/hmail-center.container` из поставки, где
уже закреплены image digest и build. `Pull=never` требует предварительной
загрузки выбранного образа.

## Первоначальная настройка и запуск

Создайте фиксированный named volume и выполните интерактивный init тем же
образом, с тем же томом и UID mapping, что использует Quadlet:

```sh
podman volume create hmail-center-data
podman run --rm -it --read-only --user 10001:10001 \
  --userns keep-id:uid=10001,gid=10001 \
  --volume hmail-center-data:/data:Z,U "$selected_image" \
  center init --data-dir /data \
  --origin https://mail-control.lan:8443 --listen 0.0.0.0:8443
```

Задайте реальный DNS/IP origin до создания TLS identity. Init спрашивает
логин, скрытый пароль и повтор пароля, затем показывает новый recovery key
и требует повторить его до commit. Сохраните ключ отдельно от данных центра;
секреты не передаются в аргументах, unit-файлах или environment.
Повторный init не заменяет существующую учётную запись.

Суффикс `:U` применяется только к выделенному тому центра и согласует владельца
с UID mapping; не используйте его для произвольных существующих каталогов.
Неверный владелец bind mount должен приводить к ошибке, а не к незаметному
запуску с root или расширенными правами. Постоянное состояние — SQLite,
bootstrap TLS и `center.json` — остаётся в `/data`. Реальный configuration owner
читает JSON; пример находится в `deploy/center.example.json`, TOML не поддержан.

Установите сгенерированные units в search path служебного пользователя:

```sh
mkdir -p "$HOME/.config/containers/systemd"
cp "$release_dir/quadlet/hmail-center.container" "$HOME/.config/containers/systemd/"
cp "$release_dir/quadlet/hmail-center-data.volume" "$HOME/.config/containers/systemd/"
systemctl --user daemon-reload
systemctl --user start hmail-center.service
systemctl --user status hmail-center.service
podman exec hmail-center /usr/local/bin/hmail-cleaner center health --data-dir /data
```

Quadlet генерирует временную `hmail-center.service`; `systemctl enable` для
неё не вызывается. `[Install] WantedBy=default.target` и linger обеспечивают
включение при старте пользовательского manager. Для приёмки отдельно проверьте
сгенерированный unit, отсутствие входа пользователя, logout и reboot ВМ.
Файл `.volume` задаёт `VolumeName=hmail-center-data`, поэтому имя не меняется
на автоматически вычисленное `systemd-*`.

Health работает по HTTPS до login/DEK, проверяет публичный CA и имя сервера,
а `--expect-build <build>` дополнительно сравнивает версию. Команда health
не открывает SQLite вторым владельцем. Для браузера получите только публичный
`ca-cert.pem`, проверьте его источник и импортируйте в доверие конкретного
устройства/браузера. `https-key.pem` и зашифрованный private CA не экспортируются.

В Quadlet `HealthCmd` задан JSON argv с прямым запуском binary без `CMD` prefix:
shell для проверки не требуется. `Containerfile` также задаёт direct-exec
`HEALTHCHECK` для Docker image. При сборке стандартного OCI через Podman/Buildah
это Docker extension может отсутствовать: [Buildah явно исключает его из
OCI format](https://github.com/podman-container-tools/buildah/blob/main/config.go).
Поэтому archive verifier требует image health metadata для Docker-produced
image, допускает отсутствие для Podman-produced OCI и проверяет безопасный
direct argv всегда, когда поле присутствует. Producer определяется полем
`engine` исходного release manifest, а не engine текущей проверки.

В native Podman health принадлежит Quadlet. Standalone lifecycle fixture
передаёт тот же JSON argv с ожидаемым build через `--health-cmd` и явные
interval/timeout/start-period/retries. Docker fixture использует image
`HEALTHCHECK` без override. Перед login lifecycle проверяет фактический
`Config.Healthcheck.Test` и ждёт `State.Health.Status=healthy`; отдельный
CLI probe продолжает проверять expected build. Docker result не подтверждает
разбор Quadlet или его systemd health timer на native rootless Podman.
Native JSON argv и нормализованный image/container `Healthcheck.Test` имеют
разные контракты: в `Test` сохраняется `CMD` sentinel, который Podman добавляет
при обработке native argv. Для CLI и Quadlet этот sentinel не передаётся.

## Ручное обновление и rollback центра

Для каждой замены выполните последовательность вручную:

1. Скачайте/перенесите и проверьте выбранный OCI archive и image digest.
2. Сохраните предыдущие image reference, build и unit. Остановите
   `systemctl --user stop hmail-center.service`; дождитесь завершения процесса.
3. Создайте согласованный backup **прежним образом** с тем же data volume
   и UID mapping. В качестве `/backup` используйте выделенный каталог
   служебного пользователя с правами `0700`; `snapshot` ещё не должен существовать:

   ```sh
   previous_image='прежний exact image.reference'
   backup_root=/absolute/path/to/new-backup-parent
   podman run --rm --read-only --user 10001:10001 \
     --userns keep-id:uid=10001,gid=10001 \
     --volume hmail-center-data:/data:Z,U \
     --volume "$backup_root":/backup:Z "$previous_image" \
     center backup --data-dir /data --destination /backup/snapshot
   ```

4. Проверьте backup manifest, build, schema/vault identity и наличие SQLite,
   bootstrap/config. Не копируйте работающую `.sqlite` отдельно от WAL.
5. Замените `Image=` и ожидаемый build в unit выбранными значениями новой
   поставки, затем `daemon-reload` и `start`. Проверьте health/version до login,
   после чего выполните обычный вход и reconcile. Restart завершает RAM-сессию;
   принятая агентом работа не должна отправляться заново.

Если **до открытия новой версии пользователям и до новых принятых операций**
потребуется rollback, остановите новую версию, сохраните её текущее состояние
отдельно и восстановите полный matching backup прежней схемы/identity/build.
Не запускайте старый image поверх мигрированной БД и не накладывайте snapshot
поверх существующих SQLite/WAL-файлов. Восстановление фиксированного named volume
должно выполняться офлайн с сохранением нынешнего тома/экспорта, проверкой
владельца и `0700`, затем выбором прежнего digest и health до login.
После опубликованных новых операций автоматического restore нет: требуется
разбор принятых job/install receipts и согласованное восстановление состояния.

Собственный updater-контейнер, `AutoUpdate=registry` и включение
`podman-auto-update.timer` не используются. Они не заменяют backup/migration
workflow. Контейнеру центра не монтируется engine socket. Для переноса образов
достаточно исходящего HTTPS либо локального OCI archive; входящий доступ
из интернета для доставки не нужен.

## Локальная проверка поставки

`mise run test:delivery` принимает `HMAIL_RELEASE_MANIFEST` либо используется
`bun tools/release/verify.ts /absolute/path/to/release-manifest.json`.
Проверяются manifest/checksums и фактический OCI image; lifecycle создаёт
только UUID-scoped disposable volume и контейнеры, выполняет настоящий
интерактивный init через Bun PTY, HTTPS health, прямые static routes, API 404,
login, SIGTERM, offline backup и recreate с отзывом прежней сессии и сохранением
vault/CA. Результат записывается в отдельный `delivery-<UUID>/delivery-report.json`.
Cleanup удаляет только ресурсы этой проверки; рабочий `hmail-center-data`
не используется. Подтверждение выполнения теста — фактический отчёт, а не
наличие этих файлов или зелёные unit tests.

Native rootless Podman/Quadlet startup после logout/reboot, генерация systemd
unit, перенос/rollback между версиями и активная работа удалённого агента
требуют выделенной Linux/Windows среды и отдельного evidence. Docker/Colima
smoke и переносимые Rust-тесты эти фазы не заменяют.

Формат Quadlet и user namespace options сверяются с
[официальной документацией Podman](https://docs.podman.io/en/latest/markdown/podman-systemd.unit.5.html);
интерактивный harness использует [Bun Terminal](https://bun.sh/docs/runtime/child-process).

## Пакеты агентов

Центр читает доверенный каталог `/opt/hmail/agent-packages`, доступный только
для чтения. Один build размещается в отдельном каталоге:

```text
/opt/hmail/agent-packages/<build>/
  manifest.json
  hmail-cleaner-agent.exe
```

Используйте пакет из `mise run build:agent`, сохранив manifest и EXE вместе.
При установке центр проверяет product/target, диапазон протокола, размер
и SHA-256 потоково. Каталог не является местом загрузки произвольных EXE
через браузер. Контрольная сумма подтверждает соответствие доверенному
пакету центра; источнику этого каталога доверяет владелец развёртывания.

## Подготовка Windows

На каждой почтовой ВМ заранее нужен совместимый Win32-OpenSSH с SFTP,
доступный от Linux-ВМ центра. Наличие Windows Server 2016 само по себе
не означает наличие OpenSSH Server. Версию пакета, установку службы sshd
и firewall проверяют отдельно на целевой ОС. Центр не устанавливает SSH
поверх отсутствующего административного канала.

Нужна административная учётная запись ОС с доступом через SSH и повышенными
правами. Прикладной суперадмин центра не заменяет эту учётную запись.
Поддержаны пароль или OpenSSH private key с необязательной passphrase.
Пароль и ключ передаются по HTTPS центру только на время операции; в SQLite
они не сохраняются. RSA и SSH compression отключены в выбранной сборке;
для исходной настройки используйте Ed25519 либо поддерживаемый ECDSA.
Для зашифрованного OpenSSH-ключа bcrypt ограничен 64 раундами и солью 64 байта,
расшифровка выполняется в отдельном worker, по одной без очереди.

## Установка из панели

1. Выберите SSH-адрес и порт. Probe получает SHA-256 fingerprint без
   авторизации. Сверьте его с ключом ВМ через доверенный административный
   канал, затем подтвердите в мастере.
2. Введите реквизиты ОС и выберите пакет. При новом соединении host key
   проверяется до передачи реквизитов. Для MVP SSH hostname должен совпадать
   с hostname адреса mTLS агента; порты могут различаться.
3. Установка выполняется в фоне. По `operation_id` можно читать checkpoint;
   потеря HTTP-ответа не требует повторной регистрации. Повтор того же ID
   не запускает установку заново: запросите его состояние.
4. На ВМ проверяются Windows build не ниже 14393, x64, PowerShell 5.1
   и административные права. Пакет передаётся SFTP в отдельный защищённый
   staging-каталог и повторно проверяется по SHA-256 перед выполнением.
5. Агент создаёт приватный ключ на ВМ и возвращает только CSR. Центр выдаёт
   сертификат; устанавливаются публичное доверие и bootstrap. SCM запускает
   ожидающий команды агент. Затем заполняется почтовый профиль отдельно.

Bootstrap — фиксированный PowerShell-код в UTF-16 Base64, значения идут
ограниченным JSON через stdin. Реквизиты MySQL и пути почтового профиля
в этот канал не передаются. Имя службы агента и каталоги установки фиксированы;
произвольный shell-код из API не выполняется. Управление почтовой службой,
полный scan и очистка при установке не вызываются.

После подтверждения по mTLS агент должен сообщить ожидаемые build, UUID
и квитанцию завершённой установки с точным operation ID и SHA-256. Одна
версия в health не считается доказательством этой операции. Проверка готовности
повторяется до 30 секунд; установка повторно не отправляется.

## Сбои и ручное обновление

В центре один общий тяжёлый слот для установки/обновления и очистки.
Native installer дополнительно атомарно запрещает приём работ на агенте:
занятое задание блокирует замену EXE. Перед обновлением вводятся свежие
реквизиты ОС и подтверждается host key. Ключ, сертификат и данные агента
сохраняются; предыдущий EXE хранится штатным установщиком.

Для просроченного сертификата выберите отдельное действие восстановления
доверия по SSH (`recover_identity`). Оно работает без действующего mTLS:
нативная команда сначала проверяет сохранённый журнал и атомарно включает
maintenance, отказывая при активном задании. Только затем центр подписывает
CSR прежнего ключа и меняет публичные сертификаты; обычный установщик
перезапускает агент и завершает maintenance. Приватный ключ не заменяется.
Отзыв доверия не отменяется этим сценарием: выпуск использует CAS прежнего
serial и проверку revoked, включая конкурентный отзыв во время восстановления.

Выход из центра прекращает допуск новых привилегированных шагов. Начатая
команда Windows может завершиться после обрыва SSH, поэтому после возможного
SCM-эффекта результат становится Unknown и удерживает слот. Reconcile проверяет
квитанцию по mTLS и не устанавливает агент снова. До начала установки отказ
помечается Failed и освобождает слот. Частично созданные файлы, identity и
staging не перезаписываются вслепую; требуется осмотр на ВМ.

При рестарте центра незавершённые установки становятся Unknown до входа,
без SSH-запросов и расшифровки. После входа суперадмин явно согласует
состояние. Если достоверной квитанции нет, слот сохраняется для ручного
разбора. Не удаляйте журналы заданий, identity или записи SQLite ради обхода
блокировки. После подтверждённой установки staging-файл этой операции
удаляется; при сбое очистки он остаётся для осмотра. Данные агента и backup
предыдущего EXE при этом не удаляются.

## Границы проверки

Переносимые тесты используют настоящий SSH transport для host-key/auth,
потоковый SFTP с проверкой размера кадров и ошибкой записи, инъекции ошибок
установочного порта и настоящий Rust-агент с mTLS для
проверки квитанций. Они не выполняют Windows PowerShell/SCM. Сценарий
на Windows Server 2016 описан в [инструкции агента](agent-installation.md).
Его native выполнение, DACL, Win32-OpenSSH/SFTP, пути с пробелами/кириллицей
и фактические RSS/CPU Windows требуют отдельной проверки на ВМ.

## Дисковая диагностика центра и агента

Работающий Rust-центр пишет `center.ndjson` в защищённый каталог данных,
агент — `agent.ndjson` в свой каталог данных. Файл открывается после захвата
эксклюзивного владения каталогом и до открытия SQLite. Команды health, backup,
инициализации и установки не открывают вращающийся журнал работающего сервиса.

На процесс сохраняется до пяти файлов **всего**, включая активный: по 10 MiB,
архивы `.1`–`.4`. При перезапуске активный файл дополняется. Полная запись NDJSON
ограничена 2 KiB; запись синхронная, с фиксированным буфером, без очереди.
Это отдельная диагностика, а не SQLite-квитанции заданий или результаты очистки.
Существующий CLI сохраняет прежнюю политику: активный файл и три архива.

Журнал содержит время UTC, закрытые коды событий, этапы, идентификатор задания
или корреляции запроса, безопасные виды ошибок и итоговые счётчики. Частые
промежуточные счётчики объединяются; безопасные ошибки записываются до такого
объединения. Пути к письмам, произвольные тексты ошибок, URI, профили, пароли,
ключи, cookies и заголовки авторизации не записываются. Для ошибки очистки
сохраняются этап, вид объекта, числовой ID сообщения при наличии и число ошибок;
итог исполнения и подтверждение сохранения квитанции — отдельные события.

Если журнал недоступен при старте, сервис завершается с явной ошибкой журнала.
Ошибка записи во время работы выдаёт фиксированное сообщение в stderr,
помечает журнал недоступным и отражается в health: центр возвращает HTTP 503,
агент перестаёт принимать новые задания. Уже начатая операция продолжает путь
восстановления службы и сохранения квитанции; исходная ошибка исполнения и
результат HTTP-запроса не заменяются ошибкой логирования. При отказе дискового
I/O последняя строка может остаться неполной. После устранения причины требуется
перезапуск сервиса; диагностическое событие, запись которого не удалась,
не считается сохранённым.
