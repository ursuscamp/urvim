# Ruby syntax fixture
# Multi-line comment
=begin
Multi-line comment
in Ruby
=end

class Greeter
  attr_reader :name, :count

  def initialize(name = "world")
    @name = name
    @count = 42
    @flag = true
  end

  def hello(name)
    @name = "Ada"
    count = 42
    flag = true
    float = 3.14
    hex = 0xFF
    octal = 0o77
    binary = 0b1010_0011
    float_exp = 1.5e-2
    letter = ?x
    symbol = :ok
    nil_val = nil
    :ok
  end

  def greet(target)
    "Hello, #{target}"
  end

  def factorial(n)
    return 1 if n <= 1
    n * factorial(n - 1)
  end

  def process
    items = [1, 2, 3]
    mapping = {"name" => "Ada", "age" => 42}
    symbols = {name: "Ada", age: 42}

    if @count == 42
      puts "answer"
    elsif @count > 0
      puts "positive"
    else
      puts "other"
    end

    items.each do |item|
      puts item
    end

    for i in 1..10
      puts i
    end

    while @count > 0
      @count -= 1
    end

    case @count
    when 42
      puts "answer"
    when (0..100)
      puts "in range"
    else
      puts "other"
    end

    result = items
      .map { |x| x * 2 }
      .select { |x| x > 2 }
      .reduce(0) { |sum, x| sum + x }
    puts result

    begin
      raise "error occurred"
    rescue StandardError => e
      puts e.message
    ensure
      puts "cleanup"
    end
  end
end

module MyModule
  def module_method
    puts "module method"
  end
end

class SubGreeter < Greeter
  include MyModule

  def hello(name)
    super
    puts "subclass hello"
  end
end

message = <<~EOF
  ruby heredoc body
  indented
EOF

regex = /hello/i
regex_match = "hello" =~ /hello/
subst = "hello".sub(/l/, 'L')
global_sub = "hello".gsub(/l/, 'L')

lambda_example = ->(x) { x * 2 }
proc_example = Proc.new { |x| x * 2 }