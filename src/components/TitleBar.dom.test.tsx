import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { TitleBar } from "./TitleBar";

describe("TitleBar UI scale selector", () => {
  it("exposes a named radio group with its selected scale and current display scale", () => {
    render(<TitleBar scale={1.25} scaleMode="1.25" onScaleModeChange={() => undefined} />);

    const group = within(screen.getByRole("group", { name: "UI倍率" }));
    const radios = group.getAllByRole("radio");
    expect(radios).toHaveLength(4);
    for (const [name, value] of [
      ["Auto", "auto"],
      ["100%", "1"],
      ["125%", "1.25"],
      ["150%", "1.5"],
    ]) {
      const radio = group.getByRole("radio", { name });
      expect(radio).toHaveAttribute("name", "ui-scale");
      expect(radio).toHaveAttribute("value", value);
      if (value === "1.25") expect(radio).toBeChecked();
      else expect(radio).not.toBeChecked();
    }
    expect(screen.getByRole("status", { name: "現在の表示倍率" })).toHaveTextContent("125%");
  });
});
