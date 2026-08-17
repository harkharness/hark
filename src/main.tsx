import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import Mother from "./Mother";
import "./styles.css";

// One bundle, two window kinds: the main window is the MOTHER (voice +
// global board/costs tabs); project windows open with ?project=<path>&name=
// and run the workbench scoped to that project (board filtered, costs in
// the scope popover only).
const params = new URLSearchParams(window.location.search);
const projectPath = params.get("project");
const projectName = params.get("name");

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    {projectPath ? (
      <App forcedProject={{ name: projectName ?? projectPath, path: projectPath }} />
    ) : (
      <Mother />
    )}
  </React.StrictMode>,
);
