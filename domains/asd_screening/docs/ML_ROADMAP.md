# Machine Learning Development, Validation & CoreML Roadmap

> **Package**: `domains/asd_screening`  
> **Document**: `domains/asd_screening/docs/ML_ROADMAP.md`  
> **Frameworks**: Python (PyTorch, Scikit-Learn, LightGBM, SHAP), Apple CoreML (`coremltools`), ONNX Runtime

---

## 1. Algorithmic Strategy: Explainable Features vs. Raw Deep Learning

A critical pitfall in pediatric medical AI literature is training deep convolutional or recurrent neural networks directly on raw, uncurated gaze time-series:
- **Small Cohort Overfitting**: Deep networks with millions of parameters memorize patient-specific artifacts rather than genuine cognitive phenotypes.
- **Low-Level Saliency Confounding**: Models easily latch onto accidental stimulus features (e.g. background cuts, high-contrast colors) rather than social attention.
- **Regulatory Unexplainability**: FDA and EU MDR regulations require interpretability. Clinicians cannot act on an uninterpretable black-box score.

### The Recommended Architecture: Interpretable Feature-Based ML
We extract **16 grounded clinical biomarkers** (dwell ratios, saccadic latencies, transition entropy, repeat bigrams) and train regularized classical models (ElasticNet Logistic Regression, LightGBM, Random Forest).

---

## 2. Dataset Splitting & Leakage Prevention

```text
INCORRECT (Sample-Level Split - Data Leakage):
[ Child 1 - Trial A (Train) ] <---> [ Child 1 - Trial B (Test) ]  ❌ LEAKAGE!

CORRECT (Grouped Participant-Level Split):
Train Set: Children {1, 2, ..., 80}
Validation Set: Children {81, 82, ..., 90}
Held-Out Test Set: Children {91, 92, ..., 100}  ✅ ZERO LEAKAGE
```

- **Grouped K-Fold**: Splitting strictly on `child_id`. No participant's trials may appear in both train and evaluation folds.
- **Multi-Site Leave-One-Site-Out**: For multi-center trials, one clinical site is held out entirely to test cross-site geographic and hardware generalization.

---

## 3. Developmental Age Normalization

Social visual engagement evolves rapidly across early toddlerhood (12–48 months). An 18-month-old naturally exhibits different fixation durations and eye contact ratios than a 36-month-old:

$$\text{Dwell}_{\text{normalized}} = \text{Dwell}_{\text{observed}} - \mu_{\text{typical}}(\text{Age})$$

The ML pipeline incorporates:
1. Chronological age (months) as an explicit polynomial covariate ($A, A^2$).
2. Age-stratified z-score normalization benchmarked against normative control populations.

---

## 4. Model Explainability via SHAP (SHapley Additive exPlanations)

Every inference outputs both a continuous risk score $P(\text{Atypical Gaze}) \in [0.0, 1.0]$ and local feature contribution attributions:

```json
{
  "risk_score": 0.74,
  "confidence_band": "elevated_social_attention_difference",
  "top_contributing_biomarkers": [
    {
      "biomarker": "social_preference_score",
      "observed_value": -0.42,
      "shap_impact": "+0.31",
      "clinical_interpretation": "Substantial visual bias toward geometric patterns over social interactions."
    },
    {
      "biomarker": "first_eye_latency_ms",
      "observed_value": 1850.0,
      "shap_impact": "+0.18",
      "clinical_interpretation": "Delayed orienting toward the eye region when presented with a natural face."
    }
  ]
}
```

---

## 5. CoreML Export & On-Device Deployment

Once a LightGBM or PyTorch tabular model is validated, `coremltools` compiles the weights into a native `.mlpackage`. The compiled model runs 100% offline on the Apple Neural Engine with $< 2\text{ ms}$ inference latency and zero network transmission.
