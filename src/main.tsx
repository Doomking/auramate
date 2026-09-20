import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import Pet from "./Pet";

const isPet = window.location.hash.replace(/^#/, "") === "pet";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>{isPet ? <Pet /> : <App />}</React.StrictMode>,
);
