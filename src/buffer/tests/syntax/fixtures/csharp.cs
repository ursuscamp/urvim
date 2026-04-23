/// C# syntax fixture
[Serializable]
public class Greeter {
    public string Name { get; set; } = "Ada";
    public int Count = 42;
    public string Escaped = @"line 1";
    public char Letter = 'x';
    public void Demo() { Render(Name); }
}
