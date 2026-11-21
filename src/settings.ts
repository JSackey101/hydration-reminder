// The settings window. Rust owns and validates the settings; this page loads
// them, sends each valid change straight away (no Save button) and shows any
// error Rust returns.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type Settings = {
  intervalMin: number;
  popupTimeoutMin: number | null;
  drankShortcut: string;
  snoozeShortcut: string;
  drankLabel: string;
  snoozeLabel: string;
  reminderShowing: boolean;
};

const interval = document.getElementById("interval") as HTMLInputElement;
const timeoutOn = document.getElementById("timeout-on") as HTMLInputElement;
const timeout = document.getElementById("timeout") as HTMLInputElement;
const login = document.getElementById("login") as HTMLInputElement;
const status = document.getElementById("status") as HTMLParagraphElement;

const DEFAULT_TIMEOUT_MIN = 2;

function show(message: string, error: boolean) {
  status.textContent = message;
  status.classList.toggle("error", error);
}

// A whole number from an input, or null if it isn't one.
function whole(input: HTMLInputElement): number | null {
  const n = Number(input.value);
  return input.value.trim() !== "" && Number.isInteger(n) ? n : null;
}

async function save() {
  const intervalMin = whole(interval);
  if (intervalMin === null) {
    show("Interval must be a whole number of minutes.", true);
    return;
  }
  let popupTimeoutMin: number | null = null;
  if (timeoutOn.checked) {
    popupTimeoutMin = whole(timeout);
    if (popupTimeoutMin === null) {
      show("Timeout must be a whole number of minutes.", true);
      return;
    }
  }
  try {
    await invoke("save_settings", { intervalMin, popupTimeoutMin });
    show("Saved.", false);
  } catch (e) {
    show(String(e), true);
  }
}

// --- Shortcuts (CAP-15) ---
// Rust validates, test-registers and stores the combos; this page records a
// key press and sends both combos (the new one plus the other, unchanged).

type Action = "drank" | "snooze";
const ACTIONS: Action[] = ["drank", "snooze"];
const keyEl = (a: Action) => document.getElementById(`${a}-key`) as HTMLElement;
const recordBtn = (a: Action) =>
  document.getElementById(`${a}-record`) as HTMLButtonElement;

const stored: Record<Action, string> = { drank: "", snooze: "" };
const labels: Record<Action, string> = { drank: "", snooze: "" };
let shortcutsLoaded = false;
let reminderShowing = false;
let recording: Action | null = null;
let saving = false;
const isWindows = navigator.userAgent.includes("Windows");

// The shortcuts have their own status line, so the other settings' messages
// never hide why Record is disabled.
const shortcutStatus = document.getElementById("shortcut-status") as HTMLParagraphElement;
function showShortcut(message: string, error: boolean) {
  shortcutStatus.textContent = message;
  shortcutStatus.classList.toggle("error", error);
}

const BUSY = "Answer the reminder first.";
const PROMPT = "Press the new shortcut, or Esc to cancel.";

function stopRecording() {
  recording = null;
  if (shortcutStatus.textContent === PROMPT) showShortcut("", false);
}

function renderShortcuts() {
  for (const a of ACTIONS) {
    const el = keyEl(a);
    const name = a === "drank" ? "Drank" : "Snooze";
    el.classList.toggle("recording", recording === a);
    el.textContent = recording === a ? "Press keys…" : labels[a];
    recordBtn(a).textContent = recording === a ? "Cancel" : "Record";
    recordBtn(a).setAttribute(
      "aria-label",
      recording === a ? `Cancel recording the ${name} shortcut` : `Record the ${name} shortcut`,
    );
    recordBtn(a).disabled =
      !shortcutsLoaded || reminderShowing || saving || (recording !== null && recording !== a);
    recordBtn(a).title = reminderShowing ? BUSY : "";
  }
}

function applyShortcuts(s: Settings) {
  stored.drank = s.drankShortcut;
  stored.snooze = s.snoozeShortcut;
  labels.drank = s.drankLabel;
  labels.snooze = s.snoozeLabel;
  shortcutsLoaded = true;
  setReminderShowing(s.reminderShowing);
}

function setReminderShowing(showing: boolean) {
  reminderShowing = showing;
  if (showing) {
    stopRecording();
    showShortcut(BUSY, false);
  } else if (shortcutStatus.textContent === BUSY) {
    showShortcut("", false);
  }
  renderShortcuts();
}

