#!/usr/bin/env bash
# Initial offline installation. Sourceable operations also support filesystem verification.
set -euo pipefail

verify_package() {
  local package=$1 line name seen='' count=0
  [[ -d "$package" && ! -L "$package" ]] || return 1
  [[ -f "$package/SHA256SUMS" && ! -L "$package/SHA256SUMS" ]] || return 1
  while IFS= read -r line || [[ -n "$line" ]]; do
    [[ "$line" =~ ^([0-9a-f]{64})\ \ ([A-Za-z0-9._-]+)$ ]] || return 1
    name=${BASH_REMATCH[2]}
    case "$name" in
      astra-updater|cosign|sigstore-trusted-root.json|install.sh|config.example.json|package-manifest.json|astra-updater.service|astra-updater.timer|astra-updater-recover.service|astra-reboot-coordinator.service) ;;
      *) return 1 ;;
    esac
    case " $seen " in *" $name "*) return 1 ;; esac
    [[ -f "$package/$name" && ! -L "$package/$name" ]] || return 1
    seen="$seen $name"
    count=$((count + 1))
  done < "$package/SHA256SUMS"
  [[ "$count" -eq 10 ]] || return 1
  (cd "$package" && sha256sum --check --strict SHA256SUMS)
}

require_new_install() {
  local install_home=$1 target
  for target in "$install_home" "$install_home/.local" "$install_home/.local/bin" "$install_home/.local/lib" \
    "$install_home/.local/lib/astra-updater" "$install_home/.local/state" "$install_home/.local/state/astra-updater" \
    "$install_home/.local/share" "$install_home/.local/share/astra-updater" \
    "$install_home/.local/share/astra-updater/staging" "$install_home/.local/share/astra-updater/backups" \
    "$install_home/.config" "$install_home/.config/astra-updater" "$install_home/.config/systemd" \
    "$install_home/.config/systemd/user" "$install_home/.config/systemd/user/hmail-center.service.d"; do
    [[ ! -L "$target" ]] || return 1
    if [[ -e "$target" && ( ! -d "$target" || ! -O "$target" ) ]]; then return 1; fi
  done
  for target in \
    "$install_home/.local/bin/astra-updater" \
    "$install_home/.local/lib/astra-updater/cosign" \
    "$install_home/.local/lib/astra-updater/sigstore-trusted-root.json" \
    "$install_home/.config/astra-updater/config.json" \
    "$install_home/.config/astra-updater/credentials.json" \
    "$install_home/.config/astra-updater/registry-auth.json" \
    "$install_home/.local/state/astra-updater/reboot-client.json" \
    "$install_home/.config/systemd/user/astra-updater.service" \
    "$install_home/.config/systemd/user/astra-updater.timer" \
    "$install_home/.config/systemd/user/astra-updater-recover.service" \
    "$install_home/.config/systemd/user/astra-reboot-coordinator.service" \
    "$install_home/.config/systemd/user/hmail-center.service.d/20-astra-recovery.conf"; do
    if [[ -e "$target" || -L "$target" ]]; then
      printf 'Refusing to overwrite existing installation: %s\n' "$target" >&2
      return 1
    fi
  done
}

