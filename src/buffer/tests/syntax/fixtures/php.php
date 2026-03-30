<?php
// PHP syntax fixture
final class Greeter {
    public string $name = "Ada";
    public int $count = 42;
    public function hello(): string {
        return "hello";
    }
}

$message = <<<EOF
php heredoc body
EOF;
