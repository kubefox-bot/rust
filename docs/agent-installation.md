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
