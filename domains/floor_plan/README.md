# Floor Plan & Vastu Domain Pack (`org.curv.domain.floor_plan`)

Interprets extracted sub-pixel curvilinear ridges as architectural walls, builds topological junction graphs, extracts closed room polygons, and applies the 9-pad Vastu Purusha Mandala for spatial and fixture compliance evaluation.

## Units

- **`WallSegmentExtractor`** (`include/curv/domain/WallSegmentExtractor.hpp`) — filters, rectifies, and merges sub-pixel ridges into straight Manhattan walls.
- **`WallJunctionGraph`** (`include/curv/domain/WallJunctionGraph.hpp`) — snaps endpoints and detects topological junctions ($L$-corners, $T$-partitions, $X$-crossings, and doorway endpoints).
- **`RoomPolygonDetector`** (`include/curv/domain/RoomPolygonDetector.hpp`) — traverses planar face cycles to identify closed rooms with exact metric area, perimeter, and centroid.
- **`VastuMandalaOverlay`** (`include/curv/domain/VastuMandalaOverlay.hpp`) — overlays the classical 9-pad Vastu Purusha Mandala oriented to true North, and evaluates architectural fixtures (beds, stoves, toilets, puja mandirs, entrances).
- **`FloorPlanDomainPack`** (`include/curv/domains/FloorPlanDomainPack.hpp`) — implements `IDomainPack`, orchestrating the pipeline and generating structured Evidence JSON + grounded Gemini VLM prompts.
