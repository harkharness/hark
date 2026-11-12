import { BarChart3, FileText, Palette, Presentation, StickyNote, Table2 } from "lucide-react";
import type { ArtifactKind } from "../types";

/** The icon of each artifact kind, the same everywhere a page is named. */
export const KIND_ICON: Record<ArtifactKind, typeof FileText> = { doc: FileText, slides: Presentation, design: Palette, sheet: Table2, board: StickyNote, data: BarChart3 };
