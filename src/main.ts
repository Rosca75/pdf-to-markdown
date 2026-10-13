import "./styles.css";
import { tauriApi } from "./api";
import { mount } from "./app";

const root = document.getElementById("app");
if (!root) throw new Error("Missing #app element");

mount(root, { api: tauriApi }).catch((e) => {
  root.textContent = `PDF2MD could not start: ${e instanceof Error ? e.message : String(e)}`;
});
