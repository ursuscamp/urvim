(* OCaml syntax fixture *)
(* Multi-line comment
   continues here
*)

let value = "hello"
let answer = 42
let flag = true
let letter = 'x'
let floating = 3.14
let neg = -42

let add a b = a + b
let result = add 3 4

let rec factorial n =
  if n <= 1 then
    1
  else
    n * factorial (n - 1)

type color = Red | Green | Blue
type point = {x: float; y: float}
type tree = Leaf | Node of int * tree * tree

let p = {x = 1.0; y = 2.0}
let x_val = p.x

let maybe = Some 42
let none = None

let demo x =
  match x with
  | Some v -> Printf.printf "%d\n" v
  | None -> ()

let list_expr = [1; 2; 3]
let hd = List.hd list_expr
let tl = List.tl list_expr

let pair = (1, "hello")

module StringMap = Map.Make(String)

let empty = StringMap.empty
let with_key = StringMap.add "key" 42 empty

let () =
  Printf.printf "hello %s\n" value;
  Printf.printf "answer: %d\n" answer
