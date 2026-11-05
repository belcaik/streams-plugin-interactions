import test from "node:test";
import assert from "node:assert/strict";
import {
  EVENT_TYPES, normalizeConfig, normalizeItem, safeAsset, queuePush, classesFor,
} from "../overlay/interactions-core.js";

const config = normalizeConfig();
const show = (extra = {}) => ({
  schema: 1, source: "command", rule: "hola", step: 0, text: "Hola Ana",
  image: null, sound: null, variant: "default", duration_ms: 4000, ...extra,
});

test("EVENT_TYPES lista los tres eventos visuales", () => {
  assert.deepEqual(EVENT_TYPES, ["interaction_show", "interaction_combo", "interaction_counter"]);
});

test("normalizeConfig aplica defaults", () => {
  assert.deepEqual(normalizeConfig(), { asset_url: "./assets/", max_queue: 30, default_duration_ms: 4000, gap_ms: 150 });
  assert.deepEqual(normalizeConfig(null), normalizeConfig());
});

test("normalizeConfig limita rangos y rechaza asset_url inseguro", () => {
  const limited = normalizeConfig({ max_queue: 500.7, default_duration_ms: 1, gap_ms: -5, asset_url: "https://x/" });
  assert.deepEqual(limited, { asset_url: "./assets/", max_queue: 100, default_duration_ms: 100, gap_ms: 0 });
  assert.equal(normalizeConfig({ asset_url: "./assets/../" }).asset_url, "./assets/");
  assert.equal(normalizeConfig({ asset_url: "./assets/pack/" }).asset_url, "./assets/pack/");
});

test("normalizeConfig no muta la entrada", () => {
  const raw = { max_queue: 0, gap_ms: 99999, extra: { keep: true } };
  const snapshot = structuredClone(raw);
  normalizeConfig(raw);
  assert.deepEqual(raw, snapshot);
});

test("normalizeItem acepta show con la forma común", () => {
  assert.deepEqual(normalizeItem("interaction_show", show({ image: "./assets/hype.svg" }), config), {
    type: "interaction_show", text: "Hola Ana", image: "./assets/hype.svg", sound: null, variant: "default", duration_ms: 4000,
  });
});

test("normalizeItem conserva datos de combo y contador", () => {
  const combo = normalizeItem("interaction_combo", show({ kind: "chat", key: "POGGERS", count: 12, threshold: 5, variant: "combo" }), config);
  assert.equal(combo.kind, "chat");
  assert.equal(combo.key, "POGGERS");
  assert.equal(combo.count, 12);
  assert.equal(combo.threshold, 5);
  const counter = normalizeItem("interaction_counter", show({ name: "muertes", value: 7, variant: "counter" }), config);
  assert.equal(counter.name, "muertes");
  assert.equal(counter.value, 7);
});

test("normalizeItem rechaza schema, tipo y campos rotos", () => {
  assert.equal(normalizeItem("interaction_emit", show(), config), null);
  assert.equal(normalizeItem("interaction_show", null, config), null);
  assert.equal(normalizeItem("interaction_show", show({ schema: 2 }), config), null);
  assert.equal(normalizeItem("interaction_show", show({ text: 5 }), config), null);
  assert.equal(normalizeItem("interaction_show", show({ text: undefined }), config), null);
  assert.equal(normalizeItem("interaction_combo", show({ kind: "otro", key: "x", count: 1, threshold: 1 }), config), null);
  assert.equal(normalizeItem("interaction_counter", show({ name: "n", value: "7" }), config), null);
});

test("normalizeItem rechaza variants maliciosos", () => {
  for (const variant of ["", "Default", "a b", "x\" onload=\"y", "-x", "a_b", "combo st-other"]) {
    assert.equal(normalizeItem("interaction_show", show({ variant }), config), null, variant);
  }
});

test("normalizeItem exige duración entera entre 100 y 300000", () => {
  for (const duration_ms of [99, 300001, 1.5, "4000", null]) {
    assert.equal(normalizeItem("interaction_show", show({ duration_ms }), config), null, String(duration_ms));
  }
  assert.equal(normalizeItem("interaction_show", show({ duration_ms: 100 }), config).duration_ms, 100);
});

test("safeAsset solo admite rutas bajo ./assets/", () => {
  assert.equal(safeAsset("./assets/hype.svg"), "./assets/hype.svg");
  assert.equal(safeAsset("./assets/sub/a-b_c.ogg"), "./assets/sub/a-b_c.ogg");
  for (const value of [
    "../assets/x.svg", "./assets/../secret", "./assets/./x", "/assets/x.svg", "./assets/",
    "data:image/svg+xml,<svg/>", "http://evil/x.svg", "https://evil/x.svg", "javascript:alert(1)",
    "./assets/a//b", "./assets/a\\b", "./assets/x.svg?y", null, 5,
  ]) {
    assert.equal(safeAsset(value), null, String(value));
  }
  assert.equal(safeAsset("./assets/x.svg", "https://evil/"), null);
});

test("normalizeItem descarta imágenes y sonidos inseguros", () => {
  assert.equal(normalizeItem("interaction_show", show({ image: "http://evil/x.png" }), config), null);
  assert.equal(normalizeItem("interaction_show", show({ sound: "../x.ogg" }), config), null);
  assert.equal(normalizeItem("interaction_show", show({ sound: "./assets/x.ogg" }), config).sound, "./assets/x.ogg");
});

test("queuePush mantiene FIFO y respeta el máximo", () => {
  const queue = [];
  assert.equal(queuePush(queue, "a", 2), true);
  assert.equal(queuePush(queue, "b", 2), true);
  assert.equal(queuePush(queue, "c", 2), false);
  assert.deepEqual(queue, ["a", "b"]);
  assert.equal(queue.shift(), "a");
});

test("classesFor combina base, variant y fase", () => {
  const item = { variant: "fullscreen" };
  assert.deepEqual(classesFor(item, "visible"), [
    "st-interactions__effect", "st-interactions__effect--fullscreen", "st-interactions__effect--visible",
  ]);
  assert.deepEqual(classesFor({ variant: "x y" }, "otra"), [
    "st-interactions__effect", "st-interactions__effect--default", "st-interactions__effect--entering",
  ]);
  for (const phase of ["entering", "visible", "leaving"]) {
    assert.equal(classesFor(item, phase)[2], `st-interactions__effect--${phase}`);
  }
});
