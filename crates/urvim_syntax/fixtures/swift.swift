// Swift syntax fixture
// Multi-line comment
/* Multi-line
   block comment */
/// Doc comment

@available(*, deprecated, message: "use newGreeter instead")
struct Greeter {
    let message = """hello
world"""
    let count = 42
    let flag = true
    let letter: Character = "x"
    let floating = 3.14
    let hex_val = 0xFF
    let binary_val = 0b1010_0011
    let octal_val = 0o77
    let float_exp = 1.5e-2

    func demo() {
        render(message)
    }

    func greet(name: String) -> String {
        return "Hello, \(name)"
    }
}

enum Color: Int {
    case red = 0
    case green = 1
    case blue = 2
}

protocol Drawable {
    func draw()
}

class Circle: Drawable {
    var radius: Double

    init(radius: Double) {
        self.radius = radius
    }

    func draw() {
        print("drawing circle")
    }

    var area: Double {
        return Double.pi * radius * radius
    }
}

func factorial(_ n: Int) -> Int {
    if n <= 1 {
        return 1
    }
    return n * factorial(n - 1)
}

func greet(_ name: String) -> String {
    "Hello, \(name)"
}

let items = [1, 2, 3]
let mapping = ["one": 1, "two": 2]
let optional: Int? = 42
let noValue: Int? = nil

for item in items {
    print(item)
}

for i in 0..<10 {
    print(i)
}

var x = 0
while x < 10 {
    x += 1
}

if let value = optional {
    print("got \(value)")
} else {
    print("no value")
}

switch count {
case 42:
    print("answer")
case let n where n > 100:
    print("large")
default:
    print("other")
}

do {
    try someFunction()
} catch let error {
    print(error)
}

extension Greeter {
    func farewell() -> String {
        return "goodbye"
    }
}

struct AsyncExample {
    func fetch() async -> String {
        return "data"
    }

    func process() async {
        let result = await fetch()
        print(result)
    }
}
