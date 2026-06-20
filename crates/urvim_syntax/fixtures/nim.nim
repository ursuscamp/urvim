## Nim syntax fixture
## Multi-line doc comment
## continues here

#[ block comment ]#
#[ multi-line
   block comment ]#

type
  Greeter = object
    name: string
    count: int

  Color = enum
    Red, Green, Blue

  OptionKind = enum
    Some, None

let answer = 42
let message = "hello"
let flag = true
let letter = 'x'
let floating = 3.14
let hex_val = 0xFF
let binary_val = 0b1010_0011
let octal_val = 0o77

let multiline = """hello
world"""

let raw_string = r"raw string\nno escape"

var count = 0
count += 1

proc greet(name: string): string =
  result = "Hello, " & name

proc factorial(n: int): int =
  if n <= 1:
    return 1
  else:
    return n * factorial(n - 1)

let greeting = greet("Ada")
let fact = factorial(10)

echo greeting
echo "count: ", count
echo "factorial: ", fact

for i in 0..<10:
  echo i

while count > 0:
  dec count

case answer:
  of 0:
    echo "zero"
  of 42:
    echo "answer"
  else:
    echo "other"

try:
  let f = open("test.txt")
  echo f.readAll()
  f.close()
except IOError:
  echo "file not found"
