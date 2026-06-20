<?php
// PHP syntax fixture
# Shell-style comment too
/* Multi-line
   block comment */
/** Doc comment */

declare(strict_types=1);

namespace App\Greeter;

use App\Person;
use function strlen;
use const PHP_VERSION;

final class Greeter {
    public string $name = "Ada";
    public int $count = 42;
    protected float $rate = 3.14;
    private bool $active = true;

    public function hello(): string {
        return "hello";
    }

    public function greet(string $name): string {
        return "Hello, $name";
    }

    public function evaluate(): void {
        $x = 42;
        $y = 3.14;
        $hex = 0xFF;
        $octal = 0o77;
        $binary = 0b1010_0011;
        $float = 1.5e-2;
        $null = null;
        $flag = false;

        if ($x > 0) {
            echo "positive\n";
        } elseif ($x == 0) {
            echo "zero\n";
        } else {
            echo "negative\n";
        }

        $items = [1, 2, 3];
        foreach ($items as $item) {
            echo $item;
        }

        for ($i = 0; $i < 10; $i++) {
            echo $i;
        }

        while ($x > 0) {
            $x--;
        }

        try {
            if ($x < 0) {
                throw new \InvalidArgumentException("negative");
            }
        } catch (\InvalidArgumentException $e) {
            echo $e->getMessage();
        } finally {
            echo "done";
        }

        match ($x) {
            42 => "answer",
            default => "other",
        };
    }
}

$message = <<<EOF
php heredoc body
EOF;

$nowdoc = <<<'EOF'
php nowdoc body
EOF;

$array = [
    "key" => "value",
    "count" => 42,
];

fn($x) => $x * 2;

#[Attribute]
class MyClass {}

$closure = function(string $name): string {
    return "Hello, $name";
};

enum Status: string {
    case Active = 'active';
    case Inactive = 'inactive';
}
