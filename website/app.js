import {Rain, parseWords} from './rain.mjs';
const $ = selector => document.querySelector(selector);
const descriptions = {
  monochrome: 'Soft, pale characters. Space between the streams. The original, quieter side of Stillterm.',
  matrix: 'Green trails, pale heads, and half-width Katakana mixed with digits and symbols. A familiar kind of rain.',
  dense: 'Layers of green, drifting at different sizes and speeds. Dense Matrix fills the screen and lets the depth settle in.'
};
let rain;
function renderSequence(words) {
  const container = $('#sequence'); container.replaceChildren();
  words.forEach((word, index) => {
    const span = document.createElement('span'); span.textContent = word; container.append(span);
    const arrow = document.createElement('b'); arrow.textContent = index === words.length - 1 ? '↻' : '→'; arrow.setAttribute('aria-hidden', 'true'); container.append(arrow);
  });
  if (!words.length) { const span = document.createElement('span'); span.textContent = 'Just the rain.'; container.append(span); }
}
rain = new Rain($('#rain'), (index, word) => {
  document.querySelectorAll('#sequence span').forEach((span, i) => span.classList.toggle('active', i === index));
  $('#word-status').textContent = `In the rain: ${word}`;
});
function updatePause() { $('#pause-label').textContent = rain.paused ? 'Play' : 'Pause'; $('#pause-icon').textContent = rain.paused ? '▷' : 'Ⅱ'; $('#pause').setAttribute('aria-label', rain.paused ? 'Play animation' : 'Pause animation'); }
updatePause();
$('#pause').addEventListener('click', () => { rain.setPaused(!rain.paused); updatePause(); });
document.querySelectorAll('[data-theme]').forEach(button => button.addEventListener('click', () => {
  document.querySelectorAll('[data-theme]').forEach(other => { const selected = other === button; other.classList.toggle('selected', selected); other.setAttribute('aria-pressed', String(selected)); });
  rain.setTheme(button.dataset.theme); $('#theme-description').textContent = descriptions[button.dataset.theme];
}));
$('#apply-words').addEventListener('click', () => {
  try {
    const words = parseWords($('#phrase-input').value); rain.setWords(words); renderSequence(words); $('#phrase-error').textContent = '';
    $('#word-status').textContent = words.length ? 'Your sequence is ready.' : 'Words disabled.';
    document.querySelector('#experience').scrollIntoView({behavior: window.matchMedia('(prefers-reduced-motion: reduce)').matches ? 'instant' : 'smooth', block: 'end'});
  } catch (error) { $('#phrase-error').textContent = error.message; }
});
const panels = {
  mac: `<p class="small-label">MACOS · BUILD 15</p><h3>Notarized. Getting ready<br>for your first quiet moment.</h3><span class="badge">Apple approval verified</span><p>The signed universal Mac download is prepared. A final installation test on a clean Mac comes before the public prerelease.</p><ol><li>Open the disk image and install Stillterm.saver.</li><li>Select Stillterm in System Settings → Screen Saver.</li><li>Choose a theme and your words in Options.</li></ol><a class="button outline" href="https://github.com/saeedet/Stillterm/releases" target="_blank" rel="noopener noreferrer">Follow the release <span aria-hidden="true">↗</span></a>`,
  windows: `<p class="small-label">WINDOWS · SOURCE BUILD</p><h3>Native rain for<br>your Windows screen.</h3><p>Build the .scr with Rust and Microsoft C++ build tools. Baseline checks passed on Windows 10 and 11; the latest phrases and Dense Matrix still need manual testing. Windows builds are unsigned.</p><div class="code-block"><pre>./scripts/build-windows.ps1</pre><button class="copy" type="button" aria-label="Copy Windows build command">Copy</button></div><a class="button outline" href="https://github.com/saeedet/Stillterm/blob/main/docs/windows.md" target="_blank" rel="noopener noreferrer">Windows setup <span aria-hidden="true">↗</span></a>`,
  terminal: `<p class="small-label">MACOS / LINUX / WINDOWS</p><h3>A little rain.<br>Right in your terminal.</h3><p>Install from source with Rust 1.88 or newer. Run the Matrix theme with a word sequence. Press q, Escape, or Ctrl+C to leave.</p><div class="code-block"><pre>git clone https://github.com/saeedet/Stillterm.git\ncd Stillterm\ncargo install --path crates/terminal --locked\nstillterm --theme matrix --words 'FREE,YOUR,MIND'</pre><button class="copy" type="button" aria-label="Copy terminal installation commands">Copy</button></div><a class="button outline" href="https://github.com/saeedet/Stillterm#install" target="_blank" rel="noopener noreferrer">Read the setup guide <span aria-hidden="true">↗</span></a>`
};
function setPlatform(platform) {
  $('#install-panel').innerHTML = panels[platform];
  document.querySelectorAll('[data-platform]').forEach(button => { const selected = button.dataset.platform === platform; button.classList.toggle('selected', selected); button.setAttribute('aria-pressed', String(selected)); });
  $('#install-panel .copy')?.addEventListener('click', async event => {
    const button = event.currentTarget; const text = $('#install-panel pre').textContent;
    try { await Promise.race([navigator.clipboard.writeText(text), new Promise((_, reject) => setTimeout(() => reject(new Error('Clipboard unavailable')), 700))]); button.textContent = 'Copied'; }
    catch { const range = document.createRange(); range.selectNodeContents($('#install-panel pre')); const selection = getSelection(); selection.removeAllRanges(); selection.addRange(range); button.textContent = 'Selected'; }
  });
}
document.querySelectorAll('[data-platform]').forEach(button => button.addEventListener('click', () => setPlatform(button.dataset.platform)));
setPlatform('mac');
