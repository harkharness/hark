// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, within } from "@testing-library/react";
import Sidebar from "./Sidebar";
import { t } from "../lib/i18n";
import type { Project } from "../types";

afterEach(cleanup);

const vox: Project = { name: "vox", path: "/p/vox" };
const lab: Project = { name: "lab", path: "/p/lab" };
const noop = () => {};
const base = {
  projects: [vox, lab],
  tasks: [],
  liveTitles: [],
  onOpen: noop,
  onRename: noop,
  onPin: noop,
  onArchive: noop,
  onResume: noop,
  onNewChat: noop,
  onAddProject: noop,
  onRemoveProject: noop,
  onOpenFiles: noop,
  onOpenGeneral: noop,
  onToggleArtifacts: noop,
  onToggleBoard: noop,
  onSearchChats: noop,
  onCreate: noop,
  onDoor: noop,
  onOpenDemands: noop,
};

describe("the two doors of the sidebar", () => {
  it("Início lists Hoje, the board and artifacts, and no project", () => {
    // The board is planning, not code: it moved to Início (05/11). The
    // Code door keeps its shortcut, which opens the board as a panel.
    const onToggleBoard = vi.fn();
    const { getByText, queryByText } = render(<Sidebar {...base} door="inicio" onToggleBoard={onToggleBoard} />);
    expect(getByText(t("side_today"))).toBeTruthy();
    expect(getByText(t("side_artifacts"))).toBeTruthy();
    fireEvent.click(getByText(t("side_board")));
    expect(onToggleBoard).toHaveBeenCalled();
    expect(queryByText("vox")).toBeNull();
    expect(queryByText(t("side_create"))).toBeNull();
  });

  it("Code lists Criar, search, artifacts, board and the projects, and no Hoje", () => {
    const { getByText, queryByText } = render(<Sidebar {...base} door="code" />);
    expect(getByText(t("side_create"))).toBeTruthy();
    expect(getByText(t("side_search_row"))).toBeTruthy();
    expect(getByText(t("side_board"))).toBeTruthy();
    expect(getByText("vox")).toBeTruthy();
    expect(queryByText(t("side_today"))).toBeNull();
  });

  it("the toggle says which door is open and asks for the other", () => {
    const onDoor = vi.fn();
    const { getByText } = render(<Sidebar {...base} door="inicio" onDoor={onDoor} />);
    expect(getByText(t("door_inicio")).closest("button")?.className).toContain("on");
    fireEvent.click(getByText(t("door_code")));
    expect(onDoor).toHaveBeenCalledWith("code");
  });

  it("Início lists Demandas with its count, and opens the list on click", () => {
    const onOpenDemands = vi.fn();
    const { getByText } = render(<Sidebar {...base} door="inicio" demandsCount={4} onOpenDemands={onOpenDemands} />);
    fireEvent.click(getByText(t("side_demands")));
    expect(onOpenDemands).toHaveBeenCalled();
    expect(getByText("4")).toBeTruthy();
  });

  it("a chat with something unread behind the bell reads strong", () => {
    const chats = [{ session_id: "s1", title: "Billing v2", cwd: "/p/vox", last_ts: "2026-11-04T10:00:00Z" }];
    const { getByText } = render(<Sidebar {...base} door="code" chats={chats} unreadSessionIds={["s1"]} />);
    expect(getByText("Billing v2").closest(".side-item")?.className).toContain("unread");
  });

  it("a history chat reads active when open and shows the dot when a worker is on it", () => {
    // A chat needs no card any more (05/11): the row is decorated by session.
    const chats = [
      { session_id: "s1", title: "Patch de segurança", cwd: "/p/vox", last_ts: "2026-11-05T08:52:00Z" },
      { session_id: "s2", title: "Outro chat", cwd: "/p/vox", last_ts: "2026-11-04T08:52:00Z" },
    ];
    const { getByText } = render(<Sidebar {...base} door="code" chats={chats} activeSessionId="s1" liveSessionIds={["s2"]} />);
    expect(getByText("Patch de segurança").closest(".side-item")?.className).toContain("active");
    expect(getByText("Outro chat").querySelector(".side-dot")?.className ?? getByText("Outro chat").parentElement?.querySelector(".side-dot")?.className).toContain("live");
  });

  it("rows are chats: a card decorates the chat it is tied to and opens on click, a loose card stays on the board", () => {
    // Rows are chats (05/11): the row keeps the CHAT's own name — the one
    // the CLI shows too — and the card lends its status and a count, its
    // name on hover; a card with no session of its own is the board's.
    const onOpen = vi.fn();
    const chats = [{ session_id: "s1", title: "patch windows", cwd: "/p/vox", last_ts: "2026-11-05T08:52:00Z" }];
    const tied = { title: "Patch de segurança", status: "waiting" as const, workspace: "/p/vox", session_ids: ["s1"], updated_at: "2026-11-05T08:52:00Z", pinned: false, subtasks: [] };
    const loose = { title: "Só no quadro", status: "doing" as const, workspace: "/p/vox", session_ids: [], updated_at: "2026-11-05T08:52:00Z", pinned: false, subtasks: [] };
    const { getByText, getByTitle, queryByText } = render(<Sidebar {...base} door="code" chats={chats} tasks={[tied, loose]} onOpen={onOpen} />);
    const row = getByText("patch windows").closest(".side-item")!;
    expect(row.className).toContain("waiting");
    expect(getByTitle(`${t("side_tasks_hint", { n: 1 })}: Patch de segurança`)).toBeTruthy();
    expect(queryByText("Patch de segurança")).toBeNull();
    expect(queryByText("Só no quadro")).toBeNull();
    fireEvent.click(getByText("patch windows"));
    expect(onOpen).toHaveBeenCalledWith(tied);
  });

  it("a chat's menu renames the chat itself, card or no card", () => {
    // The name is the chat's (05/11): it goes to the session's own
    // history, so the CLI's resume list and every window agree. A card's
    // name is the card's and is renamed on the board.
    const onRenameChat = vi.fn();
    const chats = [{ session_id: "s1", title: "patch windows", cwd: "/p/vox", last_ts: "2026-11-05T08:52:00Z" }];
    const { getByText, getByDisplayValue } = render(<Sidebar {...base} door="code" chats={chats} onRenameChat={onRenameChat} />);
    const row = getByText("patch windows").closest(".side-item") as HTMLElement;
    fireEvent.click(within(row).getByTitle(t("side_actions")));
    fireEvent.click(within(row).getByText(t("side_rename")));
    const input = getByDisplayValue("patch windows") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "Patch de segurança do Windows" } });
    fireEvent.blur(input);
    expect(onRenameChat).toHaveBeenCalledWith(chats[0], "Patch de segurança do Windows");
  });

  it("a pinned card keeps a row of its own at the top", () => {
    const pinned = { title: "Oncall", status: "doing" as const, workspace: "/p/vox", session_ids: [], updated_at: "2026-11-05T08:52:00Z", pinned: true, subtasks: [] };
    const { getByText } = render(<Sidebar {...base} door="code" tasks={[pinned]} />);
    expect(getByText("Oncall")).toBeTruthy();
  });

  it("a project group folds to one line and remembers it; its menu offers every chat", () => {
    const onMoreChats = vi.fn();
    const chats = [{ session_id: "s1", title: "Billing v2", cwd: "/p/vox", last_ts: "2026-11-04T10:00:00Z" }];
    const { getByText, queryByText, getAllByTitle } = render(<Sidebar {...base} door="code" chats={chats} onMoreChats={onMoreChats} />);
    expect(getByText("Billing v2")).toBeTruthy();
    fireEvent.click(getAllByTitle(t("side_collapse"))[0]);
    expect(queryByText("Billing v2")).toBeNull();
    expect(JSON.parse(localStorage.getItem("hark.side.collapsed") ?? "[]")).toEqual(["/p/vox"]);
    fireEvent.click(getAllByTitle(t("side_actions"))[0]);
    fireEvent.click(getByText(t("side_menu_all", { n: 1 })));
    expect(onMoreChats).toHaveBeenCalledWith(vox);
    localStorage.removeItem("hark.side.collapsed");
  });

  it("a page dropped on a chat hands the chat and the link over", () => {
    const onDropLink = vi.fn();
    const chats = [{ session_id: "s1", title: "Billing v2", cwd: "/p/vox", last_ts: "2026-11-04T10:00:00Z" }];
    const { getByText } = render(<Sidebar {...base} door="code" chats={chats} onDropLink={onDropLink} />);
    const row = getByText("Billing v2").closest(".side-item")!;
    const dataTransfer = { types: ["text/hark-artifact"], getData: () => "hark://artifacts/vox/deck" };
    fireEvent.drop(row, { dataTransfer });
    expect(onDropLink).toHaveBeenCalledWith({ title: "Billing v2", session_id: "s1" }, "hark://artifacts/vox/deck");
  });

  it("Início lists the days before today, and a day opens on click", () => {
    const onOpenDay = vi.fn();
    const day = { date: "2026-11-03", label: "ontem · ter 03", count: 2, first: "status do billing" };
    const { getByText } = render(<Sidebar {...base} door="inicio" days={[day]} onOpenDay={onOpenDay} />);
    expect(getByText(t("side_days"))).toBeTruthy();
    fireEvent.click(getByText("ontem · ter 03"));
    expect(onOpenDay).toHaveBeenCalledWith(day);
  });

  it("Criar asks the app to create, not to open the general chat", () => {
    const onCreate = vi.fn();
    const onOpenGeneral = vi.fn();
    const { getByText } = render(<Sidebar {...base} door="code" onCreate={onCreate} onOpenGeneral={onOpenGeneral} />);
    fireEvent.click(getByText(t("side_create")));
    expect(onCreate).toHaveBeenCalled();
    expect(onOpenGeneral).not.toHaveBeenCalled();
  });
});
