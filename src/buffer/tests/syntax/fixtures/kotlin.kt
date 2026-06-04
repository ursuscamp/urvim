// Kotlin syntax fixture
// Multi-line comment
/* Block comment */
/** Doc comment */

@JvmInline
value class Name(val value: String)

data class Person(
    val name: String,
    val age: Int
)

enum class Color { RED, GREEN, BLUE }

sealed class Result {
    data class Success(val data: String) : Result()
    data class Error(val message: String) : Result()
}

val message = """hello
world"""
val count = 42
val flag = true
val letter = 'x'
val floating = 3.14
val hex_val = 0xFF
val binary_val = 0b1010_0011
val exactNumber = 42
val float_exp = 1.5e-2f
val long_val = 42L
val null_val = null

fun demo() {
    render(message)
}

fun greet(name: String): String {
    return "Hello, $name"
}

fun greetWithDefault(name: String = "world"): String {
    return "Hello, $name"
}

fun factorial(n: Int): Int {
    tailrec fun loop(n: Int, acc: Int): Int {
        return if (n <= 1) acc else loop(n - 1, acc * n)
    }
    return loop(n, 1)
}

val items = listOf(1, 2, 3)
val mapping = mapOf("one" to 1, "two" to 2)
val pair = Pair("key", 42)

fun processList() {
    items
        .map { it * 2 }
        .filter { it > 2 }
        .forEach { println(it) }
}

fun controlFlow() {
    if (count > 0) {
        println("positive")
    } else {
        println("non-positive")
    }

    when (count) {
        42 -> println("answer")
        in 1..10 -> println("small")
        else -> println("other")
    }

    for (i in 1..10) {
        println(i)
    }

    var x = 0
    while (x < 10) {
        x++
    }
}

fun nullable() {
    val maybe: String? = null
    val length = maybe?.length ?: 0
    println(length)
}

suspend fun fetchData(): String {
    return "data"
}
