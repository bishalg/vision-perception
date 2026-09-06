# AWS Cloud Infrastructure — Nepal Flood Disaster Monitoring

> **Project**: `vision-perception`  
> **Milestone**: M8 — AWS Cloud Architecture & Special Award Targets  
> **Competition Track**: Real-World Impact & Special Awards (COOL, Agentic Vision)  
> **IaC Framework**: AWS CDK v2 (Python 3.12+)

---

## 1. Overview

This directory contains the complete Infrastructure-as-Code (IaC) definitions for deploying an autonomous, event-driven flood disaster evaluation and alerting pipeline on AWS.

It implements a genuine **3-node Agentic Vision loop** (retrieve evidence $\to$ reason over policy $\to$ dispatch alert) with server-side idempotency and dedicated audit storage:

```
+-------------------------------------------------------------------------------+
|                             Perception Engine (C++20)                         |
|     Sentinel-2 L2A (CDAS)  -->  curv_cli  -->  compute_flood_delta.py        |
+-------------------------------------------------------------------------------+
                                      |
                                      v (Upload events/<eventId>/flood_delta.json)
+-------------------------------------------------------------------------------+
|                 NepalFloodStorageStack (Primary S3 Data Bucket)               |
|                   Bucket: vision-perception-flood-data                        |
+-------------------------------------------------------------------------------+
                                      |
                                      v (S3 ObjectCreated Event: prefix "events/")
+-------------------------------------------------------------------------------+
|                        NepalFloodAgentStack (AWS Lambda)                      |
|                Handler: agent_trigger.py (FloodDeltaAgentTrigger)             |
+-------------------------------------------------------------------------------+
                                      |
                                      v (InvokeAgent passing ONLY eventId & S3 URI)
+-------------------------------------------------------------------------------+
|                   Amazon Bedrock Agent (Claude 3.5 Sonnet v2)                 |
|             [1. Perception]: Calls getFloodEvidence(eventId)                  |
|             [2. Decision]: Evaluates flood_signal & width_ratio >= 2.0        |
|             [3. Action]: Calls dispatchFloodAlert(eventId, evidenceToken)     |
+-------------------------------------------------------------------------------+
                    |                                           |
                    v (DynamoDB Idempotency)                    v (Audit & Broadcast)
+---------------------------------------+   +---------------------------------------+
|   DynamoDB Table: vision-perception   |   |   NepalFloodStorageStack              |
|   - PENDING -> DISPATCHED state       |   |   - Dedicated Audit S3 Bucket         |
|   - Validates evidenceToken           |   |     vision-perception-audit           |
|   - Eliminates duplicate alerts       |   |                                       |
+---------------------------------------+   |   NepalFloodAlertStack                |
                                            |   - SNS Topic: flood-disaster-alerts  |
                                            |   - SQS Queue: alerts-queue (fallback)|
                                            |   - Email: human notifications        |
                                            +---------------------------------------+
```

---

## 2. CDK Stack Topology

| Stack | File | Key Resources |
|---|---|---|
| `NepalFloodStorageStack` | [`stacks/storage_stack.py`](stacks/storage_stack.py) | Primary S3 data bucket, dedicated S3 audit bucket (`vision-perception-audit`), and DynamoDB events table (`vision-perception-events-dev`). |
| `NepalFloodAlertStack` | [`stacks/alert_stack.py`](stacks/alert_stack.py) | SNS Topic with email subscription and SQS fallback queue (`flood-disaster-alerts-queue`). |
| `NepalFloodIAMStack` | [`stacks/iam_stack.py`](stacks/iam_stack.py) | Least-privilege roles for Lambda execution (S3, DynamoDB, SNS, SQS, Bedrock) and Bedrock model invocation. |
| `NepalFloodAgentStack` | [`stacks/agent_stack.py`](stacks/agent_stack.py) | Bedrock Agent (Claude 3.5 Sonnet v2), two-tool Action Group (`getFloodEvidence` + `dispatchFloodAlert`), Lambda router. |

---

## 3. Two-Tool Agentic Perception-Decision-Action Flow

1. **Perception (`getFloodEvidence`)**:
   - Agent is invoked with `eventId`. It must call `getFloodEvidence` to inspect evidence.
   - Backend reads `flood_delta.json` from S3, registers event in DynamoDB with `alertStatus: "PENDING"`, and issues an `evidenceToken`.
   - Returns flood metrics, channel width ratio, and spatial controls.
