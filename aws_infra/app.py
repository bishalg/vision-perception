#!/usr/bin/env python3
"""AWS CDK App Entrypoint for Nepal Flood Disaster Monitoring Architecture.

Coordinates four modular stacks:
1. StorageStack — Primary S3 data bucket, dedicated audit S3 bucket, and DynamoDB event store.
2. AlertStack — SNS topic, email subscription, and SQS fallback queue.
3. IAMStack — Least-privilege roles for Lambda trigger and Bedrock Agent.
4. AgentStack — Bedrock Agent (Claude 3.5 Sonnet v2) with two-tool Action Group and Lambda event router.
"""

import os
import aws_cdk as cdk
from stacks import StorageStack, AlertStack, IAMStack, AgentStack

app = cdk.App()

# Default to us-east-1 for Bedrock Anthropic Claude 3.5 Sonnet v2 availability
aws_env = cdk.Environment(
    account=os.environ.get("CDK_DEFAULT_ACCOUNT", os.environ.get("AWS_ACCOUNT_ID")),
    region=os.environ.get("CDK_DEFAULT_REGION", os.environ.get("AWS_DEFAULT_REGION", "us-east-1")),
)

# 1. Storage Stack (Data bucket, Audit bucket, DynamoDB events table)
storage_stack = StorageStack(
    app,
    "NepalFloodStorageStack",
    description="S3 data/audit storage and DynamoDB event table for Nepal flood monitoring",
    env=aws_env,
)

# 2. Alert Stack (SNS topic, email subscription, SQS fallback queue)
alert_stack = AlertStack(
    app,
    "NepalFloodAlertStack",
    description="SNS emergency topic and SQS fallback queue for Nepal flood disaster response",
    env=aws_env,
)

# 3. IAM Stack (Least-privilege roles with DynamoDB, S3, SQS, Bedrock permissions)
iam_stack = IAMStack(
    app,
    "NepalFloodIAMStack",
    storage_bucket=storage_stack.bucket,
    audit_bucket=storage_stack.audit_bucket,
    events_table=storage_stack.events_table,
    alert_topic=alert_stack.topic,
    alert_queue=alert_stack.alert_queue,
    description="Least-privilege IAM roles for Lambda ingestion trigger and Bedrock Agent",
    env=aws_env,
)
iam_stack.add_dependency(storage_stack)
iam_stack.add_dependency(alert_stack)

# 4. Agent Stack (Bedrock Agent with getFloodEvidence + dispatchFloodAlert tools)
agent_stack = AgentStack(
    app,
    "NepalFloodAgentStack",
    storage_bucket=storage_stack.bucket,
    audit_bucket=storage_stack.audit_bucket,
    events_table=storage_stack.events_table,
    alert_topic=alert_stack.topic,
    alert_queue=alert_stack.alert_queue,
    lambda_role=iam_stack.lambda_role,
    bedrock_agent_role=iam_stack.bedrock_agent_role,
    description="Bedrock Agentic Vision coordinator and S3 flood_delta.json Lambda trigger",
    env=aws_env,
)
agent_stack.add_dependency(iam_stack)

app.synth()
