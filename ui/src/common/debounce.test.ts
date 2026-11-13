import { test } from "node:test";
import assert from "node:assert/strict";
import { debounce } from "./debounce.ts";

test("runs once with the last arguments after the delay", (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const calls: string[] = [];
  const debounced = debounce((value: string) => calls.push(value), 300);

  debounced("a");
  debounced("ab");
  t.mock.timers.tick(299);
  assert.deepEqual(calls, []);
  t.mock.timers.tick(1);
  assert.deepEqual(calls, ["ab"]);
});

test("restarts the delay on every call", (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const calls: string[] = [];
  const debounced = debounce((value: string) => calls.push(value), 300);

  debounced("a");
  t.mock.timers.tick(200);
  debounced("ab");
  t.mock.timers.tick(200);
  assert.deepEqual(calls, []);
  t.mock.timers.tick(100);
  assert.deepEqual(calls, ["ab"]);
});

test("cancel drops the pending call", (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const calls: string[] = [];
  const debounced = debounce((value: string) => calls.push(value), 300);

  debounced("a");
  debounced.cancel();
  t.mock.timers.tick(300);
  assert.deepEqual(calls, []);
});
