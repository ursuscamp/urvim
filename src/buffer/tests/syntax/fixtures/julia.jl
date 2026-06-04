# Julia syntax fixture
# Multi-line comment
# continues here
#= Multi-line
   block comment
   in Julia =#

module Greeter

@enum Mode Idle Busy

export greet, Mode

struct Person
    name::String
    age::Int
end

value = "hello"
answer = 42
flag = true
letter = 'x'
floating = 3.14
hex_val = 0xFF
octal_val = 0o77
binary_val = 0b1010_0011
float_exp = 1.5e-2

multiline = """hello
world"""

function greet(name)
    return "Hello, $name"
end

function factorial(n::Int)::Int
    if n <= 1
        return 1
    else
        return n * factorial(n - 1)
    end
end

const MAX_SIZE = 1024
const PI = 3.14159

mutable struct Counter
    count::Int
end

function increment!(c::Counter)
    c.count += 1
end

p = Person("Ada", 42)
msg = greet("world")
fact = factorial(10)

println(msg)
println("factorial: $fact")

for i in 1:10
    println(i)
end

while answer > 0
    break
end

if flag
    println("true")
elseif !flag
    println("false")
else
    println("maybe")
end

sym = :symbol
tuple_ex = (1, "hello")
dict = Dict("key" => 42, "name" => "Ada")

end
