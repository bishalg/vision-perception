# Geospatial Flood Domain Pack (`org.curv.domain.geospatial`)

Domain pack for satellite-based hydrological corridor and flood disaster perception:
interprets domain-agnostic `RidgeGraph` curvilinear segments extracted by `curv_core`
as river channels, braided tributaries, and drainage lines over Sentinel-2 imagery.

## Units

- **`GeospatialFloodPack`** (`include/curv/domains/GeospatialFloodPack.hpp`):
  Implements `IDomainPack`. Categorizes extracted curvilinear segments by arc length:
  - `river_channel`: continuous trunk features ($\ge 150$ px)
  - `tributary`: secondary river branches ($50\text{--}150$ px)
  - `minor_feature`: localized drainage gullies ($< 50$ px)
  Emits domain-specific `Evidence` items and an aggregate `domain_payload` JSON report.

## Architectural Invariant

The core (`core/`) contains strictly zero geospatial, hydrological, or remote sensing logic.
All sensor band semantics, classification thresholds, and disaster delta analytics reside in this domain pack.
