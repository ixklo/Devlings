import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Composer } from "./Composer";

describe("Composer", () => {
  it("sends trimmed text on Enter and clears", async () => {
    const onSend = vi.fn();
    render(<Composer running={false} onSend={onSend} onStop={() => {}} />);
    const box = screen.getByLabelText("Message");
    await userEvent.type(box, "  fix the tests  {Enter}");
    expect(onSend).toHaveBeenCalledWith("fix the tests");
    expect(box).toHaveValue("");
  });

  it("adds a newline on Shift+Enter instead of sending", async () => {
    const onSend = vi.fn();
    render(<Composer running={false} onSend={onSend} onStop={() => {}} />);
    const box = screen.getByLabelText("Message");
    await userEvent.type(box, "line one{Shift>}{Enter}{/Shift}line two");
    expect(onSend).not.toHaveBeenCalled();
    expect(box).toHaveValue("line one\nline two");
  });

  it("won't send blank text, and shows Stop while running", async () => {
    const onSend = vi.fn();
    const onStop = vi.fn();
    const { rerender } = render(<Composer running={false} onSend={onSend} onStop={onStop} />);
    await userEvent.type(screen.getByLabelText("Message"), "   {Enter}");
    expect(onSend).not.toHaveBeenCalled();
    rerender(<Composer running={true} onSend={onSend} onStop={onStop} />);
    await userEvent.click(screen.getByRole("button", { name: "Stop" }));
    expect(onStop).toHaveBeenCalled();
  });
});
