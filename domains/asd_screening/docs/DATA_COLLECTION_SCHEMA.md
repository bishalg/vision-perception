# Data Collection Schema: TrueDepth Multimodal Telemetry

> **Package**: `domains/asd_screening`  
> **Document**: `docs/DATA_COLLECTION_SCHEMA.md`  
> **Scope**: Complete specification of every raw data field collected per 60 Hz frame from Apple TrueDepth front camera via ARKit 6.  
> **Privacy Tier**: All fields are numeric time-series — zero raw camera frames, zero facial images, zero video stored to disk. COPPA / HIPAA compliant by design.

---

## 1. Overview: What the TrueDepth Camera Provides

The Apple TrueDepth sensor array (infrared dot projector + flood illuminator + front Bionic camera) provides three categories of numeric data per 60 Hz frame through `ARFaceAnchor`:

| Category | Source API | Hz | Fields | Privacy Risk |
| :--- | :--- | :--- | :--- | :--- |
| **Oculomotor Gaze** | `ARFaceAnchor.lookAtPoint` + eye transforms | 60 | 14 | None (numeric) |
| **Facial Affect (FACS)** | `ARFaceAnchor.blendShapes` | 60 | 52 | None (numeric) |
| **Head Pose / 6-DOF** | `ARFaceAnchor.transform` | 60 | 6 | None (numeric) |
| **Stimulus Sync** | `CADisplayLink` / `AVPlayer` | 60 | 3 | None |
| **Device & Quality** | `ARCamera` + `CMMotionManager` | 60 | 8 | None |
| **Session Metadata** | App-level | Once per session | 12 | Pseudonymized |

> [!IMPORTANT]
> **Total numeric fields per frame: 83**  
> **Storage rate (uncompressed JSONL): ~8.3 KB/s at 60 Hz**  
> **Typical 30-second session: ~249 KB raw, ~42 KB gzip-compressed**

---

## 2. Frame-Level Telemetry Schema (`GazeFrame`)

Every 60 Hz frame produces one `GazeFrame` record stored as a single JSONL row.

### 2.1 Oculomotor Gaze Fields

```json
{
  "gaze": {
    "timestamp_ms": 10420.5,
    "video_time_sec": 4.1203,
    "x_norm": 0.4215,
    "y_norm": 0.3180,
    "x_screen_pt": 346.6,
    "y_screen_pt": 375.2,
    "left_eye_x_norm": 0.4180,
    "left_eye_y_norm": 0.3160,
    "right_eye_x_norm": 0.4250,
    "right_eye_y_norm": 0.3200,
    "left_eye_openness": 0.97,
    "right_eye_openness": 0.96,
    "blink_detected": false,
    "confidence": 0.94
  }
}
```

| Field | Type | Range | Description |
| :--- | :--- | :--- | :--- |
| `timestamp_ms` | `Double` | 0.0 → ∞ | Monotonic device clock in ms since session start |
| `video_time_sec` | `Double` | 0.0 → stimulusDuration | Synchronized stimulus video presentation time |
| `x_norm` | `Double` | 0.0 → 1.0 | Combined gaze X on screen (0=left, 1=right) |
| `y_norm` | `Double` | 0.0 → 1.0 | Combined gaze Y on screen (0=top, 1=bottom) |
| `x_screen_pt` | `Double` | 0 → screenWidth | Physical screen X coordinate in points |
| `y_screen_pt` | `Double` | 0 → screenHeight | Physical screen Y coordinate in points |
| `left_eye_x_norm` | `Double` | 0.0 → 1.0 | Independent left eye screen X (binocular divergence) |
| `left_eye_y_norm` | `Double` | 0.0 → 1.0 | Independent left eye screen Y |
| `right_eye_x_norm` | `Double` | 0.0 → 1.0 | Independent right eye screen X |
| `right_eye_y_norm` | `Double` | 0.0 → 1.0 | Independent right eye screen Y |
| `left_eye_openness` | `Double` | 0.0 → 1.0 | Left eyelid aperture (0=closed, 1=fully open) |
| `right_eye_openness` | `Double` | 0.0 → 1.0 | Right eyelid aperture |
| `blink_detected` | `Bool` | — | Both eyes < 0.15 openness threshold |
| `confidence` | `Double` | 0.0 → 1.0 | ARKit face tracking confidence |

