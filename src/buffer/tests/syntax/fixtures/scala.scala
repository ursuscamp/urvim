// Scala syntax fixture
// Multi-line comment
/* Multi-line
   block comment */
/** Doc comment */

@main
def run(): Unit = {
  val value = "hello"
  val answer = 42
  val flag = true
  val letter = 'x'
  val floating = 3.14
  val hex_val = 0xFF
  val binary_val = 0b1010_0011
  val float_exp = 1.5e-2

  def demo(): Unit = println(value)

  println(s"answer: $answer")
  println(raw"raw string\nno escape")
}

class Greeter(val name: String, val count: Int = 42) {
  def greet(target: String): String = {
    s"Hello, $target"
  }

  def factorial(n: Int): Int = {
    if (n <= 1) 1
    else n * factorial(n - 1)
  }
}

object Greeter {
  def apply(name: String): Greeter = new Greeter(name)
}

enum Color:
  case Red, Green, Blue

trait Drawable:
  def draw(): Unit

case class Person(name: String, age: Int)

sealed trait Option[+T]
case class Some[+T](value: T) extends Option[T]
case object None extends Option[Nothing]

type Name = String

extension (s: String)
  def exclaim: String = s + "!"

@annotation.tailrec
def gcd(a: Int, b: Int): Int =
  if b == 0 then a else gcd(b, a % b)

def processList(): Unit = {
  val items = List(1, 2, 3)
  val result = items
    .map(_ * 2)
    .filter(_ > 2)
    .reduce(_ + _)
  println(result)
}

def controlFlow(): Unit = {
  val x = 42
  if x > 0 then println("positive")
  else println("non-positive")

  x match
    case 42 => println("answer")
    case n if n > 0 => println("positive")
    case _ => println("other")

  for i <- 1 to 10 do
    println(i)
}

def forComprehension(): Unit = {
  val result = for
    x <- List(1, 2, 3)
    y <- List(4, 5, 6)
  yield (x, y)
  println(result)
}
