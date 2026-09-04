# Universal Perception Evidence Schema Specification

> **Specification Version**: 1.0.0  
> **Status**: Approved Data Architecture Standard  
> **Namespace**: `vision::core::evidence` / `CurvEngine::Evidence`

---

## 1. Overview & Purpose

The `PerceptionEvidence` model is the canonical, immutable contract emitted by the `vision-perception` core engine. 

### Core Principles
1. **Domain Agnostic**: The evidence model contains strictly geometric, radiometric, topological, and telemetry measurements. It contains zero domain-specific terms (no palm lines, no poultry labels, no defect severity classifications).
2. **Deterministic & Serializable**: Must serialize losslessly to and from JSON for network transport, persistent storage, and offline evaluation.
3. **Sub-Pixel Coordinate System**: All spatial coordinates are normalized or floating-point pixel units referenced to the top-left origin $(0.0, 0.0)$ of the input raster frame.
4. **Extensible Domain Envelope**: Domain packs attach semantic interpretations inside an isolated `domain_payload` dictionary without altering the core schema.

---

## 2. JSON Schema Representation

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "PerceptionEvidence",
  "type": "object",
  "required": [
    "evidence_id",
    "frame_index",
    "timestamp_utc_ms",
    "frame_dimensions",
    "quality_assessment",
    "ridge_graph",
    "telemetry"
  ],
  "properties": {
    "evidence_id": {
      "type": "string",
      "format": "uuid",
      "description": "Unique identifier for this frame perception evidence"
    },
    "frame_index": {
      "type": "integer",
      "minimum": 0,
      "description": "Monotonically increasing sequence index of the processed frame"
    },
    "timestamp_utc_ms": {
      "type": "integer",
      "description": "Frame capture timestamp in milliseconds since Unix epoch"
    },
    "frame_dimensions": {
      "type": "object",
      "required": ["width", "height", "channels"],
      "properties": {
        "width": { "type": "integer", "minimum": 1 },
        "height": { "type": "integer", "minimum": 1 },
        "channels": { "type": "integer", "minimum": 1 }
      }
    },
    "quality_assessment": {
      "type": "object",
      "required": ["passed", "laplacian_variance", "illumination_mean", "contrast_ratio"],
      "properties": {
        "passed": { "type": "boolean" },
        "laplacian_variance": { "type": "number", "minimum": 0.0 },
        "illumination_mean": { "type": "number", "minimum": 0.0, "maximum": 255.0 },
        "contrast_ratio": { "type": "number", "minimum": 0.0 },
        "rejection_reasons": {
          "type": "array",
          "items": { "type": "string" }
        }
      }
    },
    "ridge_graph": {
      "type": "object",
      "required": ["segments", "junctions"],
      "properties": {
        "segments": {
          "type": "array",
          "items": {
            "type": "object",
            "required": ["id", "points", "total_length", "average_curvature", "is_closed"],
            "properties": {
              "id": { "type": "integer" },
              "total_length": { "type": "number", "minimum": 0.0 },
              "average_curvature": { "type": "number" },
              "is_closed": { "type": "boolean" },
              "points": {
                "type": "array",
                "items": {
                  "type": "object",
                  "required": ["x", "y", "intensity", "confidence"],
                  "properties": {
                    "x": { "type": "number" },
                    "y": { "type": "number" },
                    "intensity": { "type": "number" },
                    "confidence": { "type": "number", "minimum": 0.0, "maximum": 1.0 }
                  }
                }
              }
            }
          }
        },
        "junctions": {
          "type": "array",
          "items": {
            "type": "object",
            "required": ["position", "connected_segment_ids", "branch_angle"],
            "properties": {
              "position": {
                "type": "object",
                "required": ["x", "y"],
                "properties": {
                  "x": { "type": "number" },
                  "y": { "type": "number" }
                }
              },
              "connected_segment_ids": {
                "type": "array",
                "items": { "type": "integer" }
              },
              "branch_angle": { "type": "number" }
            }
          }
        }
      }
    },
    "domain_payload": {
      "type": "object",
      "description": "Optional container for domain-pack specific annotations",
      "additionalProperties": true
    },
    "telemetry": {
      "type": "object",
      "required": ["total_pipeline_us", "stages"],
      "properties": {
        "total_pipeline_us": { "type": "integer" },
        "stages": {
          "type": "object",
          "additionalProperties": { "type": "integer" }
        }
      }
    }
  }
}
```

---

## 3. Serialization Contract

1. **Precision**: Floating-point sub-pixel positions ($x, y$) must retain at least 4 decimal places when serialized to JSON.
2. **Coordinate Orientation**: Standard image coordinates where $(0.0, 0.0)$ is the upper-left corner of the raster image, $+X$ points rightward, $+Y$ points downward. Angles are expressed in radians in the range $[-\pi, \pi]$.
3. **Empty Collections**: When zero ridges or junctions are detected, the respective JSON arrays must be emitted as `[]` rather than `null`.
