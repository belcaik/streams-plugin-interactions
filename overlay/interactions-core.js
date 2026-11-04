// Lógica pura del overlay: sin DOM, timers ni red.
export const EVENT_TYPES = ["interaction_show", "interaction_combo", "interaction_counter"];

const PHASES = ["entering", "visible", "leaving"];
const DEFAULT_BASE = "./assets/";
const VARIANT = /^[a-z0-9][a-z0-9-]*$/;
const SEGMENT = /^[A-Za-z0-9._-]+$/;
const isObject = (value) => value !== null && typeof value === "object" && !Array.isArray(value);
const textOrNull = (value) => value === null || typeof value === "string";
const integer = (value, min, max) => Number.isInteger(value) && value >= min && value <= max;

function clamp(value, min, max, fallback) {
  return Number.isFinite(value) ? Math.floor(Math.min(max, Math.max(min, value))) : fallback;
}

export function normalizeConfig(raw = {}) {
  const config = isObject(raw) ? raw : {};
  const url = config.asset_url;
  return {
    asset_url: typeof url === "string" && url.startsWith(DEFAULT_BASE) && url.endsWith("/") && safePrefix(url)
      ? url
      : DEFAULT_BASE,
    max_queue: clamp(config.max_queue, 1, 100, 30),
    default_duration_ms: clamp(config.default_duration_ms, 100, 300000, 4000),
    gap_ms: clamp(config.gap_ms, 0, 60000, 150),
  };
}

function safePrefix(url) {
  const rest = url.slice(DEFAULT_BASE.length, -1);
  return rest === "" || rest.split("/").every(safeSegment);
}

function safeSegment(segment) {
  return SEGMENT.test(segment) && segment !== "." && segment !== "..";
}

export function safeAsset(value, base = DEFAULT_BASE) {
  if (typeof value !== "string" || typeof base !== "string" || !base.startsWith(DEFAULT_BASE)) return null;
  if (!value.startsWith(base)) return null;
  const rest = value.slice(base.length);
  return rest !== "" && rest.split("/").every(safeSegment) ? value : null;
}

function asset(value, base) {
  if (value === null) return { ok: true, value: null };
  const safe = safeAsset(value, base);
  return { ok: safe !== null, value: safe };
}

export function normalizeItem(type, data, config) {
  if (!EVENT_TYPES.includes(type) || !isObject(data) || data.schema !== 1) return null;
  const settings = normalizeConfig(config);
  if (!textOrNull(data.text) || typeof data.variant !== "string" || !VARIANT.test(data.variant)) return null;
  if (!integer(data.duration_ms, 100, 300000)) return null;
  const image = asset(data.image, settings.asset_url);
  const sound = asset(data.sound, settings.asset_url);
  if (!image.ok || !sound.ok) return null;
  const item = {
    type,
    text: data.text,
    image: image.value,
    sound: sound.value,
    variant: data.variant,
    duration_ms: data.duration_ms,
  };
  if (type === "interaction_combo") {
    if (!["moderators", "chat"].includes(data.kind) || typeof data.key !== "string") return null;
    if (!integer(data.count, 0, Number.MAX_SAFE_INTEGER) || !integer(data.threshold, 0, Number.MAX_SAFE_INTEGER)) return null;
    return { ...item, kind: data.kind, key: data.key, count: data.count, threshold: data.threshold };
  }
  if (type === "interaction_counter") {
    if (typeof data.name !== "string" || !Number.isFinite(data.value)) return null;
    return { ...item, name: data.name, value: data.value };
  }
  return item;
}

export function queuePush(queue, item, maximum) {
  if (!Array.isArray(queue) || queue.length >= maximum) return false;
  queue.push(item);
  return true;
}

export function classesFor(item, phase) {
  const variant = typeof item?.variant === "string" && VARIANT.test(item.variant) ? item.variant : "default";
  const state = PHASES.includes(phase) ? phase : "entering";
  return ["st-interactions__effect", `st-interactions__effect--${variant}`, `st-interactions__effect--${state}`];
}
