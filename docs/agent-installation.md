# Установка и обновление агента

Агент — нативный процесс для Windows Server 2016 x64. Служба
`hMailCleanerAgent` ждёт ручные команды центра по mTLS. Запуск службы
не запускает очистку. Центр хранит профили MySQL и путей; агент получает
снимок профиля только на время операции.

## Сборка пакета

Для локальной сборки нужны Rust из `mise.toml`, Bun и Windows MSVC tools
либо Docker с существующим cargo-xwin builder. На macOS можно явно выбрать
свой Docker context:

```sh
HMAIL_BUILD_DOCKER_CONTEXT=colima mise run build:agent
```

Результат находится в `dist/agent/windows-x64/`:

- `hmail-cleaner-agent.exe` со статическим MSVC runtime;
- `hmail-cleaner-agent.exe.sha256`;
- `manifest.json`: product, build, target, protocol_min/max и sha256.

Bun нужен при сборке manifest. Работающий агент использует один Rust EXE.
Сборка агента использует профиль `agent-release`: Thin LTO, один codegen unit
и удаление символов из EXE. Поведение обработки panic сохраняется; сборки CLI
и центра используют прежний профиль `release`.
Агент запускает один Tokio runtime с двумя рабочими потоками; SCM worker
использует тот же runtime. Дополнительные потоки для блокирующего I/O
создаются по необходимости.
Команда `mise run build:exe` сохраняет отдельный ручной CLI в корне проекта.
Пакет агента публикуется целиком после успешной сборки EXE и manifest;
сбой подготовки нового manifest сохраняет предыдущий пакет.

## Первоначальная настройка

Для установки из центра нужен заранее настроенный SSH-доступ к Windows
с правами локального администратора и подтверждённым host key. Рабочий канал
агента использует отдельный mTLS listener; сетевой доступ к выбранному порту
должен быть разрешён от Linux-ВМ центра.

Используются два отдельных плоских каталога на локальном диске:

- `C:\Program Files\hMailCleanerAgent` — установленный EXE;
- `C:\ProgramData\hMailCleanerAgent` — ключ, сертификаты, bootstrap и журнал заданий.

Provisioning заменяет DACL этих каталогов и файлов на SYSTEM/Administrators.
Имена каталогов закреплены для MVP, чтобы установщик не менял права общего
системного каталога. Junction, symlink, UNC-пути и вложенные каталоги
в защищаемом каталоге не поддерживаются.

Сначала проверяется SHA-256 staged EXE по доверенному manifest пакета центра.
Эта проверка выполняется до первого запуска EXE. Затем из повышенной консоли:

```powershell
& '.\hmail-cleaner-agent.exe' agent enroll `
  --data-dir 'C:\ProgramData\hMailCleanerAgent' `
  --agent-id '20000000-0000-4000-8000-000000000002'
```

Команда создаёт локальный `identity-key.pem` и `agent-id.json`, возвращает
JSON с CSR. Повтор с тем же UUID сохраняет ключ. Частично созданная identity
требует исправления; существующий ключ не перезаписывается.

Центр подписывает CSR и передаёт в каталог данных:

- `identity.pem` — сертификат агента с его UUID/ролью и SAN адреса;
- `trust.pem` — доверенный CA;
- `agent.json` — публичный bootstrap:

```json
{
  "listen": "0.0.0.0:9443",
  "agent_id": "20000000-0000-4000-8000-000000000002",
  "center_id": "10000000-0000-4000-8000-000000000001"
}
```

После подготовки сертификатов установка выполняется из staged пакета.
Значение `--sha256` берётся из доверенного manifest, а не вычисляется
из непроверенного EXE как самостоятельное доказательство доверия.

```powershell
$manifest = Get-Content '.\manifest.json' -Raw | ConvertFrom-Json
$package = (Resolve-Path '.\hmail-cleaner-agent.exe').Path
if ((Get-FileHash $package -Algorithm SHA256).Hash.ToLowerInvariant() -ne $manifest.sha256) {
  throw 'Agent checksum mismatch'
}
& $package agent install-service `
  --package $package `
  --executable 'C:\Program Files\hMailCleanerAgent\hmail-cleaner-agent.exe' `
  --config 'C:\ProgramData\hMailCleanerAgent\agent.json' `
  --sha256 $manifest.sha256
```

Служба работает от LocalSystem. После её запуска центр отдельно проверяет
mTLS, UUID узла, версию и протокол перед отправкой профиля. Настройки MySQL,
Data/INI и имя почтовой службы в bootstrap не записываются.