install_files() {
  local package=$1 install_home=$2 node_uuid=$3 unit
  [[ "$node_uuid" =~ ^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$ ]] || return 1
  [[ "$node_uuid" != 00000000-0000-0000-0000-000000000000 ]] || return 1
  verify_package "$package"
  require_new_install "$install_home"
  umask 077
  install -d -m 0700 \
    "$install_home/.local/bin" "$install_home/.local/lib/astra-updater" \
    "$install_home/.config/astra-updater" "$install_home/.local/state/astra-updater" \
    "$install_home/.local/share/astra-updater/staging" "$install_home/.local/share/astra-updater/backups" \
    "$install_home/.config/systemd/user/hmail-center.service.d"
  install -m 0755 "$package/astra-updater" "$install_home/.local/bin/astra-updater"
  install -m 0755 "$package/cosign" "$install_home/.local/lib/astra-updater/cosign"
  install -m 0600 "$package/sigstore-trusted-root.json" "$install_home/.local/lib/astra-updater/sigstore-trusted-root.json"
  install -m 0600 "$package/config.example.json" "$install_home/.config/astra-updater/config.json"
  printf '{}\n' > "$install_home/.config/astra-updater/credentials.json"
  printf '{"auths":{}}\n' > "$install_home/.config/astra-updater/registry-auth.json"
  printf '{"id":"%s","group":"astra"}\n' "$node_uuid" > "$install_home/.local/state/astra-updater/reboot-client.json"
  for unit in astra-updater.service astra-updater.timer astra-updater-recover.service astra-reboot-coordinator.service; do
    install -m 0600 "$package/$unit" "$install_home/.config/systemd/user/$unit"
  done
  printf '[Unit]\nRequires=astra-updater-recover.service\nAfter=astra-updater-recover.service\n' > "$install_home/.config/systemd/user/hmail-center.service.d/20-astra-recovery.conf"
}

main() {
  [[ $# -eq 0 ]] || { printf 'Usage: bash install.sh\n' >&2; return 1; }
  [[ "$(uname -s)" == Linux && "$(uname -m)" == x86_64 ]] || { printf 'Linux x86_64 is required.\n' >&2; return 1; }
  [[ "$(id -u)" == 2000 && "$(id -un)" == hmail-center ]] || { printf 'Run in the hmail-center login shell (UID 2000).\n' >&2; return 1; }
  [[ "$HOME" == /var/home/hmail-center && -d "$HOME" && -O "$HOME" && ! -L "$HOME" ]] || { printf 'Unexpected service home.\n' >&2; return 1; }
  [[ "${XDG_RUNTIME_DIR:-}" == /run/user/2000 && "${DBUS_SESSION_BUS_ADDRESS:-}" == unix:path=/run/user/2000/bus ]] || { printf 'Set the documented login-shell XDG runtime and user bus environment.\n' >&2; return 1; }
  local required package node_uuid
  for required in sha256sum install systemctl podman cat; do command -v "$required" >/dev/null; done
  systemctl --user is-system-running
  [[ "$(podman info --format '{{.Host.Security.Rootless}} {{.Host.CgroupsVersion}}')" == 'true v2' ]] || { printf 'Rootless Podman with cgroup v2 is required.\n' >&2; return 1; }
  [[ -f "$HOME/.config/containers/systemd/hmail-center.container" && ! -L "$HOME/.config/containers/systemd/hmail-center.container" ]] || { printf 'Install and initialize the center release first.\n' >&2; return 1; }
  systemctl --user is-active --quiet hmail-center.service
  [[ -S "$XDG_RUNTIME_DIR/astra-maintenance/control.sock" ]] || { printf 'Install a center release with the maintenance socket before delivery setup.\n' >&2; return 1; }
  package=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
  node_uuid=$(cat /proc/sys/kernel/random/uuid)
  install_files "$package" "$HOME" "$node_uuid"
  systemctl --user daemon-reload
  systemctl --user start astra-updater-recover.service
  systemctl --user enable --now astra-reboot-coordinator.service astra-updater.timer
  printf '\nInstalled initial updater. Empty GitHub/registry credentials are placeholders; configure private access using protected files, never command-line tokens.\n'
  printf 'Check: systemctl --user status astra-updater.timer astra-reboot-coordinator.service\n'
  printf 'Initial root operator step, only after coordinator is active: write /etc/zincati/config.d/55-astra-identity.toml with:\n'
  printf '[identity]\nnode_uuid = "%s"\ngroup = "astra"\n' "$node_uuid"
  printf 'Then restart system Zincati. Until identity matches, reboot stays deferred.\n'
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then main "$@"; fi
