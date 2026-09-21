import { glyphFor, hueFor, markStateOf, monogramOf, VENDOR_HUE, type MarkGlyph, type MarkState } from "../lib/agentMark";

/** Drawn here, on lucide's 24×24 grid. Geometric stand-ins tinted with
 *  the vendor's hue — never a vendor's own logo file. */
const GLYPH: Record<Exclude<MarkGlyph, "monogram">, JSX.Element> = {
  anthropic: (
    <>
      <path d="M7.2 17 11 6.6h2L16.8 17" />
      <path d="M9.3 13.6h5.4" />
    </>
  ),
  google: <path d="M12 4.5c.9 4 2.6 5.7 6.6 6.6-4 .9-5.7 2.6-6.6 6.6-.9-4-2.6-5.7-6.6-6.6 4-.9 5.7-2.6 6.6-6.6Z" fill="currentColor" stroke="none" />,
  openai: (
    <>
      <circle cx="12" cy="12" r="6.4" />
      <path d="M12 5.6v12.8M6.5 8.8l11 6.4M6.5 15.2l11-6.4" strokeWidth="1.3" />
    </>
  ),
  deepseek: (
    <>
      <circle cx="12" cy="12" r="5" />
      <path d="M12 3.6a8.4 8.4 0 0 1 8.4 8.4" />
    </>
  ),
  aws: (
    <>
      <path d="M4.5 14.6c4.4 2.9 10.6 2.9 15 0" />
      <path d="M7.5 9.6h9" />
    </>
  ),
};

/**
 * The agent's face in the catalog. The ring carries the state, which is
 * why no card needs a pill to say "installed" or "off".
 */
export default function AgentMark({
  vendor,
  id,
  state,
  size = 34,
}: {
  vendor: string;
  id: string;
  state: MarkState;
  size?: number;
}) {
  const glyph = glyphFor(vendor);
  const hue = glyph === "monogram" ? hueFor(id) : VENDOR_HUE[glyph];
  return (
    <span
      className="agent-mark"
      data-state={state}
      style={{ "--mark": hue, width: size, height: size } as React.CSSProperties}
      aria-hidden
    >
      {glyph === "monogram" ? (
        <span className="agent-mark-letter" style={{ fontSize: Math.round(size * 0.4) }}>
          {monogramOf(id)}
        </span>
      ) : (
        <svg
          width={Math.round(size * 0.62)}
          height={Math.round(size * 0.62)}
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          {GLYPH[glyph]}
        </svg>
      )}
    </span>
  );
}

export { markStateOf };
