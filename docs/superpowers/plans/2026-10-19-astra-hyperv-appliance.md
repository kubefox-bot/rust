# Astra Hyper-V appliance implementation plan

**Goal:** Поставлять проверенный VHDX с локальным приложением и безопасным первым запуском.

**Architecture:** Офлайн-упаковка официального FCOS VHDX, bootstrap через Ignition/KVP, существующие rootless Podman/Quadlet и updater. Публикация и native Hyper-V приёмка остаются отдельными этапами.

**Tech Stack:** Rust, Bun/TypeScript, Bash, PowerShell 5.1, libguestfs, Fedora CoreOS, GitHub Actions.

**Spec:** [Утверждённый проект](../specs/2026-10-19-astra-hyperv-appliance-design.md).

**Execution:** Пользователь подтвердил проект и запросил реализацию. План выполняется в этом чате в отдельном worktree; параллельные изменения очистки не переносятся.

## Global constraints

- Один source SHA для OCI, Windows agent, updater и appliance; распространяемый build только clean.
- Схема центра 2, поддерживаемые backup 1 и 2; неизвестные версии отклоняются.
- FCOS не загружается при изготовлении шаблона; уникальные identities возникают у заказчика.
- Только собственный каталог payload в shared var; разметка и загрузчик upstream сохраняются.
- Нет секретов в VHDX/KVP/аргументах/журналах. Init интерактивный; повтор не перезаписывает данные.
- Native FCOS/Hyper-V/Windows результаты отделяются от локальных unit/mocked проверок.

## Review focus

1. Настоящий backup должен совпадать с объявленной схемой, а не только с тестовой фикстурой.
2. Потеря ответа после init не допускает повторной инициализации или удаления volume.
3. Поломка payload/путь с symlink/неизвестный OSTree layout должны завершать подготовку до запуска приложения.
4. Повтор installer после частично созданной ВМ не изменяет существующие ресурсы.
5. CI receipt должен обнаруживать изменение любого appliance input/asset после проверки.

## Task 1 — Compatibility contract

**Owners:** protocol delivery contract, center storage, updater descriptor/backup, release metadata.

- [x] Написать RED-тест передачи настоящего backup центра updater и тесты schema mismatch/unsupported schema.
- [x] Ввести единый Rust-контракт и машинный export для tooling; генерировать проверяемую JSON-метаинформацию.
- [x] Перевести consumers на schema 2; явно разрешить backup 1/2 и проверить SQLite user_version.
- [ ] Проверить targeted Rust tests, contract reproducibility и release tests.

## Task 2 — First boot and initialization

**Owners:** deploy/appliance, deploy/updater/install.sh, iac/fcos.

- [ ] Тесты на отказ повреждённого payload, приватные файлы, retry и раздельные prepare/activate.
- [ ] Реализовать подготовку пользователя/OCI/volume/units с durable этапами и guard до init.
- [ ] Реализовать интерактивный init, восстановление после потерянного ответа и health; timer остаётся выключенным.
- [ ] Проверить Bash boundaries с изолированными command adapters; native SELinux/socket отметить отдельно.

## Task 3 — Image factory and installer

**Owners:** tools/appliance, deploy/appliance/Install-Astra.ps1, lock-файлы.

- [ ] Проверки same-SHA inputs, allowlist, unsafe paths/symlinks, hash mismatch и changed inputs.
- [ ] Добавить закреплённые FCOS/KVP inputs, streaming download, payload и libguestfs image assembly/inspection.
- [ ] Собрать installer и bundle с manifest/checksums; installer создаёт только новую ВМ и свой диск.
- [ ] Проверить PowerShell на Windows через fixture boundaries, Linux builder через реальные tools при наличии окружения.

## Task 4 — CI and publication

**Owners:** workflows, release receipts/gate/assets, mise tasks.

- [ ] RED-тесты binding appliance bytes и отклонения подмены/отсутствия receipt.
- [ ] Включить source inputs iac/fcos, сборку после same-SHA release и validation перед upload.
- [ ] Связать appliance hashes с CI receipt, добавить явные published assets, сохранить existing signatures/gates.
- [ ] Добавить PowerShell tests и отдельные boot/native acceptance receipts; не выдавать mock за Hyper-V.

## Task 5 — Verification and handoff

- [ ] Runbook установки/инициализации/retry/backup и native acceptance checklist.
- [ ] Выполнить обязательные source checks и доступную сборку, исправить внесённые ошибки.
- [ ] Провести review изменённых contracts, trust boundaries и recovery; проверить отсутствие секретов/случайных inputs.
- [ ] Отчитаться об артефакте, выполненных проверках и точных NOT RUN без публикации/установки у заказчика.
