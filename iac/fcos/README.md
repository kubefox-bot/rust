# Fedora CoreOS: первоначальная настройка центра

Используйте официальный **FCOS Stable live ISO** нужной архитектуры.
CPU/RAM, диск, прошивку, сеть и гипервизор настраивает оператор.
Один и тот же Ignition готовит гостевую ОС независимо от Hyper-V, KVM
или VMware; это не подтверждение проверки всех этих платформ.

Конфигурация создаёт:

- SSH-доступ к административному `core` по вашему публичному ключу;
- обычного пользователя/группу `hmail-center`, UID/GID 2000,
  home `/var/home/hmail-center`, без sudo и SSH-ключей;
- private home и каталоги пользовательских Quadlet с правами 0700;
- subordinate UID/GID диапазон `200000:65536`, сохраняя остальные записи;
- linger, запускающий systemd user manager без входа пользователя;
- стратегию Zincati FleetLock: перезагрузка ОС требует разрешения локальной
  службы Astra; до её установки автоматическая перезагрузка откладывается.

Podman уже входит в FCOS. Конфигурация не устанавливает компиляторы,
Bun/Node, MySQL или новые пакетные слои, не меняет сеть/hostname/firewall
и не запускает неинициализированный центр. Оператор обеспечивает доступ
к SSH и к HTTPS центра на 8443 по правилам своей сети.

## Исходники и параметры установки

- `config.bu` — исходник Butane, schema 1.6.0.
- `files/zincati.toml` — FleetLock endpoint на loopback и группа `astra`.
- `local/admin.pub` — ваш реальный публичный SSH-ключ, вне Git.
- `target/iac/fcos/` от корня репозитория — готовый JSON и его SHA-256, вне Git.

Создайте `iac/fcos/local/` и скопируйте туда только публичный `.pub` файл
под именем `admin.pub`. Пример назначения файлов:
[local.example](local.example/README.md). Приватный ключ остаётся у вас.
Пароль суперадмина, recovery key, MySQL и приватные TLS-ключи сюда не входят.
Принимаются обычные строки публичных ключей OpenSSH без authorized_keys options;
проверяется формат каждой непустой строки и тип ключа из `ssh -Q key`.
Одна проверка `ssh-keygen -l` недостаточна: она принимает и приватные ключи.
Не используйте ключи из тестовых отчётов: они одноразовые и не дают вам доступ.

## Подготовка на Linux с установленными инструментами

