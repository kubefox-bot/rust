# Проверка VHDX-поставки — 19 октября 2026

Реализация завершена в ветке `ci/astra-hyperv-appliance`. Локально собран настоящий
VHDX-комплект с OCI приложения и host-updater, выполнены проверки ниже.
Публикация, удалённый CI и приёмка на Hyper-V не выполнялись.

## Идентичность сборки

- Source SHA всех компонентов: `2880e2e88db44cb1d368e80b97084268204a0fcb`.
- Build: `git-2880e2e88db44cb1d368e80b97084268204a0fcb`.
- Каталог: `target/releases/git-2880e2e88db44cb1d368e80b97084268204a0fcb`.
- ZIP: `appliance/astra-hyperv-git-2880e2e88db44cb1d368e80b97084268204a0fcb.zip`.
- Размер ZIP: 1 158 132 982 байта.
- SHA-256 ZIP: `39d02e15d1fbf9620abaf699559a917f1d8a95f93ccacfcbc5b21eed9e761b56`.
- OCI digest: `sha256:9bb028903c02bec694f7dec5f1c4e429443d93aafa59167f051db88fd2448d10`.
- Appliance manifest SHA-256: `e831eabde2206477d8aa0c7768e46914db2b7783ba3147309b940e3d2012db18`.
- Assembly report SHA-256: `d8c8f787c4fbe99c5c6e007ff78576a0ba999d088f043b2c023095132d6463f2`.

Последующий коммит меняет CI и документацию: добавляет проверку Ignition официальным
закреплённым validator image. Он не меняет байты уже собранных компонентов;
этот комплект остаётся привязанным к указанному source SHA. Следующий CI build
создаст все компоненты с SHA своего checkout.

ZIP содержит VHDX, `Install-Astra.ps1`, installer library, `kvpctl.exe`, `base.ign`,
README, manifest, assembly report и SHA256SUMS. FCOS-шаблон не загружался;
в нём нет инициализированной БД, паролей, SSH host keys или identities экземпляра.

## Выполненные проверки

| Проверка | Результат и граница доказательства |
| --- | --- |
| `mise run check` | PASS: Rust format, workspace tests, Clippy, build driver; 486,63 с |
| `mise run web:check` | PASS: 55 tests, 0 failures |
| Release/appliance tooling | PASS: 59 tests, 234 assertions, 27 files; TypeScript checks |
| Browser и delivery contracts | PASS: воспроизводимый export; schema 2 и backup schemas 1/2 |
| Настоящий backup центра → updater | PASS: SQLite header сверяется с manifest; mismatches/unknown schemas отклоняются |
| Shell | PASS: syntax и ShellCheck для изменённых bootstrap/install scripts |
| Workflows | PASS: actionlint 1.7.12, `center-ci.yml` и `center-delivery.yml` |
| PowerShell fixtures | PASS на PowerShell 7.6.7: hashes, key/origin, existing targets, KVP failure без Start-VM |
| Installer на настоящем комплекте | PASS: точный состав, размеры и SHA-256 |
| Сборка FCOS VHDX | PASS: guestfish 1.54.1, qemu-img 10.0.13; offline injection под umask 077 |
| Readback VHDX | PASS: allowlist, hashes, root ownership, public payload modes, UUID/layout, boot partition hashes, чистое first-boot state |
| Сохранность upstream | PASS: исходный VHDX не изменён; upstream boot partitions сохранены |
| Ignition | PASS: реальный `base.ign` проверен официальным `ignition-validate` image по закреплённому digest |
| Rootless Podman | PASS только загрузка финального OCI в чистое storage, поиск по digest и сравнение config ID; Podman 5.8.9 |
| Docker lifecycle финального OCI | PASS: HTTPS/health до входа, static routes, API 404, login, graceful SIGTERM, backup остановленного центра, recreate vault/CA, отзыв сессии после restart |
| Receipt и publication gate | PASS: настоящий `verification.json` связывает source, OCI, updater и ZIP/manifest/report; повторная проверка байтов через штатную configuration boundary |

Машинные результаты сохранены рядом с артефактами в `verification.json` и
`appliance/assembly-report.json`. Локальные диагностические логи находятся
в `/tmp/astra-appliance-*.log`; они не являются release assets.

## NOT RUN

- Удалённый GitHub Actions run: ветка не отправлялась.
- Windows PowerShell 5.1: fixtures добавлены в Windows job, локально запускался 7.6.7.
- Native Hyper-V Gen2/Secure Boot/KVP/first boot и две независимые ВМ из одного ZIP.
- Native FCOS SELinux Enforcing, root growth, Quadlet logout/reboot, maintenance socket
  и согласованная перезагрузка Zincati/updater.
- Активная очистка через настоящий удалённый Windows agent и Windows Server 2016 SCM/FS.

Полный rootless container startup в временной amd64 OrbStack VM под Rosetta
не подтверждён: окружение завершалось ошибкой crun/cgroup. Результат импорта OCI
не считается доказательством запуска Quadlet/FCOS. Собственная временная VM
использовалась только для этой проверки.

До рабочего использования требуется [native acceptance из runbook](hyperv-appliance.md#приёмка-перед-рабочим-использованием).
Публикационный gate сохранён; `ASTRA_DELIVERY_ENABLED` не включался.

## Решения при реализации

1. Повторно использован существующий игнорируемый Cargo cache через явный
   `CARGO_TARGET_DIR`, с блокировками Cargo. Исходники между checkout не копировались.
   Цена ошибки: конфликт/повторная сборка кэша, а не изменение пользовательских данных.
2. Совместимость backup проверяется по реальному SQLite `user_version` и manifest,
   а не только номеру в JSON. Цена ошибки: строгий отказ при несогласованном backup;
   такие копии требуют диагностики вместо автоматического восстановления.
3. Добавлен read-only `center init-status`, чтобы повтор после потерянного ответа
   распознал уже завершённый init. Цена ошибки: при неопределённом/частичном состоянии
   нужна ручная диагностика; автоматического удаления данных нет.
4. YAML modes записаны явными десятичными 448/420 для одинакового разбора Bun/Butane.
   Цена ошибки: неправильные permissions; проверены parser regression и реальный readback.
5. Installer расширяет только новую копию VHDX до 64 ГБ по умолчанию; factory сохраняет
   upstream partitions. Цена ошибки: нехватка места на хосте или неподтверждённый root growth;
   размер параметризован, рост root остаётся native acceptance.

## Review

Один независимый review всего изменения выявил три замечания: сравнение Podman
config ID, унаследованные права payload и implementation в public index. Все
исправлены; первые два воспроизведены RED → GREEN, затем выполнены общий tooling
suite, реальный VHDX readback и импорт финального OCI. Public index оставлен
только точкой экспорта. Отложенных замечаний нет.

## Передача

[Инструкция установки, init, retry и обновления](hyperv-appliance.md).
Ветка и managed worktree сохранены; исходный checkout с параллельной работой
по очистке не изменялся. Merge, push, публикация и установка у заказчика не выполнялись.
