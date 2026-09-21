/**
 * 1-based line of the `[table]` header in a TOML text, or null. Exact
 * table only: `[agents.twin.models]` and `[agents.twinkle]` are not
 * `[agents.twin]`. Spaces inside the brackets and a trailing comment are
 * tolerated, the way TOML itself tolerates them.
 */
export function sectionLine(text: string, table: string): number | null {
  const escaped = table.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const header = new RegExp(`^\\s*\\[\\s*${escaped}\\s*\\]\\s*(#.*)?$`);
  const lines = text.split("\n");
  for (let i = 0; i < lines.length; i++) {
    if (header.test(lines[i])) return i + 1;
  }
  return null;
}
