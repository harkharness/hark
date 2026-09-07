// Spike: what does `gemini --acp` ACTUALLY say on the wire?
// Sends the ACP handshake over stdio and records every line it gets back.
// Handshake only — initialize/authenticate do not reach the model, so
// this costs nothing.
import { spawn } from "node:child_process";
import { writeFileSync } from "node:fs";

const child = spawn("gemini", ["--acp"], { stdio: ["pipe", "pipe", "pipe"] });
const seen = [];
let buf = "";

child.stdout.on("data", (d) => {
  buf += d.toString();
  let at;
  while ((at = buf.indexOf("\n")) !== -1) {
    const line = buf.slice(0, at).trim();
    buf = buf.slice(at + 1);
    if (line) {
      seen.push(line);
      console.log("<<", line.slice(0, 400));
    }
  }
});
child.stderr.on("data", (d) => console.error("[err]", d.toString().trim().slice(0, 300)));

const send = (msg) => {
  const line = JSON.stringify(msg);
  console.log(">>", line.slice(0, 300));
  child.stdin.write(line + "\n");
};

send({
  jsonrpc: "2.0",
  id: 1,
  method: "initialize",
  params: {
    protocolVersion: 1,
    clientCapabilities: { fs: { readTextFile: false, writeTextFile: false } },
  },
});

setTimeout(() => {
  // Ask what a session looks like too, if initialize got us that far.
  send({
    jsonrpc: "2.0",
    id: 2,
    method: "session/new",
    params: { cwd: process.cwd(), mcpServers: [] },
  });
}, 1500);

setTimeout(() => {
  writeFileSync(
    new URL("./handshake.jsonl", import.meta.url),
    seen.join("\n") + "\n",
  );
  console.log(`\n[spike] ${seen.length} lines recorded`);
  child.kill();
  process.exit(0);
}, 5000);
