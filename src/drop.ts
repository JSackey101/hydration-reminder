// The small drop (CAP-16). Rust owns all state; a click asks it to swap the
// drop for the normal popup.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

const drop = document.getElementById("drop");

// Anywhere in the window counts, so a click just outside the round bubble
// (still inside the transparent window on Windows) isn't lost.
document.addEventListener("click", () => {
  invoke("open_reminder").catch((e) => console.error("[hydration] open_reminder failed", e));
});

// One gentle pulse each time the drop appears. The window is built once and
// hidden between reminders, so Rust says when it's shown.
function pulse() {
  if (!drop) return;
  drop.classList.remove("pulse");
  void drop.offsetWidth; // restart the animation
  drop.classList.add("pulse");
}

listen("drop-shown", pulse).catch((e) =>
  console.error("[hydration] drop-shown listen failed", e),
);
// A reminder due at startup can show the drop before this page is listening;
// pulse on load too (harmless while hidden).
pulse();
