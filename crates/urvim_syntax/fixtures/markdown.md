# Markdown syntax fixture

This paragraph has *emphasis*, **strong text**, `inline code`, a [link](https://example.com), and plain prose with Capitalized SCREAMY_CASE words.

## Second Heading

> quoted text

- list item
- another item

---

```rust
fn main() {
    let value = Some("hello");
    println!("{}", value.unwrap_or("world"));
}
```

```javascript
const value = 1;
```

```wat
const value = 1;
```

Plain text after the fence.

Setext heading
=============

Reference link [ref][id] and image reference ![alt][img].
[id]: https://example.com
[img]: https://example.com/image.png

Autolink: <https://example.com>

_underscore emphasis_ and __underscore strong__.

    indented code block

~~~python
print("hi")
~~~
