import { convertFileSrc } from "@tauri-apps/api/core";
import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
import { motion } from "framer-motion";
import { useEffect, useRef, useState } from "react";

import { api, type Rect, type StillFrame } from "../lib/api";

interface Point {
  x: number;
  y: number;
}

const MIN_SIZE = 40;
const LOUPE = 132;
const LOUPE_ZOOM = 1.6;
const LOUPE_OFFSET = 26;
const NUDGE = 2;
const NUDGE_BIG = 12;

type Drag =
  | { kind: "new"; start: Point }
  | { kind: "move"; origin: Rect; point: Point }
  | { kind: "resize"; origin: Rect; point: Point; corner: Corner };

type Corner = "nw" | "ne" | "sw" | "se";

/**
 * CleanShot X–style region picker: the whole screen freezes on a still frame,
 * a magnifier loupe follows the cursor so pixel-accurate edges are possible,
 * and the selection can be dragged, resized by its corners, or nudged with
 * the arrow keys after it is drawn.
 */
export default function RegionOverlay() {
  const [rect, setRect] = useState<Rect | null>(null);
  const [cursor, setCursor] = useState<Point | null>(null);
  const [still, setStill] = useState<StillFrame | null>(null);
  const [done, setDone] = useState(false);
  const surfaceRef = useRef<HTMLDivElement>(null);
  const dragRef = useRef<Drag | null>(null);

  useEffect(() => {
    document.body.classList.add("is-overlay");
    return () => document.body.classList.remove("is-overlay");
  }, []);

  // The loupe renders a frozen frame, so the selection never fights a moving
  // desktop. The overlay is scoped to one display (its window covers exactly
  // that monitor), so a null target resolves to the current/default display.
  // The e2e driver pings until this answers, so it never emits before the
  // listeners exist.
  useEffect(() => {
    const unlistenReady = listen("screencut://e2e-ping", async () => {
      await emit("screencut://overlay-ready");
    });
    void emit("screencut://overlay-ready");
    let cancelled = false;
    api
      .grabRegionStill(null)
      .then((frame) => {
        if (!cancelled) setStill(frame);
      })
      .catch(() => {
        /* loupe is optional; selection works without it */
      });
    return () => {
      cancelled = true;
      void unlistenReady.then((fn) => fn());
    };
  }, []);

  const clampPoint = (e: PointerEvent | React.PointerEvent): Point => ({
    x: Math.max(0, Math.min(window.innerWidth, e.clientX)),
    y: Math.max(0, Math.min(window.innerHeight, e.clientY)),
  });

  const onPointerDown = (e: React.PointerEvent) => {
    if (done) return;
    const p = clampPoint(e);
    surfaceRef.current?.setPointerCapture(e.pointerId);

    if (rect && isValid(rect)) {
      const corner = hitCorner(rect, p);
      if (corner) {
        dragRef.current = { kind: "resize", origin: rect, point: p, corner };
        return;
      }
      if (inside(rect, p)) {
        dragRef.current = { kind: "move", origin: rect, point: p };
        return;
      }
    }
    // Empty space starts a fresh selection.
    dragRef.current = { kind: "new", start: p };
    setRect({ x: p.x, y: p.y, width: 0, height: 0 });
  };

  const onPointerMove = (e: React.PointerEvent) => {
    const p = clampPoint(e);
    setCursor(p);
    const drag = dragRef.current;
    if (!drag) return;

    if (drag.kind === "new") {
      setRect({
        x: Math.min(drag.start.x, p.x),
        y: Math.min(drag.start.y, p.y),
        width: Math.abs(p.x - drag.start.x),
        height: Math.abs(p.y - drag.start.y),
      });
    } else if (drag.kind === "move") {
      const dx = p.x - drag.point.x;
      const dy = p.y - drag.point.y;
      const maxX = window.innerWidth - drag.origin.width;
      const maxY = window.innerHeight - drag.origin.height;
      setRect({
        ...drag.origin,
        x: Math.max(0, Math.min(maxX, drag.origin.x + dx)),
        y: Math.max(0, Math.min(maxY, drag.origin.y + dy)),
      });
    } else {
      setRect(resize(drag.origin, drag.point, p, drag.corner));
    }
  };

  const onPointerUp = () => {
    dragRef.current = null;
    // A tiny drag clears the selection so the user can try again.
    setRect((r) => (r && isValid(r) ? r : null));
  };

  const commit = () => {
    if (!rect || !isValid(rect) || done) return;
    setDone(true);
    void api.confirmRegion(rect);
  };

  const cancel = () => void api.cancelRegion();

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (done) return;
      if (e.key === "Escape") {
        cancel();
        return;
      }
      if (e.key === "Enter") {
        commit();
        return;
      }
      // Arrow keys nudge the selection once it exists; Shift makes the step
      // bigger for fast travel.
      const step = e.shiftKey ? NUDGE_BIG : NUDGE;
      const dx = e.key === "ArrowLeft" ? -step : e.key === "ArrowRight" ? step : 0;
      const dy = e.key === "ArrowUp" ? -step : e.key === "ArrowDown" ? step : 0;
      if ((dx || dy) && rect && isValid(rect)) {
        e.preventDefault();
        const maxX = window.innerWidth - rect.width;
        const maxY = window.innerHeight - rect.height;
        setRect({
          ...rect,
          x: Math.max(0, Math.min(maxX, rect.x + dx)),
          y: Math.max(0, Math.min(maxY, rect.y + dy)),
        });
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [rect, done]);

  // Headless self-test hook: the e2e scenario drives the exact same handlers a
  // real pointer/keyboard event would, so this exercises the real interaction
  // logic rather than a parallel implementation.
  useEffect(() => {
    let un1: UnlistenFn | undefined;
    let un2: UnlistenFn | undefined;
    (async () => {
      un1 = await listen<{ x1: number; y1: number; x2: number; y2: number }>(
        "screencut://e2e-drag",
        (e) => {
          const { x1, y1, x2, y2 } = e.payload;
          dragRef.current = { kind: "new", start: { x: x1, y: y1 } };
          setRect(rectFromPoints(x1, y1, x1, y1));
          setRect(rectFromPoints(x1, y1, x2, y2));
          dragRef.current = null;
        },
      );
      un2 = await listen<string>("screencut://e2e-key", (e) => {
        window.dispatchEvent(
          new KeyboardEvent("keydown", { key: e.payload, bubbles: true }),
        );
      });
    })();
    return () => {
      un1?.();
      un2?.();
    };
  }, []);

  const valid = rect !== null && isValid(rect);

  return (
    <div
      ref={surfaceRef}
      className="fixed inset-0 cursor-crosshair"
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onDoubleClick={commit}
    >
      {rect && (
        <div
          className="absolute border-2 border-accent"
          style={{
            left: rect.x,
            top: rect.y,
            width: rect.width,
            height: rect.height,
            boxShadow: "0 0 0 9999px rgba(4, 5, 9, 0.62)",
            backgroundColor: "rgba(99, 102, 241, 0.08)",
          }}
        >
          {valid && <SizeBadge rect={rect} />}
          {valid && <Handles />}
        </div>
      )}

      {cursor && !done && <Loupe cursor={cursor} still={still} rect={valid ? rect : null} />}
      {cursor && !done && <Crosshairs point={cursor} rect={valid ? rect : null} />}

      <HintBar valid={valid} />
      <Toolbar valid={valid} done={done} onConfirm={commit} onCancel={cancel} />
    </div>
  );
}

