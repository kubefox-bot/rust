#!/usr/bin/env bash
# Installed through Ignition, so verification runs before any disk payload code.
set -euo pipefail
umask 077
verify_payload() {
  local payload=$1 expected=$2 line name seen='' count=0 actual
  [[ "$expected" =~ ^[0-9a-f]{64}$ && -d "$payload" && ! -L "$payload" &&
     -f "$payload/SHA256SUMS" && ! -L "$payload/SHA256SUMS" ]] || return 1
  actual=$(sha256sum "$payload/SHA256SUMS" | cut -d' ' -f1)
  [[ "$actual" == "$expected" ]] || return 1
  [[ -z "$(find "$payload" ! -type f ! -type d -print -quit)" ]] || return 1
  while IFS= read -r line || [[ -n "$line" ]]; do
    [[ "$line" =~ ^[0-9a-f]{64}\ \ ([A-Za-z0-9._/-]+)$ ]] || return 1
    name=${BASH_REMATCH[1]}
    case "$name" in
      center.oci.tar|release-manifest.json|quadlet/hmail-center.container|quadlet/hmail-center-data.volume|host-updater/astra-updater-linux-amd64.tar.gz|host-updater/package-manifest.json|scripts/prepare.sh|scripts/prepare-user.sh|scripts/initialize.sh|scripts/lib/state.sh|build|image|image-config-digest|initialization.conf|astra-initialize|payload-manifest.json) ;;
      *) return 1;; esac
    case " $seen " in *" $name "*) return 1;; esac
    [[ -f "$payload/$name" && ! -L "$payload/$name" ]] || return 1
    seen="$seen $name"; count=$((count+1))
  done < "$payload/SHA256SUMS"
  [[ "$count" -eq 16 && "$(find "$payload" -type f | wc -l)" -eq 17 ]] || return 1
  (cd "$payload" && sha256sum --check --strict SHA256SUMS)
}
main() {
  [[ "$(id -u)" == 0 && $# -eq 0 ]] || return 1
  local payload=/var/lib/astra-appliance install_state=/var/lib/astra-appliance-install node_uuid line temporary
  verify_payload "$payload" "${ASTRA_PAYLOAD_SHA256:?Missing trusted payload checksum}"
  restorecon -RF "$payload"
  [[ -f "$install_state/origin" && ! -L "$install_state/origin" ]] || return 1
  runuser -u hmail-center -- env HOME=/var/home/hmail-center XDG_RUNTIME_DIR=/run/user/2000 \
    DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/2000/bus /usr/bin/bash "$payload/scripts/prepare-user.sh"
  IFS= read -r line < /var/home/hmail-center/.local/state/astra-updater/reboot-client.json
  [[ "$line" =~ ^\{\"id\":\"([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})\",\"group\":\"astra\"\}$ ]] || return 1
  node_uuid=${BASH_REMATCH[1]}
  [[ "$node_uuid" != 00000000-0000-0000-0000-000000000000 ]] || return 1
  temporary=$(mktemp /etc/zincati/config.d/.astra-identity.XXXXXX)
  printf '[identity]\nnode_uuid = "%s"\ngroup = "astra"\n' "$node_uuid" > "$temporary"
  chmod 0644 "$temporary"
  sync -f "$temporary"
  mv "$temporary" /etc/zincati/config.d/55-astra-identity.toml
  printf '%s\n' "$ASTRA_PAYLOAD_SHA256" > "$install_state/prepared"
  sync -f "$install_state"
  printf 'Appliance prepared. Run sudo -iu hmail-center /var/home/hmail-center/.local/bin/astra-initialize over SSH.\n'
}
if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then main "$@"; fi
