# Готовая поставка для Hyper-V

Основной путь первой установки — ZIP с VHDX, локальным OCI и updater. Работающая
ВМ обновляется через существующий updater; замена её диска новым шаблоном стирает
пользовательское состояние и не является обновлением.

## Сборка и проверки

CI `center-ci.yml` сначала выполняет переносимые проверки, MySQL tests и Windows
agent build. После этого один source SHA используется для OCI, web, агента,
host-updater и appliance. Dirty source и несовпадающий fingerprint отклоняются.

```bash
mise run check
mise run release:contracts:check
mise run release:test
mise run appliance:test
```

Сборка выполняется на Linux с Bun из `mise.toml`, Git, libguestfs/guestfish,
qemu-img, XFS tools, tar, unzip и zip. Для supermin нужен читаемый Linux kernel
с соответствующими modules. CI готовит собственную читаемую копию kernel;
системные права рабочего сервера менять не нужно. Дополнительный изолированный
builder описан в `deploy/appliance/Builder.Containerfile`; ему не нужны privileged,
диски хоста или доступ к container-engine socket.

Предварительно соберите чистую same-SHA поставку и updater по
[CI/CD runbook](center-cicd.md). Каталог release должен содержать `center.oci.tar`,
manifest/Quadlet/agent и `host-updater/{astra-updater-linux-amd64.tar.gz,package-manifest.json}`.

```bash
export HMAIL_DELIVERY_SOURCE_SHA="$(git rev-parse HEAD)"
export HMAIL_DELIVERY_DIRECTORY="$PWD/target/releases/git-$HMAIL_DELIVERY_SOURCE_SHA"
export HMAIL_APPLIANCE_OUTPUT="$HMAIL_DELIVERY_DIRECTORY/appliance"
export LIBGUESTFS_BACKEND=direct
export LIBGUESTFS_BACKEND_SETTINGS=force_tcg
mise run appliance:build
```

Путь вывода должен быть новым; частичный результат автоматически не затирается.
`HMAIL_APPLIANCE_CACHE` задаёт кэш публичных upstream ZIP; кэш проверяется по hash.
FCOS/KVP версии и hashes закреплены в `deploy/appliance/inputs.json`. Исходный
официальный VHDX проверяется до/после сборки и не изменяется. В staging-копию
записывается только `/var/lib/astra-appliance` внутри shared OSTree var.
При неоднозначной разметке, identities или нехватке места сборка останавливается.

После закрытия writable guestfs диск открывается заново только для чтения:
проверяются состав, SHA-256 каждого файла, layout, filesystem UUID и отсутствие
machine-id/SSH host keys. Шаблон FCOS при сборке не загружается. Запускается только
служебный appliance libguestfs для доступа к файловой системе.

Результат — `astra-hyperv-git-<SHA>.zip`, manifest, `SHA256SUMS` и
`assembly-report.json`. ZIP сжимается и проверяется на лимит GitHub release asset.
Report фиксирует версии инструментов и офлайн-проверки. `native_hyperv` и
`two_vm_identity` в нём остаются `NOT_RUN`; они не выводятся из unit tests.
CI receipt связывает hashes ZIP/manifest/report с lifecycle центра. Publication
gate повторно проверяет байты; assets явно перечислены. Доставка по-прежнему
выключена, пока оператор не включил `ASTRA_DELIVERY_ENABLED` после приёмки.
Перед receipt CI также запускает официальный `ignition-validate` из закреплённого
OCI image. Эта проверка схемы конфигурации не заменяет запуск ВМ.

## Создание ВМ на созвоне с заказчиком

1. Согласовать имя/DNS панели, HTTPS origin, DHCP reservation, существующий
   virtual switch, место для диска и публичный SSH-ключ оператора. Приватный ключ
   не передаётся. Разрешить SSH и порт панели по правилам сети заказчика.
2. Получить одобренный SHA-256 ZIP отдельно от архива из доверенных GitHub
   release metadata/API либо согласованного канала. Проверить скачанный ZIP
   через `Get-FileHash -Algorithm SHA256`, затем распаковать. Внутренний
   `SHA256SUMS` подтверждает состав, но сам не удостоверяет издателя.
