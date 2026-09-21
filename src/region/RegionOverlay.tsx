import { useEffect, useRef, useState } from "react";

import { api, type Rect } from "../lib/api";

interface Point {
  x: number;
  y: number;
}

const MIN_SIZE = 40;

export default function RegionOverlay() {
  const [start, setStart] = useState<Point | null>(null);
  const [rect, setRect] = useState<Rect | null>(null);
  const [done, setDone] = useState(false);
  const surfaceRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    document.body.classList.add("is-overlay");
    return () => document.body.classList.remove("is-overlay");
  }, []);

  const clampPoint = (e: PointerEvent | React.PointerEvent): Point => {
    const w = window.innerWidth;
    const h = window.innerHeight;
    return {
      x: Math.max(0, Math.min(w, e.clientX)),
      y: Math.max(0, Math.min(h, e.clientY)),
    };
  };

  const onPointerDown = (e: React.PointerEvent) => {
    if (done) return;
    const p = clampPoint(e);
    setStart(p);
    setRect({ x: p.x, y: p.y, width: 0, height: 0 });
    surfaceRef.current?.setPointerCapture(e.pointerId);
  };

  const onPointerMove = (e: React.PointerEvent) => {
    if (!start || done) return;
    const p = clampPoint(e);
    setRect({
      x: Math.min(start.x, p.x),
      y: Math.min(start.y, p.y),
      width: Math.abs(p.x - start.x),
      height: Math.abs(p.y - start.y),
    });
  };

  const onPointerUp = (e: React.PointerEvent) => {
    if (!start || done) return;
    const p = clampPoint(e);
    const finalRect: Rect = {
      x: Math.min(start.x, p.x),
      y: Math.min(start.y, p.y),
      width: Math.abs(p.x - start.x),
      height: Math.abs(p.y - start.y),
    };
    setStart(null);
    if (finalRect.width < MIN_SIZE || finalRect.height < MIN_SIZE) {
      setRect(null);
    } else {
      setRect(finalRect);
    }
  };

  const onDoubleClick = () => {
    if (!rect || !isValid(rect)) return;
    setDone(true);
    void api.confirmRegion(rect);
  };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        void api.cancelRegion();
      } else if (e.key === "Enter" && rect && isValid(rect) && !done) {
        setDone(true);
        void api.confirmRegion(rect);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [rect, done]);

  const valid = rect !== null && isValid(rect);

  return (
    <div
      ref={surfaceRef}
      className="fixed inset-0 cursor-crosshair"
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onDoubleClick={onDoubleClick}
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

      <HintBar valid={valid} />
      <Toolbar
        valid={valid}
        done={done}
        onConfirm={() => {
          if (!rect) return;
          setDone(true);
          void api.confirmRegion(rect);
        }}
        onCancel={() => void api.cancelRegion()}
      />
    </div>
  );
}

function isValid(rect: Rect): boolean {
  return rect.width >= MIN_SIZE && rect.height >= MIN_SIZE;
}

function SizeBadge({ rect }: { rect: Rect }) {
  const above = rect.y > 44;
  return (
    <div
      className="glass absolute left-1/2 flex -translate-x-1/2 items-center gap-1.5 rounded-lg border border-line-2 px-2.5 py-1 font-mono text-[12px] text-fg"
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
      <div className="glass flex items-center gap-2.5 rounded-full border border-line-2 px-4 py-2 text-[12.5px] text-fg-2 shadow-soft">
        <span className="h-1.5 w-1.5 rounded-full bg-accent" />
        {valid ? "双击或点击「完成」确认区域 · Esc 取消" : "按住拖拽框选录制区域 · Esc 取消"}
      </div>
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
