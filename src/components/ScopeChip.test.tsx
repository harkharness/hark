// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import ScopeChip from "./ScopeChip";
import type { Project } from "../types";

const hark: Project = { name: "hark", path: "/Users/x/Projects/hark" };
const landing: Project = { name: "landing", path: "/Users/x/Projects/landing" };

afterEach(cleanup);

describe("the scope chip", () => {
  it("is a picker when nothing is open: every project, and all of them", () => {
    const onPick = vi.fn();
    render(<ScopeChip scope={{ kind: "all" }} projects={[hark, landing]} onPick={onPick} />);
    fireEvent.click(screen.getByRole("button", { name: /Todos os projetos/ }));
    fireEvent.click(screen.getByRole("menuitem", { name: /landing/ }));
    expect(onPick).toHaveBeenCalledWith(landing);
  });

  it("goes back to all projects from a draft in one of them", () => {
    const onPick = vi.fn();
    render(<ScopeChip scope={{ kind: "project", project: hark }} projects={[hark, landing]} onPick={onPick} />);
    fireEvent.click(screen.getByRole("button", { name: /hark/ }));
    fireEvent.click(screen.getByRole("menuitem", { name: /Todos os projetos/ }));
    expect(onPick).toHaveBeenCalledWith(null);
  });

  it("only says where the message lands inside a chat: no menu to open", () => {
    render(<ScopeChip scope={{ kind: "chat", title: "Billing v2", project: hark }} projects={[hark]} onPick={vi.fn()} />);
    expect(screen.queryByRole("button")).toBeNull();
    expect(screen.getByText("Billing v2")).toBeTruthy();
  });

  it("stays a label where no picker was given (a project window)", () => {
    render(<ScopeChip scope={{ kind: "project", project: hark }} />);
    expect(screen.queryByRole("button")).toBeNull();
  });
});
