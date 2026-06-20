#!/usr/bin/env bash
# Bash syntax fixture

build() {
  local -a targets=("release" "debug")
  declare -r MODE="debug"
  typeset -i count=2
  local escaped="line 1\nline 2"

  if [[ -n ${MODE} ]]; then
    printf '%s\n' "${targets[0]}"
  fi

  if (( count > 1 )); then
    echo $'ansi\nquote'
  fi

  cat <<EOF
bash heredoc
EOF
}

build