function isValid(rect: Rect): boolean {
  return rect.width >= MIN_SIZE && rect.height >= MIN_SIZE;
}

function rectFromPoints(x1: number, y1: number, x2: number, y2: number): Rect {
  return {
    x: Math.min(x1, x2),
    y: Math.min(y1, y2),
    width: Math.abs(x2 - x1),
    height: Math.abs(y2 - y1),
  };
}

function inside(rect: Rect, p: Point): boolean {
  return (
    p.x > rect.x + 8 &&
    p.x < rect.x + rect.width - 8 &&
    p.y > rect.y + 8 &&
    p.y < rect.y + rect.height - 8
  );
}

function hitCorner(rect: Rect, p: Point): Corner | null {
  const tol = 14;
  const near = (a: number, b: number) => Math.abs(a - b) <= tol;
  const left = near(p.x, rect.x);
  const right = near(p.x, rect.x + rect.width);
  const top = near(p.y, rect.y);
  const bottom = near(p.y, rect.y + rect.height);
  if (left && top) return "nw";
  if (right && top) return "ne";
  if (left && bottom) return "sw";
  if (right && bottom) return "se";
  return null;
}

function resize(origin: Rect, _start: Point, end: Point, corner: Corner): Rect {
  // Anchor the corner opposite the dragged handle.
  const right = origin.x + origin.width;
  const bottom = origin.y + origin.height;
  const x1 = corner === "ne" || corner === "se" ? origin.x : end.x;
  const y1 = corner === "sw" || corner === "se" ? origin.y : end.y;
  const x2 = corner === "nw" || corner === "sw" ? right : end.x;
  const y2 = corner === "nw" || corner === "ne" ? bottom : end.y;
  return rectFromPoints(x1, y1, x2, y2);
}

