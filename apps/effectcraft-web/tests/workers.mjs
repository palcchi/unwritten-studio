// Job-worker plumbing of the web app (js/host.js) under Node with a fake Worker,
// no browser or Wasm build needed:
//
//   node apps/effectcraft-web/tests/workers.mjs
//
// - A reused worker is sent a file again when it was replaced, even by one of the same size
//   (#218), and not when it is unchanged.
// Exits non-zero on failure.
import assert from "node:assert/strict";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
globalThis.self = globalThis;

// A Worker that starts on "init" and finishes each job at once.
let failStart = false;
const workers = [];
class FakeWorker {
  constructor() {
    this.posted = [];
    this.terminated = false;
    workers.push(this);
  }
  postMessage(m) {
    this.posted.push(m);
    setTimeout(() => {
      if (m.type === "init") {
        if (failStart) this.onerror?.({ message: "worker failed to start" });
        else this.onmessage?.({ data: { type: "ready" } });
      } else if (m.type === "job") {
        this.onmessage?.({ data: { type: "reply", json: "{}", done: true } });
      }
    });
  }
  terminate() {
    this.terminated = true;
  }
}
globalThis.Worker = FakeWorker;

const host = await import(pathToFileURL(join(here, "../js/host.js")));
const run = (id, files) =>
  new Promise((resolve) => host.workerRun(id, "{}", files, "http://localhost/", (kind, json) => kind !== "file" && resolve({ kind, json })));
const sent = (w) => w.posted.filter((m) => m.type === "file").map((m) => m.bytes[0]);

// #218: equal-length replacement.
const red = new Uint8Array(512).fill(1);
const blue = new Uint8Array(512).fill(2);
assert.equal((await run(1, [["/collision.png", red, 1]])).kind, "reply");
assert.equal((await run(2, [["/collision.png", blue, 2]])).kind, "reply");
assert.equal((await run(3, [["/collision.png", blue, 2]])).kind, "reply");
assert.equal(workers.length, 1, "the worker is reused");
assert.deepEqual(sent(workers[0]), [1, 2], "the replacement is sent once, the unchanged file not again");

console.log("workers: ok");
