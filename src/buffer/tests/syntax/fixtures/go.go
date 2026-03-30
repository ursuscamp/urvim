// Go syntax fixture
package main

import "fmt"

func main() {
  raw := `hello
world`
  value := 'a'
  count := 1_000
  var ok bool = false
  if true {
    fmt.Println(value, count, ok)
  }
}
