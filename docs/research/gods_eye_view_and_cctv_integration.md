# Technical Research: God's Eye View & Critical CCTV Ingestion Pipeline

> **Document ID**: `DOC-RES-004-GEV-CCTV`  
> **Target Repository**: `vision-perception`  
> **Origin Reference**: [`bilawalsidhu/gods-eye-view`](https://github.com/bilawalsidhu/gods-eye-view) by Bilawal Sidhu  
> **Status**: Approved for Architecture Review & Phase 1 Ingestion  
> **Priority Area**: **Public CCTV Video & Snapshot Stream Ingestion (Top Priority)**  

---

## 1. Executive Summary & Context

`bilawalsidhu/gods-eye-view` (GEV) is an open-source browser application built with **CesiumJS, Vite, and vanilla JavaScript/WebGL**. It aggregates heterogeneous live spatial intelligence (CCTV camera networks, ADS-B flights, AIS maritime vessels, CelesTrak satellite orbits, NASA FIRMS active fires, and USGS earthquakes) into a photorealistic 3D globe with military-style tactical HUD shaders (FLIR, NVG, CRT).

In the **`vision-perception`** monorepo, our core capability is a **domain-agnostic C++20 / OpenCV sub-pixel curvilinear ridge extraction engine** (`core/`) grounded in differential geometry, scale-space Hessian tensor decomposition, and immutable evidence generation (`evidence.json`).

### High-Level Synergy
```text
┌────────────────────────────────────────────────────────────────────────┐
│                        God's Eye View (GEV)                            │
│  • Public Data Ingestion Layer (CCTV, Esri Satellite, NASA FIRMS, AIS) │
│  • 3D Camera Pose, Calibration Priors & Viewshed Geometry              │
│  • Photorealistic 3D Digital Twin & Sensor Shaders (FLIR, NVG)         │
└──────────────────┬────────────────────────────────▲────────────────────┘
                   │ Live Frames (MJPEG/HLS/JPEG)   │ GeoJSON 3D Vector Ridges,
                   │ Calibration Intrinsic/Extrinsic│ Detections, Flood Bounds
┌──────────────────▼────────────────────────────────┴────────────────────┐
│                       vision-perception Monorepo                       │
│  • C++20 / OpenCV Steger Sub-Pixel Differential Geometry Core          │
│  • Quality Gates (Laplacian Blur, Contrast, Valid Alpha Norm)          │
│  • Domain Packs: `domains/geospatial`, `domains/surface_inspection`,   │
│    `domains/traffic` (Planned: lane detection, vehicle dwell, crack)   │
│  • AWS Bedrock Multi-Modal Agentic Vision Loop                         │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Deep Dive: CCTV Pipeline & Footage Extraction (Top Priority)

Public Closed-Circuit Television (CCTV) cameras deployed by municipal and highway Departments of Transportation (DOT) provide uncurated, continuous real-world imagery. For `vision-perception`, CCTV feeds represent the critical data source for testing sub-pixel edge extraction, road lane geometry, pavement crack detection, vehicle flow, and low-light quality degradation.

### 2.1. CCTV Feeds Discovered & Utilized in GEV

GEV interfaces with three major open municipal camera ecosystems. None of these require private enterprise licenses or payment:

| Agency / Network | Coverage | Stream Protocol / Format | Update Rate | Public Endpoint / API Pattern |
| :--- | :--- | :--- | :--- | :--- |
| **Caltrans (California DOT)** | California Highways (I-5, I-80, US-101, Bay Area, LA) | Direct static JPEG snapshots & HLS `.m3u8` streams | 30–60 seconds (snapshots), 30 fps (HLS) | State JSON catalog (`cwwp2.dot.ca.gov/data/d{district}/cctv/cctvStatusD{district}.json`) + direct image URLs |
| **City of Austin Mobility** | Austin, Texas urban corridors and signalized intersections | Static JPEG snapshots & HTTP video | Real-time / 15–30s | City of Austin Socrata Open Data Portal (`data.austintexas.gov/resource/b4k4-adkb.json`) |
| **Transport for London (TfL)** | Greater London major roads & intersections (JamCams) | Direct JPEG snapshots & `.mp4` video clips | 1–2 minutes (snapshots), 10s video loops | TfL Unified API (`api.tfl.gov.uk/Place/Type/JamCam`) |

---

### 2.2. CCTV Camera Schema & Data Structure

GEV normalizes each CCTV sensor into a unified metadata structure containing geodetic coordinates, mounting elevation, and viewing angles:

```json
{
  "id": "cctv_caltrans_d3_i80_drum_forebay",
  "name": "I-80 at Drum Forebay",
  "agency": "Caltrans District 3",
  "location": {
    "latitude": 39.3142,
    "longitude": -120.7321,
    "elevation_m": 1420.0
  },
  "pose": {
    "heading_deg": 65.0,
    "pitch_deg": -15.0,
    "roll_deg": 0.0,
    "fov_horizontal_deg": 55.0,
    "range_m": 350.0
  },
  "media": {
    "snapshot_url": "https://cwwp2.dot.ca.gov/data/d3/cctv/image/i80atdrumforebay/i80atdrumforebay.jpg",
    "video_hls_url": "https://cwwp2.dot.ca.gov/data/d3/cctv/hls/i80atdrumforebay/i80atdrumforebay.m3u8",
    "is_active": true,
    "last_updated_epoch": 1725883200
  }
}
```

---

### 2.3. Architectural Technology: 3D Camera Pose & Viewshed Frustum

One of the most valuable geometric techniques developed in GEV is the **camera viewshed**:
1. **Camera Extrinsics**: $\mathbf{P}_w = [\text{lat}, \text{lon}, \text{alt}]^T$, and Euler orientation matrix $\mathbf{R} = \mathbf{R}_z(\text{heading}) \mathbf{R}_y(\text{pitch}) \mathbf{R}_x(\text{roll})$.
2. **Camera Intrinsics**: Horizontal field-of-view $\theta_H$, aspect ratio $r = W/H$, and maximum observation range $d_{\max}$.
3. **Frustum Polyhedron in World Space**:
   - Near apex at $\mathbf{P}_w$.
   - 4 far-plane vertices computed via ray casting through the terrain mesh.
4. **Significance for `vision-perception`**:
   - Enables **2D-to-3D back-projection**: When our C++ Steger engine extracts a sub-pixel line candidate $(u, v)$ in pixel coordinates, we can raycast using the camera matrix $\mathbf{K}$ and pose $[\mathbf{R} | \mathbf{t}]$ to obtain real-world metric coordinates on the ground plane.

---

### 2.4. CCTV Ingestion Implementation Patterns for `vision-perception`

#### Pattern A: Real-Time Snapshot Poller & Quality Gate (Python / C++)
Automates pulling frames at regular intervals, filtering degraded images using the core Laplacian blur gate:

```python
import time
import urllib.request
import cv2
import numpy as np

class PublicCCTVStreamReader:
    def __init__(self, snapshot_url: str, min_laplacian_var: float = 80.0):
        self.snapshot_url = snapshot_url
        self.min_laplacian_var = min_laplacian_var
        self.headers = {"User-Agent": "Mozilla/5.0 (vision-perception/1.0)"}

    def fetch_latest_frame(self) -> np.ndarray:
        req = urllib.request.Request(self.snapshot_url, headers=self.headers)
        with urllib.request.urlopen(req, timeout=8) as resp:
            data = resp.read()
            arr = np.asarray(bytearray(data), dtype=np.uint8)
            frame = cv2.imdecode(arr, cv2.IMREAD_COLOR)
            return frame

    def is_frame_valid(self, frame: np.ndarray) -> bool:
        if frame is None or frame.size == 0:
            return False
        # Match core/src/quality/LaplacianBlurGate.cpp
        gray = cv2.cvtColor(frame, cv2.COLOR_BGR2GRAY)
        lap_var = cv2.Laplacian(gray, cv2.CV_64F).var()
        return lap_var >= self.min_laplacian_var

# Usage Example:
reader = PublicCCTVStreamReader("https://cwwp2.dot.ca.gov/data/d3/cctv/image/i80atdrumforebay/i80atdrumforebay.jpg")
frame = reader.fetch_latest_frame()
if reader.is_frame_valid(frame):
    cv2.imwrite("data/cctv_frame_latest.png", frame)
    # Trigger C++ Steger Ridge CLI:
    # ./build/tools/curv_cli --input data/cctv_frame_latest.png --config configs/cctv_road_ridge.json
```

#### Pattern B: Live RTSP / HLS Stream Capture via OpenCV `cv::VideoCapture`
For municipal cameras that offer HLS (`.m3u8`) or RTSP endpoints:

```cpp
#include <opencv2/opencv.hpp>
#include <iostream>

int main(int argc, char** argv) {
    std::string hls_url = "https://cwwp2.dot.ca.gov/data/d3/cctv/hls/sample/sample.m3u8";
    cv::VideoCapture cap(hls_url, cv::CAP_FFMPEG);

    if (!cap.isOpened()) {
        std::cerr << "Failed to open live video stream: " << hls_url << std::endl;
        return 1;
    }

    cv::Mat frame;
    while (cap.read(frame)) {
        if (frame.empty()) break;

        // Pass frame to Steger Ridge Pipeline
        // curv::pipeline::ImagePipeline pipeline(config);
        // auto evidence = pipeline.process(frame);
    }
    return 0;
}
```

---

## 3. Other Feeds & Technologies Developed in God's Eye View

In addition to CCTV, GEV integrates multiple live spatial intelligence layers. Below is the comprehensive technical breakdown:

### 3.1. Optical Sensor Shaders (GLSL Post-Processing)
GEV implements full-screen post-processing fragment shaders using WebGL/Cesium custom stages:

1. **FLIR / Ironbow Thermal Simulation**:
   - Converts RGB luminance to radiometric pseudo-temperature $T = 0.299R + 0.587G + 0.114B$.
   - Samples a 1D Ironbow color lookup table (black $\to$ deep purple $\to$ orange $\to$ yellow $\to$ white).
   - High utility for `vision-perception`: Can be used to synthetically stress-test our ridge extraction on low-contrast thermal boundaries.
2. **NVG (Night Vision Goggle)**:
   - Green phosphor emission LUT ($\lambda \approx 530\text{ nm}$).
   - High-frequency per-pixel temporal noise + dynamic vignetting and horizontal raster scanlines.
3. **Detection Bounding Box Projection**:
   - Computes screen-space bounding boxes around 3D tracked objects, similar to tactical target designations.

### 3.2. ADS-B Flight Tracking & 3D Cockpit Controller
- **Feeds**: OpenSky Network REST API (`opensky-network.org/api/states/all`) and `adsb.lol` crowdsourced feed.
- **Data Payload**: ICAO 24-bit address, callsign, latitude, longitude, barometric altitude, velocity, track heading, vertical rate.
- **Cockpit Mode Logic**: Interpolates smooth 6-DOF camera positions along the aircraft's velocity vector, maintaining ground clearance above digital elevation models (DEM).

### 3.3. AIS Maritime Vessel Tracking
- **Feeds**: AISStream WebSocket & AISHub open feed.
- **Data Payload**: MMSI, ship name, IMO number, navigational status, speed over ground (SOG), course over ground (COG), destination.
- **Synergy**: Testing harbor boundary extraction, shoreline wake tracking, and maritime perimeter security.

### 3.4. Satellite Orbital Tracking
- **Feeds**: CelesTrak TLE (Two-Line Element sets).
- **Propagation**: Real-time SGP4 / SDP4 orbital perturbation propagator (`satellite.js`).
- **Synergy**: Predicts the exact overpass window for Sentinel-2, Landsat, or PlanetScope satellites over target areas (e.g., Bhote Koshi, Nepal) to automate post-disaster data acquisition.

### 3.5. Natural Disaster Feeds (NASA FIRMS & USGS)
- **NASA FIRMS (Fire Information for Resource Management System)**:
  - Near-real-time thermal anomaly vectors from MODIS and VIIRS satellites.
  - Returns GeoJSON / CSV with brightness temperature, fire radiative power (FRP), and confidence.
- **USGS Earthquakes**:
  - Global real-time seismic event feed updated every minute (`earthquake.usgs.gov/earthquakes/feed/v1.0/summary/all_hour.geojson`).
  - Magnitude, hypocenter depth, shake map bounds.

---

## 4. Integration Roadmap for `vision-perception`

### Phase 1: CCTV Ingestion & Benchmarking (Immediate)
- [ ] Create `tools/cctv_fetcher`: Lightweight CLI to pull periodic frames from Caltrans and Austin open camera endpoints.
- [ ] Build `data/fixtures/cctv/`: Curate a benchmark dataset of 50 day/night traffic frames to test Steger ridge robustness on lane lines, curbs, and asphalt cracks.
- [ ] Implement `adapters/cctv/`: Adapter providing standardized frame acquisition with automatic retry, HTTP timeout handling, and Laplacian blur validation.

### Phase 2: Camera Calibration & 2D-to-3D Projection (Medium-Term)
- [ ] Adopt GEV's viewshed parameterization (heading, pitch, elevation, FOV) into a C++ spatial geometry adapter `curv::geometry::CameraPose`.
- [ ] Map detected sub-pixel ridge centerlines from image plane $(u, v)$ to world geodetic coordinates $(X, Y, Z)$ using planar homography / DEM raycasting.

### Phase 3: Bi-directional 3D Digital Twin Loop (Future)
- [ ] Output `vision-perception` evidence in GeoJSON format (`LineString` and `Polygon` with sub-pixel metric coordinates).
- [ ] Ingest the generated GeoJSON into GEV / CesiumJS to render detected flood channels, building perimeters, or road hazards in a browser-based 3D digital twin.

---

## 5. Architectural Notes & References for Future Research

- **CesiumJS 3D Tiles 1.1 Specification**: [OGC 3D Tiles Standard](https://www.ogc.org/standard/3dtiles/) — Hierarchical spatial data structure for streaming massive 3D photorealistic meshes.
- **SGP4 Satellite Orbit Propagation**: [CelesTrak SGP4 Documentation](https://celestrak.org/software/sgp4.php) — Mathematical models for calculating orbital positions from TLE vectors.
- **Caltrans Automated CCTV API**: [Caltrans CWWP2 Data Gateway](https://cwwp2.dot.ca.gov) — District status JSON manifests and public camera snapshot endpoints.
- **City of Austin Open Data**: [Austin Transportation & Public Works Cameras](https://data.austintexas.gov/Transportation/Traffic-Cameras/b4k4-adkb) — Open Socrata REST endpoints.
- **Transport for London Unified API**: [TfL API JamCam Documentation](https://api.tfl.gov.uk) — Free registration API for London urban transit monitoring.
- **NASA FIRMS Live Fire Data**: [NASA Earthdata FIRMS](https://firms.modaps.eosdis.nasa.gov/api/) — VIIRS/MODIS active wildfire coordinates.
- **Steger Curvilinear Differential Geometry**: Steger, C. (1998). *An Unbiased Detector of Curvilinear Structures*. IEEE TPAMI, 20(2), 113–125.
