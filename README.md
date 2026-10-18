# hMail Cleaner

Консольная программа на Rust для очистки файлов и MySQL-базы hMailServer.
Сначала удаляет файлы и подтверждает их отсутствие, затем очищает связанные
записи БД. Работает непосредственно на Windows-сервере с почтовым хранилищем.

```cmd
hmail-cleaner.exe
```

Без аргументов открывает русское меню. Настройки по умолчанию читает из
`cleaner.env` рядом с EXE. Образец: [cleaner.env.example](cleaner.env.example).

```cmd
hmail-cleaner preview
hmail-cleaner clean --manage-service
hmail-cleaner clean --scan full --before "2024-09-15T23:00:00" --manage-service
```

Оба режима рекурсивно проверяют корень Data и все вложенные папки по дате
создания файла. `full` дополнительно проверяет исторические записи без родителя
и сверяет оставшиеся старые файлы. Быстрый предпросмотр доступен при работающей
почте. Для полного предпросмотра служба должна быть остановлена; очистка
с `--manage-service` останавливает и восстанавливает её сама.

## Разработка и сборка

```sh
mise run check
mise run test:db
mise run build:exe
```

Последняя команда создаёт `hmail-cleaner.exe` и `hmail-cleaner.exe.sha256`
в корне проекта. На Windows нужны MSVC Build Tools и Windows SDK; на macOS
нужен работающий Docker. Первый cross-build загружает Rust и Windows SDK,
следующие используют кэш. Node.js, Yarn, Tauri и WebView2 не нужны.

Подробности, настройки службы и ограничения: [инструкция CLI](docs/cleanup-cli.md).

Отдельный ручной агент центра собирается через `mise run build:agent` в
`dist/agent/windows-x64`. Установка, mTLS bootstrap, миграция и проверки:
[инструкция агента](docs/agent-installation.md).

## Центр управления

MVP добавляет Rust-центр с SQLite и статическими Astro/Svelte-страницами:
выбор почтового сервера, ручной предпросмотр/очистка, прогресс, журнал,
история, профили, агенты и учётная запись суперадмина. Агент запускается
нативно на каждой почтовой Windows-ВМ; центр предназначен для Linux-ВМ.

```sh
mise run web:check
mise run build:center
mise run release:local
mise run test:mvp
mise run measure:mvp
```

Локальная поставка через Podman/Quadlet, первоначальный вход и ручное
обновление описаны в [инструкции центра](docs/center-deployment.md).
Подготовка Fedora CoreOS без привязки к гипервизору: [IaC](iac/README.md).
Границы фактической проверки перечислены в [приёмке MVP](docs/mvp-acceptance.md).
Методика и результаты замеров — в [ресурсном отчёте](docs/mvp-resource-report.md),
принятые при реализации решения — в [журнале решений](docs/mvp-implementation-decisions.md).
JavaScript runtime нужен для сборки; в образе работает Rust-процесс.
