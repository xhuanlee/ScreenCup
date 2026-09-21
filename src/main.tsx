import { createRoot } from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";

import App from "./App";
import E2eRunner from "./e2e/E2eRunner";
import RecordingBar from "./bar/RecordingBar";
import RegionOverlay from "./region/RegionOverlay";
import "./styles.css";

// Boot beacon: surfaces how far the frontend gets, and any top-level error,
// through the same channel the app already uses.
invoke("log_frontend", { message: "boot: main.tsx evaluated" }).catch(() => {});
window.addEventListener("error", (e) => {
  invoke("log_frontend", {
    message: `error: ${e.message} @ ${e.filename}:${e.lineno}`,
  }).catch(() => {});
});
window.addEventListener("unhandledrejection", (e) => {
  invoke("log_frontend", {
    message: `promise: ${String(e.reason)}`,
  }).catch(() => {});
});

function routeOf(hash: string): string {
  const clean = hash.replace(/^#\/?/, "");
  return clean.split(/[/?]/)[0];
}

const route = routeOf(window.location.hash);

createRoot(document.getElementById("root")!).render(
  route === "region" ? (
    <RegionOverlay />
  ) : route === "bar" ? (
    <RecordingBar />
  ) : route.startsWith("e2e") ? (
    <E2eRunner mode={route.replace(/^e2e-?/, "") as never} />
  ) : (
    <App />
  ),
);

// The backend sets the hash after load when SCREENCUT_E2E is set.
window.addEventListener("hashchange", () => {
  const r = routeOf(window.location.hash);
  if (r.startsWith("e2e")) {
    createRoot(document.getElementById("root")!).render(
      <E2eRunner mode={r.replace(/^e2e-?/, "") as never} />,
    );
  }
});
