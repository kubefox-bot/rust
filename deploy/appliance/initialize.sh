#!/usr/bin/env bash
set -euo pipefail
umask 077
# shellcheck source=lib/state.sh
source "$(dirname -- "${BASH_SOURCE[0]}")/lib/state.sh"
readonly HEALTH_ATTEMPTS=60
readonly HEALTH_RETRY_SECONDS=2

probe_initialized() {
  local image=$1
  podman run --rm --read-only --user 10001:10001 --userns keep-id:uid=10001,gid=10001 \
    --volume hmail-center-data:/data:Z,U "$image" center init-status --data-dir /data >/dev/null 2>&1
}
initialize_center() {
  local payload=$1 state=$2 origin=$3 image=$4 build=$5 status=0 init_result=0 attempt
  ensure_state "$state"
  if marker_matches "$state" ready "$build"; then
    printf 'Astra was already initialized; existing data retained.\n'; return;
  fi
  if [[ -e "$state/ready" ]]; then printf 'Unexpected initialization marker.\n' >&2; return 1; fi
  if ! marker_matches "$state" initialized "$build"; then
    probe_initialized "$image" || status=$?
    case "$status" in
      0) ;;
      3)
        if [[ -e "$state/initialization.attempted" ]]; then
          printf 'Previous initialization is incomplete; inspect existing data before retry.\n' >&2
          return 1
        fi
        write_state "$state" initialization.attempted "$build"
        # An interrupted response does not prove that the transaction failed.
        podman run --rm -it --read-only --user 10001:10001 --userns keep-id:uid=10001,gid=10001 \
          --volume hmail-center-data:/data:Z,U "$image" center init --data-dir /data \
          --origin "$origin" --listen 0.0.0.0:8443 || init_result=$?
        if ! probe_initialized "$image"; then
          printf 'Initialization was not confirmed (init exit %s); existing data retained.\n' "$init_result" >&2; return 1;
        fi
        ;;
      *) printf 'Cannot inspect center initialization; no init was attempted.\n' >&2; return 1 ;;
    esac
    write_state "$state" initialized "$build"
  fi
  systemctl --user daemon-reload
  systemctl --user start hmail-center.service
  for ((attempt=0; attempt<HEALTH_ATTEMPTS; attempt++)); do
    if podman exec hmail-center /usr/local/bin/hmail-cleaner center health \
      --data-dir /data --expect-build "$build"; then
      systemctl --user enable --now astra-reboot-coordinator.service
      write_state "$state" ready "$build"
      printf 'Astra is ready. Discovery timer remains disabled.\n'
      return
    fi
    sleep "$HEALTH_RETRY_SECONDS"
  done
  printf 'Center health was not confirmed; initialization will not be repeated.\n' >&2
  return 1
}
main() {
  [[ $# -eq 0 ]] || { printf 'Usage: astra-initialize\n' >&2; return 1; }
  require_service_user
  local payload=/var/lib/astra-appliance state="$HOME/.local/state/astra-appliance" image build origin
  IFS= read -r image < "$payload/image"
  IFS= read -r build < "$payload/build"
  IFS= read -r origin < /var/lib/astra-appliance-install/origin
  [[ "$image" =~ ^ghcr.io/kubefox-bot/astra-center@sha256:[0-9a-f]{64}$ &&
     "$build" =~ ^git-[0-9a-f]{40}$ && "$origin" == https://* ]] || return 1
  [[ -f "$state/prepared" && ! -L "$state/prepared" ]] || {
    printf 'Appliance preparation has not completed.\n' >&2; return 1;
  }
  exec 9>"$state/initialization.lock"
  flock -n 9 || { printf 'Initialization is already running.\n' >&2; return 1; }
  initialize_center "$payload" "$state" "$origin" "$image" "$build"
}
if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then main "$@"; fi
