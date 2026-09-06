# AWS Cloud Infrastructure — Nepal Flood Disaster Monitoring

> **Project**: `vision-perception`  
> **Milestone**: M8 — AWS Cloud Architecture & Special Award Targets  
> **Competition Track**: Real-World Impact & Special Awards (COOL, Agentic Vision)  
> **IaC Framework**: AWS CDK v2 (Python 3.12+)

---

## 1. Overview

This directory contains the complete Infrastructure-as-Code (IaC) definitions for deploying an autonomous, event-driven flood disaster evaluation and alerting pipeline on AWS.

```
+-------------------------------------------------------------------------------+
|                             Perception Engine (C++20)                         |
|     Sentinel-2 L2A (CDAS)  -->  curv_cli  -->  compute_flood_delta.py        |
+-------------------------------------------------------------------------------+
                                      |
                                      v (Upload flood_delta.json)
+-------------------------------------------------------------------------------+
|                       NepalFloodStorageStack (Amazon S3)                      |
|                  Bucket: vision-perception-flood-data                         |
+-------------------------------------------------------------------------------+
                                      |
                                      v (S3 ObjectCreated Event)
+-------------------------------------------------------------------------------+
|                        NepalFloodAgentStack (AWS Lambda)                      |
|                 Handler: agent_trigger.py (FloodDeltaAgentTrigger)            |
+-------------------------------------------------------------------------------+
                                      |
                                      v (bedrock-agent-runtime:InvokeAgent)
+-------------------------------------------------------------------------------+
|                    Amazon Bedrock Agent (Claude 3.5 Sonnet v2)                |
|           Evaluates: flood_signal == true AND width_ratio >= 2.0              |
+-------------------------------------------------------------------------------+
                                      |
                                      v (Action Group: publish_flood_alert)
+-------------------------------------------------------------------------------+
|                        NepalFloodAlertStack (Amazon SNS)                      |
|                       Topic: flood-disaster-alerts                            |
|             Subscribers: NEOC Nepal, Local Hydropower, Email Dispatch         |
+-------------------------------------------------------------------------------+
```

---

## 2. CDK Stack Topology

| Stack | File | Key Resources |
|---|---|---|
| `NepalFloodStorageStack` | [`stacks/storage_stack.py`](stacks/storage_stack.py) | S3 Bucket with HTTPS policy, versioning, CORS, 90-day IA transition. |
| `NepalFloodAlertStack` | [`stacks/alert_stack.py`](stacks/alert_stack.py) | SNS Topic with email subscription (`SNS_ALERT_EMAIL`). |
| `NepalFloodIAMStack` | [`stacks/iam_stack.py`](stacks/iam_stack.py) | Least-privilege roles for Lambda execution & Bedrock agent invocation. |
| `NepalFloodAgentStack` | [`stacks/agent_stack.py`](stacks/agent_stack.py) | Bedrock Agent (`anthropic.claude-3-5-sonnet-20241022-v2:0`), Action Group, Lambda trigger. |

---

## 3. Prerequisites

- **AWS CLI v2**: Configured with credentials and target region (`us-east-1` recommended for Claude 3.5 Sonnet v2 availability).
  ```bash
  aws configure
  ```
- **Node.js**: v18+ (for `npx cdk` CLI runner).
- **Python**: 3.10+ (Python 3.12 recommended).

---

## 4. Setup & Local Synthesis (No Cloud Resources Created)

1. Create and activate a Python virtual environment:
   ```bash
   cd aws_infra
   python3 -m venv .venv
   source .venv/bin/activate
   pip install -r requirements-cdk.txt
   ```

2. Synthesize CloudFormation templates locally (validates syntax and dependency graph without touching AWS):
   ```bash
   npx cdk synth
   ```

3. Review the generated CloudFormation templates in `cdk.out/`.

---

## 5. Deployment (Requires AWS Credentials)

> [!IMPORTANT]
> Deployment incurs minimal AWS charges. Ensure you have appropriate permissions and active credentials before proceeding.

1. **Bootstrap CDK Environment** (one-time per account/region):
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

4. **Confirm Email Subscription**:
   Check the inbox of the configured email address and confirm the AWS SNS subscription.

---

## 6. Testing the Agentic Vision Loop

Once deployed, you can verify the end-to-end autonomous perception-decision-action loop by uploading the verified `flood_delta.json` from the Nepal disaster:

```bash
BUCKET_NAME=$(aws cloudformation describe-stacks \
  --stack-name NepalFloodStorageStack \
  --query "Stacks[0].Outputs[?ExportName=='FloodDataBucketName'].OutputValue" \
  --output text)

# Upload the verified delta artifact
aws s3 cp ../data/flood_nepal_2026/out/flood_delta.json \
  s3://${BUCKET_NAME}/events/2026-08-26/flood_delta.json
```

### Expected Behavior:
1. S3 fires `ObjectCreated` event to `FloodDeltaAgentTrigger` Lambda.
2. Lambda reads the delta (`flood_signal_detected: true`, `width_ratio: 2.211`).
3. Bedrock Agent (Claude 3.5 Sonnet v2) ingests the report and reasons that both disaster thresholds are breached (+88.4% flood surge, +121.1% river widening).
4. Bedrock Agent calls `FloodAlertActionGroup` tool (`publish_flood_alert`).
5. Lambda dispatches an emergency notification to SNS topic `flood-disaster-alerts`.
6. Email notification arrives with georeferenced coordinates and severity assessment.

---

## 7. Cost Estimate & Teardown

### Cost Estimate:
- **S3**: Storage < 1 GB (~$0.02/month).
- **Lambda**: Free tier eligible (~$0.00).
- **SNS**: Free tier eligible (~$0.00).
- **Bedrock Agent**: Claude 3.5 Sonnet v2 (~$0.003 per evaluation invocation).
- **Total test cost**: < $0.10.

### Clean Teardown:
To remove all AWS resources created by CDK:
```bash
npx cdk destroy --all
```
