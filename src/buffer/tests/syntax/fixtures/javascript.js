// JavaScript syntax fixture
/* block comment */

async function loadThing(url) {
  const result = await fetch(url);
  const data = { ok: true, count: 3, name: "Ada" };
  const message = `hello ${url}`;
  const escaped = "line 1\nline 2";
  const multiline = `first line
${url}
third line`;
  class Thing extends Error {}
  return result ? data : null;
}

const numbers = [1, 2, 3];
const text = "plain string";
const extra = 'single quoted';

const regex = /ab+c/i;
const big = 1_000_000n;
const hex = 0xffn;
const bin = 0b1010_0011;
const oct = 0o755;

class Secret {
  #value = 1;
  getValue() {
    return this.#value;
  }
}
