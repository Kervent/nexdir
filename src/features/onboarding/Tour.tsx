import { useCallback, useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import { ArrowLeft, ArrowRight, PartyPopper, Sparkles } from "lucide-react";
import { Dialog } from "../../components/Dialog";
import logo from "../../../assets/branding/png/nexdir-logo-128.png";
import type { TourStep } from "./onboarding";

const MARGIN = 12;
const GAP = 14;
const PAD = 6;

interface Box {
  top: number;
  left: number;
  width: number;
  height: number;
}

/** Union of all visible elements marked `data-tour="name"`, clipped to the window. */
export function measureTarget(name: string): Box | null {
  const vw = window.innerWidth;
  const vh = window.innerHeight;
  let top = Infinity;
  let left = Infinity;
  let right = -Infinity;
  let bottom = -Infinity;
  for (const el of document.querySelectorAll<HTMLElement>(`[data-tour~="${name}"]`)) {
    if (el.closest("[hidden]")) continue;
    const r = el.getBoundingClientRect();
    if (r.width <= 0 || r.height <= 0) continue;
    top = Math.min(top, r.top);
    left = Math.min(left, r.left);
    right = Math.max(right, r.right);
    bottom = Math.max(bottom, r.bottom);
  }
  if (top === Infinity) return null;
  // Keep highlights inside the window, even for elements partly scrolled away.
  top = Math.max(top - PAD, 2);
  left = Math.max(left - PAD, 2);
  right = Math.min(right + PAD, vw - 2);
  bottom = Math.min(bottom + PAD, vh - 2);
  if (right <= left || bottom <= top) return null;
  return { top, left, width: right - left, height: bottom - top };
}

/** Places a `w`×`h` card next to `anchor` (right, below, left, above), always fully inside the window. */
export function placeCard(anchor: Box | null, w: number, h: number, vw: number, vh: number): { top: number; left: number } {
  const clampX = (x: number) => Math.max(MARGIN, Math.min(x, vw - w - MARGIN));
  const clampY = (y: number) => Math.max(MARGIN, Math.min(y, vh - h - MARGIN));
  if (!anchor) return { left: clampX((vw - w) / 2), top: clampY((vh - h) / 2) };
  const right = anchor.left + anchor.width;
  const bottom = anchor.top + anchor.height;
  if (right + GAP + w <= vw - MARGIN) return { left: right + GAP, top: clampY(anchor.top) };
  if (bottom + GAP + h <= vh - MARGIN) return { left: clampX(anchor.left), top: bottom + GAP };
  if (anchor.left - GAP - w >= MARGIN) return { left: anchor.left - GAP - w, top: clampY(anchor.top) };
  if (anchor.top - GAP - h >= MARGIN) return { left: clampX(anchor.left), top: anchor.top - GAP - h };
  // No room beside the highlight (small window): overlap it rather than leave the window.
  return { left: clampX(vw - w - MARGIN), top: clampY(vh - h - MARGIN) };
}

export function WelcomeDialog({ onStart, onSkip }: { onStart: () => void; onSkip: () => void }) {
  return (
    <Dialog
      title="Welcome to Nexdir"
      onClose={onSkip}
      initialFocus=".is-primary"
      footer={
        <>
          <button type="button" className="nx-button" onClick={onSkip}>
            Skip for Now
          </button>
          <button type="button" className="nx-button is-primary" onClick={onStart}>
            <Sparkles size={14} /> Start Tutorial
          </button>
        </>
      }
    >
      <div className="nx-about">
        <img src={logo} width={56} height={56} alt="" />
        <div>
          <p className="nx-about-name">A smarter way to manage your files.</p>
        </div>
      </div>
      <p>Welcome! Would you like a quick introduction to the most important features? It takes about a minute, and you can stop at any time.</p>
      <p className="nx-muted">You can start the tour later from Help → Take a Tour.</p>
    </Dialog>
  );
}

interface TourProps {
  steps: TourStep[];
  /** `completed` is false when the tour was skipped or closed early. */
  onFinish: (completed: boolean) => void;
}

/**
 * Guided tour over the real interface. Each step highlights the elements named in
 * `targets`; steps whose elements are missing or hidden show their text in the middle
 * of the window instead. Keyboard: ←/→ to move, Esc to leave, Tab stays inside the card.
 */
export function Tour({ steps, onFinish }: TourProps) {
  const [index, setIndex] = useState(0);
  const [boxes, setBoxes] = useState<Box[]>([]);
  const [viewport, setViewport] = useState({ w: window.innerWidth, h: window.innerHeight });
  const [pos, setPos] = useState<{ top: number; left: number } | null>(null);
  const cardRef = useRef<HTMLDivElement>(null);
  const primaryRef = useRef<HTMLButtonElement>(null);
  const titleId = useId();
  const bodyId = useId();
  const final = index >= steps.length;
  const step = final ? null : steps[index]!;
  const total = steps.length;

  const measure = useCallback(() => {
    setViewport({ w: window.innerWidth, h: window.innerHeight });
    setBoxes(step ? step.targets.map(measureTarget).filter((b): b is Box => b !== null) : []);
  }, [step]);

  // Bring the step's elements into view, then measure; re-measure whenever layout may change.
  useLayoutEffect(() => {
    if (step) {
      for (const t of step.targets) document.querySelector<HTMLElement>(`[data-tour~="${t}"]`)?.scrollIntoView?.({ block: "nearest", inline: "nearest" });
    }
    measure();
    const settle = window.setTimeout(measure, 120);
    window.addEventListener("resize", measure);
    window.addEventListener("scroll", measure, true);
    const observer = new ResizeObserver(() => measure());
    observer.observe(document.body);
    return () => {
      window.clearTimeout(settle);
      window.removeEventListener("resize", measure);
      window.removeEventListener("scroll", measure, true);
      observer.disconnect();
    };
  }, [measure, step]);

  const anchor = boxes[0] ?? null;
  useLayoutEffect(() => {
    const card = cardRef.current;
    if (!card) return;
    const { width, height } = card.getBoundingClientRect();
    setPos(placeCard(anchor, width, height, viewport.w, viewport.h));
  }, [anchor?.top, anchor?.left, anchor?.width, anchor?.height, viewport.w, viewport.h, index]);

  // The card is invisible until it has been placed, and invisible elements cannot take focus.
  const placed = pos !== null;
  useEffect(() => {
    if (placed) primaryRef.current?.focus();
  }, [index, placed]);

  const next = useCallback(() => (final ? onFinish(true) : setIndex((i) => i + 1)), [final, onFinish]);
  const back = useCallback(() => setIndex((i) => Math.max(0, i - 1)), []);

  // Esc and ←/→ work wherever focus is (e.g. after a click on the dimmed area), and never
  // reach the file list or the app's shortcuts underneath.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const action = { Escape: () => onFinish(false), ArrowRight: next, ArrowLeft: back }[e.key];
      if (!action) return;
      e.preventDefault();
      e.stopPropagation();
      action();
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [next, back, onFinish]);

  const onKeyDown = (e: React.KeyboardEvent) => {
    e.stopPropagation();
    if (e.key === "Tab" && cardRef.current) {
      const focusable = Array.from(cardRef.current.querySelectorAll<HTMLElement>("button:not(:disabled)"));
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last?.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first?.focus();
      }
    }
  };

  const maskId = `${titleId}-mask`;
  return (
    <div className="nx-tour" onKeyDown={onKeyDown}>
      <svg className="nx-tour-shade" width={viewport.w} height={viewport.h} aria-hidden="true">
        <defs>
          <mask id={maskId}>
            <rect width="100%" height="100%" fill="white" />
            {boxes.map((b, i) => (
              <rect key={i} x={b.left} y={b.top} width={b.width} height={b.height} rx={8} fill="black" />
            ))}
          </mask>
        </defs>
        <rect width="100%" height="100%" className="nx-tour-dim" mask={`url(#${maskId})`} />
        {boxes.map((b, i) => (
          <rect key={i} x={b.left} y={b.top} width={b.width} height={b.height} rx={8} className="nx-tour-ring" />
        ))}
      </svg>

      <div
        ref={cardRef}
        className={`nx-tour-card${final ? " is-final" : ""}`}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={bodyId}
        style={pos ? { top: pos.top, left: pos.left } : { visibility: "hidden" }}
      >
        {step ? (
          <>
            <p className="nx-tour-progress">
              Step {index + 1} of {total}
            </p>
            <h2 id={titleId} className="nx-tour-title">
              {step.title}
            </h2>
            <div id={bodyId} className="nx-tour-body">
              {step.body.map((p) => (
                <p key={p}>{p}</p>
              ))}
              {step.points && (
                <ul>
                  {step.points.map((p) => (
                    <li key={p}>{p}</li>
                  ))}
                </ul>
              )}
              {boxes.length === 0 && <p className="nx-muted">This part of the window is not visible right now — make the window larger to see it highlighted.</p>}
            </div>
          </>
        ) : (
          <div className="nx-tour-done">
            <PartyPopper size={32} aria-hidden="true" />
            <h2 id={titleId} className="nx-tour-title">
              You're ready to explore Nexdir!
            </h2>
            <p id={bodyId}>Take the tour again any time from Help → Take a Tour.</p>
          </div>
        )}
        <div className="nx-tour-dots" role="progressbar" aria-label="Tour progress" aria-valuemin={1} aria-valuemax={total + 1} aria-valuenow={index + 1}>
          {[...steps, null].map((_, i) => (
            <span key={i} className={i === index ? "is-current" : i < index ? "is-done" : ""} />
          ))}
        </div>
        <div className="nx-tour-actions">
          {!final && (
            <button type="button" className="nx-link-button" onClick={() => onFinish(false)}>
              Skip tour
            </button>
          )}
          <span className="nx-spacer" />
          <button type="button" className="nx-button" onClick={back} disabled={index === 0}>
            <ArrowLeft size={14} /> Back
          </button>
          <button ref={primaryRef} type="button" className="nx-button is-primary" onClick={next}>
            {final ? (
              "Get Started"
            ) : (
              <>
                {index === total - 1 ? "Finish" : "Next"} <ArrowRight size={14} />
              </>
            )}
          </button>
        </div>
        <p className="nx-sr-only" aria-live="polite">
          {step ? `Step ${index + 1} of ${total}: ${step.title}` : "Tour complete"}
        </p>
      </div>
    </div>
  );
}
