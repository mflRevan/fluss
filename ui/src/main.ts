import "@fontsource-variable/inter";
import "./app.css";
import { mount } from "svelte";
import { inTauri } from "./lib/backend";

async function boot() {
  const params = new URLSearchParams(location.search);
  let popup = params.has("popup");
  if (inTauri) {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    popup = getCurrentWindow().label === "popup";
  }
  const target = document.getElementById("app")!;
  // Dev only: preview at the real window size inside a big browser window.
  if (!inTauri && params.has("frame")) {
    const [w, h] = (params.get("frame") || "1080x720").split("x").map(Number);
    Object.assign(target.style, { width: `${w}px`, height: `${h}px`, margin: "24px", position: "relative", overflow: "hidden", borderRadius: "10px", boxShadow: "0 0 0 1px #333" });
    document.body.style.background = "#555";
  }
  if (popup) {
    document.body.classList.add("popup-mode");
    const { default: Popup } = await import("./lib/Popup.svelte");
    mount(Popup, { target });
  } else {
    const { default: App } = await import("./App.svelte");
    mount(App, { target });
  }
}

boot();