2. **Decision (Reasoning)**:
   - Agent inspects indicators: `flood_signal_detected == true`, `width_ratio >= 2.0`, `spatial_control_passes == true`.
3. **Action (`dispatchFloodAlert`)**:
   - Agent calls `dispatchFloodAlert` with `evidenceToken`.
   - Backend validates token and checks DynamoDB idempotency: if already `DISPATCHED`, returns `ALREADY_DISPATCHED` without duplicate SNS publish.
   - Publishes alert to SNS (fanning out to email and SQS queue).
   - Writes immutable action receipt to `vision-perception-audit` S3 bucket.
   - Updates DynamoDB to `DISPATCHED`.

---

## 4. Prerequisites

- **AWS CLI v2**: Configured with credentials and target region (`us-east-1` recommended for Claude 3.5 Sonnet v2 availability).
  ```bash
  aws configure
  ```
- **Node.js**: v18+ (for `npx cdk` CLI runner).
- **Python**: 3.10+ (Python 3.12 recommended).

---

## 5. Setup & Local Synthesis (No Cloud Resources Created)

1. Create and activate a Python virtual environment:
   ```bash
   cd aws_infra
   python3 -m venv .venv
   source .venv/bin/activate
   pip install -r requirements-cdk.txt
   ```

2. Run local unit tests (mocked offline, no AWS credentials required):
   ```bash
   python3 -m unittest discover -s tests -v
   ```

3. Synthesize CloudFormation templates locally:
   ```bash
   npx cdk synth
   ```

---

## 6. Deployment (Requires AWS Credentials)

1. **Bootstrap CDK Environment**:
   ```bash
   npx cdk bootstrap aws://<ACCOUNT_ID>/us-east-1
   ```

2. **Configure Alert Email** (Optional, defaults to `alerts@example.com`):
   ```bash
   export SNS_ALERT_EMAIL="your-email@example.com"
   ```

3. **Deploy All Stacks**:
   ```bash
   npx cdk deploy --all
   ```

---

## 7. Testing the Two-Tool Agentic Vision Loop

Once deployed, upload `flood_delta.json` under the `events/` prefix:

```bash
DATA_BUCKET=$(aws cloudformation describe-stacks \
  --stack-name NepalFloodStorageStack \
  --query "Stacks[0].Outputs[?ExportName=='FloodDataBucketName'].OutputValue" \
  --output text)

# Upload the verified delta artifact
aws s3 cp ../data/flood_nepal_2026/out/flood_delta.json \
  s3://${DATA_BUCKET}/events/lende-khola-2026-08-26/flood_delta.json
```

### Expected Behavior & Verification:
1. S3 fires `ObjectCreated` event to `FloodDeltaAgentTrigger` Lambda.
2. Lambda invokes Bedrock Agent passing ONLY `eventId: 'lende-khola-2026-08-26'`.
3. Bedrock Agent calls `getFloodEvidence` tool $\to$ receives `evidenceToken` and metrics (+88.4% flood surge, +121.1% river widening).
4. Bedrock Agent evaluates thresholds and calls `dispatchFloodAlert` with the `evidenceToken`.
5. Lambda validates token, publishes emergency alert to SNS (and SQS fallback queue), writes receipt to S3 audit bucket, and marks DynamoDB `DISPATCHED`.
6. Email notification arrives in inbox, or poll the SQS queue programmatically:
   ```bash
   QUEUE_URL=$(aws cloudformation describe-stacks \
     --stack-name NepalFloodAlertStack \
     --query "Stacks[0].Outputs[?ExportName=='FloodAlertQueueUrl'].OutputValue" \
     --output text)
   aws sqs receive-message --queue-url "${QUEUE_URL}"
   ```

---

## 8. Cost Estimate & Teardown

### Cost Breakdown:
- **S3 (Data & Audit)**: < 1 GB (~$0.03/month).
- **DynamoDB**: `PAY_PER_REQUEST` (~$0.00 for hackathon evaluation).
- **Lambda**: Free tier eligible (~$0.00).
- **SNS & SQS**: Free tier eligible (~$0.00).
- **Bedrock Agent**: Claude 3.5 Sonnet v2 (~$0.005 per two-tool evaluation).
- **Total test cost**: < $0.10.

### Clean Teardown:
```bash
npx cdk destroy --all
```
