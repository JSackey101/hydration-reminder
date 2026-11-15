// The reminder popup. Rust owns all state; this page only sends the answer.
import { invoke } from "@tauri-apps/api/core";

for (const id of ["drank", "snooze"] as const) {
  document.getElementById(id)?.addEventListener("click", () => {
    invoke(id).catch((e) => console.error(`[hydration] ${id} failed`, e));
  });
}
