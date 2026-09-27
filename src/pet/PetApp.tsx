import { useCallback, useEffect, useReducer, useRef, useState, type CSSProperties } from "react";
import { api } from "../shared/api";
import { threadForOpen } from "../shared/threads";
import { useNow } from "../shared/time";
import type { Placement, PetState, ProjectEntry, Snapshot, ThreadInfo } from "../shared/types";
import { useSnapshot } from "../shared/useSnapshot";
import {
  animReducer,
  becameReady,
  DRAG_SETTLE_MS,
  dragDirection,
  initialAnim,
  resolveClip,
} from "../sprite/petAnimation";
import { usePetSprite, useReducedMotion } from "../sprite/SpriteView";
import { useAnnouncements } from "./announce";
import { showApprovalsIntro } from "./ApprovalCard";
import { BubbleStack } from "./BubbleStack";
import { ComposerCard } from "./ComposerCard";
import { ControlBar } from "./ControlBar";
import { hiddenCount } from "./CountPill";
import { PetSprite } from "./PetSprite";
import { activateThread } from "./ThreadCard";
import { ThreadView } from "./ThreadView";
import { visibleUpdate } from "./UpdateCard";
import { useConversation } from "./useConversation";
import { useHitRegions } from "./useHitRegions";
import "./pet.css";

type View =
  | { kind: "bubbles" }
  | { kind: "compose" }
  | { kind: "thread"; project: string; sessionId: string | null; initialPrompt?: string };

const BUBBLES: View = { kind: "bubbles" };
const HOVER_GRACE_MS = 350;
const ARROWS: Record<string, [number, number]> = {
  ArrowLeft: [-20, 0],
  ArrowRight: [20, 0],
  ArrowUp: [0, -20],
  ArrowDown: [0, 20],
};

const STATE_LABEL: Record<PetState, string> = {
  idle: "idle",
  running: "working",
  needs_input: "needs you",
  ready: "done",
  blocked: "something went wrong",
  setup: "needs setup",
};

function mostRecent(projects: ProjectEntry[]): ProjectEntry | undefined {
  return projects.reduce<ProjectEntry | undefined>((best, p) => (!best || p.lastSeen > best.lastSeen ? p : best), undefined);
}

/** A pending permission request makes the pet wait, like a thread that needs input (setup still wins). */
export function petStateFor(snap: Snapshot): PetState {
  return snap.approvals.length > 0 && snap.petState !== "setup" ? "needs_input" : snap.petState;
}

function setupDetail(snap: Snapshot): string {
  const s = snap.setup;
  if (s.claudeError) return s.claudeError;
  if (s.auth?.status === "refused") return s.auth.reason;
  if (s.hookServerError) return s.hookServerError;
  return "A couple of things need a look.";
}