3. Открыть **Windows PowerShell 5.1+ от администратора** на Hyper-V host и выполнить:

   ```powershell
   .\Install-Astra.ps1 -Name Astra -SwitchName External -Destination D:\VMs\Astra `
     -SshPublicKeyPath C:\Users\Admin\.ssh\id_ed25519.pub `
     -Origin https://astra.example:8443
   ```

4. Installer проверит hashes и входы, создаст новую Gen2 ВМ, отдельную копию VHDX,
   Secure Boot `MicrosoftUEFICertificateAuthority` и передаст Ignition через KVP
   **до первого старта**. По умолчанию 4 ГБ RAM, 2 CPU, новый thin VHDX 64 ГБ;
   параметры `-MemoryGB`, `-CpuCount`, `-DiskSizeGB` меняют размеры. FCOS расширяет
   root при первом запуске. Существующие ВМ/switch/диски не изменяются.
5. Найти адрес через DHCP, подключиться `ssh core@ADDRESS`, проверить подготовку:

   ```bash
   sudo systemctl status astra-appliance-prepare.service
   sudo journalctl -u astra-appliance-prepare.service -b
   sudo -iu hmail-center /var/home/hmail-center/.local/bin/astra-initialize
   ```

6. Интерактивно задать администратора и сохранить выданный recovery key.
   Проверить fingerprint начального HTTPS-сертификата по доверенному каналу,
   открыть панель и выполнить вход. Настроить агент/MySQL/пути через существующий
   интерфейс. Первую проверку проводить предпросмотром на согласованных данных.

## Повтор, диагностика и обновления

- Повтор installer при существующей ВМ или каталоге завершается **до изменений**.
  Частичные созданные ресурсы сохраняются для диагностики; installer не удаляет
  их автоматически и не выдаёт частичную установку за готовую.
- Bootstrap проверяет hash payload из доверенного Ignition и allowlist до запуска
  файлов с диска. Он создаёт rootless Podman storage от имени UID 2000. Чужой
  существующий `hmail-center-data` без marker принадлежности не подхватывается.
- `astra-initialize` берёт lock. Команда `center init-status` отличает готовое
  состояние (exit 0), отсутствие init (exit 3) и ошибку проверки (exit 1). Если
  init успел завершиться до потери ответа, повтор распознает состояние и не
  пересоздаёт пароль/CA. После незавершённой попытки нужен осмотр данных;
  автоматического удаления volume/SQLite и сброса ключей нет.
- Состояние находится в rootless volume `hmail-center-data`; markers/updater
  config — в private home `hmail-center`. Не клонировать инициализированную ВМ.
  Backup/restore выполнять существующими командами из
  [deployment runbook](center-deployment.md). Updater принимает backup schemas
  1/2 и сверяет manifest с SQLite `user_version`; неизвестная схема отклоняется.
- Подготовка updater устанавливает его файлы/units, но оставляет discovery timer
  выключенным. После init/health включается только reboot coordinator; его
  уникальный node UUID совпадает с Zincati identity. До готового coordinator
  FCOS FleetLock откладывает перезагрузку.
- Настроить protected GitHub/registry credentials и backup/restore, затем явно
  активировать updater в login-shell `hmail-center`:

  ```bash
  bash ~/.local/state/astra-appliance/updater-package/install.sh --activate
  systemctl --user status astra-updater.timer astra-reboot-coordinator.service
  ```

## Приёмка перед рабочим использованием

На настоящем Hyper-V создать **две ВМ из двух свежих копий одного ZIP**. Проверить
успешный Ignition/KVP, Secure Boot, DHCP/SSH, root growth, `getenforce` = Enforcing,
rootless cgroup v2, user units, maintenance socket и весь init/login/preview flow.
Сравнить machine-id, SSH host key fingerprints, CA fingerprints, vault ID и
updater/Zincati node UUID: значения должны отличаться. Повтор init должен
сохранить identities и данные. Затем выполнить restart, backup/restore, условие
отложенной перезагрузки и обновление без прерывания активной очистки.

Приёмку агента на Windows Server 2016 и реальных FS attributes/SCM проводить
отдельно. Windows PowerShell fixtures в CI проверяют валидацию/отказы installer
и не создают ВМ. Успешная офлайн-сборка, green CI и native acceptance — разные
результаты; сохраняйте отдельные receipts с build, hashes, средой и итогом.
