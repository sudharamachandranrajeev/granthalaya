// Motion helpers: view transitions between screens and spring tilt on cards.
import { flushSync } from 'svelte'

export const reducedMotion = () => window.matchMedia('(prefers-reduced-motion: reduce)').matches

/** Run a state change as a View Transition, so shared elements morph between screens. */
export function transition(update: () => void, kind: 'forward' | 'back' = 'forward') {
  const doc = document as Document & { startViewTransition?: (cb: () => void) => { finished: Promise<void> } }
  if (!doc.startViewTransition || reducedMotion()) {
    update()
    return
  }
  document.documentElement.dataset.nav = kind
  const t = doc.startViewTransition(() => flushSync(update))
  t.finished.finally(() => delete document.documentElement.dataset.nav)
}

/**
 * Spring-driven 3D tilt that follows the pointer, with a moving glare.
 * Critically damped-ish spring stepped on rAF; stops when settled.
 */
export function tilt(node: HTMLElement, opts: { max?: number } = {}) {
  const max = opts.max ?? 9
  let tx = 0, ty = 0, x = 0, y = 0, vx = 0, vy = 0
  let gx = 50, gy = 50
  let raf = 0
  const k = 0.12, damping = 0.72

  const step = () => {
    vx = (vx + (tx - x) * k) * damping
    vy = (vy + (ty - y) * k) * damping
    x += vx
    y += vy
    node.style.setProperty('--rx', `${y.toFixed(2)}deg`)
    node.style.setProperty('--ry', `${x.toFixed(2)}deg`)
    node.style.setProperty('--gx', `${gx}%`)
    node.style.setProperty('--gy', `${gy}%`)
    if (Math.abs(tx - x) + Math.abs(ty - y) + Math.abs(vx) + Math.abs(vy) > 0.01) raf = requestAnimationFrame(step)
    else raf = 0
  }
  const kick = () => { if (!raf) raf = requestAnimationFrame(step) }

  const move = (e: PointerEvent) => {
    if (reducedMotion() || e.pointerType === 'touch') return
    const r = node.getBoundingClientRect()
    const px = (e.clientX - r.left) / r.width
    const py = (e.clientY - r.top) / r.height
    tx = (px - 0.5) * 2 * max
    ty = -(py - 0.5) * 2 * max
    gx = Math.round(px * 100)
    gy = Math.round(py * 100)
    kick()
  }
  const leave = () => { tx = 0; ty = 0; gx = 50; gy = 50; kick() }

  node.addEventListener('pointermove', move)
  node.addEventListener('pointerleave', leave)
  return {
    destroy() {
      cancelAnimationFrame(raf)
      node.removeEventListener('pointermove', move)
      node.removeEventListener('pointerleave', leave)
    },
  }
}

/** Horizontal swipe detection for touch page turns. */
export function swipe(node: HTMLElement, onSwipe: (dir: 'left' | 'right') => void) {
  let sx = 0, sy = 0, t = 0
  const down = (e: PointerEvent) => { sx = e.clientX; sy = e.clientY; t = performance.now() }
  const up = (e: PointerEvent) => {
    const dx = e.clientX - sx, dy = e.clientY - sy
    if (performance.now() - t < 600 && Math.abs(dx) > 60 && Math.abs(dx) > Math.abs(dy) * 1.5) onSwipe(dx < 0 ? 'left' : 'right')
  }
  node.addEventListener('pointerdown', down)
  node.addEventListener('pointerup', up)
  return { destroy() { node.removeEventListener('pointerdown', down); node.removeEventListener('pointerup', up) } }
}
