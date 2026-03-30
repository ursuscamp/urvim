// Java syntax fixture
/** doc comment */
package demo;

import java.util.List;

public class Demo<T> {
  @Override
  public String toString() {
    return """
      hello
      world
      """;
  }

  char ch = '\n';
  int count = 1_000;
  boolean enabled = true;
}
