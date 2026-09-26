import { useEffect, useRef } from "react";
import ReactMarkdown from "react-markdown";
import rehypeHighlight from "rehype-highlight";
import "highlight.js/styles/github-dark.css";
import type { ChatTurn } from "../shared/types";

export function Conversation({ turns, petName }: { turns: ChatTurn[]; petName: string }) {
  const end = useRef<HTMLDivElement>(null);
  useEffect(() => {
    end.current?.scrollIntoView?.({ block: "end" });
  }, [turns]);
  return (
    <div className="conversation">
      {turns.map((t, i) =>
        t.role === "note" ? (
          <p key={i} className="note">
            {t.text}
          </p>
        ) : (
          <div key={i} className={`turn ${t.role}`}>
            <div className="who">{t.role === "user" ? "You" : petName}</div>
            <ReactMarkdown rehypePlugins={[rehypeHighlight]}>{t.text || (t.pending ? "…" : "")}</ReactMarkdown>
          </div>
        ),
      )}
      <div ref={end} />
    </div>
  );
}
