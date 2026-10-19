#!/usr/bin/env bash
# Durable, private bootstrap markers. Callers own their application state directory.
ensure_state() {
  local directory=$1 current='' part
  [[ "$directory" == /* ]] || return 1
  local components
  IFS=/ read -r -a components <<< "$directory"
  for part in "${components[@]}"; do
    [[ -n "$part" ]] || continue
    [[ "$part" != . && "$part" != .. ]] || return 1
    current="$current/$part"
    [[ ! -L "$current" ]] || return 1
  done
  (umask 077; mkdir -p -- "$directory")
  [[ -d "$directory" && -O "$directory" ]] || return 1
  chmod 0700 "$directory"
}
marker_matches() {
  local directory=$1 name=$2 expected=$3 actual
  [[ -f "$directory/$name" && ! -L "$directory/$name" && -O "$directory/$name" ]] || return 1
  IFS= read -r actual < "$directory/$name"
  [[ "$actual" == "$expected" ]]
}
write_state() {
  local directory=$1 name=$2 value=$3 temporary
  case "$name" in prepared|volume.claim|initialization.attempted|initialized|ready) ;; *) return 1 ;; esac
  ensure_state "$directory"
  [[ ! -L "$directory/$name" ]] || return 1
  temporary=$(mktemp "$directory/.marker.XXXXXX")
  chmod 0600 "$temporary"
  printf '%s\n' "$value" > "$temporary"
  sync -f "$temporary"
  mv -- "$temporary" "$directory/$name"
  sync -f "$directory"
}
require_service_user() {
  [[ "$(id -u)" == 2000 && "$(id -un)" == hmail-center &&
     "$HOME" == /var/home/hmail-center && ! -L "$HOME" ]] || {
    printf 'Use the hmail-center login shell (UID 2000).\n' >&2; return 1;
  }
  export XDG_RUNTIME_DIR=/run/user/2000
  export DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/2000/bus
}
