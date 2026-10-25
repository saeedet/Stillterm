// Browser interpretation for the product page, independent of the Rust renderer.
export function parseWords(text) {
  const entries = text.split(/[,\r\n]/u).map(value => value.trim()).filter(Boolean);
  if (entries.length > 16) throw new Error('Use up to 16 words or phrases.');
  if (entries.some(value => Array.from(value).length > 64)) throw new Error('Keep each phrase to 64 characters or fewer.');
  if (entries.some(value => /[\p{Cc}\p{Cf}]/u.test(value))) throw new Error('Use printable characters in your phrases.');
  return entries;
}
const alphabet = Array.from('ｱｲｳｴｵｶｷｸｹｺｻｼｽｾｿﾀﾁﾂﾃﾄﾅﾆﾇﾈﾉﾊﾋﾌﾍﾎﾏﾐﾑﾒﾓﾔﾕﾖﾗﾘﾙﾚﾛﾜ012345789:<>*+-');
const mono = Array.from('0123456789.:+*');
export class Rain {
  constructor(canvas, onWord) {
    this.canvas = canvas; this.context = canvas.getContext('2d'); this.onWord = onWord;
    this.theme = 'dense'; this.words = ['FREE', 'YOUR', 'MIND']; this.next = 0;
    this.paused = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    this.visible = true; this.active = null; this.elapsed = 0; this.due = 2400; this.last = 0; this.frame = 0;
    this.resizeObserver = new ResizeObserver(() => this.resize()); this.resizeObserver.observe(canvas.parentElement);
    document.addEventListener('visibilitychange', () => this.sync());
    this.intersection = new IntersectionObserver(entries => { this.visible = entries[0].isIntersecting; this.sync(); });
    this.intersection.observe(canvas.parentElement);
    this.resize(); this.sync();
  }
  resize() {
    const box = this.canvas.parentElement.getBoundingClientRect();
    this.width = box.width; this.height = box.height;
    const ratio = Math.min(window.devicePixelRatio || 1, 1.5, Math.sqrt(1800000 / Math.max(1, box.width * box.height)));
    this.canvas.width = Math.max(1, Math.round(box.width * ratio)); this.canvas.height = Math.max(1, Math.round(box.height * ratio));
    this.context.setTransform(ratio, 0, 0, ratio, 0, 0);
    this.rebuild(); this.draw();
  }
  rebuild() {
    const sizes = this.theme === 'dense' ? [10, 14, 20] : [19];
    this.layers = sizes.map((size, index) => {
      const columns = Math.ceil(this.width / size); const rows = Math.ceil(this.height / (size * 1.25));
      return {size, rows, alpha: sizes.length === 1 ? .85 : [.23, .43, .88][index],
        streams: Array.from({length: columns}, (_, column) => ({column, head: Math.random() * rows * 1.8 - rows * .4,
          speed: 2 + Math.random() * 4 + index, tail: this.theme === 'dense' ? rows * (1 + Math.random()) : 9 + Math.random() * 20,
          salt: Math.floor(Math.random() * 100000), active: Math.random() < (this.theme === 'dense' ? .96 : .58)}))};
    });
    this.active = null; this.due = this.elapsed + 1600;
  }
  setTheme(theme) { this.theme = theme; this.rebuild(); this.draw(); }
  setWords(words) { this.words = words; this.next = 0; this.active = null; this.due = this.elapsed + 1000; this.draw(); }
  setPaused(value) { this.paused = value; this.sync(); }
  sync() {
    cancelAnimationFrame(this.frame); this.frame = 0; this.last = 0;
    if (!this.paused && this.visible && !document.hidden) this.frame = requestAnimationFrame(time => this.tick(time));
  }
  tick(time) {
    const delta = this.last ? time - this.last : 34;
    if (delta < 32) { this.frame = requestAnimationFrame(next => this.tick(next)); return; }
    this.last = time; const step = Math.min(delta, 100);
    this.elapsed += step;
    for (const layer of this.layers) for (const stream of layer.streams) {
      stream.head += stream.speed * step / 1000;
      if (stream.head - stream.tail > layer.rows) { stream.head = -2; stream.salt = Math.floor(Math.random() * 100000); }
    }
    this.updateWords(); this.draw(); this.frame = requestAnimationFrame(next => this.tick(next));
  }
  updateWords() {
    const layer = this.layers.at(-1);
    if (this.active) {
      const current = this.active;
      if (current.finished === null && current.stream.head >= current.row + current.letters.length - 1) current.finished = this.elapsed;
      if (current.finished !== null && this.elapsed - current.finished > 1000 + (current.letters.length - 1) * 300) {
        this.active = null; this.next = (this.next + 1) % this.words.length; this.due = this.elapsed + 2600;
      }
      return;
    }
    if (!this.words.length || this.elapsed < this.due) return;
    const phrase = this.words[this.next]; const letters = Array.from(phrase);
    if (letters.length > layer.rows - 2) { this.next = (this.next + 1) % this.words.length; this.due = this.elapsed + 500; return; }
    const candidates = layer.streams.filter(stream => stream.active && stream.column * layer.size > this.width * .57 && stream.column * layer.size < this.width * .94 && stream.head > 1 && stream.head < layer.rows - letters.length - 1);
    if (!candidates.length) return;
    const stream = candidates[Math.floor(Math.random() * candidates.length)];
    this.active = {stream, row: Math.ceil(stream.head) + 1, letters, finished: null}; this.onWord?.(this.next, phrase);
  }
  draw() {
    const ctx = this.context; if (!ctx || !this.layers) return;
    ctx.fillStyle = '#060a06'; ctx.fillRect(0, 0, this.width, this.height);
    const characters = this.theme === 'monochrome' ? mono : alphabet;
    for (const layer of this.layers) {
      ctx.font = `${layer.size}px ui-monospace, "SFMono-Regular", Menlo, monospace`;
      ctx.textAlign = 'center'; ctx.textBaseline = 'top';
      for (const stream of layer.streams) {
        if (!stream.active) continue;
        const start = Math.max(0, Math.floor(stream.head - stream.tail)); const end = Math.min(layer.rows, Math.floor(stream.head));
        for (let row = start; row <= end; row++) {
          const distance = stream.head - row; const level = Math.max(0, 1 - distance / stream.tail);
          const alpha = level * level * layer.alpha * (this.theme === 'dense' ? Math.min(1, (layer.rows - row) / (layer.rows * .4)) : 1);
          const leading = distance < 1.05;
          ctx.fillStyle = this.theme === 'monochrome' ? `rgba(220,229,214,${alpha})` : leading ? `rgba(216,255,223,${layer.alpha})` : `rgba(73,206,71,${alpha})`;
          const hash = Math.abs(Math.imul(stream.salt + row * 179 + Math.floor(this.elapsed / 1600), 2654435761) >>> 0);
          ctx.fillText(characters[hash % characters.length], (stream.column + .5) * layer.size, row * layer.size * 1.25);
        }
      }
    }
    if (this.active) {
      const current = this.active; const layer = this.layers.at(-1);
      ctx.font = `${layer.size}px ui-monospace, "SFMono-Regular", Menlo, monospace`;
      current.letters.forEach((letter, index) => {
        if (current.stream.head < current.row + index) return;
        const age = current.finished === null ? 0 : Math.max(0, this.elapsed - current.finished - index * 300);
        const alpha = Math.max(0, 1 - age / 1000); if (!alpha) return;
        ctx.shadowColor = this.theme === 'monochrome' ? '#d9e6d0' : '#b6ffa0'; ctx.shadowBlur = 10;
        ctx.fillStyle = `rgba(226,255,216,${alpha})`;
        ctx.fillText(letter, (current.stream.column + .5) * layer.size, (current.row + index) * layer.size * 1.25);
      });
      ctx.shadowBlur = 0;
    }
  }
}
