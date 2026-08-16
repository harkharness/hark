import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeHighlight from "rehype-highlight";
import bash from "highlight.js/lib/languages/bash";
import json from "highlight.js/lib/languages/json";
import yaml from "highlight.js/lib/languages/yaml";
import rust from "highlight.js/lib/languages/rust";
import typescript from "highlight.js/lib/languages/typescript";
import python from "highlight.js/lib/languages/python";
import diff from "highlight.js/lib/languages/diff";
import sql from "highlight.js/lib/languages/sql";

// Only the grammars this workflow actually shows, to keep the bundle small.
const languages = { bash, json, yaml, rust, typescript, python, diff, sql };

/** Assistant prose rendered like a real markdown document. */
export default function Markdown({ children }: { children: string }) {
  return (
    <div className="md">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[[rehypeHighlight, { languages, detect: false }]]}
      >
        {children}
      </ReactMarkdown>
    </div>
  );
}
