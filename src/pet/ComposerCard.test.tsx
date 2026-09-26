import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import type { Snapshot } from "../shared/types";
import { fakeTransport } from "../test/fakeTransport";
import { makeSnapshot } from "../test/fixtures";
import { ComposerCard } from "./ComposerCard";

function Harness({ snap, onSend }: { snap: Snapshot; onSend: (project: string, text: string) => Promise<void> }) {
  const [draft, setDraft] = useState("");
  const [project, setProject] = useState<string | null>("C:\\code\\app");
  return (
    <ComposerCard
      snap={snap}
      project={project}
      draft={draft}
      onDraftChange={setDraft}
      onProjectChange={setProject}
      onSend={onSend}
      onClose={() => {}}
      onOpenThread={() => {}}
    />
  );
}

describe("ComposerCard", () => {
  it("sends to the selected project and clears the box", async () => {
    fakeTransport();
    const onSend = vi.fn(async () => {});
    render(<Harness snap={makeSnapshot()} onSend={onSend} />);
    await userEvent.type(screen.getByLabelText("Message"), "why is CI red?{Enter}");
    expect(onSend).toHaveBeenCalledWith("C:\\code\\app", "why is CI red?");
    expect(screen.getByLabelText("Message")).toHaveValue("");
  });

  it("shows the credits notice inline on the first send, then sends after confirming", async () => {
    const { commands } = fakeTransport();
    let seen = false;
    const onSend = vi.fn(async () => {
      if (!seen) throw "credits_notice";
    });
    render(<Harness snap={makeSnapshot()} onSend={onSend} />);
    await userEvent.type(screen.getByLabelText("Message"), "first ask{Enter}");

    const notice = await screen.findByRole("alertdialog", { name: "Before your first ask" });
    expect(notice).toHaveTextContent("never an API key");
    // The prompt is kept while the notice is up, and nothing else can be sent.
    expect(screen.getByLabelText("Message")).toHaveValue("first ask");
    expect(screen.getByRole("button", { name: "Send" })).toBeDisabled();

    seen = true;
    await userEvent.click(screen.getByRole("button", { name: "Got it, send" }));
    await waitFor(() => expect(onSend).toHaveBeenCalledTimes(2));
    expect(onSend).toHaveBeenLastCalledWith("C:\\code\\app", "first ask");
    expect(commands()).toEqual(["mark_credits_notice_seen"]);
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(screen.getByLabelText("Message")).toHaveValue("");
  });

  it("cancelling the notice keeps the draft and sends nothing", async () => {
    const { commands } = fakeTransport();
    const onSend = vi.fn(async () => {
      throw "credits_notice";
    });
    render(<Harness snap={makeSnapshot()} onSend={onSend} />);
    await userEvent.type(screen.getByLabelText("Message"), "hello{Enter}");
    await userEvent.click(await screen.findByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(screen.getByLabelText("Message")).toHaveValue("hello");
    expect(commands()).toEqual([]);
    expect(onSend).toHaveBeenCalledTimes(1);
  });

  it("shows other errors inline and keeps the draft", async () => {
    fakeTransport();
    const onSend = vi.fn(async () => {
      throw "Already working on this project.";
    });
    render(<Harness snap={makeSnapshot()} onSend={onSend} />);
    await userEvent.type(screen.getByLabelText("Message"), "again{Enter}");
    expect(await screen.findByRole("alert")).toHaveTextContent("Already working on this project.");
    expect(screen.getByLabelText("Message")).toHaveValue("again");
  });

  it("picks a recent project or adds a folder from the project chip", async () => {
    const { calls, transport } = fakeTransport();
    (transport.chooseFolder as ReturnType<typeof vi.fn>).mockResolvedValue("C:\\code\\new");
    render(<Harness snap={makeSnapshot()} onSend={vi.fn(async () => {})} />);

    await userEvent.click(screen.getByRole("button", { name: "Project: app" }));
    const items = screen.getAllByRole("menuitemradio").map((el) => el.textContent);
    expect(items[0]).toContain("app");
    expect(items[1]).toContain("api");
    await userEvent.click(screen.getByRole("menuitemradio", { name: /api/ }));
    expect(screen.getByRole("button", { name: "Project: api" })).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Project: api" }));
    await userEvent.click(screen.getByRole("menuitem", { name: "Choose folder…" }));
    await waitFor(() => expect(calls).toContainEqual(["add_project", { path: "C:\\code\\new" }]));
    expect(await screen.findByRole("button", { name: "Project: new" })).toBeInTheDocument();
  });

  it("changes the permission mode for the project", async () => {
    const { calls } = fakeTransport();
    render(<Harness snap={makeSnapshot()} onSend={vi.fn(async () => {})} />);
    await userEvent.click(screen.getByRole("button", { name: "Mode: Edit files" }));
    await userEvent.click(screen.getByRole("menuitemradio", { name: /Read only/ }));
    expect(calls).toEqual([["set_permission_mode", { project: "C:\\code\\app", mode: "read_only" }]]);
  });

  it("blocks sending while the project already has a run, and links to it", async () => {
    fakeTransport();
    const onSend = vi.fn(async () => {});
    render(<Harness snap={makeSnapshot({ running: ["C:\\code\\app"] })} onSend={onSend} />);
    expect(screen.getByText(/Working in/)).toBeInTheDocument();
    await userEvent.type(screen.getByLabelText("Message"), "more{Enter}");
    expect(onSend).not.toHaveBeenCalled();
  });
});
