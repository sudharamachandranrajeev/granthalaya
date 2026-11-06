// Screen navigation with View Transitions and the system back button
// (Android back and browser history both pop a screen).
import { transition } from './motion'

export type Screen = { name: 'home' } | { name: 'paper' } | { name: 'library' } | { name: 'reader'; id: string }

class Nav {
  stack = $state<Screen[]>([{ name: 'home' }])
  get current(): Screen {
    return this.stack[this.stack.length - 1]
  }
  go(s: Screen) {
    history.pushState({ depth: this.stack.length + 1 }, '')
    transition(() => { this.stack = [...this.stack, s] }, 'forward')
  }
  back() {
    if (this.stack.length > 1) history.back()
  }
  popped() {
    if (this.stack.length > 1) transition(() => { this.stack = this.stack.slice(0, -1) }, 'back')
  }
}

export const nav = new Nav()
window.addEventListener('popstate', () => nav.popped())
