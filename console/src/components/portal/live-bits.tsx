import { useEffect, useRef, useState } from "react"
import { cn } from "@/lib/utils"

/// Honours the user's reduced-motion preference for JS-driven animation (the
/// global CSS override already flattens keyframe-based effects).
export function usePrefersReducedMotion(): boolean {
  const [reduced] = useState(
    () => window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false,
  )
  return reduced
}

/// Rolls a displayed number from its previous value to the new one with a
/// short ease-out, so live counters tick instead of jumping between polls.
export function CountUp({ value, format, className }: {
  value: number
  format?: (value: number) => string
  className?: string
}) {
  const reduced = usePrefersReducedMotion()
  const current = useRef(value)
  const [shown, setShown] = useState(value)
  useEffect(() => {
    if (reduced || current.current === value) {
      current.current = value
      setShown(value)
      return
    }
    const origin = current.current
    const start = performance.now()
    let raf = 0
    const tick = (now: number) => {
      const k = Math.min(1, (now - start) / 400)
      const next = origin + (value - origin) * (1 - Math.pow(1 - k, 3))
      current.current = next
      setShown(next)
      if (k < 1) raf = requestAnimationFrame(tick)
    }
    raf = requestAnimationFrame(tick)
    return () => cancelAnimationFrame(raf)
  }, [value, reduced])
  const render = format ?? ((n: number) => String(Math.round(n)))
  return <span className={className}>{render(shown)}</span>
}

/// Three bouncing bars — the "something is streaming right now" signal.
export function EqBars({ active }: { active: boolean }) {
  return (
    <span className="flex h-3.5 items-end gap-[2px]" aria-hidden>
      {[0, 1, 2].map((index) => (
        <span
          key={index}
          className={cn(
            "w-[3px] rounded-sm",
            active ? "bg-state-healthy activity-eq-bar" : "bg-muted-foreground/40",
          )}
          style={active ? { animationDelay: `${index * 140}ms` } : { height: `${30 + index * 20}%` }}
        />
      ))}
    </span>
  )
}

/// A tiny polyline of recent throughput samples — htop-flavoured liveness for
/// one session. Grows a point per poll; flat lines mean the session paused.
export function Sparkline({ samples, online }: { samples: Array<number>; online: boolean }) {
  const width = 64
  const height = 16
  if (samples.length < 2) return null
  const max = Math.max(1, ...samples)
  const step = width / (samples.length - 1)
  const points = samples
    .map((value, index) => {
      const x = (index * step).toFixed(1)
      const y = (height - 2 - (value / max) * (height - 4)).toFixed(1)
      return `${x},${y}`
    })
    .join(" ")
  return (
    <svg
      width={width}
      height={height}
      viewBox={`0 0 ${width} ${height}`}
      className={cn("shrink-0 overflow-visible", online ? "text-state-healthy" : "text-muted-foreground/50")}
      aria-hidden
    >
      <polyline points={points} fill="none" stroke="currentColor" strokeWidth="1.25" strokeLinejoin="round" />
    </svg>
  )
}