## Миграция с планировщика и обновление

Если уже существует старая служба `hMailCleaner`, установщик требует явного
указания её доверенного EXE и env-файла:

```powershell
& $package agent install-service `
  --package $package `
  --executable 'C:\Program Files\hMailCleanerAgent\hmail-cleaner-agent.exe' `
  --config 'C:\ProgramData\hMailCleanerAgent\agent.json' `
  --sha256 $manifest.sha256 `
  --legacy-executable 'C:\MailCleaner\hmail-cleaner.exe' `
  --legacy-env 'C:\MailCleaner\cleaner.env'
```

SCM command line должна точно совпасть с указанной установкой. Неизвестная
служба под зарезервированным именем блокирует установку. Старая служба
останавливается с ожиданием завершения работы/восстановления почты и получает
Disabled до активации агента. Она остаётся в SCM для проверки оператором.

Обновление использует тот же `install-service` из нового staged пакета.
Установщик берёт файловую блокировку и атомарно запрещает приём новых работ
через SQLite. Accepted, Running, Cancelling и Restoring блокируют замену EXE.
Действующий агент останавливается корректно; принудительное завершение
процесса и замена файла во время восстановления не используются.

При ошибке установки maintenance может остаться включённым. Исправьте
причину и повторите установку доверенного пакета: повтор заново проверяет
отсутствие активной работы. SQLite, HMAC receipt key, identity key и доверие
сохраняются между версиями. Откат выполняется на предыдущий пакет агента;
прежний планировщик остаётся Disabled.

## Проверки

Переносимые проверки:

```sh
mise run check
bun test tools/release/agent-manifest.test.ts
mise run test:agent-db
```

В отдельной Windows-среде, из повышенной консоли:

```powershell
cargo build -p hmail-cleaner-agent --example service_fixture --locked
$env:HMAIL_AGENT_SCM_FIXTURE_EXE = (Resolve-Path '.\target\debug\examples\service_fixture.exe').Path
cargo test -p hmail-cleaner-agent --test service --locked -- --ignored --test-threads=1
```

Fixture создаёт только службу с уникальным префиксом
`hMailCleanerAgentFixture_`, использует fake mail/database ports и проверяет
ожидание ручной команды и завершение restore перед SCM Stopped. Проверка
настоящего управления hMailServer, ACL, TLS и запуска EXE на Windows Server
2016 проводится отдельно; переносимые тесты и cross-build её не заменяют.

## Профили и продление сертификатов

Центр хранит технический профиль целиком в зашифрованном виде и создаёт новую
версию при каждом изменении. Предыдущая версия сохраняется для уже принятого
задания. Чтение формы возвращает признак наличия пароля; сохранение явно
выбирает Keep или Replace. Пути Windows передаются агенту без преобразования
путей на Linux. Службы самого Cleaner нельзя назначить почтовой службой.

Продление выполняется из панели после входа. Агент создаёт CSR существующим
локальным ключом, проверяет новый сертификат и заменяет только `identity.pem`.
Приватный ключ не передаётся центру. Активное задание блокирует замену;
операция не вызывает Stop/Start почтовой службы. После записи сертификата
агент закрывает старые TLS-соединения, поэтому ответ установки может потеряться.
Центр предварительно сохраняет публичный кандидат и при повторе проверяет
именно его по mTLS, затем атомарно меняет serial доверия. Отозванное доверие
не восстанавливается незавершённым продлением.

При этой же ручной операции центр обновляет свой ClientAuth-сертификат,
если тот истёк или осталось не больше 14 дней. Ключ остаётся зашифрованным
в SQLite; профили и DEK не меняются. Фоновый опрос состояния сертификаты
не перевыпускает и сессию не продлевает. Просроченный сертификат самого
агента требует восстановления через подтверждённый SSH, поскольку обычный
mTLS-канал уже не проходит проверку.

Сбой записи после входа в maintenance оставляет допуск заданий закрытым,
а агент отмечается недоступным до исправления и перезапуска. Не удаляйте
`jobs.sqlite`, ключ квитанций или identity, чтобы обойти эту защиту:
используйте проверенный пакет и локальную/SSH-процедуру восстановления.
Функции SCM, DACL и поведение NTFS нужно отдельно проверить на целевой
Windows Server 2016; переносимые тесты и cross-build этого не подтверждают.
