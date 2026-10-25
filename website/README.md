# Stillterm website

Buildless HTML, CSS, and JavaScript. No dependencies or analytics. The browser
rain interpretation is independent of the Rust screensaver engine.

From this folder, start a static server:

    python3 -m http.server 8000

Open http://localhost:8000. Serve this folder at a static host's root for deployment.
The live Sites preview is a separate private deployment; its hosting identity and
credentials are intentionally outside this public repository.

Theme selection, pause/play, phrase editing, validation, and platform setup panels
work in the browser. Animation pauses outside the viewport and in hidden tabs,
respects reduced-motion preferences, and is capped at 30 FPS.

The Mac download panel remains gated until the clean-Mac check is complete and
the GitHub prerelease is published. At that point, update app.js and the FAQ in
index.html with the actual release/download URLs. Keep setup instructions and
release status consistent with the main README.

When updating the Sites preview, synchronize these files into that Site's dist/
folder, test, then package and deploy using its retained Site identity.
