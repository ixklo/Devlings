import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { fakeTransport } from "../test/fakeTransport";
import { Messages } from "./Messages";

function show(text: string) {
  return render(<Messages turns={[{ role: "assistant", text }]} loading={false} activity={null} empty={null} />);
}

describe("Messages Markdown", () => {
  it("never loads remote images; shows their alt text instead", () => {
    fakeTransport();
    const { container } = show("Look: ![a chart of results](https://example.com/x.png)");
    expect(container.querySelector("img")).toBeNull();
    expect(screen.getByText("[image: a chart of results]")).toBeInTheDocument();
  });

  it("opens web links in the browser and ignores other schemes", () => {
    const { transport } = fakeTransport();
    show("[docs](https://example.com/docs) and [local](file:///etc/passwd)");
    fireEvent.click(screen.getByText("docs"));
    expect(transport.openUrl).toHaveBeenCalledWith("https://example.com/docs");
    fireEvent.click(screen.getByText("local"));
    expect(transport.openUrl).toHaveBeenCalledTimes(1);
  });
});
