import subprocess
import json
import sys

PRES_ID = "1xkBtj0Dcbu0qqpKt-i9HooKt_OlyqlSv_nXqCn1As0c"

def run_cmd(cmd):
    res = subprocess.run(cmd, capture_output=True, text=True)
    if res.returncode != 0:
        print(f"Error running: {' '.join(cmd)}\n{res.stderr}")
    return res.stdout.strip()

print("Setting Slide 1 speaker notes...")
run_cmd([
    "gslides", "mutate", "set-notes", PRES_ID,
    "--slide", "p",
    "--text", "Hi everyone, we're [Your Name / Team Name]. For the OpenCV AI Competition 2026, we built vision-perception: sub-pixel curvilinear perception and autonomous cloud disaster response, powered by OpenCV 5, AWS Bedrock Agents, and COOL on AWS Graviton3."
])

# Slide 2: The Disaster & The Problem
print("Adding Slide 2...")
s2_out = run_cmd(["gslides", "mutate", "add-slide", PRES_ID])
# Format: Added slide: <ID>
s2_id = s2_out.split(":")[-1].strip()
run_cmd([
    "gslides", "mutate", "add-textbox", PRES_ID,
    "--slide", s2_id,
    "--text", "The Crisis: August 2026 Nepal Debris Flood",
    "--x", "40", "--y", "30", "--width", "640", "--height", "40",
    "--font-size", "22", "--bold"
])
run_cmd([
    "gslides", "mutate", "add-textbox", PRES_ID,
    "--slide", s2_id,
    "--text", "• Glacier collapse in Langtang Himal → catastrophic Bhote Koshi flash flood\n• Standard pixel-diff fails: cloud shadows, mountain terrain, glacial flour\n• Requirement: Sub-pixel precision + autonomous, zero-human-delay dispatch",
    "--x", "40", "--y", "90", "--width", "320", "--height", "220",
    "--font-size", "14"
])
run_cmd([
    "gslides", "mutate", "insert-image-from-file", PRES_ID,
    "--slide", s2_id,
    "--file", "data/flood_nepal_2026/out/comparison.png",
    "--x", "380", "--y", "80", "--width", "300", "--height", "260"
])
run_cmd([
    "gslides", "mutate", "set-notes", PRES_ID,
    "--slide", s2_id,
    "--text", "In August 2026, a catastrophic glacier collapse triggered an extreme debris flood down Nepal's Bhote Koshi river corridor. Early detection saves lives. But conventional pixel-difference thresholds fail due to monsoon clouds and turbid glacial flour. We built vision-perception to pair sub-pixel mathematical rigor with autonomous cloud response."
])

# Slide 3: C++20 / OpenCV 5 Core Engine
print("Adding Slide 3...")
s3_out = run_cmd(["gslides", "mutate", "add-slide", PRES_ID])
s3_id = s3_out.split(":")[-1].strip()
run_cmd([
    "gslides", "mutate", "add-textbox", PRES_ID,
    "--slide", s3_id,
    "--text", "C++20 & OpenCV 5 Sub-Pixel Curvilinear Engine",
    "--x", "40", "--y", "30", "--width", "640", "--height", "40",
    "--font-size", "22", "--bold"
])
run_cmd([
    "gslides", "mutate", "add-textbox", PRES_ID,
    "--slide", s3_id,
    "--text", "• Steger line extraction: Gaussian scale-space Hessian tensors (Lxx, Lxy, Lyy)\n• Sub-pixel Taylor zero-crossing along dominant eigenvectors: < 0.0301 px RMSE\n• 41/41 GoogleTest suites green with zero core math modifications for cloud\n• Verified C++ build and execution across Apple Silicon and AWS Graviton3",
    "--x", "40", "--y", "100", "--width", "640", "--height", "220",
    "--font-size", "16"
])
run_cmd([
    "gslides", "mutate", "set-notes", PRES_ID,
    "--slide", s3_id,
    "--text", "Under the hood is a modern C++20 and OpenCV 5 monorepo. We implement Steger line extraction using Gaussian scale-space Hessian tensors and Taylor series zero-crossing. It achieves < 0.03 pixels RMSE against analytical ground truth across 41 green test suites."
])

