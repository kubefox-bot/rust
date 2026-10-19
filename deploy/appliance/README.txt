Astra для Hyper-V

До распаковки проверьте SHA-256 всего ZIP по значению, полученному отдельно
из доверенных release metadata/API. SHA256SUMS внутри архива не подтверждает
издателя сам по себе. Не запускайте installer из непроверенного архива.

Нужны Windows PowerShell 5.1+, Hyper-V, права администратора, существующий
virtual switch, DHCP/DHCP reservation и разрешённые SSH/HTTPS-соединения.
Комплект содержит FCOS x86_64 и приложение. Registry при первом запуске не нужен.
Паролей, приватных SSH-ключей, готовой БД и CA в шаблоне нет.

Пример из административного PowerShell:
  .\Install-Astra.ps1 -Name Astra -SwitchName External -Destination D:\VMs\Astra `
    -SshPublicKeyPath C:\Users\Admin\.ssh\id_ed25519.pub -Origin https://astra.example:8443

Имя ВМ и каталог должны быть новыми. Installer создаёт Gen2 ВМ, включает
Microsoft UEFI Certificate Authority и передаёт Ignition через KVP до запуска.
Новая копия диска расширяется до 64 ГБ (-DiskSizeGB); FCOS расширяет root при
первом запуске. По умолчанию ВМ получает 4 ГБ RAM и 2 CPU.
При ошибке частичные ресурсы сохраняются; не удаляйте их без проверки.

После получения DHCP-адреса:
  ssh core@ADDRESS
  sudo systemctl status astra-appliance-prepare.service
  sudo -iu hmail-center /var/home/hmail-center/.local/bin/astra-initialize

Init интерактивно создаёт администратора и уникальные ключи. Сохраните recovery
key в согласованном защищённом месте. Доверие к начальному HTTPS-сертификату
проверяйте отдельно. При прерывании повтор команды не стирает данные.
Если init остался незавершённым, сначала проверьте диагностику и существующий
том; автоматического сброса пароля/CA/SQLite нет.

Автообновление приложения изначально выключено. Перед активацией настройте
protected credentials updater и registry, проверьте доступ и backup/restore.
Затем в login-shell hmail-center:
  bash ~/.local/state/astra-appliance/updater-package/install.sh --activate

Данные живут в rootless volume hmail-center-data, вне обновляемого OCI.
VHDX — шаблон только для первой установки; обновлять работающую ВМ заменой
диска нельзя. Не клонируйте уже инициализированную ВМ.

assembly-report.json подтверждает офлайн-упаковку и readback. Он не заменяет
приёмку первого запуска двух независимых ВМ на настоящем Hyper-V, SELinux,
разных machine-id/SSH/CA/vault/node UUID, backup/restore и Windows Server 2016.
