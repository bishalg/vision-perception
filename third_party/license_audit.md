# Third-Party License Audit Log

> **Repository**: `vision-perception`  
> **Last Audit**: September 2026  
> **Auditor**: Core Architecture Team  

---

## Current Dependency Inventory

| Package Name | Version / Commit | License | Usage Location | Copyleft Risk | Status |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **OpenCV** | 4.x / 5.x | Apache-2.0 | `adapters/opencv/` | None (Permissive) | Approved (System / Package Manager) |
| **ONNX Runtime** | 1.18+ | MIT | `adapters/onnxruntime/` | None (Permissive) | Approved (Dynamic Link / C-API) |
| **ttk592/spline** | Master (latest) | MIT | `third_party/spline/` | None (Permissive) | Approved (Candidate Header) |
| **GoogleTest** | v1.14.0 | BSD-3-Clause | `core/tests/` | None (Permissive) | Approved (Test Harness Only) |

---

## Audit Checklist
- [x] Zero GPL / AGPL dependencies linked into core.
- [x] Zero non-commercial / research-only dependencies integrated.
- [x] All candidate libraries carry verified permissive licenses.
- [x] No proprietary secrets or hardcoded credentials present.
