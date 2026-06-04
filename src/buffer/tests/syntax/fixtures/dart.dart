// Dart syntax fixture
// Multi-line comment
/* Block comment */
/// Doc comment

@immutable
class Greeter {
  final String name = "Ada";
  final int count = 42;
  final bool flag = true;
  final double pi = 3.14;
  final hex = 0xFF;
  final raw = r"raw string\nno escape";
  final rawMulti = r"""raw
multiline""";
  final multi = """hello
world""";
  final ok = true;
  final letter = 'x';

  void demo() {
    render(name);
  }

  String greet(String name) => "Hello, $name";

  int factorial(int n) {
    if (n <= 1) {
      return 1;
    }
    return n * factorial(n - 1);
  }
}

enum Color { red, green, blue }

mixin Printable {
  void print() {
    // mixin method
  }
}

abstract class Shape {
  double get area;
}

class Circle implements Shape {
  final double radius;
  Circle(this.radius);

  @override
  double get area => 3.14159 * radius * radius;
}

typedef IntList = List<int>;

extension StringExtensions on String {
  int get charCount => length;
}

Future<void> fetchData() async {
  final response = await Future.value("data");
  print(response);
}

Stream<int> countStream() async* {
  for (var i = 0; i < 10; i++) {
    yield i;
  }
}

void main() {
  var items = [1, 2, 3];
  var map = {"one": 1, "two": 2};
  var set = {1, 2, 3};

  for (var item in items) {
    print(item);
  }

  items.forEach((item) {
    print(item);
  });

  final result = items
    .map((x) => x * 2)
    .where((x) => x > 2)
    .toList();

  print(result);

  try {
    throw Exception("error");
  } on Exception catch (e) {
    print(e);
  } finally {
    print("done");
  }

  switch (count) {
    case 42:
      print("answer");
    default:
      print("other");
  }
}
