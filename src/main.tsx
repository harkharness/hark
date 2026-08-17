import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import Mother from "./Mother";
import "./styles.css";

// One bundle, three window kinds: the main window is the MOTHER (the voice
// that orchestrates); project windows open with ?project=<path>&name=<n>
// and run the workbench scoped to that project; ?hq=1 opens the global HQ
// (board + costs across every project, no filter).
const params = new URLSearchParams(window.location.search);
const projectPath = params.get("project");
const projectName = params.get("name");
const isHq = params.get("hq") != null;
const tabParam = params.get("tab");
const initialTab =
  tabParam === "board" || tabParam === "custos" ? tabParam : undefined;

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    {projectPath ? (
      <App forcedProject={{ name: projectName ?? projectPath, path: projectPath }} />
    ) : isHq ? (
      <App initialTab={initialTab ?? "board"} />
    ) : (
      <Mother />
    )}
  </React.StrictMode>,
);
