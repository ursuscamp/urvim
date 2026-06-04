// Go syntax fixture
// Multi-line comment

package main

import (
	"fmt"
	"os"
	"strconv"
	"strings"
)

const (
	maxSize = 1024
	appName = "urvim"
)

var globalCount = 42

type Person struct {
	Name string
	Age  int
}

type Color int

const (
	Red Color = iota
	Green
	Blue
)

func greet(name string) string {
	return fmt.Sprintf("Hello, %s", name)
}

func factorial(n int) int {
	if n <= 1 {
		return 1
	}
	return n * factorial(n-1)
}

func main() {
	raw := `hello
world`
	value := 'a'
	count := 1_000
	hex_val := 0xFF
	octal_val := 0o77
	binary_val := 0b1010_0011
	float_val := 3.14
	float_exp := 1.5e-2

	var ok bool = false
	var name string = "Ada"
	var pi float64 = 3.14159

	if true {
		fmt.Println(value, count, ok)
	} else if false {
		fmt.Println("unreachable")
	} else {
		fmt.Println("also unreachable")
	}

	for i := 0; i < 10; i++ {
		fmt.Println(i)
	}

	items := []int{1, 2, 3}
	for index, item := range items {
		fmt.Printf("items[%d] = %d\n", index, item)
	}

	mapping := map[string]int{
		"one": 1,
		"two": 2,
	}

	switch count {
	case 42:
		fmt.Println("answer")
	default:
		fmt.Println("other")
	}

	p := Person{Name: "Ada", Age: 42}
	fmt.Println(p.Name)
	ptr := &p
	ptr.Age = 43

	result := greet(name)
	fmt.Println(result)

	s := "hello"
	slice := s[1:3]
	fmt.Println(slice)

	defer fmt.Println("done")
	go func() {
		fmt.Println("goroutine")
	}()

}