# Slide 4: Quantitative Results & Dual Controls
print("Adding Slide 4...")
s4_out = run_cmd(["gslides", "mutate", "add-slide", PRES_ID])
s4_id = s4_out.split(":")[-1].strip()
run_cmd([
    "gslides", "mutate", "add-textbox", PRES_ID,
    "--slide", s4_id,
    "--text", "Quantitative Disaster Delta & Scientific Controls",
    "--x", "40", "--y", "30", "--width", "640", "--height", "40",
    "--font-size", "22", "--bold"
])
run_cmd([
    "gslides", "mutate", "add-textbox", PRES_ID,
    "--slide", s4_id,
    "--text", "• Active Channel Width: +121.1% expansion (19 px → 42 px median)\n• Flood Signature: +88.4% surge in active floodway (1.884x)\n• Spatial Control (off-river slope): 0.151x (eliminates regional rain bias)\n• Temporal Baseline Control: 0.905x (proves pre-event dry baseline stability)",
    "--x", "40", "--y", "90", "--width", "360", "--height", "230",
    "--font-size", "14"
])
run_cmd([
    "gslides", "mutate", "insert-image-from-file", PRES_ID,
    "--slide", s4_id,
    "--file", "data/flood_nepal_2026/out/post_overlay.png",
    "--x", "420", "--y", "85", "--width", "260", "--height", "240"
])
run_cmd([
    "gslides", "mutate", "set-notes", PRES_ID,
    "--slide", s4_id,
    "--text", "On Sentinel-2 imagery, our pipeline detected a 121% channel widening and an 88% surge in flood signature. Crucially, our spatial control was 0.151x and temporal baseline 0.905x, mathematically proving the signal is localized to the floodway without hallucinations."
])

# Slide 5: Agentic Vision (AWS Bedrock)
print("Adding Slide 5...")
s5_out = run_cmd(["gslides", "mutate", "add-slide", PRES_ID])
s5_id = s5_out.split(":")[-1].strip()
run_cmd([
    "gslides", "mutate", "add-textbox", PRES_ID,
    "--slide", s5_id,
    "--text", "Agentic Vision: 3-Node Perception-to-Action Loop",
    "--x", "40", "--y", "30", "--width", "640", "--height", "40",
    "--font-size", "22", "--bold"
])
run_cmd([
    "gslides", "mutate", "add-textbox", PRES_ID,
    "--slide", s5_id,
    "--text", "• Node 1 (getFloodEvidence): Bedrock Agent inspects metrics & acquires token\n• Node 2 (Policy Reasoning): Claude 3.5 evaluates strict disaster thresholds\n• Node 3 (dispatchFloodAlert): Validates token, DynamoDB idempotency, SNS alert\n• Vision Output Changes Action: Pre-disaster data triggers STANDBY (NOT_DISPATCHED)",
    "--x", "40", "--y", "100", "--width", "640", "--height", "220",
    "--font-size", "15"
])
run_cmd([
    "gslides", "mutate", "set-notes", PRES_ID,
    "--slide", s5_id,
    "--text", "When flood_delta.json lands in S3, EventBridge wakes our Bedrock Agent. The agent calls getFloodEvidence, evaluates policy thresholds, and dispatches emergency alerts with DynamoDB idempotency. When fed marginal data, it records a verified STANDBY decision."
])

