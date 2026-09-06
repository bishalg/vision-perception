"""AWS Lambda Handler for S3 Ingestion and Bedrock Agent Action Group Execution.

Handles two operational flows:
1. S3 Event Notification: Triggered when `flood_delta.json` is written to S3.
   Validates the payload, invokes the Amazon Bedrock Agentic Vision Coordinator,
   and streams reasoning traces.
2. Bedrock Action Group Execution: Invoked by Bedrock Agent to dispatch
   the verified disaster alert to Amazon SNS.
"""

import json
import logging
import os
import urllib.parse
import uuid
try:
    import boto3
    from botocore.exceptions import ClientError
except ImportError:  # pragma: no cover - fallback when running outside AWS Lambda / venv
    boto3 = None

    class ClientError(Exception):
        """Fallback ClientError exception."""
        def __init__(self, error_response=None, operation_name=None):
            self.response = error_response or {}
            self.operation_name = operation_name
            super().__init__(str(error_response))

logger = logging.getLogger()
logger.setLevel(logging.INFO)

if boto3:
    s3_client = boto3.client("s3")
    bedrock_agent_runtime = boto3.client("bedrock-agent-runtime")
    sns_client = boto3.client("sns")
else:
    s3_client = None
    bedrock_agent_runtime = None
    sns_client = None

BEDROCK_AGENT_ID = os.environ.get("BEDROCK_AGENT_ID", "")
BEDROCK_AGENT_ALIAS_ID = os.environ.get("BEDROCK_AGENT_ALIAS_ID", "TSTALIASID")
SNS_TOPIC_ARN = os.environ.get("SNS_TOPIC_ARN", "")


def handler(event, context):
    """Entry point for Lambda invocations."""
    logger.info("Received event: %s", json.dumps(event, default=str))

    # Flow A: Bedrock Action Group Tool Execution
    if "actionGroup" in event:
        return handle_action_group(event)

    # Flow B: S3 ObjectCreated Event Notification
    if "Records" in event and len(event["Records"]) > 0:
        return handle_s3_event(event)

    # Flow C: Direct JSON Payload / Test Invocation
    if "assessment" in event or "flood_signal_detected" in event:
        return invoke_bedrock_agent(event)

    logger.warning("Unrecognized event structure received.")
    return {
        "statusCode": 400,
        "body": json.dumps({"error": "Unrecognized event format"}),
    }


def handle_s3_event(event):
    """Parses S3 event, downloads flood_delta.json, and invokes Bedrock Agent."""
    record = event["Records"][0]
    bucket_name = record["s3"]["bucket"]["name"]
    object_key = urllib.parse.unquote_plus(
        record["s3"]["object"]["key"], encoding="utf-8"
    )

    logger.info("S3 event detected: s3://%s/%s", bucket_name, object_key)

    if not object_key.endswith("flood_delta.json"):
        logger.info("Ignoring non-delta file: %s", object_key)
        return {
            "statusCode": 200,
            "body": json.dumps({"message": f"Ignored {object_key}"}),
        }

    try:
        response = s3_client.get_object(Bucket=bucket_name, Key=object_key)
        payload_text = response["Body"].read().decode("utf-8")
        payload = json.loads(payload_text)
    except ClientError as e:
        logger.error("Failed to read S3 object: %s", e)
        raise
    except json.JSONDecodeError as e:
        logger.error("Failed to decode JSON from S3 object: %s", e)
        raise

    return invoke_bedrock_agent(payload)


