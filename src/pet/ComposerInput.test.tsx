import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { ComposerInput } from "./ComposerInput";

function Harness(props: { onSubmit: (t: string) => void; onEscape?: () => void; running?: boolean; onStop?: () => void }) {
  const [value, setValue] = useState("");
  return (
    <ComposerInput
      value={value}
      onChange={setValue}
      onSubmit={(t) => {
        props.onSubmit(t);
        setValue("");
      }}
      onEscape={props.onEscape}
      running={props.running}
      onStop={props.onStop}
      placeholder="Ask Claude Code…"
    />
  );
}

describe("ComposerInput", () => {
  it("sends trimmed text on Enter", async () => {
    const onSubmit = vi.fn();
    render(<Harness onSubmit={onSubmit} />);
    const box = screen.getByLabelText("Message");
    await userEvent.type(box, "  fix the tests  {Enter}");
    expect(onSubmit).toHaveBeenCalledWith("fix the tests");
    expect(box).toHaveValue("");
  });

  it("adds a newline on Shift+Enter instead of sending", async () => {
    const onSubmit = vi.fn();
    render(<Harness onSubmit={onSubmit} />);
    const box = screen.getByLabelText("Message");
    await userEvent.type(box, "line one{Shift>}{Enter}{/Shift}line two");
    expect(onSubmit).not.toHaveBeenCalled();
    expect(box).toHaveValue("line one\nline two");
  });

  it("won't send blank text, and the send button is disabled until there is text", async () => {
    const onSubmit = vi.fn();
    render(<Harness onSubmit={onSubmit} />);
    expect(screen.getByRole("button", { name: "Send" })).toBeDisabled();
    await userEvent.type(screen.getByLabelText("Message"), "   {Enter}");
    expect(onSubmit).not.toHaveBeenCalled();
    await userEvent.type(screen.getByLabelText("Message"), "ok");
    await userEvent.click(screen.getByRole("button", { name: "Send" }));
    expect(onSubmit).toHaveBeenCalledWith("ok");
  });

  it("closes on Escape", async () => {
    const onEscape = vi.fn();
    render(<Harness onSubmit={() => {}} onEscape={onEscape} />);
    await userEvent.type(screen.getByLabelText("Message"), "draft{Escape}");
    expect(onEscape).toHaveBeenCalled();
  });

  it("shows Stop instead of Send while running", async () => {
    const onStop = vi.fn();
    const onSubmit = vi.fn();
    render(<Harness onSubmit={onSubmit} running onStop={onStop} />);
    await userEvent.type(screen.getByLabelText("Message"), "more{Enter}");
    expect(onSubmit).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Stop" }));
    expect(onStop).toHaveBeenCalled();
  });
});
