# Retinal Perception Test Fixtures (`data/fixtures/retina/`)

> **Directory**: `data/fixtures/retina/`  
> **Lifecycle Status**: Active Test Fixtures  
> **Domain**: Ophthalmology / Retinal Biometrics & Micro-Vessel Perception (`org.curv.domain.retina`)

---

## 1. Overview

This directory provides standardized reference retinal fundus images for validating Steger curvilinear ridge extraction, vascular tree tracing, vessel density, and arteriolar/venular tortuosity analysis.

In color fundus photography:
- The **Green Channel ($G$)** provides maximal contrast between retinal blood vessels and the choroid background due to the optical absorption spectrum of hemoglobin ($\sim 540\text{--}575\text{ nm}$).
- Blood vessels appear as dark curvilinear valleys against a bright orange/red retina.
- Multi-scale Steger ridge extraction with scale parameters $\sigma \in [1.0, 3.0]$ captures both primary central retinal vessels and fine microvascular capillaries.

---

## 2. Directory Contents

- `sample_fundus_healthy.png`: Reference fundus image exhibiting characteristic optic disc, macula, and branching vascular tree.
- `metadata.json`: Ground truth metadata, camera resolution, field of view (FOV), and vessel statistics.
- `README.md`: This reference specification.

---

## 3. Data Acquisition & Provenance

Fixtures in this directory are acquired or synthesized via `scripts/download_retina_samples.py`. All external data originates from open-access public domain or CC-BY medical repositories (e.g. Wikimedia Commons, NIH open datasets) and is documented in [`third_party/SOURCE_PROVENANCE.md`](../../../third_party/SOURCE_PROVENANCE.md).
