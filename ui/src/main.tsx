import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";

// Self-hosted fonts (no external requests to Google)
import "@fontsource/inter/400.css";
import "@fontsource/inter/500.css";
import "@fontsource/inter/600.css";
import "@fontsource/inter/700.css";
import "@fontsource/jetbrains-mono/400.css";
import "@fontsource/jetbrains-mono/500.css";

// Apply dark mode based on system preference
const applyDarkMode = () => {
  if (window.matchMedia("(prefers-color-scheme: dark)").matches) {
    document.documentElement.classList.add("dark");
  } else {
    document.documentElement.classList.remove("dark");
  }
};

// Initial check
applyDarkMode();

// Listen for system preference changes
window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", applyDarkMode);

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