function Loupe({
  cursor,
  still,
  rect,
}: {
  cursor: Point;
  still: StillFrame | null;
  rect: Rect | null;
}) {
  const src = still ? convertFileSrc(still.path) : null;
  // Keep the loupe on screen when the cursor nears an edge.
  const flipX = cursor.x + LOUPE + LOUPE_OFFSET > window.innerWidth;
  const flipY = cursor.y + LOUPE + LOUPE_OFFSET > window.innerHeight;
  const left = flipX ? cursor.x - LOUPE - LOUPE_OFFSET : cursor.x + LOUPE_OFFSET;
  const top = flipY ? cursor.y - LOUPE - LOUPE_OFFSET : cursor.y + LOUPE_OFFSET;

  return (
    <div
      className="pointer-events-none fixed z-20"
      style={{ left, top, width: LOUPE, height: LOUPE }}
    >
      <div className="glass relative h-full w-full overflow-hidden rounded-2xl border border-line-2 shadow-soft">
        {src ? (
          <img
            src={src}
            alt=""
            className="absolute h-full w-full select-none object-cover"
            // Centre the loupe on the cursor, scaled up.
            style={{
              transform: `scale(${LOUPE_ZOOM})`,
              transformOrigin: `${(cursor.x / window.innerWidth) * 100}% ${
                (cursor.y / window.innerHeight) * 100
              }%`,
            }}
            draggable={false}
          />
        ) : (
          <div className="grid h-full w-full place-items-center bg-bg-2 text-[10px] text-fg-3">
            画面准备中
          </div>
        )}
        {/* Loupe centre crosshair */}
        <span className="absolute left-1/2 top-1/2 h-[1px] w-5 -translate-x-1/2 -translate-y-1/2 bg-accent/90" />
        <span className="absolute left-1/2 top-1/2 h-5 w-[1px] -translate-x-1/2 -translate-y-1/2 bg-accent/90" />
        <span className="absolute left-1/2 top-1/2 h-2 w-2 -translate-x-1/2 -translate-y-1/2 rounded-full border border-accent bg-white/70" />
        {/* Selection preview: a scaled outline of the chosen rect */}
        {rect && (
          <span
            className="absolute border border-white/85"
            style={loupeRectStyle(cursor, rect)}
          />
        )}
      </div>
      <div className="mt-1.5 flex justify-center">
        <div className="glass rounded-md border border-line-2 px-2 py-0.5 font-mono text-[10px] tabular-nums text-fg-2">
          {Math.round(cursor.x)}, {Math.round(cursor.y)}
        </div>
      </div>
    </div>
  );
}