def invoke_bedrock_agent(payload):
    """Validates delta metrics and invokes Bedrock Agent for decision reasoning."""
    # Basic schema validation
    assessment = payload.get("assessment", {})
    flood_signal = assessment.get(
        "flood_signal_detected", payload.get("flood_signal_detected", False)
    )
    channel_width = payload.get("channel_width_proxy", {})
    width_ratio = channel_width.get(
        "width_ratio", payload.get("width_ratio", 1.0)
    )

    logger.info(
        "Validated delta payload — flood_signal: %s, width_ratio: %.3f",
        flood_signal,
        width_ratio,
    )

    if not BEDROCK_AGENT_ID:
        logger.warning(
            "BEDROCK_AGENT_ID not configured; skipping agent invocation."
        )
        return {
            "statusCode": 200,
            "body": json.dumps(
                {
                    "message": "Bedrock Agent ID not configured, dry run successful",
                    "flood_signal_detected": flood_signal,
                    "width_ratio": width_ratio,
                }
            ),
        }

    session_id = f"flood-eval-{uuid.uuid4().hex[:12]}"
    prompt = (
        "New disaster observation available. Please evaluate the following flood_delta.json "
        "report and execute emergency alerting if critical criteria are met:\n\n"
        f"{json.dumps(payload, indent=2)}"
    )

    logger.info("Invoking Bedrock Agent %s with session %s", BEDROCK_AGENT_ID, session_id)

    try:
        response = bedrock_agent_runtime.invoke_agent(
            agentId=BEDROCK_AGENT_ID,
            agentAliasId=BEDROCK_AGENT_ALIAS_ID,
            sessionId=session_id,
            inputText=prompt,
            enableTrace=True,
        )

        agent_completion = []
        event_stream = response.get("completion", [])
        for evt in event_stream:
            if "chunk" in evt:
                chunk_text = evt["chunk"]["bytes"].decode("utf-8")
                agent_completion.append(chunk_text)
            elif "trace" in evt:
                logger.info("Agent trace: %s", json.dumps(evt["trace"], default=str))

        full_response = "".join(agent_completion)
        logger.info("Agent final response: %s", full_response)

        return {
            "statusCode": 200,
            "body": json.dumps(
                {
                    "session_id": session_id,
                    "agent_response": full_response,
                }
            ),
        }
    except ClientError as e:
        logger.error("Bedrock Agent invocation failed: %s", e)
        raise


def handle_action_group(event):
    """Executes the publish_flood_alert action called by the Bedrock Agent."""
    action_group = event.get("actionGroup", "")
    api_path = event.get("apiPath", "")
    http_method = event.get("httpMethod", "POST")

    logger.info("Executing action group: %s, path: %s", action_group, api_path)

    # Extract parameters
    params = {}
    for p in event.get("parameters", []):
        params[p.get("name")] = p.get("value")

    request_body = event.get("requestBody", {}).get("content", {}).get(
        "application/json", {}
    ).get("properties", [])
    for prop in request_body:
        params[prop.get("name")] = prop.get("value")

    message = params.get("message", "CRITICAL DISASTER ALERT: Glacier-collapse flood detected in Lende Khola corridor.")
    severity = params.get("severity", "CRITICAL")
    location = params.get("location", "Lende Khola / Bhote Koshi, Rasuwa, Nepal")
    ratio = params.get("channel_width_ratio", "2.211x")

    alert_subject = f"[{severity}] Nepal Flood Disaster Alert — {location}"
    alert_body = (
        f"NEPAL GLACIER-COLLAPSE FLOOD MONITORING SYSTEM\n"
        f"===============================================\n"
        f"Severity: {severity}\n"
        f"Location: {location}\n"
        f"Channel Widening: {ratio}\n"
        f"\nDetails:\n{message}\n"
        f"\nSource: Automated C++20 Curvilinear Perception Engine & AWS Bedrock Agent\n"
    )

    sns_message_id = "DRY_RUN_NO_SNS"
    if SNS_TOPIC_ARN:
        try:
            sns_res = sns_client.publish(
                TopicArn=SNS_TOPIC_ARN,
                Subject=alert_subject[:100],
                Message=alert_body,
            )
            sns_message_id = sns_res.get("MessageId", "unknown")
            logger.info("Dispatched SNS disaster alert: %s", sns_message_id)
        except ClientError as e:
            logger.error("Failed to publish SNS alert: %s", e)
            raise
    else:
        logger.warning("SNS_TOPIC_ARN not set, alert logged but not sent via SNS.")

    response_body = {
        "application/json": {
            "body": json.dumps(
                {
                    "status": "ALERT_PUBLISHED",
                    "sns_message_id": sns_message_id,
                    "location": location,
                    "severity": severity,
                }
            )
        }
    }

    action_response = {
        "messageVersion": "1.0",
        "response": {
            "actionGroup": action_group,
            "apiPath": api_path,
            "httpMethod": http_method,
            "httpStatusCode": 200,
            "responseBody": response_body,
        },
    }
    return action_response