---

### 2.2 Head Pose / 6-DOF Fields

```json
{
  "head_pose": {
    "pitch_rad": -0.041,
    "yaw_rad": 0.082,
    "roll_rad": 0.015,
    "translation_x_m": 0.021,
    "translation_y_m": -0.014,
    "distance_to_screen_cm": 48.5
  }
}
```

| Field | Type | Range | Description |
| :--- | :--- | :--- | :--- |
| `pitch_rad` | `Double` | -π/2 → π/2 | Head tilt up/down (positive = looking up) |
| `yaw_rad` | `Double` | -π/2 → π/2 | Head turn left/right (positive = turned right) |
| `roll_rad` | `Double` | -π/2 → π/2 | Head lateral tilt (ear toward shoulder) |
| `translation_x_m` | `Double` | ~-0.3 → 0.3 | Face center horizontal offset from camera in meters |
| `translation_y_m` | `Double` | ~-0.3 → 0.3 | Face center vertical offset from camera in meters |
| `distance_to_screen_cm` | `Double` | 20 → 100 | Estimated face-to-screen distance from Z translation |

---

### 2.3 Facial Affect & FACS Blend Shapes (52 Fields)

Apple ARKit provides all 52 **Facial Action Coding System (FACS)** muscle coefficients per frame. All values lie in $[0.0, 1.0]$ — 0 is fully relaxed, 1 is maximum contraction.

#### Grouped by Emotion Detection

**Joy / Duchenne Social Smile (FACS AU6 + AU12)**
```json
{
  "affect": {
    "mouth_smile_left": 0.72,
    "mouth_smile_right": 0.70,
    "cheek_squint_left": 0.48,
    "cheek_squint_right": 0.45
  }
}
```
> 🔴 **Clinical relevance**: Measures positive social responsiveness — smiling at cartoon characters or responding to caregiver interaction.

**Surprise / High Visual Attention (FACS AU1+2+5+27)**
```json
{
  "affect": {
    "brow_inner_up": 0.61,
    "brow_outer_up_left": 0.55,
    "brow_outer_up_right": 0.58,
    "eye_wide_left": 0.44,
    "eye_wide_right": 0.42,
    "jaw_open": 0.28
  }
}
```
> 🔴 **Clinical relevance**: Captures visual alertness when a novel character or event appears. Low surprise response to social stimuli is a developmental marker.

**Distress / Negative Affect (FACS AU4+15+17)**
```json
{
  "affect": {
    "brow_down_left": 0.34,
    "brow_down_right": 0.31,
    "mouth_frown_left": 0.22,
    "mouth_frown_right": 0.19,
    "mouth_roll_lower": 0.15
  }
}
```
> 🔴 **Clinical relevance**: Detects sensory overwhelm, discomfort, or frustration — useful for session management and safety monitoring.

**Complete 52-Field Reference Table:**