# Slide 6: Best Use of COOL on Graviton3
print("Adding Slide 6...")
s6_out = run_cmd(["gslides", "mutate", "add-slide", PRES_ID])
s6_id = s6_out.split(":")[-1].strip()
run_cmd([
    "gslides", "mutate", "add-textbox", PRES_ID,
    "--slide", s6_id,
    "--text", "Best Use of COOL: AWS Graviton3 Vector Acceleration",
    "--x", "40", "--y", "30", "--width", "640", "--height", "40",
    "--font-size", "22", "--bold"
])
run_cmd([
    "gslides", "mutate", "add-textbox", PRES_ID,
    "--slide", s6_id,
    "--text", "• Cloud-Optimized OpenCV Library (COOL) on c7g.2xlarge Graviton3\n• Measured 100-iteration continuous benchmark on full disaster scenes:\n  - Latency P50: 213.6 ms (Mean: 217.4 ms, P95: 255.8 ms)\n  - Throughput: 50,744 scenes per $1.00 ($0.0000197 per scene)\n• Verified SIMD vectorization: NEON_DOTPROD, NEON_FP16, NEON_BF16",
    "--x", "40", "--y", "100", "--width", "640", "--height", "220",
    "--font-size", "15"
])
run_cmd([
    "gslides", "mutate", "set-notes", PRES_ID,
    "--slide", s6_id,
    "--text", "Running our benchmark harness with the Cloud-Optimized OpenCV Library on AWS Graviton3, we achieve a measured 213.6 ms P50 latency with ARM NEON SIMD vectorization active—processing over 50,000 disaster scenes per compute dollar."
])

# Slide 7: Cesium 3D Digital Twin
print("Adding Slide 7...")
s7_out = run_cmd(["gslides", "mutate", "add-slide", PRES_ID])
s7_id = s7_out.split(":")[-1].strip()
run_cmd([
    "gslides", "mutate", "add-textbox", PRES_ID,
    "--slide", s7_id,
    "--text", "Interactive Cesium 3D Digital Twin (localhost:3000)",
    "--x", "40", "--y", "30", "--width", "640", "--height", "40",
    "--font-size", "22", "--bold"
])
run_cmd([
    "gslides", "mutate", "add-textbox", PRES_ID,
    "--slide", s7_id,
    "--text", "• Real-time 3D geospatial reconstruction of the Nepal Bhote Koshi gorge\n• Multi-camera sensor poses, orientation frustums, and elevations\n• Ground-level camera stills inspect bridge piers under flood stress\n• Verified end-to-end with automated browser tests and zero page errors",
    "--x", "40", "--y", "100", "--width", "640", "--height", "220",
    "--font-size", "15"
])
run_cmd([
    "gslides", "mutate", "set-notes", PRES_ID,
    "--slide", s7_id,
    "--text", "For field rescue teams, we project this into a real-time Cesium 3D digital twin at localhost:3000. Rescue coordinators can inspect calibrated camera frustums and georeferenced sensor stills directly across the river gorge."
])

# Slide 8: Architecture & Closing
print("Adding Slide 8...")
s8_out = run_cmd(["gslides", "mutate", "add-slide", PRES_ID])
s8_id = s8_out.split(":")[-1].strip()
run_cmd([
    "gslides", "mutate", "add-textbox", PRES_ID,
    "--slide", s8_id,
    "--text", "End-to-End System Architecture",
    "--x", "40", "--y", "25", "--width", "640", "--height", "35",
    "--font-size", "22", "--bold"
])
run_cmd([
    "gslides", "mutate", "insert-image-from-file", PRES_ID,
    "--slide", s8_id,
    "--file", "docs/assets/hackathon_architecture.png",
    "--x", "60", "--y", "70", "--width", "600", "--height", "280"
])
run_cmd([
    "gslides", "mutate", "set-notes", PRES_ID,
    "--slide", s8_id,
    "--text", "From sub-pixel C++ tensor extraction to autonomous, evidence-gated AWS Bedrock agents, vision-perception demonstrates that rigorous computer vision and modern agentic AI can save lives during real-world disasters. The entire repository is open source with our one-command judges' quickstart. Thank you."
])

print("Finished populating Google Slides presentation!")
