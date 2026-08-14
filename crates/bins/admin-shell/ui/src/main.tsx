import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import App from "@/App";
import "@/index.css";

const rootEl = document.getElementById("root");
if (!rootEl) {
  throw new Error("root element not found — index.html is missing <div id=\"root\">");
}

// BrowserRouter (not HashRouter): Tauri serves the built frontend over
// its own `tauri://localhost` (desktop) / custom protocol, which
// supports real paths the same way a normal web server does — no need
// for HashRouter's `#/route` workaround, which exists for static file
// hosts that can't rewrite arbitrary paths to index.html.
createRoot(rootEl).render(
  <StrictMode>
    <BrowserRouter>
      <App />
    </BrowserRouter>
  </StrictMode>,
);
