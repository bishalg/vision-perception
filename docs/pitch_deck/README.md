# Curv ASD Gaze Screening — Pitch Deck Assets

## Files
| File | Purpose | Audience |
|------|---------|----------|
| `ASD_Gaze_App_Pitch_Deck.md` | Full narrative deck (12 sections + appendix) | Investors, partners, clinical advisors |
| `PITCH_DECK_Slides.md` | Slide-by-slide Marp deck (16 slides + appendix) | Live presentations, PDF export |
| `ONE_PAGER_Executive_Summary.md` | Single-page leave-behind | Quick intros, email attachments, event tables |

## Quick Render Commands

### Option 1: VS Code + Marp Extension (Recommended)
1. Install **Marp for VS Code** extension
2. Open `PITCH_DECK_Slides.md`
3. Press `Cmd+Shift+P` → "Marp: Export Slide Deck" → PDF / PPTX / HTML

### Option 2: CLI (marp-cli)
```bash
npm install -g @marp-team/marp-cli
marp --pdf PITCH_DECK_Slides.md -o Curv_ASD_Pitch.pdf
marp --pptx PITCH_DECK_Slides.md -o Curv_ASD_Pitch.pptx
marp --html PITCH_DECK_Slides.md -o Curv_ASD_Pitch.html
```

### Option 3: GitHub Pages (Auto-deploy)
Push to `docs/pitch_deck/` — GitHub Actions can render to `gh-pages` branch for a live slide deck URL.

## Custom Theme (Curv Branding)
Create `theme.css` in this folder:
```css
/* curv-theme.css */
:root {
  --curv-teal: #00d2c4;
  --curv-emerald: #00e676;
  --curv-deep: #0a1628;
  --curv-deep-2: #0f1f3a;
}
section {
  background: linear-gradient(135deg, var(--curv-deep), var(--curv-deep-2));
  color: #e8f5f5;
  font-family: 'SF Pro Display', -apple-system, sans-serif;
}
h1 { color: var(--curv-emerald); font-weight: 700; }
h2 { color: var(--curv-teal); font-weight: 600; }
strong { color: var(--curv-emerald); }
code { background: rgba(255,255,255,0.08); padding: 0.15em 0.4em; border-radius: 4px; }
table { border-collapse: collapse; width: 100%; }
th, td { border: 1px solid rgba(255,255,255,0.12); padding: 0.5em; }
th { background: rgba(0,210,196,0.15); }
```

Then add to slide frontmatter:
```yaml
---
theme: curv-theme
paginate: true
---
```

## Assets Needed
| Asset | Spec | Status |
|-------|------|--------|
| `assets/product_flow.png` | 1920×1080, product walkthrough diagram | 🎨 TODO |
| `assets/architecture.mmd` | Mermaid diagram for appendix | 🎨 TODO |
| `assets/competitive_matrix.png` | Visual competitive comparison | 🎨 TODO |

## Version Control
- All decks versioned with repo (`git log docs/pitch_deck/`)
- Tag releases: `git tag pitch-deck-v1.0`
- Share via: private repo access + PDF export

---

*Last updated: September 2026*