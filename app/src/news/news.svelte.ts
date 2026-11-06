// Newspaper state shared by the page and the article view: feedback and preferences.
import { api, type Feedback, type Prefs, type TopicInfo } from '../lib/api'

class News {
  feedback = $state<Feedback>({})
  prefs = $state<Prefs>({ blocked_outlets: [], muted_topics: [], followed_topics: [], onboarded: false })
  topics = $state<TopicInfo[]>([])
  loaded = $state(false)

  async load() {
    const [f, p, t] = await Promise.all([api.feedbackAll(), api.prefsGet(), api.topics()])
    this.feedback = f
    this.prefs = p
    this.topics = t
    this.loaded = true
  }

  vote(id: string, v: 1 | -1) {
    const [cur, stars] = this.feedback[id] ?? [null, null]
    const next = cur === v ? null : v
    this.feedback[id] = [next, stars]
    return api.storyFeedback(id, next, stars)
  }

  rate(id: string, stars: number | null) {
    const [vote] = this.feedback[id] ?? [null, null]
    this.feedback[id] = [vote, stars]
    return api.storyFeedback(id, vote, stars)
  }

  topicLabel(id: string) {
    return this.topics.find((t) => t.id === id)?.label ?? id
  }

  private toggle(list: 'blocked_outlets' | 'muted_topics' | 'followed_topics', id: string) {
    const l = this.prefs[list]
    this.prefs[list] = l.includes(id) ? l.filter((x) => x !== id) : [...l, id]
    return api.prefsSet($state.snapshot(this.prefs))
  }
  block(outlet: string) { return this.toggle('blocked_outlets', outlet) }
  mute(topic: string) {
    if (this.prefs.followed_topics.includes(topic)) this.prefs.followed_topics = this.prefs.followed_topics.filter((x) => x !== topic)
    return this.toggle('muted_topics', topic)
  }
  follow(topic: string) {
    if (this.prefs.muted_topics.includes(topic)) this.prefs.muted_topics = this.prefs.muted_topics.filter((x) => x !== topic)
    return this.toggle('followed_topics', topic)
  }
  async finishOnboarding(followed: string[]) {
    this.prefs.followed_topics = followed
    this.prefs.onboarded = true
    await api.prefsSet($state.snapshot(this.prefs))
  }
}

export const news = new News()
