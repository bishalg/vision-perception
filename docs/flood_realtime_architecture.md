# Glacier-Collapse & Flash Flood Real-Time Monitoring Architecture

> **Document ID**: `DOC-SPEC-007-FLOOD-REALTIME`  
> **Target Repository**: `vision-perception`  
> **Domain Pack**: `domains/geospatial` (`domains/flood`)  
> **Reference Event**: Lende Khola / Bhote Koshi Glacier Collapse (Nepal, 2026)  
> **Lifecycle Status**: Production Architectural Specification  

---

## 1. Threat Profile & Operational Challenges

High-altitude Himalayan river basins (such as the Lende Khola / Bhote Koshi corridor in Rasuwa District, Nepal) face severe flash flood and Glacier Lake Outburst Flood (GLOF) risks triggered by permafrost degradation, rock avalanches, and monsoonal cloudbursts.

### The Operational Challenge
1. **Revisit Latency**: Optical satellite platforms (e.g., ESA Sentinel-2) have a revisit cadence of 5 days, which is insufficient for immediate hours-after flash flood alerts.
2. **Cloud Occlusion**: Monsoon convective cloud sheets frequently obscure optical sensors during peak risk periods (June–September).
3. **Sensor Silos**: Hydrological gauges, local river CCTV cameras, seismic ground sensors, and orbital imagery traditionally operate in isolated silos without unified geometric cross-correlation.

