import { memo, useEffect, useLayoutEffect, useRef, type ReactNode } from "react";
import ReactMarkdown, { type Components } from "react-markdown";
import rehypeHighlight from "rehype-highlight";
import { api } from "../shared/api";
import { IconAlert } from "../shared/icons";
import type { ChatTurn } from "../shared/types";

const components: Components = {
  // Never navigate the pet window; open links in the browser instead.
  a: ({ href, children }) => (
    <a
      href={href}
      onClick={(e) => {
        e.preventDefault();
        if (href && /^https?:/i.test(href)) void api.openUrl(href);
      }}
    >
      {children}
    </a>
  ),
  // No remote content in the webview (the CSP blocks it too); show what the image was.
  img: ({ alt }) => <span className="md-img-alt">{alt ? `[image: ${alt}]` : "[image]"}</span>,
};

const Markdown = memo(function Markdown({ text }: { text: string }) {
  return (
    <ReactMarkdown rehypePlugins={[rehypeHighlight]} components={components}>
      {text}
    </ReactMarkdown>
  );
});

interface Props {
  turns: ChatTurn[];
  loading: boolean;
  /** Shown as a live "working" line while a run is going and nothing is streaming yet. */
  activity: string | null;
  empty: ReactNode;
}

/** The scrolling conversation inside the thread view. */
export function Messages({ turns, loading, activity, empty }: Props) {
  const scroller = useRef<HTMLDivElement>(null);
  const pinned = useRef(true);

  useEffect(() => {
    const el = scroller.current;
    if (!el) return;
    const onScroll = () => {
      pinned.current = el.scrollHeight - el.scrollTop - el.clientHeight < 48;
    };
    el.addEventListener("scroll", onScroll, { passive: true });
    return () => el.removeEventListener("scroll", onScroll);
  }, []);

  useLayoutEffect(() => {
    const el = scroller.current;
    if (el && pinned.current) el.scrollTop = el.scrollHeight;
  }, [turns, activity]);

  const last = turns[turns.length - 1];
  const streaming = last?.role === "assistant" && last.pending;

  return (
    <div className="messages" ref={scroller} role="log" aria-live="polite" aria-busy={loading}>
      {loading && turns.length === 0 && (
        <div className="messages-skeleton" aria-label="Loading conversation">
          <span style={{ width: "46%" }} />
          <span style={{ width: "88%" }} />
          <span style={{ width: "72%" }} />
        </div>
      )}
      {!loading && turns.length === 0 && !activity && <div className="messages-empty">{empty}</div>}
      {turns.map((t, i) =>
        t.role === "note" ? (
          <p key={i} className="msg-note">
            <IconAlert size={13} />
            <span>{t.text}</span>
          </p>
        ) : t.role === "user" ? (
          <div key={i} className="msg-user">
            <div className="msg-user-bubble">{t.text}</div>
          </div>
        ) : (
          <div key={i} className={`msg-assistant md${t.pending ? " is-streaming" : ""}`}>
            {t.text ? <Markdown text={t.text} /> : <span className="typing" aria-label="Writing" />}
          </div>
        ),
      )}
      {activity && !streaming && (
        <div className="msg-activity">
          <span className="typing" aria-hidden="true" />
          <span>{activity}</span>
        </div>
      )}
    </div>
  );
}
