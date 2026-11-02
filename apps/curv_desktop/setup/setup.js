// Local setup page. Its only power is the `set_server_url` command, which validates the
// address again in Rust (the checks here are just for quick feedback).
"use strict";

(function () {
  const form = document.getElementById("setup-form");
  const input = document.getElementById("server-url");
  const error = document.getElementById("error");
  const save = document.getElementById("save");

  if (typeof window.__CURV_CURRENT_SERVER__ === "string") {
    input.value = window.__CURV_CURRENT_SERVER__;
  }
  input.focus();

  function showError(message) {
    error.textContent = message;
    error.hidden = !message;
  }

  form.addEventListener("submit", async function (event) {
    event.preventDefault();
    showError("");
    const ipc = window.__TAURI_INTERNALS__;
    if (!ipc || typeof ipc.invoke !== "function") {
      showError("This page only works inside the Curv Console desktop app.");
      return;
    }
    save.disabled = true;
    try {
      await ipc.invoke("set_server_url", { url: input.value });
      // On success Rust opens the console window and closes this one.
    } catch (err) {
      showError(typeof err === "string" ? err : "Could not save the address.");
    } finally {
      save.disabled = false;
    }
  });
})();