### The Real-Time Solution
This architecture couples our sub-pixel C++20 differential geometry engine (`vision-perception`) with a multi-source real-time ingestion layer (inspired by [`bilawalsidhu/gods-eye-view`](https://github.com/bilawalsidhu/gods-eye-view)) and an AWS Bedrock agentic dispatch loop.

---

## 2. Multi-Source Ingestion & Fusion Matrix

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                    Tier 1: Multi-Modal Ingestion Feeds                      │
├──────────────────────┬────────────────────────┬─────────────────────────────┤
│ Optical EO Imagery   │ All-Weather Radar      │ Terrestrial Real-Time Feeds │
│ • Sentinel-2 L2A BOA │ • Sentinel-1 SAR (GRD) │ • River Corridor CCTV       │
│ • 10m Multispectral  │ • VV/VH Polarization   │ • Hydrology Gauges (DHM)    │
│ • Cloud-Filtered     │ • Penetrates Clouds    │ • Seismic Alerts (USGS)     │
└──────────┬───────────┴───────────┬────────────┴──────────────┬──────────────┘
           │                       │                           │
           ▼                       ▼                           ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                 Tier 2: Perception Core (`domains/flood`)                   │
├─────────────────────────────────────────────────────────────────────────────┤
│ 1. Sub-Pixel Co-Registration (OpenCV ECC Algorithm, Error < 0.2 px)         │
│ 2. Dynamic Masking (HSV Cloud Gate + Alpha Normalization)                   │
│ 3. Steger Scale-Space Curvilinear Extraction (Dual-Polarity Union, σ = 1.5) │
│ 4. Channel Width Proxy Calculation (Continuous Horizontal Run-Length)        │
│ 5. Dual Control Cross-Validation (Spatial Slope Control + Temporal Baseline)│
└──────────────────────────────────────┬──────────────────────────────────────┘
                                       │ Immutable `flood_delta.json`
                                       ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                Tier 3: AWS Cloud Agentic Loop (`aws_infra/`)                │
├─────────────────────────────────────────────────────────────────────────────┤
│ • S3 Immutable Audit Bucket (`vp-evidence-vault-*`)                         │
│ • DynamoDB Deduplication & Idempotency Key (SHA-256 Hash Gate)              │
│ • Amazon Bedrock Agentic Loop (`getFloodEvidence` → `dispatchFloodAlert`)   │
│ • Multi-Channel Emergency Dispatch (SNS SMS / Email / SQS Civil Defense)    │
└──────────────────────────────────────┬──────────────────────────────────────┘
                                       │ GeoJSON 3D Vector Geometry
                                       ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│              Tier 4: 3D Tactical Digital Twin (CesiumJS / GEV)              │
├─────────────────────────────────────────────────────────────────────────────┤
│ • 3D Terrain Extrusion & Dynamic River Flood Frustum                        │
│ • Viewshed Projection from Local Highway / Hydropower CCTV                  │
│ • Real-time HUD Telemetry (Width Ratio, Surge %, Alert Trigger Level)       │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Mathematical & Algorithmic Perception Engine

### 3.1. Scale-Space Steger Ridge Extraction
The core curvilinear centerline extraction relies on the differential geometry of scale-space Hessian second derivatives:

1. **Gaussian Derivative Convolutions**:
   $$r_x = I * G_x(\sigma), \quad r_y = I * G_y(\sigma)$$
   $$r_{xx} = I * G_{xx}(\sigma), \quad r_{xy} = I * G_{xy}(\sigma), \quad r_{yy} = I * G_{yy}(\sigma)$$
2. **Eigenvalue Decomposition**:
   $$\mathbf{H} = \begin{bmatrix} r_{xx} & r_{xy} \\ r_{xy} & r_{yy} \end{bmatrix}, \quad \det(\mathbf{H} - \lambda \mathbf{I}) = 0$$
   The primary eigenvector $\hat{\mathbf{n}} = (n_x, n_y)^T$ corresponds to the maximum eigenvalue $|\lambda_1| \ge |\lambda_2|$.
3. **Sub-Pixel Centerline Local Extremum**:
   Using a second-order Taylor expansion along $\hat{\mathbf{n}}$, the sub-pixel line displacement $t \in [-0.5, 0.5]$ is:
   $$t = -\frac{r_x n_x + r_y n_y}{r_{xx} n_x^2 + 2 r_{xy} n_x n_y + r_{yy} n_y^2}$$
   Centerline point: $(p_x, p_y) = (x + t \cdot n_x, \; y + t \cdot n_y)$.

### 3.2. Dual-Polarity Union Resolution
During disaster events, river corridors exhibit an optical polarity inversion:
- **Pre-Disaster Baseline**: Bright polarity dominates ($F_1 = 0.1426$), where dry gravel banks and terrace flanks appear bright against dark active water.
- **Post-Disaster Surge**: Dark polarity dominates ($F_1 = 0.2008$), where light-colored glacial sediment blankets the valley while fast-flowing incised water threads appear dark.
- **Operational Rule**: The production config (`configs/geospatial_wide_ridge.json`) runs a **dual-polarity union** ($\sigma = 1.5, \text{low} = 0.5, \text{high} = 1.5$) to ensure zero signal loss across both conditions.

### 3.3. Channel Width Proxy Formulation
Channel width is calculated row-by-row across the gorge corridor:
$$W(y) = \max_{x_1 \le x_2} \{ x_2 - x_1 + 1 \mid \forall x \in [x_1, x_2], M_{\text{flood}}(x, y) = 1 \} \quad \text{clamped to } [3, 200]\text{ px}$$
The median channel width $\widetilde{W} = \text{median}(\{W(y)\})$ serves as the primary metric for hydrological expansion.

---

## 4. Dual-Control Rigor & False-Alarm Suppression

To eliminate false triggers caused by seasonal illumination or cloud shifts, every flood assessment must pass **two independent control experiments**:

| Control Experiment | Test Subject | Threshold Condition | Observed Result | Status |
| :--- | :--- | :--- | :--- | :--- |
| **Spatial Control** | West Vegetated Slope Window (`x=50, y=400, w=150, h=200`) | Surge Ratio $< 1.10\times$ | **$0.151\times$** (Signature dropped from 4.04% to 0.61%) | **PASSED** |
| **Temporal Control** | Pre-Disaster Baseline Window (12 Aug vs 24 Aug 2026) | Channel Width Ratio $< 1.10\times$ | **$0.905\times$** (Width shifted $21.0 \to 19.0\text{ px}$) | **PASSED** |
| **Active Corridor** | Lende Khola / Bhote Koshi Confluence Neck | Flood Signature $\ge 1.10\times$, Width $\ge 1.25\times$ | **$1.884\times$** (+88.4% surge), **$2.211\times$** (+121.1% width) | **TRIGGERED** |

---

## 5. Real-Time CCTV Ingestion in the River Corridor

Drawing from our analysis of [`bilawalsidhu/gods-eye-view`](https://github.com/bilawalsidhu/gods-eye-view), terrestrial CCTV cameras deployed at strategic choke points provide immediate, sub-minute verification:

```text
Choke Point 1: Rasuwagadhi Border Bridge (Headwaters)
               │
               ▼ (Distance: 6.2 km)
Choke Point 2: Upper Trishuli-1 Hydropower Dam Intake
               │
               ▼ (Distance: 14.8 km)
Choke Point 3: Syaphrubesi Highway Bridge & River Gauge
```

### Camera Pose & Dynamic Water Level Tracking
Each CCTV sensor is calibrated with:
- **Intrinsics**: Focal length $f$, optical center $(c_x, c_y)$, radial distortion parameters.
- **Extrinsics**: Camera height $H_c$ above the dry riverbed, tilt angle $\theta_{\text{tilt}}$, azimuth $\phi_{\text{azimuth}}$.
- **Measurement**: Sub-pixel Steger edge detection tracks the waterline against concrete pier calibration markers, yielding continuous metric river stage ($h_{\text{water}}$ in meters) without physical submerged sensors.

---

## 6. AWS Cloud Agentic Loop & Evidence Dispatch

The perception output integrates directly with AWS CDK v2 cloud infrastructure (`aws_infra/`):

```text
[ C++ Perception Engine ]
           │
           ▼ Writes JSON Evidence
[ S3 Vault: s3://vp-evidence-vault-{id}/evidence/ ]
           │
           ▼ S3 ObjectCreated Event
[ AWS Lambda: getFloodEvidence Tool ]
           │
           ▼ Evaluates Evidence Vector
[ Amazon Bedrock Agent: Claude 3.5 Sonnet ]
           │
           ├── If Flood Surge >= 1.10x AND Channel Width >= 1.25x
           │   ├── Verify Spatial Control Passed (< 1.10x)
           │   └── Check DynamoDB Idempotency Table
           │           │
           │           ▼ [ dispatchFloodAlert Tool ]
           │       Dispatches SNS Alert to Disaster Management Authority
           │
           └── If Thresholds NOT Met
               └── Log Audit Trail to S3 (No Alert Triggered)
```

---

## 7. 3D Tactical Digital Twin (God's Eye View / Cesium)

To enable human-in-the-loop decision-making, the perception results serialize into OGC GeoJSON and stream into a CesiumJS 3D globe:

1. **River Corridor Polyline Layer**: Extracted sub-pixel ridge centerlines rendered as glowing 3D vector paths clamped to terrain.
2. **Flood Inundation Polygon**: Extruded 3D water volume matching the calculated channel width $W(y)$ and river stage $h_{\text{water}}$.
3. **CCTV Viewshed Frustum**: 3D geometric cone projected from camera mount to ground, showing live camera coverage over the flooded gorge.
4. **Tactical Sensor Shader**: FLIR/Ironbow thermal shader to highlight water-sediment boundaries under low-visibility night operations.

---

## 8. Phased Development Plan

### Phase 1: CCTV Ingestion & Local Gauge Fusion (Immediate)
- Implement `adapters/cctv/CctvStreamAdapter.cpp` to ingest HLS and MJPEG feeds from highway/hydropower cameras.
- Implement watermark/pier edge tracking to calculate real-time river stage.
- Curate real-world CCTV flood fixtures in `data/fixtures/cctv_flood/`.

### Phase 2: All-Weather Sentinel-1 SAR Cloud Penetration
- Integrate automated Copernicus CDAS API client for Sentinel-1 GRD SAR scenes.
- Implement speckle filtering (Enhanced Lee filter) and dual-pol backscatter thresholding to detect water under monsoon cloud cover.

### Phase 3: Cloud Agentic Deployment & Graviton3 Optimization
- Deploy AWS CDK v2 stack (`aws_infra/`) with Bedrock Agentic Vision loop.
- Compile and benchmark `curv` engine on AWS Graviton3 ARM64 architecture (`docs/aws_benchmark_guide.md`).

### Phase 4: Full 3D Digital Twin Client
- Build lightweight CesiumJS tactical viewer compatible with GEV layer standards.
- Real-time WebSocket bridge pushing `flood_delta.json` updates to the 3D globe within 2 seconds of acquisition.