/** The transparent overlay window: bubbles or a card above the pet, control bar below. */
export function PetApp() {
  const snap = useSnapshot();
  const conversation = useConversation();
  const reduced = useReducedMotion();
  const now = useNow();
  const [view, setView] = useState<View>(BUBBLES);
  const [project, setProject] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  const [expanded, setExpanded] = useState(false);
  const [hover, setHover] = useState(false);
  // "Later" on the update card: hidden until the next launch or a newer version.
  const [laterUpdate, setLaterUpdate] = useState<string | null>(null);
  // How `.stage` lays out for the sprite's current on-screen spot (design D9). `stageRoom:
  // Infinity` until the backend's first placement arrives, so nothing is clamped before then.
  const [placement, setPlacement] = useState<Placement>({ cardsBelow: false, shiftX: 0, stageRoom: Infinity });
  const [anim, dispatch] = useReducer(animReducer, initialAnim);
  const src = usePetSprite(snap ? snap.config.petId : null);
  const announcement = useAnnouncements(snap);
  const mainRef = useRef<HTMLElement>(null);

  const snapRef = useRef(snap);
  snapRef.current = snap;
  const viewRef = useRef(view);
  viewRef.current = view;

  const refreshHits = useHitRegions((regions) => {
    api.setHitRegions(regions).catch(() => {});
  });

  // Composer defaults to the most recently used project.
  useEffect(() => {
    if (snap && !project) {
      const p = mostRecent(snap.projects);
      if (p) setProject(p.path);
    }
  }, [snap, project]);

  // One jump when a thread becomes ready, then the review loop.
  const prevThreads = useRef<ThreadInfo[] | null>(null);
  useEffect(() => {
    if (!snap) return;
    if (!reduced && becameReady(prevThreads.current, snap.threads)) dispatch({ type: "ready" });
    prevThreads.current = snap.threads;
  }, [snap, reduced]);

  // Window moves: run left/right while dragging, and remember where the pet sits.
  useEffect(() => {
    let prev: { x: number; y: number } | null = null;
    let settle: number | undefined;
    let save: number | undefined;
    const unlisten = api.onMoved((pos) => {
      if (prev) {
        const dir = dragDirection(pos.x - prev.x);
        if (dir) dispatch({ type: "drag", dir });
      }
      prev = pos;
      window.clearTimeout(settle);
      settle = window.setTimeout(() => dispatch({ type: "dragEnd" }), DRAG_SETTLE_MS);
      window.clearTimeout(save);
      save = window.setTimeout(() => api.savePetPosition(pos.x, pos.y).catch(() => {}), 400);
    });
    return () => {
      unlisten.then((f) => f());
      window.clearTimeout(settle);
      window.clearTimeout(save);
    };
  }, []);

  // The backend flips/shifts `.stage` as the sprite's on-screen spot changes (design D9).
  // Its first layout is sent at startup, before this page listens, so ask for the current one too; a live
  // event that arrives first wins.
  useEffect(() => {
    let live = false;
    const unlisten = api.onPetPlacement((p) => {
      live = true;
      setPlacement(p);
    });
    unlisten
      .then(() => api.getPetPlacement())
      .then((p) => {
        if (p && !live) setPlacement(p);
      })
      .catch(() => {});
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  const openThread = useCallback(
    (path: string, sessionId: string | null = null) => {
      conversation.open(path);
      setProject(path);
      setView({ kind: "thread", project: path, sessionId });
    },
    [conversation.open],
  );

  // The tray, the shortcut and notification clicks ask the pet to open a view. A thread opens exactly as a click on
  // its card would: an Ask thread's mini chat, or a Watch thread's project.
  useEffect(() => {
    const unlisten = api.onPetOpen((o) => {
      if (o.view === "compose") {
        setView({ kind: "compose" });
        return;
      }
      const thread = threadForOpen(o, snapRef.current);
      if (thread) void activateThread(thread, (t) => openThread(t.project, t.sessionId)).catch(() => {});
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, [openThread]);

  // Keyboard: Esc closes the open card (or re-homes the pet), arrows nudge the window.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      // Inside a card, keys belong to the card (carets, scrolling, menus).
      const typing = !!target?.closest?.("textarea, input, .card, [role='menu'], [aria-haspopup]");
      if (e.key === "Escape") {
        if (viewRef.current.kind !== "bubbles") setView(BUBBLES);
        else if (!typing) api.resetPetPosition().catch(() => {});
        return;
      }
      const delta = ARROWS[e.key];
      if (delta && !typing) {
        e.preventDefault();
        api.movePetBy(delta[0], delta[1]).catch(() => {});
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const hoverTimer = useRef<number | undefined>(undefined);
  const dragSafety = useRef<number | undefined>(undefined);
  const enterDock = () => {
    window.clearTimeout(hoverTimer.current);
    setHover(true);
  };
  const leaveDock = () => {
    window.clearTimeout(hoverTimer.current);
    hoverTimer.current = window.setTimeout(() => setHover(false), HOVER_GRACE_MS);
  };

  // Hover as the backend's cursor poll sees it. The webview's own enter/leave events go stale,
  // because once the window turns click-through it never hears the pointer leave.
  const reducedRef = useRef(reduced);
  reducedRef.current = reduced;
  useEffect(() => {
    let last: string | null = null;
    const unlisten = api.onPetPointer((id) => {
      if (id === "pet" && last !== "pet" && !reducedRef.current) dispatch({ type: "hover", now: performance.now() });
      if (id === "pet" || id === "bar") enterDock();
      else if (last === "pet" || last === "bar") leaveDock();
      last = id;
    });
    return () => {
      unlisten.then((f) => f());
    };
    // enterDock/leaveDock only touch a ref and a state setter, so the first render's copies are fine.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    refreshHits();
  }, [view, expanded, hover, snap, src, laterUpdate, refreshHits]);

  // A card that held focus just closed (Esc, Close, Send's chat closing): give focus back to the pet
  // instead of dropping it on the page, so the keyboard carries on from where the card was opened.
  const prevKind = useRef(view.kind);
  useEffect(() => {
    const was = prevKind.current;
    prevKind.current = view.kind;
    if (was === "bubbles" || view.kind !== "bubbles") return;
    const active = document.activeElement;
    if (!active || active === document.body) {
      mainRef.current?.querySelector<HTMLElement>(".pet")?.focus({ preventScroll: true });
    }
  }, [view.kind]);

  const onClipDone = useCallback(() => dispatch({ type: "clipDone" }), []);

  if (!snap) return null;

  const { config } = snap;
  const collapsed = config.threadsCollapsed;
  const threads = snap.threads;
  const barVisible = hover || view.kind !== "bubbles" || threads.length > 0;
  const petState = petStateFor(snap);
  // Approval buttons re-arm whenever the stage moves as a whole.
  const layoutKey = `${placement.cardsBelow}|${placement.shiftX}|${placement.stageRoom}`;
  const clip = resolveClip(petState, anim, reduced);
  const toggleComposer = () => setView((v) => (v.kind === "bubbles" ? { kind: "compose" } : BUBBLES));
  const closeView = () => setView(BUBBLES);

  // Switch to the thread right away and let it send, so the chat appears instantly
  // and closing it while Claude Code starts up isn't undone when the send resolves.
  const sendNew = async (path: string, text: string) => {
    conversation.open(path);
    setView({ kind: "thread", project: path, sessionId: null, initialPrompt: text });
  };

  const openSetup = () => api.openSettings(config.onboarded ? "settings" : "onboarding").catch(() => {});
  // The count pill: the same as the chevron's "Show threads". It leaves once the threads show, so a
  // pill that held focus hands it to the pet first, as a closing card does.
  const showThreads = (hadFocus: boolean) => {
    if (hadFocus) mainRef.current?.querySelector<HTMLElement>(".pet")?.focus({ preventScroll: true });
    setExpanded(false);
    api.setThreadsCollapsed(false).catch(() => {});
  };
  const hidden = collapsed ? hiddenCount(threads, snap.approvals) : null;
  const updateVersion = visibleUpdate(snap.update, snap.running, laterUpdate);

  return (
    <main ref={mainRef} className="overlay" data-pet-state={petState} data-cards-below={placement.cardsBelow}>
      <div
        className="stage"
        style={
          {
            transform: placement.shiftX ? `translateX(${placement.shiftX}px)` : undefined,
            "--stage-room": Number.isFinite(placement.stageRoom) ? `${placement.stageRoom}px` : undefined,
          } as CSSProperties
        }
      >
        {view.kind === "compose" && (
          <ComposerCard
            snap={snap}
            project={project}
            draft={draft}
            onDraftChange={setDraft}
            onProjectChange={setProject}
            onSend={sendNew}
            onClose={closeView}
            onOpenThread={(p) => openThread(p)}
          />
        )}
        {view.kind === "thread" && (
          <ThreadView
            key={view.project}
            snap={snap}
            project={view.project}
            initialSessionId={view.sessionId}
            initialPrompt={view.initialPrompt}
            conversation={conversation}
            onClose={closeView}
            layoutKey={layoutKey}
          />
        )}
        {view.kind === "bubbles" && (
          <BubbleStack
            threads={collapsed ? [] : threads}
            now={now}
            expanded={expanded}
            onToggleExpanded={() => setExpanded((x) => !x)}
            onOpenThread={(t) => openThread(t.project, t.sessionId)}
            setup={snap.setup.needsSetup ? { detail: setupDetail(snap), onOpen: openSetup } : null}
            update={
              updateVersion
                ? { version: updateVersion, onRestart: api.installUpdate, onLater: () => setLaterUpdate(updateVersion) }
                : null
            }
            approvals={snap.approvals}
            approvalsHidden={collapsed}
            holdMs={config.approvalHoldSecs * 1000}
            layoutKey={layoutKey}
            intro={!collapsed && showApprovalsIntro(snap) ? { petName: config.petName } : null}
            folded={hidden ? { hidden, onShow: showThreads } : null}
          />
        )}
      </div>
      <div className="dock" onPointerEnter={enterDock} onPointerLeave={leaveDock} onFocus={enterDock} onBlur={leaveDock}>
        <PetSprite
          src={src}
          scale={config.petScale || 0.6}
          clip={clip}
          onClipDone={onClipDone}
          label={`${config.petName}, ${STATE_LABEL[petState]}. Click to ask, drag to move.`}
          describedBy="pet-keys"
          onActivate={toggleComposer}
          onHover={() => {
            if (!reduced) dispatch({ type: "hover", now: performance.now() });
          }}
          onDragStart={(dir) => {
            dispatch({ type: "drag", dir });
            // In case neither a buttonless move nor the post-drag move events ever arrive.
            window.clearTimeout(dragSafety.current);
            dragSafety.current = window.setTimeout(() => dispatch({ type: "dragEnd" }), 15_000);
          }}
          onDragEnd={() => {
            window.clearTimeout(dragSafety.current);
            dispatch({ type: "dragEnd" });
          }}
        />
        <ControlBar
          visible={barVisible}
          composerOpen={view.kind === "compose"}
          notifications={config.notifications}
          collapsed={collapsed}
          onCompose={toggleComposer}
          onToggleNotifications={() => api.setNotifications(!config.notifications).catch(() => {})}
          onToggleCollapsed={() => {
            setExpanded(false);
            if (view.kind !== "bubbles") {
              // From a card, the chevron goes back to the threads.
              setView(BUBBLES);
              if (collapsed) api.setThreadsCollapsed(false).catch(() => {});
              return;
            }
            api.setThreadsCollapsed(!collapsed).catch(() => {});
          }}
        />
        <span id="pet-keys" className="sr-only">
          Press Enter to ask. Arrow keys move {config.petName}, Escape sends it home, and Shift+F10 opens its menu.
        </span>
      </div>
      {/* The one live region: status changes, once each (announce.ts). Never the streaming reply. */}
      <div className="sr-only" role="status" aria-live="polite" aria-atomic="true">
        {announcement}
      </div>
    </main>
  );
}
