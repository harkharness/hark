// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/react";
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
  it("Início lists Hoje and artifacts, and no project", () => {
    const { getByText, queryByText } = render(<Sidebar {...base} door="inicio" />);
    expect(getByText(t("side_today"))).toBeTruthy();
    expect(getByText(t("side_artifacts"))).toBeTruthy();
    expect(queryByText("vox")).toBeNull();
    expect(queryByText(t("side_create"))).toBeNull();
    expect(queryByText(t("side_board"))).toBeNull();
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
    const task = { title: "Billing v2", status: "doing" as const, workspace: "/p/vox", session_ids: [], updated_at: "2026-11-04T10:00:00Z", pinned: false, subtasks: [] };
    const { getByText } = render(<Sidebar {...base} door="code" tasks={[task]} unreadTitles={["Billing v2"]} />);
    expect(getByText("Billing v2").closest(".side-item")?.className).toContain("unread");
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
