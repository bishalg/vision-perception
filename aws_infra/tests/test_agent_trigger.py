"""Comprehensive Unit Tests for Two-Tool Bedrock Agent Action Group & Lambda Router.

Tests:
1. S3 Event Routing: Ingestion notification transmits ONLY eventId and S3 URI.
2. Tool 1 (getFloodEvidence): S3 evidence retrieval, evidenceToken issuance, DynamoDB PENDING write.
3. Tool 2 (dispatchFloodAlert): Token validation, SNS publish, S3 audit receipt write, DynamoDB DISPATCHED update.
4. Idempotency Gate: Duplicate alert dispatch returns ALREADY_DISPATCHED without double-publishing SNS.
5. Security Gate: Mismatched evidenceToken is rejected with HTTP 403.
"""

import io
import json
import os
import sys
import unittest
from unittest.mock import MagicMock, patch

sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "..")))
from agent import agent_trigger
from agent.agent_trigger import ClientError


class TestAgentTriggerTwoTool(unittest.TestCase):
    """Test suite for the two-tool Agentic Vision perception-decision-action loop."""

    def setUp(self):
        """Set up test fixtures and mocks."""
        if agent_trigger.s3_client is None:
            agent_trigger.s3_client = MagicMock()
        if agent_trigger.dynamodb_client is None:
            agent_trigger.dynamodb_client = MagicMock()
        if agent_trigger.bedrock_agent_runtime is None:
            agent_trigger.bedrock_agent_runtime = MagicMock()
        if agent_trigger.sns_client is None:
            agent_trigger.sns_client = MagicMock()

        self.sample_delta = {
            "event": "Glacier-collapse flood — Lende Khola / Bhote Koshi, Rasuwa, Nepal",
            "channel_width_proxy": {"width_ratio": 2.2105},
            "color_mask_delta": {"combined_flood_signature": {"ratio": 1.8838}},
            "spatial_control": {"passes_acceptance": True, "control_ratio": 0.1509},
            "assessment": {
                "flood_signal_detected": True,
                "primary_ratio_fired": True,
                "width_ratio_fired": True,
            },
        }

    def test_unrecognized_event(self):
        """Unrecognized event format returns 400."""
        res = agent_trigger.handler({}, None)
        self.assertEqual(res["statusCode"], 400)

    def test_s3_non_delta_file_ignored(self):
        """Files not ending with flood_delta.json are ignored."""
        event = {
            "Records": [
                {
                    "s3": {
                        "bucket": {"name": "test-data-bucket"},
                        "object": {"key": "imagery/post_20260827_bgr.png"},
                    }
                }
            ]
        }
        res = agent_trigger.handler(event, None)
        self.assertEqual(res["statusCode"], 200)
        body = json.loads(res["body"])
        self.assertIn("Ignored", body["message"])

    @patch.object(agent_trigger, "invoke_bedrock_agent")
    def test_s3_event_routes_only_event_id(self, mock_invoke):
        """S3 event notification sends prompt containing ONLY eventId and URI, not pre-digested metrics."""
        mock_invoke.return_value = {"statusCode": 200, "body": json.dumps({"status": "invoked"})}

        event = {
            "Records": [
                {
                    "s3": {
                        "bucket": {"name": "test-data-bucket"},
                        "object": {"key": "events/lende-khola-2026-08-26/flood_delta.json"},
                    }
                }
            ]
        }
        res = agent_trigger.handler(event, None)
        self.assertEqual(res["statusCode"], 200)
        mock_invoke.assert_called_once()
        call_event_id, call_prompt = mock_invoke.call_args[0]
        self.assertEqual(call_event_id, "lende-khola-2026-08-26")
        self.assertIn("getFloodEvidence", call_prompt)
        self.assertNotIn("2.2105", call_prompt)  # Ensures agent receives no pre-digested metrics

    @patch.object(agent_trigger, "s3_client")
    @patch.object(agent_trigger, "dynamodb_client")
    def test_tool_get_flood_evidence(self, mock_ddb, mock_s3):
        """Tool 1 (getFloodEvidence) fetches evidence from S3, registers token in DynamoDB, and returns schema."""
        raw_bytes = json.dumps(self.sample_delta).encode("utf-8")
        mock_stream = MagicMock()
        mock_stream.read.return_value = raw_bytes
        mock_s3.get_object.return_value = {"Body": mock_stream}

        event = {
            "messageVersion": "1.0",
            "actionGroup": "FloodDisasterActionGroup",
            "apiPath": "/getFloodEvidence",
            "httpMethod": "POST",
            "parameters": [{"name": "eventId", "value": "lende-khola-2026-08-26"}],
        }

        res = agent_trigger.handler(event, None)
        self.assertEqual(res["messageVersion"], "1.0")
        self.assertEqual(res["response"]["httpStatusCode"], 200)

        body = json.loads(res["response"]["responseBody"]["application/json"]["body"])
        self.assertEqual(body["eventId"], "lende-khola-2026-08-26")
        self.assertTrue(body["evidenceToken"].startswith("tok-"))
        self.assertTrue(body["floodSignalDetected"])
        self.assertAlmostEqual(body["channelWidthRatio"], 2.2105)
        self.assertTrue(body["spatialControlPasses"])

        # Verify DynamoDB recorded PENDING status with evidenceToken
        mock_ddb.put_item.assert_called_once()
        put_item = mock_ddb.put_item.call_args[1]["Item"]
        self.assertEqual(put_item["alertStatus"]["S"], "PENDING")
        self.assertEqual(put_item["evidenceToken"]["S"], body["evidenceToken"])

    @patch.object(agent_trigger, "dynamodb_client")
    @patch.object(agent_trigger, "sns_client")
    @patch.object(agent_trigger, "s3_client")
    def test_tool_dispatch_flood_alert_success(self, mock_s3, mock_sns, mock_ddb):
        """Tool 2 (dispatchFloodAlert) validates token, publishes to SNS, writes audit receipt, updates DynamoDB."""
        token = "tok-valid-test-token-1234"
        # Mock DynamoDB existing record with matching token and PENDING status
        mock_ddb.get_item.return_value = {
            "Item": {
                "eventId": {"S": "lende-khola-2026-08-26"},
                "evidenceToken": {"S": token},
                "alertStatus": {"S": "PENDING"},
            }
        }
        mock_sns.publish.return_value = {"MessageId": "msg-sns-test-9999"}

        event = {
            "messageVersion": "1.0",
            "actionGroup": "FloodDisasterActionGroup",
            "apiPath": "/dispatchFloodAlert",
            "httpMethod": "POST",
            "parameters": [
                {"name": "eventId", "value": "lende-khola-2026-08-26"},
                {"name": "evidenceToken", "value": token},
                {"name": "severity", "value": "CRITICAL"},
                {"name": "channelWidthRatio", "value": "2.211x"},
                {"name": "decisionReason", "value": "Channel widening +121.1% exceeds disaster threshold."},
            ],
        }

        with patch.object(agent_trigger, "SNS_TOPIC_ARN", "arn:aws:sns:us-east-1:123456789012:test-topic"):
            res = agent_trigger.handler(event, None)
            self.assertEqual(res["response"]["httpStatusCode"], 200)

            body = json.loads(res["response"]["responseBody"]["application/json"]["body"])
            self.assertEqual(body["status"], "ALERT_DISPATCHED")
            self.assertEqual(body["snsMessageId"], "msg-sns-test-9999")
            self.assertIn("s3://", body["actionReceiptUri"])

            # Verify SNS published
            mock_sns.publish.assert_called_once()
            # Verify Audit Receipt saved to S3 audit bucket
            mock_s3.put_object.assert_called_once()
            # Verify DynamoDB marked DISPATCHED
            mock_ddb.update_item.assert_called_once()

    @patch.object(agent_trigger, "dynamodb_client")
    @patch.object(agent_trigger, "sns_client")
    def test_tool_dispatch_flood_alert_idempotency(self, mock_sns, mock_ddb):
        """Tool 2 Idempotency Gate: When already DISPATCHED, returns ALREADY_DISPATCHED without duplicate SNS publish."""
        token = "tok-valid-test-token-1234"
        mock_ddb.get_item.return_value = {
            "Item": {
                "eventId": {"S": "lende-khola-2026-08-26"},
                "evidenceToken": {"S": token},
                "alertStatus": {"S": "DISPATCHED"},
                "snsMessageId": {"S": "msg-previously-sent"},
                "actionReceiptUri": {"S": "s3://vision-perception-audit/receipt.json"},
            }
        }

        event = {
            "messageVersion": "1.0",
            "actionGroup": "FloodDisasterActionGroup",
            "apiPath": "/dispatchFloodAlert",
            "httpMethod": "POST",
            "parameters": [
                {"name": "eventId", "value": "lende-khola-2026-08-26"},
                {"name": "evidenceToken", "value": token},
                {"name": "severity", "value": "CRITICAL"},
                {"name": "channelWidthRatio", "value": "2.211x"},
                {"name": "decisionReason", "value": "Duplicate retry attempt."},
            ],
        }

        res = agent_trigger.handler(event, None)
        self.assertEqual(res["response"]["httpStatusCode"], 200)

        body = json.loads(res["response"]["responseBody"]["application/json"]["body"])
        self.assertEqual(body["status"], "ALREADY_DISPATCHED")
        self.assertEqual(body["snsMessageId"], "msg-previously-sent")

        # Crucial check: Zero duplicate SNS messages published
        mock_sns.publish.assert_not_called()

    @patch.object(agent_trigger, "dynamodb_client")
    def test_tool_dispatch_flood_alert_invalid_token(self, mock_ddb):
        """Security Gate: Mismatched evidenceToken is rejected with HTTP 403."""
        mock_ddb.get_item.return_value = {
            "Item": {
                "eventId": {"S": "lende-khola-2026-08-26"},
                "evidenceToken": {"S": "tok-authentic-token"},
                "alertStatus": {"S": "PENDING"},
            }
        }

        event = {
            "messageVersion": "1.0",
            "actionGroup": "FloodDisasterActionGroup",
            "apiPath": "/dispatchFloodAlert",
            "httpMethod": "POST",
            "parameters": [
                {"name": "eventId", "value": "lende-khola-2026-08-26"},
                {"name": "evidenceToken", "value": "tok-tampered-or-fake"},
                {"name": "severity", "value": "CRITICAL"},
                {"name": "channelWidthRatio", "value": "2.211x"},
                {"name": "decisionReason", "value": "Tampered token attempt."},
            ],
        }

        res = agent_trigger.handler(event, None)
        self.assertEqual(res["response"]["httpStatusCode"], 403)
        body = json.loads(res["response"]["responseBody"]["application/json"]["body"])
        self.assertIn("Invalid evidenceToken", body["error"])

    def test_tool_dispatch_missing_required_parameters(self):
        """Missing required eventId or evidenceToken returns HTTP 400."""
        event = {
            "messageVersion": "1.0",
            "actionGroup": "FloodDisasterActionGroup",
            "apiPath": "/dispatchFloodAlert",
            "httpMethod": "POST",
            "parameters": [{"name": "severity", "value": "CRITICAL"}],
        }

        res = agent_trigger.handler(event, None)
        self.assertEqual(res["response"]["httpStatusCode"], 400)


if __name__ == "__main__":
    unittest.main()