const MODIFIER_CODES = new Set(
  ["Shift", "Control", "Alt", "Meta", "OS"].flatMap((m) => [`${m}Left`, `${m}Right`]),
);

// The key part of a combo. Windows registers by layout key, so letters and
// digits come from `e.key` there (when it is one); everything else, and every
// key on macOS, from the physical `e.code`.
function keyPart(e: KeyboardEvent): string {
  if (isWindows && /^(Key[A-Z]|Digit[0-9])$/.test(e.code) && /^[a-z0-9]$/i.test(e.key)) {
    return e.key.toUpperCase();
  }
  return e.code;
}

function combo(e: KeyboardEvent): string {
  const parts: string[] = [];
  if (e.shiftKey) parts.push("shift");
  if (e.ctrlKey) parts.push("control");
  if (e.altKey) parts.push("alt");
  if (e.metaKey) parts.push("super");
  parts.push(keyPart(e));
  return parts.join("+");
}

async function saveShortcut(action: Action, value: string) {
  saving = true;
  renderShortcuts();
  const next = { ...stored, [action]: value };
  try {
    await invoke("save_shortcuts", { drank: next.drank, snooze: next.snooze });
    applyShortcuts(await invoke<Settings>("get_settings"));
    if (!reminderShowing) showShortcut("Saved.", false);
  } catch (e) {
    showShortcut(String(e), true);
  } finally {
    saving = false;
    renderShortcuts();
  }
}

window.addEventListener(
  "keydown",
  (e) => {
    if (recording === null) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.repeat || MODIFIER_CODES.has(e.code)) return;
    const action = recording;
    stopRecording();
    if (e.key === "Escape") {
      renderShortcuts();
      return;
    }
    saveShortcut(action, combo(e));
  },
  true,
);

for (const a of ACTIONS) {
  recordBtn(a).addEventListener("click", () => {
    if (recording === a) {
      stopRecording();
    } else {
      recording = a;
      showShortcut(PROMPT, false);
    }
    renderShortcuts();
  });
}

// Recording ends if the window loses focus, so it can't be left half-done.
window.addEventListener("blur", () => {
  if (recording !== null) {
    stopRecording();
    renderShortcuts();
  }
});

async function load() {
  const s = await invoke<Settings>("get_settings");
  applyShortcuts(s);
  interval.value = String(s.intervalMin);
  timeoutOn.checked = s.popupTimeoutMin !== null;
  timeout.value = String(s.popupTimeoutMin ?? DEFAULT_TIMEOUT_MIN);
  // The inputs start disabled, so nothing can be sent before (or without) a
  // successful load.
  interval.disabled = false;
  timeoutOn.disabled = false;
  timeout.disabled = !timeoutOn.checked;
}

// Launch at login lives in the OS (a LaunchAgent or a Run registry entry), so
// it's read from there each time and sent on its own, not with the settings.
async function loadLogin() {
  login.checked = await invoke<boolean>("get_launch_at_login");
  login.disabled = false;
}

async function saveLogin() {
  const enabled = login.checked;
  login.disabled = true;
  try {
    await invoke("set_launch_at_login", { enabled });
    show("Saved.", false);
  } catch (e) {
    login.checked = !enabled;
    show(String(e), true);
  } finally {
    login.disabled = false;
  }
}

login.addEventListener("change", saveLogin);
interval.addEventListener("change", save);
timeout.addEventListener("change", save);
timeoutOn.addEventListener("change", () => {
  timeout.disabled = !timeoutOn.checked;
  if (timeoutOn.checked && timeout.value.trim() === "") {
    timeout.value = String(DEFAULT_TIMEOUT_MIN);
  }
  save();
});

// Separate, so a login-setting failure leaves the other settings usable (its
// checkbox stays disabled).
// Listen before loading, so a reminder that starts or ends between the two
// can't leave the Record buttons in the wrong state.
listen<boolean>("reminder-showing", (e) => setReminderShowing(e.payload))
  .catch((e) => console.error("[hydration] reminder-showing listen failed", e))
  .then(() => load())
  .catch((e) => show(`Couldn't load settings: ${e}`, true))
  .then(() => loadLogin())
  .catch((e) => show(String(e), true));
