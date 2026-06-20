#!/usr/bin/env fish
# Fish syntax fixture

function greet
    set -l name "Ada"
    set -l escaped "line 1\nline 2"
    if true
        echo "hello $name"
    end
end

echo (string upper -- $name)
echo (math 1 + 2)

greet
