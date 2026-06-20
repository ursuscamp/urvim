/// C# syntax fixture
// Multi-line comment
/* Multi-line
   block comment */

[Serializable]
public class Greeter {
    public string Name { get; set; } = "Ada";
    public int Count = 42;
    public double Pi = 3.14;
    public bool Flag = true;
    public string Escaped = @"line 1
line 2";
    public string Interpolated = $"Hello, {Name}";
    public string Raw = """raw string""";
    public char Letter = 'x';
    public const int MaxSize = 1024;
    public readonly string AppName = "urvim";

    public string Greet(string name) {
        return $"Hello, {name}";
    }

    public void Demo() {
        Render(Name);
    }
}

public enum Color { Red, Green, Blue }

public struct Point {
    public double X { get; set; }
    public double Y { get; set; }

    public Point(double x, double y) {
        X = x;
        Y = y;
    }
}

public interface IDrawable {
    void Draw();
}

public class Circle : IDrawable {
    public double Radius { get; set; }

    public Circle(double radius) {
        Radius = radius;
    }

    public void Draw() {
        Console.WriteLine("drawing circle");
    }
}

public static class MathHelper {
    public static int Factorial(int n) {
        if (n <= 1) return 1;
        return n * Factorial(n - 1);
    }

    public static void ProcessList() {
        var items = new List<int> { 1, 2, 3 };
        var result = items
            .Where(x => x > 1)
            .Select(x => x * 2)
            .ToList();
        Console.WriteLine(result);
    }
}

public record Person(string Name, int Age);

public class AsyncDemo {
    public async Task<string> FetchDataAsync() {
        await Task.Delay(100);
        return "data";
    }
}
