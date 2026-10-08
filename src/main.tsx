import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./app/App";
import { AppProviders } from "./app/providers";
import "./styles/global.css";
import "./styles/theme.css";

const root = document.getElementById("root");

if (!root) {
  throw new Error("ClipRiva could not find its root element.");
}

createRoot(root).render(
  <StrictMode>
    <AppProviders>
      <App />
    </AppProviders>
  </StrictMode>,
);
