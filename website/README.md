# Stillterm website

Public site: https://saeedet.github.io/Stillterm/

Buildless HTML, CSS, and JavaScript. No dependencies or analytics. The browser
rain interpretation is independent of the Rust screensaver engine.

## Preview and check

From this folder, run `python3 -m http.server 8000` and open
http://localhost:8000. All asset paths are relative so the same files work at the
GitHub project URL and at a static host's root.

From the repository root, run `node scripts/check-website.mjs` to check JavaScript,
asset/anchor paths, and ordered phrase input.

Theme selection, pause/play, phrase editing, validation, and platform setup panels
work in the browser. Animation pauses outside the viewport and in hidden tabs,
respects reduced-motion preferences, and is capped at 30 FPS.

## Publishing

`.github/workflows/pages.yml` checks and deploys this folder on pushes to `main`
that change the website, its check script, or the workflow. It also supports manual
runs. Repository Settings → Pages must use GitHub Actions. The `github-pages`
environment receives deployment status and the live URL.

Mac downloads link to the separately signed/notarized GitHub Release asset.
Changing this website does not change that binary or require notarization. When
releasing a new build, update app.js, the FAQ in index.html, and the main README.
The original private Sites preview is separate from this public deployment.
