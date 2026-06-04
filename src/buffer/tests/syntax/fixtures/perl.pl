# Perl syntax fixture
# Multi-line comment
=pod
Multi-line comment
in Perl pod format
=cut

use strict;
use warnings;
use feature 'say';

my $name = "Ada";
my @items = (1, 2, 3);
my %mapping = (name => "Ada", age => 42);
my $count = 42;
my $flag = 1;
my $empty = undef;
my $floating = 3.14;
my $hex = 0xFF;
my $octal = 0777;
my $binary = 0b1010_0011;
my $float_exp = 1.5e-2;
my $char = 'x';
my $interp = "hello $name";
my $escaped = "line 1\nline 2";

my $re = m/hello/;
my $gre = $name =~ /Ada/;
my $subst = $name =~ s/Ada/Grace/r;
my $qr = qr/pattern/;
my $match = "hello world" =~ m/world/;

my $message = <<EOF;
perl heredoc body
EOF

my $nowdoc = <<'EOF';
perl nowdoc body
EOF

my @sliced = @items[0, 1];
my $first = $items[0];
$mapping{name} = "Grace";

sub greet {
    my ($name) = @_;
    return "Hello, $name";
}

sub factorial {
    my ($n) = @_;
    return 1 if $n <= 1;
    return $n * factorial($n - 1);
}

my $result = greet("Ada");

if ($count == 42) {
    say "answer";
} elsif ($count > 0) {
    say "positive";
} else {
    say "other";
}

foreach my $item (@items) {
    say $item;
}

for (my $i = 0; $i < 10; $i++) {
    say $i;
}

while ($count > 0) {
    $count--;
}

eval {
    die "error occurred";
};
if ($@) {
    warn "caught: $@";
}

package MyGreeter;
sub new {
    my $class = shift;
    my $self = {name => shift // "world"};
    return bless $self, $class;
}
sub greet {
    my $self = shift;
    return "Hello, $self->{name}";
}
1;
