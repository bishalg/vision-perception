# Retinal Vascular Biomarkers & Quantitative Metrics

> **Package**: `domains/retina`  
> **Document**: `domains/retina/docs/CLINICAL_BIOMARKERS.md`  
> **Clinical Scope**: Quantitative Ophthalmology, Cardiovascular Risk Stratification, Micro-Vascular Remodeling

---

## 1. Vascular Arc-Chord Tortuosity Index ($\tau$)

Vascular tortuosity measures the abnormal twisting, curling, and dilation of retinal arterioles and venules, serving as an early indicator of systemic hypertension and diabetic retinopathy:

$$\tau = \frac{L_{\text{arc}}}{L_{\text{chord}}} = \frac{\sum_{i=1}^{N-1} \sqrt{(x_{i+1} - x_i)^2 + (y_{i+1} - y_i)^2}}{\sqrt{(x_N - x_1)^2 + (y_N - y_1)^2}}$$

- **Straight Segment**: $\tau = 1.00$
- **Typical Healthy Arcades**: $1.02 \le \tau \le 1.15$
- **Pathological Tortuosity**: $\tau > 1.25$

---

## 2. Vessel Density Ratio (VDR)

VDR quantifies the proportion of retinal tissue perfused by the detected vascular network:

$$\text{VDR} = \frac{\sum_{k} (L_k \times \bar{W}_k)}{\text{Area}_{\text{retina}}}$$

Where:
- $L_k$: Arc-length of segment $k$ in pixels.
- $\bar{W}_k$: Mean vessel caliber/width of segment $k$.
- $\text{Area}_{\text{retina}}$: Total pixel area of the circular retinal field of view (excluding black outer borders).

---

## 3. Bifurcation & Branching Complexity

- Extracted directly from topological junction nodes where $\text{degree} \ge 3$.
- Healthy vascular networks exhibit self-similar fractal branching (fractal dimension $D_f \approx 1.70$).
- Microvascular rarefaction (pruning of bifurcations) correlates with advanced hypertensive target-organ damage.

---

## 4. Vessel Hierarchy Classification

| Vessel Class | Length Criterion | Caliber Criterion | Morphological Context |
| :--- | :--- | :--- | :--- |
| **Primary Arcade** | $\ge 60.0\text{ px}$ | $\ge 3.0\text{ px}$ | Superior and inferior temporal/nasal vascular trunks radiating from optic disc. |
| **Secondary Branch** | $30.0\text{--}60.0\text{ px}$ | $1.8\text{--}3.0\text{ px}$ | Arteriole and venule ramifications perfusing quadrants. |
| **Micro-Capillary** | $< 30.0\text{ px}$ | $< 1.8\text{ px}$ | Fine terminal vessel branches. |
