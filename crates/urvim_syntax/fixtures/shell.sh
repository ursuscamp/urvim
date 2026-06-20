#!/usr/bin/env sh
# Shell syntax fixture

greet() {
  local name="Ada"
  readonly count=1
  export PATH="/usr/bin:$PATH"
  local escaped="line 1\nline 2"

  if true; then
    echo "hello $name"
  fi

  case "$name" in
    Ada) echo "named Ada" ;;
    *) echo "other" ;;
  esac
}

greet

echo "${name:-world}"
echo "$(date)"
echo "$((1 + 2))"
echo `whoami`

cat <<'EOF'
heredoc body
EOF

cat <<-EOF
tabbed heredoc body
	EOF
EOF