| BlendShape Key | FACS AU | Emotion Group |
| :--- | :--- | :--- |
| `mouth_smile_left` / `right` | AU12 | Joy |
| `cheek_squint_left` / `right` | AU6 | Joy (Duchenne) |
| `brow_inner_up` | AU1 | Surprise / Concern |
| `brow_outer_up_left` / `right` | AU2 | Surprise |
| `eye_wide_left` / `right` | AU5 | Surprise / Attention |
| `jaw_open` | AU27 | Surprise / Vocalization |
| `brow_down_left` / `right` | AU4 | Distress / Confusion |
| `mouth_frown_left` / `right` | AU15 | Distress |
| `mouth_roll_lower` / `upper` | AU30 | Distress / Contempt |
| `mouth_pucker` | AU18 | Pout / Disapproval |
| `eye_look_down_left` / `right` | — | Oculomotor (downward) |
| `eye_look_in_left` / `right` | — | Convergence |
| `eye_look_out_left` / `right` | — | Divergence |
| `eye_look_up_left` / `right` | — | Oculomotor (upward) |
| `eye_squint_left` / `right` | AU44 | Concentration / Scrutiny |
| `eye_blink_left` / `right` | AU46 | Blink |
| `mouth_dimple_left` / `right` | AU14 | Mild smile |
| `mouth_funnel` | — | Vocalization shape |
| `mouth_left` / `right` | — | Lateral mouth movement |
| `mouth_lower_down_left` / `right` | AU16 | Distress / Cry prep |
| `mouth_press_left` / `right` | AU28 | Lip press / suppressed emotion |
| `mouth_shrug_lower` / `upper` | AU29 | Shrug |
| `mouth_stretch_left` / `right` | AU20 | Fear / extreme stretch |
| `mouth_upper_up_left` / `right` | AU10 | Disgust / Sneer |
| `nose_sneer_left` / `right` | AU9 | Disgust |
| `cheek_puff` | AU32 | Neutral / Exhale |
| `jaw_forward` / `left` / `right` | — | Jaw displacement |
| `tongue_out` | AU19 | Playful / Non-social |

---

### 2.4 AOI & Stimulus Synchronization Fields

```json
{
  "stimulus": {
    "active_aoi_names": ["Social_Actor"],
    "aoi_dwell_ms_current": 240.0,
    "stimulus_event_tag": "character_laugh_onset"
  }
}
```

| Field | Type | Description |
| :--- | :--- | :--- |
| `active_aoi_names` | `[String]` | AOI names where gaze currently falls |
| `aoi_dwell_ms_current` | `Double` | Cumulative ms of continuous fixation in current AOI |
| `stimulus_event_tag` | `String?` | Clinical event marker (e.g. `"character_laugh_onset"`) tied to affect-gaze synchrony analysis |

---

### 2.5 Device & Quality Gate Fields

```json
{
  "quality": {
    "verdict": "optimal",
    "face_tracked": true,
    "device_yaw_deg": 2.1,
    "device_pitch_deg": -1.4,
    "device_roll_deg": 0.8,
    "ambient_light_lux": 420.0,
    "frame_dropped": false,
    "calibration_offset_applied": true
  }
}
```

| Field | Type | Description |
| :--- | :--- | :--- |
| `verdict` | `String` | `optimal` / `acceptable` / `degraded` / `rejected` |
| `face_tracked` | `Bool` | ARKit has a valid face anchor this frame |
| `device_yaw_deg` | `Double` | Device IMU yaw (caregiver hand stability) |
| `device_pitch_deg` | `Double` | Device IMU pitch |
| `device_roll_deg` | `Double` | Device IMU roll |
| `ambient_light_lux` | `Double?` | ARCamera estimated ambient illuminance |
| `frame_dropped` | `Bool` | Processing lagged below 60 Hz this tick |
| `calibration_offset_applied` | `Bool` | Polynomial calibration correction active |

---

## 3. Session-Level Metadata Record (`SessionHeader`)

Written **once** as the first line of each session JSONL file:

```json
{
  "type": "session_header",
  "schema_version": "1.1.0",
  "session_id": "7F8B-C291-A4E0-9D12",
  "participant_token": "child-uuid-v4-no-pii",
  "stimulus_battery": "GeoPref_v1",
  "stimulus_duration_sec": 30.0,
  "calibration_quality": "validated",
  "calibration_error_deg": 1.2,
  "device_model": "iPhone16Pro",
  "screen_width_pt": 393.0,
  "screen_height_pt": 852.0,
  "screen_width_m": 0.0713,
  "screen_height_m": 0.1546,
  "arkit_version": "6.0",
  "app_version": "1.0.0",
  "session_start_utc": "2026-09-13T03:00:00Z"
}
```

