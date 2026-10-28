import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import Hud from "./Hud";
import "./styles.css";

// One bundle, two window kinds: the main window is the cabin (every
// project, the general chat, board, costs and settings as views); project
// windows open with ?project=<path>&name= and run the same workbench
// scoped to one project. The mother is retired from the main window
// (phase 10.1b); its file goes in 10.1d.
const params = new URLSearchParams(window.location.search);
const projectPath = params.get("project");
const projectName = params.get("name");
// Opened from a card on the global board: land in that task's chat.
const taskTitle = params.get("task");
const taskSession = params.get("session");

// The frameless voice HUD paints on a transparent window.
if (params.get("hud")) document.documentElement.classList.add("hud-window");

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    {params.get("hud") ? (
      <Hud />
    ) : projectPath ? (
      <App
        forcedProject={{ name: projectName ?? projectPath, path: projectPath }}
        initialTask={
          taskTitle ? { title: taskTitle, sessionId: taskSession ?? undefined } : undefined
        }
      />
    ) : (
      <App />
    )}
  </React.StrictMode>,
);
