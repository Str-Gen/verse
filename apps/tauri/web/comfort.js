export class Comfort {
  constructor(config = { preparation: 2000, ramp: 30000, brief_pause: 30000 }) { this.config = config; this.started = null; this.paused = null; this.began = false; }
  start(now) {
    const brief = this.began && this.paused !== null && now - this.paused <= this.config.brief_pause;
    if (!brief) this.began = false;
    this.started = brief ? null : now; this.paused = null;
    return brief ? 0 : this.config.preparation;
  }
  pause(now) {
    this.began ||= this.started !== null && now - this.started >= this.config.preparation;
    this.started = null; this.paused = now;
  }
  fraction(now) {
    if (this.started === null) return 1;
    const progress = Math.max(0, Math.min(1, (now - this.started - this.config.preparation) / this.config.ramp));
    return .5 + .25 * (1 + Math.tanh((progress * 2 - 1) * 2) / Math.tanh(2));
  }
}
