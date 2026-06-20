#!/usr/bin/env zsh
# Zsh syntax fixture

say_hi() {
  setopt localoptions
  local who="world"
  local escaped="line 1\nline 2"
  print "hello ${who:h}"
  print *(.)
  typeset -a items=("one" "two")
}

say_hi
