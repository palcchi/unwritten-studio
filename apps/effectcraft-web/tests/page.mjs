// Page-side helpers of the web app (js/host.js) under Node with a fake DOM, no browser or Wasm
// build needed:
//
//   node apps/effectcraft-web/tests/page.mjs
//
// - Add Files… / Upload Folder…: a picked file that can't be read is listed with its error
//   instead of failing the whole pick silently, and cancelling picks nothing (#226).
// Exits non-zero on failure.
import assert from "node:assert/strict";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
globalThis.self = globalThis;

class FakeElement {
  constructor(tag) {
    this.tag = tag;
    this.children = [];
    this.attributes = {};
    this.textContent = "";
  }
  setAttribute(k, v) {
    this.attributes[k] = v;
  }
  append(...c) {
    this.children.push(...c);
  }
  *all() {
    yield this;
    for (const c of this.children) yield* c.all();
  }
}
let picker;
globalThis.document = {
  body: new FakeElement("body"),
  createElement(tag) {
    const el = new FakeElement(tag);
    if (tag === "input") {
      picker = el;
      el.click = () => {};
    }
    return el;
  },
  getElementById(id) {
    return [...this.body.all()].find((e) => e.id === id) ?? null;
  },
};

const host = await import(pathToFileURL(join(here, "../js/host.js")));

// #226: one readable and one unreadable file.
const file = (name, read) => ({ name, webkitRelativePath: "", arrayBuffer: read });
const picked = host.browsePickFiles(false);
picker.files = [
  file("ok.png", async () => new Uint8Array([1, 2, 3]).buffer),
  file("red-corner.png", async () => {
    throw new DOMException("Generated unreadable selected file", "NotReadableError");
  }),
];
await picker.onchange();
const out = await picked;
assert.equal(out.length, 2);
assert.deepEqual([out[0].path, [...out[0].bytes]], ["ok.png", [1, 2, 3]]);
assert.deepEqual(out[1], { path: "red-corner.png", error: "Generated unreadable selected file" });
const cancelled = host.browsePickFiles(false);
picker.oncancel();
assert.deepEqual(await cancelled, [], "cancelling picks nothing");

console.log("page: ok");
