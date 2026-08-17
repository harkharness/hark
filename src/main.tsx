import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import Mother from "./Mother";
import "./styles.css";

// One bundle, two window kinds: the main window is the MOTHER (the voice
// that orchestrates); project windows open with ?project=<path>&name=<n>
// and run the full workbench scoped to that project.
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
