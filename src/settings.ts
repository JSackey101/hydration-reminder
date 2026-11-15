// The settings window. Rust owns and validates the settings; this page loads
// them, sends each valid change straight away (no Save button) and shows any
// error Rust returns.
import { invoke } from "@tauri-apps/api/core";

type Settings = { intervalMin: number; popupTimeoutMin: number | null };

const interval = document.getElementById("interval") as HTMLInputElement;
const timeoutOn = document.getElementById("timeout-on") as HTMLInputElement;
const timeout = document.getElementById("timeout") as HTMLInputElement;
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

async function load() {
  const s = await invoke<Settings>("get_settings");
  interval.value = String(s.intervalMin);
  timeoutOn.checked = s.popupTimeoutMin !== null;
  timeout.value = String(s.popupTimeoutMin ?? DEFAULT_TIMEOUT_MIN);
  // The inputs start disabled, so nothing can be sent before (or without) a
  // successful load.
  interval.disabled = false;
  timeoutOn.disabled = false;
  timeout.disabled = !timeoutOn.checked;
}

interval.addEventListener("change", save);
timeout.addEventListener("change", save);
timeoutOn.addEventListener("change", () => {
  timeout.disabled = !timeoutOn.checked;
  if (timeoutOn.checked && timeout.value.trim() === "") {
    timeout.value = String(DEFAULT_TIMEOUT_MIN);
  }
  save();
});

load().catch((e) => show(`Couldn't load settings: ${e}`, true));
