// The reminder popup. Rust owns all state; this page only sends the answer
// and shows the shortcut for each button (CAP-15).
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type Labels = { drank: string; snooze: string };

for (const id of ["drank", "snooze"] as const) {
  document.getElementById(id)?.addEventListener("click", () => {
    invoke(id).catch((e) => console.error(`[hydration] ${id} failed`, e));
  });
}

function showLabels(l: Labels) {
  for (const id of ["drank", "snooze"] as const) {
    const el = document.getElementById(`${id}-key`);
    if (el) el.textContent = l[id];
  }
}

// Sent each time the shortcuts are registered, so a newly recorded combo shows
// on the next reminder; the command covers the first one after load.
listen<Labels>("shortcut-labels", (e) => showLabels(e.payload)).catch((e) =>
  console.error("[hydration] shortcut-labels listen failed", e),
);
invoke<Labels>("get_shortcut_labels")
  .then(showLabels)
  .catch((e) => console.error("[hydration] get_shortcut_labels failed", e));
