import { EVENT_TYPES, normalizeConfig, normalizeItem, safeAsset, queuePush, classesFor } from "./interactions-core.js";

const root = document.querySelector(".st-interactions");
const PHASE_CLASSES = ["entering", "visible", "leaving"].map((phase) => `st-interactions__effect--${phase}`);
// El CSS personalizado puede cambiar las transiciones; el respaldo es fijo y solo de UI.
const TRANSITION_FALLBACK_MS = 1000;
const queue = [];
let config = normalizeConfig();
let draining = false;

function enqueue(type, data) {
  const item = normalizeItem(type, data, config);
  if (!item) {
    console.warn(`[interactions] se descarta ${type} inválido`);
    return;
  }
  if (!queuePush(queue, item, config.max_queue)) {
    console.warn(`[interactions] cola llena; se descarta ${type}`);
    return;
  }
  if (!draining) void drain();
}

async function drain() {
  draining = true;
  try {
    while (queue.length) {
      const item = queue.shift();
      try {
        await show(item);
      } catch (error) {
        console.warn("[interactions] no se pudo mostrar una interacción", error);
      }
      await sleep(config.gap_ms);
    }
  } finally {
    draining = false;
  }
}

async function show(item) {
  const element = build(item);
  root.replaceChildren(element);
  playSound(item.sound);
  await nextFrame();
  setPhase(element, item, "visible");
  await waitForTransition(element);
  await sleep(item.duration_ms);
  setPhase(element, item, "leaving");
  await waitForTransition(element);
  element.remove();
}

function build(item) {
  const element = document.createElement("article");
  element.classList.add(...classesFor(item, "entering"));
  element.dataset.type = item.type;
  const image = safeAsset(item.image, config.asset_url);
  if (image) {
    const picture = document.createElement("img");
    picture.className = "st-interactions__image";
    picture.alt = "";
    picture.src = image;
    element.append(picture);
  }
  if (item.text !== null) {
    const text = document.createElement("p");
    text.className = "st-interactions__text";
    text.textContent = item.text;
    element.append(text);
  }
  const meta = metaText(item);
  if (meta !== null) {
    const detail = document.createElement("p");
    detail.className = "st-interactions__meta";
    detail.textContent = meta;
    element.append(detail);
  }
  if (!image && item.text === null && meta === null) element.setAttribute("aria-label", "Interacción con sonido");
  return element;
}

function metaText(item) {
  if (item.type === "interaction_combo") return `${item.key} x${item.count}`;
  if (item.type === "interaction_counter") return `${item.name}: ${item.value}`;
  return null;
}

function setPhase(element, item, phase) {
  element.classList.remove(...PHASE_CLASSES);
  element.classList.add(classesFor(item, phase)[2]);
}

function playSound(url) {
  const sound = safeAsset(url, config.asset_url);
  if (!sound) return;
  const audio = new Audio(sound);
  audio.play().catch((error) => console.warn("[interactions] no se pudo reproducir el sonido", error));
}

function waitForTransition(element) {
  const durations = getComputedStyle(element).transitionDuration.split(",").map((value) => Number.parseFloat(value));
  if (!durations.some((value) => value > 0)) return Promise.resolve();
  return new Promise((resolve) => {
    const finish = () => {
      clearTimeout(timer);
      element.removeEventListener("transitionend", onEnd);
      resolve();
    };
    const onEnd = (event) => {
      if (event.target === element) finish();
    };
    const timer = setTimeout(finish, TRANSITION_FALLBACK_MS);
    element.addEventListener("transitionend", onEnd);
  });
}

// Dos frames garantizan que la fase entering se pinte antes de la transición.
function nextFrame() {
  return new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function loadConfig() {
  try {
    const response = await fetch("./config.json");
    return response.ok ? await response.json() : {};
  } catch (_) {
    return {};
  }
}

async function initialize() {
  config = normalizeConfig(await loadConfig());
  for (const type of EVENT_TYPES) window.streams.on(type, (data) => enqueue(type, data));
}

void initialize();
