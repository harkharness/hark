# Chrome: the app's visual grammar

`styles.css` is one stylesheet for the whole app. This is the part of it
that is a DECISION rather than a detail — the rules that kept getting
broken, and what breaking them looked like.

The `.design/` directory holds the canvases these came from
(`visual-amigavel/` is the original harmonisation; `cabecalhos-leves/`
is the header and divider pass). The `.dc.html` artboards are versioned;
the seeded `.html` is not.

## Measure. Do not look.

Every spacing bug in this app was found by measuring and missed by
eyeballing. A panel header sat at 44px through three rounds of "tighten
the padding" because the thing setting its height was the `+` button —
a tab box with a button's own padding inside it, 25px against 22px
controls. No amount of padding work reaches that.

When a bar is too tall, print the height of every child first:

```js
[...head.children].flatMap(c => [c, ...c.children])
  .map(e => ({ who: e.className, h: e.offsetHeight }))
```

The tallest child is the bar. Fix that, or accept the height.

Rendering against the real `styles.css` beats reasoning about it: build a
throwaway page that inlines the stylesheet and uses the REAL class names
and DOM order (`.design/*/**-harness.html` are gitignored). A harness
with hand-written markup can agree with a bug the real tree does not
have — the terminal header matched its mockup in a harness while the
shipped one put the action button on the wrong side, because the real
component passed it through a different slot.

And a harness INLINES the stylesheet, so it goes stale the moment you
edit `styles.css`. Re-inline before measuring: a reading taken from a
stale harness is a confident number about the previous version.

## Tokens

| token | what it is for |
|---|---|
| `--bg` | the page the thread is written on |
| `--bg-deep` | one step UNDER the page: the sidebar rail |
| `--panel` | one step OVER it: cards, popovers, panel headers |
| `--panel-2` | chips, hover, active rows, filled small controls |
| `--text-strong` | headings and emphasis, never a hue |
| `--text` / `--dim` | body, and everything secondary |
| `--border` | hairlines that melt; `--border-strong` is the rare visible one |
| `--accent` + semantics | in DOSES — a dot, a check, a diffstat, one pill |

Small controls are FILLED with `--panel-2` instead of being given a
border. Borders on everything was the strongest "terminal app" tell.

Sans for the chrome, mono only for what the machine says: code, paths,
hashes, numbers, terminals.

## Separate by what the content DOES, not by drawing a boundary

Two dead ends, in order, on one header:

1. A black shadow on near-black is invisible. `box-shadow: 0 8px 18px
   -12px rgba(0,0,0,.95)` under the chat's title bar did nothing at all.
2. Lifting the bar onto `--panel` made it visible and made it a STRIP
   BOLTED ON TOP — a second surface over a page that has only one.

What works is neither: the header takes the same `--bg` as the thread
(no bar, no rule, nothing drawn) and a gradient hangs BELOW it, over the
transcript's first rows, so text dissolves into the page as it scrolls
up instead of meeting an edge.

```css
.chat-title-head { background: var(--bg); }
.chat-title-head::after {          /* takes no layout, eats no clicks */
  content: ""; position: absolute; left: 0; right: 0; top: 100%; height: 34px;
  background: linear-gradient(to bottom, var(--bg) 30%, transparent);
  pointer-events: none;
}
```

`pointer-events: none` is not optional — the band sits over the first
message.

No rules across the window. Headers have no `border-bottom` here — three
panels in a rail meant three lines of furniture.

## Dividers are hairlines with a grab area

A resize edge has to be EASY TO HIT. It does not have to be visible
furniture, and it must not cost layout.

```css
.rhandle { width: 2px; position: relative; }
.rhandle::after {            /* reaches past the line, occupies nothing */
  content: ""; position: absolute; top: 0; bottom: 0; left: -6px; right: -6px;
}
```

2px of line, 14px of reach. The overlap lands in the neighbouring
panels' own margins, so it never steals a click from their content.

Watch for a gap being paid TWICE: the rail's panels had `margin: 8px`
(16px between two of them) and the handle added its own 5px plus
margins. 23px of nothing in a column that is mostly content. The frames
keep 2px vertical and 4px horizontal; the handle is the divider.

## One column, one inset

Anything stacked in the chat column shares the composer's inset (16px):
the composer card, the turn status, the terminal-held banner. At full
bleed the banner read as a different column from the input it was
warning about.

The transcript itself is wider on purpose — `clamp(22px, 7vw, 110px)` —
because prose is the product and gets a reading column.

That clamp is for PROSE ONLY. The chat's title bar once inherited it and
sat indented like a paragraph; it is chrome, and chrome uses the edges.
Every bar in the app — chat title, panel headers — starts at 12px and
ends at 8px.

## The overlay title bar is ours to use

The project window is built with `TitleBarStyle::Overlay` and
`hidden_title(true)`, which reserves ~34px at the top. That strip is not
empty space to leave alone:

- the chat column starts at y=0 and its header IS the window's title bar,
  level with the traffic lights and the sidebar toggle;
- only the SIDEBAR pays a top offset, because only it sits under the
  lights;
- the header carries `data-tauri-drag-region` (a drag region only drags
  when the click lands on itself, so buttons inside still click);
- its `z-index` must clear `.titlebar-drag`, which spans the whole strip
  and will otherwise sit on top and swallow every click in the header;
- with the sidebar collapsed the chat column reaches the window edge, so
  the header takes a left offset in that state only (`.app.side-closed`).

## Panel headers

One bar: grip, name, the panel's tabs, its actions, then collapse,
expand, close. 34px — the height of its own 22px controls plus 6px of
air, which is the floor.

What does not belong in it, learned the hard way in a 230px rail:

- a tab strip with ONE tab. A single tab is not a choice, and it repeated
  the panel's own name. Tabs appear from the second one.
- a button carrying a WORD. "retomar sessão" truncated to "resum" and ate
  the space the tabs needed. Icon plus tooltip.

The panel's own action goes through PanelFrame's `actions` slot, not
inside `tabs` — in `tabs` it renders on the left among the tab chips, and
the rule that keeps an action clear of the window controls
(`.frame-actions > :not(.frame-ctl)`) never reaches it.

## The cascade trap

`styles.css` is one file, so a generic descendant selector reaches
further than its author meant. `.inputbar button` — written for the mic
and send circles — had the same specificity as four other rules and sat
later in the file, so it silently won against all of them: it squared off
the composer pills (999px → 8px), centred every row of the mode and model
menus, and took the baseline alignment off the slash popover. The menus
looked "designed centred". They were not.

Scope generic rules to the element that owns them (`.inputbar-row >
button`), and when a component looks wrong in a way nobody would have
chosen, suspect the cascade before the design.
