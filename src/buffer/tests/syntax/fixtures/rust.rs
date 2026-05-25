// Rust syntax fixture
/* block comment */

fn main() {
    let value: Option<String> = Some("hello");
    let number = 42.5;
    let ch = 'x';
    let escaped_ch = '\n';
    let ok = Ok::<_, String>("done");
    let path = PathBuf::from("/tmp/example");
    let escaped = "line 1\nline 2";

    if value.is_some() && true {
        println!("{} {:?} {}", value, number, ch);
    } else {
        return;
    }

    let result = match ok {
        Ok(text) => Some(text),
        Err(err) => None,
    };

    println!("{:?}", result);
    let formatted = format!("Hello, {}!", value);
    let formatted_named = format!("{name:04}", name = value);
    let escaped = format!("{{literal}}");
}

fn completion_fixture() {
    let guard = String::from("ready");
    let result: Option<String> = Some(guard);
    println!("{:?}", result);
}

/// Doc comment
//! Inner doc comment
#[inline]
fn raw_examples<'a>(input: &'a str) -> &'a str {
    let raw = r#"raw "string""#;
    let raw_multiline = r#"first
second"#;
    let bytes = b"abc\n";
    let raw_bytes = br#"raw bytes"#;
    let byte = b'x';
    let hex = 0xff_u8;
    let bin = 0b1010_0011usize;
    let oct = 0o77;
    let float = 1.5e-2_f64;
    'label: loop { break 'label; }
    std::mem::drop(input);
    input
}

static GLOBAL_VARIABLES: usize = 0;
let GLOBAL_STATE = 1;
