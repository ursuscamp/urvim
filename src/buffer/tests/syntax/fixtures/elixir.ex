# Elixir syntax fixture
# Multi-line comment

defmodule Urvim.Greeter do
  @moduledoc """
  A module for greeting people.
  """

  @name :urvim
  @default_count 42

  def hello(name) do
    value = "hello"
    multiline = """hello
world"""
    count = 42
    flag = true
    letter = ?x
    floating = 3.14
    hex_val = 0xFF
    octal_val = 0o77
    binary_val = 0b1010_0011
    atom = :ok
    :ok
  end

  def greet(name \\ "world") do
    "Hello, #{name}"
  end

  def factorial(0), do: 1
  def factorial(n) when n > 0 do
    n * factorial(n - 1)
  end

  def list_example do
    [1, 2, 3]
  end

  def tuple_example do
    {:ok, "hello"}
  end

  def map_example do
    %{name: "Ada", age: 42}
  end

  def pattern_match do
    {:ok, value} = {:ok, 42}
    [head | tail] = [1, 2, 3]
    %{name: name} = %{name: "Ada", age: 42}
    name
  end

  def conditional do
    cond do
      1 + 1 == 2 -> "math works"
      true -> "fallback"
    end
  end

  def pipeline do
    [1, 2, 3]
    |> Enum.map(&(&1 * 2))
    |> Enum.filter(&(&1 > 2))
    |> Enum.sum()
  end

  def sigils do
    ~r{hello}i
    ~w(foo bar baz)
    ~c(hello)
  end
end