> [!CAUTION]
> `participant_token` MUST be a random UUIDv4 with **no link** to child name, date of birth, or caregiver identity. The mapping table (token ↔ identity) must be maintained by the clinical coordinator **offline** and **never** stored on the same device.

---

## 4. Complete File Format (JSONL)

Each session produces a single **JSON Lines** (`.jsonl`) file:
- **Line 1**: `SessionHeader`
- **Lines 2 → N**: One `GazeFrame` per 60 Hz tick

```text
{"type":"session_header","schema_version":"1.1.0","session_id":"7F8B",...}
{"type":"gaze_frame","gaze":{...},"head_pose":{...},"affect":{...},"stimulus":{...},"quality":{...}}
{"type":"gaze_frame","gaze":{...},...}
```

### File Size Estimates (30-second session)

| Format | Size | Notes |
| :--- | :--- | :--- |
| Raw JSONL | ~249 KB | 1,800 frames × 138 bytes/frame |
| Gzip compressed | ~42 KB | ~83% compression ratio |
| AES-GCM-256 encrypted | ~42.1 KB | 16-byte IV + 16-byte auth tag |
| Binary MessagePack (future) | ~18 KB | Phase D5 optimization |

---

## 5. Derived Clinical Metrics (Computed at Session End)

All computed in-memory from the raw frame stream. No additional data is stored beyond the JSONL session file:

| Metric | Formula | Category |
| :--- | :--- | :--- |
| **Social Preference Score** | $(T_{\text{social}} - T_{\text{geo}}) / (T_{\text{social}} + T_{\text{geo}})$ | Gaze — ASD Biomarker 1 |
| **Eye Dwell Ratio** | $T_{\text{eye\_AOI}} / T_{\text{face\_AOI}}$ | Gaze — ASD Biomarker 2 |
| **Mouth Dwell Ratio** | $T_{\text{mouth\_AOI}} / T_{\text{face\_AOI}}$ | Gaze — ASD Biomarker 3 |
| **First-Eye Fixation Latency** | $t(\text{first eye AOI hit}) - t_0$ | Gaze — ASD Biomarker 4 |
| **Transition Entropy** | $H(\text{AOI transition matrix})$ | Gaze — ASD Biomarker 5 |
| **Recurrence Rate (DTW)** | Self-similarity of scanpath | Gaze — ASD Biomarker 6 |
| **Blink Rate** | Blinks per minute | Gaze — Cognitive Load |
| **Affect-Gaze Synchrony** | Cross-correlation: $\text{smile}(t) \times \mathbb{1}[\text{gaze} \in \text{social\_AOI}(t)]$ | Emotion — NEW Biomarker 1 |
| **Smile Reactivity Index** | $\Delta\text{smile\_intensity}$ within 800ms of stimulus event | Emotion — NEW Biomarker 2 |
| **Flat Affect Score** | $1 - \sigma^2(\text{all 52 blend shapes across session})$ | Emotion — NEW Biomarker 3 |
| **Surprise Engagement Index** | $\text{surpriseScore}(t)$ at novel stimulus events | Emotion — NEW Biomarker 4 |
| **Valid Sample Ratio** | Frames with `verdict ≠ rejected` / total frames | Quality Gate |

---

## 6. Schema Versioning

| Schema Version | Fields Added |
| :--- | :--- |
| `1.0.0` | Gaze + head_pose + quality + stimulus |
| `1.1.0` *(current)* | Add full 52-field `affect` + `GazeFrame` + 4 emotion biomarkers |
| `1.2.0` *(planned)* | Binocular vergence angle, pupil dilation estimate |
| `2.0.0` *(planned)* | Binary MessagePack encoding for ~75% storage reduction |