Нужны Git, OpenSSH `ssh` и `ssh-keygen`, `butane`, `ignition-validate` и `sha256sum`.
Установите официальные инструменты поддерживаемым для вашей Linux способом.
Проверьте `butane --version` и `ignition-validate --version`; валидатор должен
поддерживать Ignition 3.5.0, в который Butane преобразует schema 1.6.0.
[Официальный Butane](https://coreos.github.io/butane/getting-started/).

Выполните блок в Bash из любого каталога **внутри этого Git-репозитория**.
При ошибке любой команды блок завершается; прежний проверенный JSON не
заменяется. В staging попадают только два перечисленных входных файла.

```bash
(
  set -euo pipefail
  umask 077
  repo_root=$(git rev-parse --show-toplevel)
  output_dir="$repo_root/target/iac/fcos"
  public_key="$repo_root/iac/fcos/local/admin.pub"
  mkdir -p "$output_dir"
  stage=$(mktemp -d "$output_dir/.build.XXXXXX")
  trap 'rm -rf -- "$stage"' EXIT

  test -s "$public_key"
  awk -v key_types="$(ssh -Q key | tr '\n' ' ')" '
    BEGIN {
      size = split(key_types, types, " ")
      for (i = 1; i <= size; i++) allowed[types[i]] = 1
    }
    /^[[:space:]]*($|#)/ { next }
    NF < 2 || !($1 in allowed) || $2 !~ /^[A-Za-z0-9+\/=]+$/ { bad = 1 }
    { count++ }
    END { exit (bad || count == 0) }
  ' "$public_key"
  ssh-keygen -l -f "$public_key" >/dev/null
  cp -- "$public_key" "$stage/admin.pub"
  cp -- "$repo_root/iac/fcos/files/zincati.toml" "$stage/zincati.toml"

  butane --pretty --strict --files-dir "$stage" \
    "$repo_root/iac/fcos/config.bu" > "$stage/config.ign"
  ignition-validate "$stage/config.ign"
  mv -f -- "$stage/config.ign" "$output_dir/config.ign"
  cd "$output_dir"
  sha256sum config.ign > config.ign.sha256
  cat config.ign.sha256
)
```

Исходный YAML редактируется, JSON генерируется заново. Ненулевой exit code
Butane/validator или ошибка ключа означает, что устанавливать новый результат
нельзя. Проверка схемы не доказывает загрузку ВМ или правильность вашей сети.
Одновременную генерацию в один `output_dir` не запускайте.

## Альтернатива: официальные контейнеры

Bash-блок ниже использует Podman на Linux. Для Docker замените только
`engine=(podman)` на `engine=(docker)`; на нашей машине проверки использован
`engine=(docker --context colima)` без смены глобального context.
Не нужен `--privileged`, доступ к `/dev`, всему HOME или engine socket.

Закреплены digests инструментов, с которыми выполнены проверки 2026-10-18:
Butane **2.28.0**, Ignition validator **2.28.0**, Linux arm64. Для другой
архитектуры проверьте доступность соответствующего образа и его версию.
Обновление этих ссылок — отдельное явное изменение с повторной проверкой.

```bash
(
  set -euo pipefail
  umask 077
  engine=(podman)
  butane_image='quay.io/coreos/butane@sha256:143e7efd568e46e6f2bcb6f4250d01b0b49227232cc21a7150f8d39e577b9ea5'
  validator_image='quay.io/coreos/ignition-validate@sha256:3b21a2271b4cfe295bc5fea25f61ab732d0578654863971013fb47a528ca836a'
  repo_root=$(git rev-parse --show-toplevel)
  output_dir="$repo_root/target/iac/fcos"
  public_key="$repo_root/iac/fcos/local/admin.pub"
  mkdir -p "$output_dir"
  stage=$(mktemp -d "$output_dir/.build.XXXXXX")
  trap 'rm -rf -- "$stage"' EXIT

  test -s "$public_key"
  awk -v key_types="$(ssh -Q key | tr '\n' ' ')" '
    BEGIN {
      size = split(key_types, types, " ")
      for (i = 1; i <= size; i++) allowed[types[i]] = 1
    }
    /^[[:space:]]*($|#)/ { next }
    NF < 2 || !($1 in allowed) || $2 !~ /^[A-Za-z0-9+\/=]+$/ { bad = 1 }
    { count++ }
    END { exit (bad || count == 0) }
  ' "$public_key"
  ssh-keygen -l -f "$public_key" >/dev/null
  cp -- "$public_key" "$stage/admin.pub"
  cp -- "$repo_root/iac/fcos/files/zincati.toml" "$stage/zincati.toml"

  "${engine[@]}" run --rm \
    --mount "type=bind,source=$stage,target=/files,readonly" \
    --mount "type=bind,source=$repo_root/iac/fcos/config.bu,target=/config.bu,readonly" \
    "$butane_image" --pretty --strict --files-dir /files /config.bu \
    > "$stage/config.ign"
  "${engine[@]}" run --rm \
    --mount "type=bind,source=$stage/config.ign,target=/config.ign,readonly" \
    "$validator_image" /config.ign

  mv -f -- "$stage/config.ign" "$output_dir/config.ign"
  cd "$output_dir"
  sha256sum config.ign > config.ign.sha256
  cat config.ign.sha256
)
```

На Linux с SELinux для bind mounts может понадобиться корректная метка
выделенного staging. Не отключайте SELinux на хосте: используйте native tools
из предыдущего раздела либо отдельно настройте доступ контейнера к этим файлам.
macOS-проверка через Colima не подтверждает SELinux-доступ на Linux-хосте сборки.

Зафиксированные инструменты не закрепляют постоянно движущийся FCOS Stable.
Сохраняйте точную версию исходного ISO, его официальную проверку целостности,
версию IaC и SHA-256 конечного JSON в записи конкретной установки.

## Передача Ignition и установка с обычного ISO

1. Создайте новую машину и подключите официальный FCOS live ISO.
2. Подготовьте сеть в live среде, например через `nmtui` или `nmcli`.
   При DHCP ручной профиль может не понадобиться.
3. Передайте `config.ign` и `config.ign.sha256` на отдельном носителе,
   например на втором ISO с файлами, и подключите его к ВМ.
4. Смонтируйте носитель в каталог, перейдите в него и выполните
   `sha256sum --check config.ign.sha256`. Проверьте источник контрольной суммы.
5. Посмотрите `lsblk -o NAME,SIZE,TYPE,MOUNTPOINTS` и выберите целевой диск.
   **Установка перезаписывает выбранный диск.** В IaC нет автоматического
   выбора устройства; не копируйте имя диска от другой ВМ.

Пример команды из live среды после выбора устройства и проверки файла
(значения в угловых скобках заменяются вашими реальными путями):

```text
sudo coreos-installer install \
  --ignition-file /путь/к/config.ign \
  --copy-network <выбранный-целевой-диск>
```

`--copy-network` переносит NetworkManager keyfiles из live среды, но не создаёт
отсутствующую конфигурацию и не заменяет проверку сети. Проверьте hostname/DNS,
маршруты и firewall самостоятельно. После успешной установки отключите ISO
и перезагрузите ВМ с диска. Конфигурация применяется к **установленной ОС**;
live Ignition и destination Ignition — разные настройки.

Вместо носителя установщик умеет получить JSON через `--ignition-url`.
Используйте доверенный HTTPS-адрес. Для HTTP обязательно передайте
`--ignition-hash sha256-<проверенный-хеш-конечного-JSON>`; хеш получают
независимо от незащищённого скачивания. `--insecure-ignition` не используем.
[Параметры coreos-installer](https://coreos.github.io/coreos-installer/cmd/install/).

## Проверка после первого запуска

Подключитесь `ssh core@<адрес-ВМ>`, проверив SSH host key подходящим для вашей
инфраструктуры способом. Затем:

```bash
getent passwd hmail-center
getent group hmail-center
sudo stat -c '%U:%G %a %n' \
  /var/home/hmail-center \
  /var/home/hmail-center/.config \
  /var/home/hmail-center/.config/containers \
  /var/home/hmail-center/.config/containers/systemd
loginctl show-user hmail-center -p Linger
systemctl is-active user@2000.service
```

Ожидается UID/GID 2000, home `/var/home/hmail-center`, владелец всех четырёх
каталогов `hmail-center:hmail-center`, mode 700, `Linger=yes`, manager `active`.

Проверьте **все** диапазоны в обоих файлах. FCOS может дополнительно выделить
диапазон при создании пользователя. Несколько непересекающихся диапазонов
для одного пользователя допустимы; записи `core` и других пользователей
должны сохраниться. Настроенный `hmail-center:200000:65536` должен присутствовать.
Проверка на пересечения (каждый файл отдельно):

```bash
(
set -e
for file in /etc/subuid /etc/subgid; do
  sudo awk -F: '
    NF == 0 { next }
    NF != 3 || $2 !~ /^[0-9]+$/ || $3 !~ /^[0-9]+$/ || $3 < 1 {
      print "Invalid range:", $0; bad = 1; next
    }
    {
      start = $2; finish = $2 + $3
      for (i = 1; i <= count; i++) {
        if (start < ends[i] && starts[i] < finish) {
          print "Overlapping ranges:", owners[i], $1; bad = 1
        }
      }
      count++; starts[count] = start; ends[count] = finish; owners[count] = $1
    }
    END { if (bad) exit 1; print "No overlapping ranges:", FILENAME }
  ' "$file"
done
)
```

При конфликте диапазонов не запускайте приложение. При переносе существующих
данных сохраняйте согласованность UID/GID и **всех** subordinate ranges:
схема rootless mapping зависит от полных файлов этой машины.

Для работы с центром откройте shell именно служебного пользователя:

```bash
sudo -iu hmail-center
export XDG_RUNTIME_DIR="/run/user/$(id -u)"
export DBUS_SESSION_BUS_ADDRESS="unix:path=$XDG_RUNTIME_DIR/bus"
cd "$HOME"
systemctl --user is-system-running
podman info --format '{{.Host.Security.Rootless}} {{.Host.CgroupsVersion}}'
podman unshare cat /proc/self/uid_map /proc/self/gid_map
```

User manager должен быть `running`, Podman — `true v2`. Linger уже создал
`/run/user/2000` и bus; не создавайте их вручную с чужим владельцем.
Bare `sudo -u` недостаточно: у процесса могут остаться недоступный CWD и
окружение административного пользователя. После завершения проверки shell
можно закрыть командой `exit`; пользовательские службы должны оставаться
доступными без интерактивной сессии.

## Установка приложения и дальнейшее обслуживание

Оставаясь в корректном shell `hmail-center`, продолжайте по
[инструкции центра](../../docs/center-deployment.md#подготовка-linux-вм-для-rootless-podman):
проверьте и загрузите локальную поставку через `podman load`, создайте named
volume, интерактивно выполните `center init`, сохраните recovery key отдельно,
установите **сгенерированные из этой поставки** Quadlet и проверьте HTTPS health.
Шаблон из `deploy/quadlet` напрямую не устанавливается. Доверие публичному CA
в браузере настраивается отдельно. Изменять `Pull=never` не нужно.

Ignition запускается только при первом запуске. Изменение файла в Git
не меняет работающую ОС; повторный запуск Ignition на машине с данными
не является способом обновления. Приложение обновляется отдельно по runbook.
После перезапуска центра нужен вход суперадмина для разблокировки настроек.

Zincati управляет **обновлениями ОС** через локальный FleetLock coordinator
на `127.0.0.1:35355`. Coordinator разрешает новую перезагрузку в воскресенье
03:00–04:00 UTC после закрытия admission и завершения активных заданий,
подготовки агентов и записей. Занятый updater, недоступный центр или неизвестное
состояние откладывают reboot. Periodic и FleetLock не включаются одновременно:
прежнее окно теперь проверяет coordinator. Одна ВМ означает перерыв доступа
к панели при разрешённой перезагрузке.

Первичная установка updater создаёт уникальный UUID узла и сохраняет его
в private `~/.local/state/astra-updater/reboot-client.json`. Администратор
добавляет соответствующий `[identity] node_uuid` в отдельный Zincati fragment,
сохраняя `group = "astra"`, запускает user unit
`astra-reboot-coordinator.service` и затем перезапускает Zincati. Произвольный
локальный HTTP-запрос без FleetLock header и правильных node UUID/group
не получает разрешение. Полные команды устанавливает runbook доставки.

Grant содержит UUID операции и Linux boot_id, хранится на диске до
`/v1/steady-state`: этот запрос после reboot либо отмены перезагрузки
снимает только собственный marker. При недоступном центре coordinator сохраняет
grant и отказывает в подтверждении. При отмене без запроса Zincati оператор
может отправить тот же FleetLock steady-state с известной identity; чужой
marker обновления так не снимается. Перезапуск службы coordinator не удаляет
grant и не открывает admission.

[Восстановление](../../docs/recovery.md) требует согласованной резервной копии
за пределами потерянной ВМ. IaC восстанавливает окружение, а не SQLite и
TLS identities. Настройка доставки и её приёмка описываются отдельно
в runbook центра.

## Фактическая проверка

Результаты и границы предыдущей конфигурации с periodic strategy:
[приёмка IaC](../../docs/fcos-iac-acceptance.md). Они подтверждают bootstrap,
rootless Podman и linger; runtime-проверка новой FleetLock strategy на FCOS
и целевом Hyper-V требуется отдельно.
