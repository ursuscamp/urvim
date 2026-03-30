// TypeScript syntax fixture
/* block comment */

export interface Person {
  name: string;
  age: number;
}

@sealed
export class Child extends Error {
  #secret = 1n;
}

export function makeThing(value: number): Promise<string> {
  const count = 1_000;
  const message = `hello ${value}`;
  const escaped = "line 1\nline 2";
  const maybe = value ?? "fallback";
  return Promise.resolve(message);
}

const tuple: [string, number] = ["Ada", 42];
const view = <Button kind="primary" disabled />;
const jsx = <Card title={message} count={tuple[1]} />;
type Mode = "a" | "b";
enum Flags { A = 1, B = 2 }
