#!/usr/bin/env bash
set -euo pipefail
umask 077
# shellcheck source=lib/state.sh
source "$(dirname -- "${BASH_SOURCE[0]}")/lib/state.sh"

install_matching() {
  local source=$1 target=$2 mode=$3
  [[ ! -L "$target" ]] || return 1
  if [[ -e "$target" ]]; then
    [[ -f "$target" && -O "$target" ]] && cmp "$source" "$target"
  else
    install -m "$mode" "$source" "$target"
  fi
}
claim_volume() {
  local state=$1 build=$2
  if ! marker_matches "$state" volume.claim "$build"; then
    [[ ! -e "$state/volume.claim" ]] || return 1
    if podman volume exists hmail-center-data; then
      printf 'Refusing to adopt an existing unclaimed center volume.\n' >&2; return 1
    else
      [[ $? -eq 1 ]] || return 1
    fi
    write_state "$state" volume.claim "$build"
  fi
  if podman volume exists hmail-center-data; then return; else [[ $? -eq 1 ]] || return 1; fi
  podman volume create hmail-center-data >/dev/null
}
verify_image() {
  local image=$1 config_digest=$2 actual
  actual=$(podman image inspect --format '{{.Id}}' "$image")
  actual=${actual#sha256:}
  [[ "$actual" =~ ^[0-9a-f]{64}$ && "$config_digest" == "sha256:$actual" ]] || {
    printf 'Loaded image identity differs from the release.\n' >&2; return 1;
  }
}
prepare_user() {
  local payload=$1 state=$2 build=$3 image=$4 config_digest=$5 package entry
  ensure_state "$state"
  if marker_matches "$state" prepared "$build"; then return; fi
  [[ ! -e "$state/prepared" ]] || return 1
  claim_volume "$state" "$build"
  podman load --input "$payload/center.oci.tar"
  verify_image "$image" "$config_digest"
  install -d -m 0700 "$HOME/.config/containers/systemd" "$HOME/.config/systemd/user/hmail-center.service.d" "$HOME/.local/bin"
  install_matching "$payload/quadlet/hmail-center.container" "$HOME/.config/containers/systemd/hmail-center.container" 0600
  install_matching "$payload/quadlet/hmail-center-data.volume" "$HOME/.config/containers/systemd/hmail-center-data.volume" 0600
  install_matching "$payload/initialization.conf" "$HOME/.config/systemd/user/hmail-center.service.d/10-astra-initialization.conf" 0600
  install_matching "$payload/astra-initialize" "$HOME/.local/bin/astra-initialize" 0755
  package="$state/updater-package"
  # The root preparation already verified the hash of the entire archive.
  # Reject unexpected members before extracting into this private directory.
  local seen='' count=0
  while IFS= read -r entry; do
    case "$entry" in astra-updater|cosign|sigstore-trusted-root.json|install.sh|config.example.json|package-manifest.json|SHA256SUMS|astra-updater.service|astra-updater.timer|astra-updater-recover.service|astra-reboot-coordinator.service) ;;
      *) printf 'Unexpected updater archive entry.\n' >&2; return 1;; esac
    case " $seen " in *" $entry "*) return 1;; esac
    seen="$seen $entry"; count=$((count+1))
  done < <(tar -tzf "$payload/host-updater/astra-updater-linux-amd64.tar.gz")
  [[ "$count" -eq 11 ]] || return 1
  ensure_state "$package"
  tar -xzf "$payload/host-updater/astra-updater-linux-amd64.tar.gz" --no-same-owner --no-same-permissions -C "$package"
  bash "$package/install.sh" --prepare
  write_state "$state" prepared "$build"
}
main() {
  require_service_user
  local payload=/var/lib/astra-appliance state="$HOME/.local/state/astra-appliance" build image config_digest
  ensure_state "$state"
  exec 9>"$state/preparation.lock"
  flock -n 9 || { printf 'Preparation is already running.\n' >&2; return 1; }
  IFS= read -r build < "$payload/build"
  IFS= read -r image < "$payload/image"
  IFS= read -r config_digest < "$payload/image-config-digest"
  prepare_user "$payload" "$state" "$build" "$image" "$config_digest"
}
if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then main "$@"; fi
