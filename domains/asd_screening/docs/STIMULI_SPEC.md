# Pediatric Visual Stimuli & Dynamic AOI Specification

> **Package**: `domains/asd_screening`  
> **Document**: `domains/asd_screening/docs/STIMULI_SPEC.md`  
> **Target Cohort**: Toddlers aged 18–48 months  
> **Total Test Duration**: $\le 3\text{ minutes}$ (including breaks)

---

## 1. Stimulus Battery Architecture

To reliably capture multi-dimensional social visual attention phenotypes, the screening battery consists of three brief, validated stimuli tasks interspersed with audio-visual rewards:

```text
┌───────────────────────────┐
│ 5-Point Bubble Calibration│ 20–30 sec
└─────────────┬─────────────┘
              ▼
┌───────────────────────────┐
│ Task 1: GeoPref Task      │ 30 sec (Split-Screen: Social vs Geometric)
└─────────────┬─────────────┘
              ▼
┌───────────────────────────┐
│ Animated Break / Reward   │ 10 sec (Dancing Star / Sound)
└─────────────┬─────────────┘
              ▼
┌───────────────────────────┐
│ Task 2: Face Scanning     │ 30 sec (Naturalistic Talking Actor)
└─────────────┬─────────────┘
              ▼
┌───────────────────────────┐
│ Animated Break / Reward   │ 10 sec
└─────────────┬─────────────┘
              ▼
┌───────────────────────────┐
│ Task 3: Joint Attention   │ 30 sec (Actor Gaze Following Shift)
└───────────────────────────┘
```

---

## 2. Detailed Task Specifications

### Task 1: GeoPref (Social vs. Geometric Preference)
- **Clinical Paradigm**: Evaluates visual preference between human social interactions and repetitive dynamic geometric patterns (Pierce et al., 2011, 2016).
- **Visual Presentation**:
  - Split-screen layout ($1920 \times 1080$ or iPad $4:3$ display).
  - **Left Sector ($[0.0, 0.0] \to [0.5, 1.0]$)**: Natural video of children engaged in social play, laughing, and dynamic bodily movement.
  - **Right Sector ($[0.5, 0.0] \to [1.0, 1.0]$)**: Moving abstract geometric shapes with continuous, repetitive rotational symmetry.
- **Biomarker Derived**:
  $$\text{Social Preference Score} = \frac{\text{Dwell}_{\text{social}} - \text{Dwell}_{\text{geometric}}}{\text{Dwell}_{\text{social}} + \text{Dwell}_{\text{geometric}}} \in [-1.0, +1.0]$$
- **Counter-Balancing**: Left/right presentation positions are randomized across successive administrations to eliminate side bias.

---

### Task 2: Naturalistic Face Scanning
- **Clinical Paradigm**: Quantifies visual attention allocation across the hierarchical facial anatomy (Klin et al., 2002; Jones & Klin, 2013).
- **Visual Presentation**: Centered high-definition video of a friendly actor reciting a gentle nursery rhyme with child-directed speech.
- **Hierarchical Dynamic AOIs**:
  - `Eye_Region`: Polygon tightly bounding orbit, sclera, iris, and eyebrows.
  - `Mouth_Region`: Polygon bounding lips, oral cavity, and chin.
  - `Face_Oval`: Outer boundary of the facial contour.
  - `Inanimate_Background`: Unfilled peripheral canvas.
- **Biomarkers Derived**:
  - $\text{Eye Dwell Ratio} = T_{\text{eyes}} / T_{\text{total\_valid}}$
  - $\text{Mouth Dwell Ratio} = T_{\text{mouth}} / T_{\text{total\_valid}}$
  - $\text{First Eye Latency} = t_{\text{onset}}(\text{Fixation}_1 \in \text{Eye\_Region})$

---

### Task 3: Gaze Following & Joint Attention
- **Clinical Paradigm**: Evaluates response to joint attention (RJA) cues (Mundy et al., 2007; Bedford et al., 2012).
- **Visual Presentation**:
  - Actor sits at a desk with two novel, silent mechanical toys on the left and right sides.
  - At $t = 10.0\text{s}$, actor turns head and directs visual gaze decisively toward the left toy for $5.0\text{s}$.
  - At $t = 20.0\text{s}$, actor turns and directs visual gaze toward the right toy for $5.0\text{s}$.
- **Biomarkers Derived**:
  - Saccadic Latency to Target Toy ($t_{\text{toy\_fixation}} - t_{\text{actor\_turn}}$).
  - Proportion of Successful Gaze-Following Shifts.

---

## 3. Low-Level Saliency Controls

1. **Photometric Equivalence**: Mean luminance (Y-channel) and RMS contrast between social and geometric sectors must match within $\pm 3.0\%$.
2. **Motion Energy Balance**: Optical flow magnitude ($\|u, v\|$) computed via Farnebäck dense flow must be balanced across sectors.
3. **Acoustic Normalization**: Stereo audio tracks must be centered with equal channel power and peak volume $< 68\text{ dBA}$ at 50 cm.