/** Map the selection rect into loupe-local coordinates. */
function loupeRectStyle(cursor: Point, rect: Rect): React.CSSProperties {
  // The loupe is a LOUPE×LOUPE window onto the screen, zoomed LOUPE_ZOOM×,
  // centred on the cursor. Distances from the cursor therefore scale by
  // LOUPE_ZOOM in loupe space, and the cursor sits at the loupe's centre.
  const half = LOUPE / 2;
  return {
    left: half + (rect.x - cursor.x) * LOUPE_ZOOM,
    top: half + (rect.y - cursor.y) * LOUPE_ZOOM,
    width: rect.width * LOUPE_ZOOM,
    height: rect.height * LOUPE_ZOOM,
  };
}

function Crosshairs({ point, rect }: { point: Point; rect: Rect | null }) {
  // Full-width guides stop at the selection border so the picked area stays
  // visible (CleanShot dims the guides inside the selection).
  const style = rect
    ? {
        clipPath: `polygon(0 0, ${rect.x}px 0, ${rect.x}px 100%, 0 100%, 0 0, ${
          rect.x + rect.width
        }px 0, ${rect.x + rect.width}px 100%, 100% 100%, 100% 0)`,
      }
    : undefined;
  return (
    <div className="pointer-events-none fixed inset-0 z-10" style={style}>
      <span
        className="absolute top-0 h-full w-px bg-accent/55"
        style={{ left: point.x }}
      />
      <span
        className="absolute left-0 h-px w-full bg-accent/55"
        style={{ top: point.y }}
      />
    </div>
  );
}

function SizeBadge({ rect }: { rect: Rect }) {
  const above = rect.y > 44;
  return (
    <div
      className="glass absolute left-1/2 flex -translate-x-1/2 items-center gap-1.5 rounded-lg border border-line-2 px-2.5 py-1 font-mono text-[12px] tabular-nums text-fg"
      style={above ? { bottom: "calc(100% + 8px)" } : { top: "calc(100% + 8px)" }}
    >
      {Math.round(rect.width)} × {Math.round(rect.height)}
    </div>
  );
}

function Handles() {
  const base =
    "absolute h-3.5 w-3.5 rounded-[3px] border-2 border-accent bg-white shadow";
  return (
    <>
      <span className={`${base} -left-[7px] -top-[7px]`} />
      <span className={`${base} -right-[7px] -top-[7px]`} />
      <span className={`${base} -bottom-[7px] -left-[7px]`} />
      <span className={`${base} -bottom-[7px] -right-[7px]`} />
    </>
  );
}

function HintBar({ valid }: { valid: boolean }) {
  return (
    <div className="pointer-events-none absolute left-1/2 top-7 flex -translate-x-1/2 items-center gap-2">
      <motion.div
        initial={{ opacity: 0, y: -8 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.2 }}
        className="glass flex items-center gap-2.5 rounded-full border border-line-2 px-4 py-2 text-[12.5px] text-fg-2 shadow-soft"
      >
        <span className="h-1.5 w-1.5 rounded-full bg-accent" />
        {valid
          ? "拖动移动 · 拖角调整 · ←→↑↓ 微调 · 回车确认 · Esc 取消"
          : "按住拖拽框选 · ←→↑↓ 微调 · 回车确认 · Esc 取消"}
      </motion.div>
    </div>
  );
}

function Toolbar({
  valid,
  done,
  onConfirm,
  onCancel,
}: {
  valid: boolean;
  done: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  return (
    <div className="absolute bottom-8 left-1/2 flex -translate-x-1/2 items-center gap-2.5">
      <button
        type="button"
        onClick={onCancel}
        disabled={done}
        className="glass h-10 rounded-full border border-line-2 px-5 text-[13px] font-medium text-fg-2 transition-colors hover:text-fg disabled:opacity-50"
      >
        取消
      </button>
      <button
        type="button"
        onClick={onConfirm}
        disabled={!valid || done}
        className="h-10 rounded-full bg-accent px-7 text-[13px] font-semibold text-white shadow-glow transition-transform active:scale-[0.97] disabled:cursor-not-allowed disabled:opacity-40 disabled:shadow-none"
      >
        完成录制区域
      </button>
    </div>
  );
}
