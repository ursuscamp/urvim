// F# syntax fixture
// Multi-line comment
(* Multi-line
   block comment *)
/// XML doc comment

module Greeter

open System

type Person = { Name: string; Age: int }

type Color =
  | Red
  | Green
  | Blue

type Tree =
  | Leaf
  | Node of int * Tree * Tree

let value = "hello"
let multiline = """hello
world"""
let answer = 42
let letter = 'x'
let flag = true
let floating = 3.14
let hex_val = 0xFF
let octal_val = 0o77
let binary_val = 0b1010_0011

let greet name = sprintf "Hello, %s" name

let factorial n =
  let rec loop acc n =
    if n <= 1 then
      acc
    else
      loop (acc * n) (n - 1)
  loop 1 n

let result = factorial 10

let person = { Name = "Ada"; Age = 42 }
let name = person.Name

let maybeValue = Some 42
let noValue = None

let listExample = [1; 2; 3]
let mappedList = listExample |> List.map (fun x -> x * 2)

let pair = (1, "hello")

let demo () =
  printfn "hello world"
  let x = 42
  printfn "%d" x

let asyncDemo =
  async {
    let! data = async { return "data" }
    return data.Length
  }

type IGreeter =
  abstract Greet: string -> string

type EnglishGreeter() =
  interface IGreeter with
    member this.Greet name = sprintf "Hello, %s" name

[<EntryPoint>]
let main argv =
  demo ()
  0
