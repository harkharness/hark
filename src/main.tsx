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
// Opened from a card on the global board: land in that task's chat.
const taskTitle = params.get("task");
const taskSession = params.get("session");

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    {projectPath ? (
      <App
        forcedProject={{ name: projectName ?? projectPath, path: projectPath }}
        initialTask={
          taskTitle ? { title: taskTitle, sessionId: taskSession ?? undefined } : undefined
        }
      />
    ) : (
      <Mother />
    )}
  </React.StrictMode>,
);
