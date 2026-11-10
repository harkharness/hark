// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/react";
import MarkLayer, { MarkBar, boxOf, type Mark } from "./MarkLayer";
import { t } from "../lib/i18n";

afterEach(cleanup);

const marks: Mark[] = [
  { id: "c1", n: 1, anchor: { kind: "region", x: 100, y: 50, w: 200, h: 40, slide: 0 } },
  { id: "c2", n: 2, anchor: { kind: "region", x: 10, y: 10, w: 0, h: 0, slide: 1 } },
];

/** The layer is drawn at half size on screen: page pixels are twice the screen's. */
const halfSize = (svg: SVGSVGElement) => {
  svg.getBoundingClientRect = () => ({ left: 0, top: 0, width: 640, height: 360, right: 640, bottom: 360, x: 0, y: 0, toJSON: () => ({}) });
};

describe("the markup layer over a page", () => {
  it("shows the marks of the slide on screen, numbered", () => {
    const { container } = render(<MarkLayer w={1280} h={720} tool="select" marks={marks} slide={0} onMark={() => {}} />);
    expect(container.querySelectorAll("g.mark").length).toBe(1);
    expect(container.querySelector("g.mark text")!.textContent).toBe("1");
    expect(container.querySelector("svg")!.className.baseVal).not.toContain("drawing");
  });

  it("the mark just drawn shows dashed until it is a comment", () => {
    const { container } = render(<MarkLayer w={1280} h={720} tool="select" marks={[]} onMark={() => {}} pending={{ kind: "region", x: 1, y: 2, w: 30, h: 40 }} />);
    expect(container.querySelector("rect.mark-draft")).toBeTruthy();
  });

  it("a rectangle dragged on screen comes back in page pixels, on the slide", () => {
    const onMark = vi.fn();
    const { container } = render(<MarkLayer w={1280} h={720} tool="rect" marks={[]} slide={2} onMark={onMark} />);
    const svg = container.querySelector("svg") as SVGSVGElement;
    halfSize(svg);
    fireEvent.mouseDown(svg, { clientX: 50, clientY: 25 });
    fireEvent.mouseMove(svg, { clientX: 150, clientY: 75 });
    fireEvent.mouseUp(svg);
    expect(onMark).toHaveBeenCalledWith({ kind: "region", slide: 2, x: 100, y: 50, w: 200, h: 100 });
  });

  it("the pen keeps its points and the text tool marks a point", () => {
    const onMark = vi.fn();
    const { container, rerender } = render(<MarkLayer w={1280} h={720} tool="pen" marks={[]} onMark={onMark} />);
    const svg = container.querySelector("svg") as SVGSVGElement;
    halfSize(svg);
    fireEvent.mouseDown(svg, { clientX: 10, clientY: 10 });
    fireEvent.mouseMove(svg, { clientX: 20, clientY: 30 });
    fireEvent.mouseMove(svg, { clientX: 40, clientY: 10 });
    fireEvent.mouseUp(svg);
    expect(onMark).toHaveBeenLastCalledWith(expect.objectContaining({ kind: "region", x: 20, y: 20, w: 60, h: 40, points: [[20, 20], [40, 60], [80, 20]] }));
    rerender(<MarkLayer w={1280} h={720} tool="text" marks={[]} onMark={onMark} />);
    fireEvent.mouseDown(svg, { clientX: 100, clientY: 100 });
    expect(onMark).toHaveBeenLastCalledWith({ kind: "region", slide: null, x: 200, y: 200, w: 0, h: 0 });
    expect(boxOf([[1, 2], [5, 9]])).toEqual({ x: 1, y: 2, w: 4, h: 7 });
  });

  it("the bar names its tools and closes", () => {
    const onTool = vi.fn();
    const onClose = vi.fn();
    const { getByLabelText } = render(<MarkBar tool="select" onTool={onTool} onClose={onClose} />);
    fireEvent.click(getByLabelText(t("mk_rect")));
    expect(onTool).toHaveBeenCalledWith("rect");
    fireEvent.click(getByLabelText(t("mk_close")));
    expect(onClose).toHaveBeenCalled();
  });
});
