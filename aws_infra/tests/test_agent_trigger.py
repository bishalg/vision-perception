"""Unit tests for aws_infra.agent.agent_trigger Lambda handler.

Uses unittest.mock to mock boto3 S3, Bedrock Agent Runtime, and SNS clients.
"""

import io
import json
import unittest
from unittest.mock import MagicMock, patch
import sys
import os

sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "..")))
from agent import agent_trigger
from agent.agent_trigger import ClientError

try:
    from botocore.response import StreamingBody
except ImportError:
    class StreamingBody:
        """Mock StreamingBody for local test execution without botocore."""
        def __init__(self, raw_stream, content_length):
            self._raw_stream = raw_stream
            self._content_length = content_length

        def read(self):
            return self._raw_stream.read()


class TestAgentTrigger(unittest.TestCase):
    """Test suite for S3 ingestion, validation, and Bedrock Action Group execution."""

    def setUp(self):
        """Set up test fixtures."""
        self.sample_delta = {
            "event": "Glacier-collapse flood — Lende Khola / Bhote Koshi, Nepal",
            "channel_width_proxy": {
                "width_ratio": 2.2105,
                "pre_median_px": 19.0,
                "post_median_px": 42.0,
            },
            "spatial_control": {
                "control_ratio": 0.1509,
                "passes_acceptance": True,
            },
            "assessment": {
                "flood_signal_detected": True,
                "primary_ratio_fired": True,
                "width_ratio_fired": True,
                "spatial_control_verified": True,
            },
        }
        if agent_trigger.s3_client is None:
            agent_trigger.s3_client = MagicMock()
        if agent_trigger.bedrock_agent_runtime is None:
            agent_trigger.bedrock_agent_runtime = MagicMock()
        if agent_trigger.sns_client is None:
            agent_trigger.sns_client = MagicMock()

    def test_unrecognized_event(self):
        """Unrecognized event format returns 400."""
        res = agent_trigger.handler({}, None)
        self.assertEqual(res["statusCode"], 400)

    def test_s3_non_delta_file_ignored(self):
        """Files not ending with flood_delta.json are safely ignored."""
        event = {
            "Records": [
                {
                    "s3": {
                        "bucket": {"name": "test-bucket"},
                        "object": {"key": "imagery/post_20260827_bgr.png"},
                    }
                }
            ]
        }
        res = agent_trigger.handler(event, None)
        self.assertEqual(res["statusCode"], 200)
        body = json.loads(res["body"])
        self.assertIn("Ignored", body["message"])

    @patch.object(agent_trigger, "s3_client")
    def test_s3_event_success_dry_run(self, mock_s3):
        """When BEDROCK_AGENT_ID is not configured, S3 event returns 200 dry run."""
        raw_bytes = json.dumps(self.sample_delta).encode("utf-8")
        mock_body = StreamingBody(io.BytesIO(raw_bytes), len(raw_bytes))
        mock_s3.get_object.return_value = {"Body": mock_body}

        event = {
            "Records": [
                {
                    "s3": {
                        "bucket": {"name": "test-bucket"},
                        "object": {"key": "events/2026-08-26/flood_delta.json"},
                    }
                }
            ]
        }

        with patch.object(agent_trigger, "BEDROCK_AGENT_ID", ""):
            res = agent_trigger.handler(event, None)
            self.assertEqual(res["statusCode"], 200)
            body = json.loads(res["body"])
            self.assertTrue(body["flood_signal_detected"])
            self.assertAlmostEqual(body["width_ratio"], 2.2105)

    @patch.object(agent_trigger, "bedrock_agent_runtime")
    @patch.object(agent_trigger, "s3_client")
    def test_s3_event_invokes_bedrock_agent(self, mock_s3, mock_bedrock):
        """When BEDROCK_AGENT_ID is set, invokes Bedrock Agent and parses chunks."""
        raw_bytes = json.dumps(self.sample_delta).encode("utf-8")
        mock_body = StreamingBody(io.BytesIO(raw_bytes), len(raw_bytes))
        mock_s3.get_object.return_value = {"Body": mock_body}

        # Mock Bedrock streaming response
        mock_bedrock.invoke_agent.return_value = {
            "completion": [
                {"chunk": {"bytes": b"Critical alert published: "}},
                {"chunk": {"bytes": b"Widening ratio 2.211x confirmed."}},
            ]
        }

        event = {
            "Records": [
                {
                    "s3": {
                        "bucket": {"name": "test-bucket"},
                        "object": {"key": "events/2026-08-26/flood_delta.json"},
                    }
                }
            ]
        }

        with patch.object(agent_trigger, "BEDROCK_AGENT_ID", "TESTAGENTID"):
            res = agent_trigger.handler(event, None)
            self.assertEqual(res["statusCode"], 200)
            body = json.loads(res["body"])
            self.assertIn("Critical alert published", body["agent_response"])

    @patch.object(agent_trigger, "s3_client")
    def test_s3_client_error(self, mock_s3):
        """S3 ClientError is propagated."""
        mock_s3.get_object.side_effect = ClientError(
            {"Error": {"Code": "NoSuchKey", "Message": "The specified key does not exist."}},
            "GetObject",
        )
        event = {
            "Records": [
                {
                    "s3": {
                        "bucket": {"name": "test-bucket"},
                        "object": {"key": "missing/flood_delta.json"},
                    }
                }
            ]
        }
        with self.assertRaises(ClientError):
            agent_trigger.handler(event, None)

    @patch.object(agent_trigger, "sns_client")
    def test_action_group_execution(self, mock_sns):
        """Action group execution parses parameters and dispatches alert via SNS."""
        mock_sns.publish.return_value = {"MessageId": "msg-12345-abc"}

        action_event = {
            "messageVersion": "1.0",
            "actionGroup": "FloodAlertActionGroup",
            "apiPath": "/publish-flood-alert",
            "httpMethod": "POST",
            "parameters": [
                {"name": "severity", "value": "CRITICAL"},
                {"name": "channel_width_ratio", "value": "2.211x"},
                {"name": "location", "value": "Lende Khola, Nepal"},
                {"name": "message", "value": "Glacier collapse detected."},
            ],
        }

        with patch.object(agent_trigger, "SNS_TOPIC_ARN", "arn:aws:sns:us-east-1:123456789012:test-topic"):
            res = agent_trigger.handler(action_event, None)
            self.assertEqual(res["messageVersion"], "1.0")
            self.assertEqual(res["response"]["httpStatusCode"], 200)
            body = json.loads(res["response"]["responseBody"]["application/json"]["body"])
            self.assertEqual(body["status"], "ALERT_PUBLISHED")
            self.assertEqual(body["sns_message_id"], "msg-12345-abc")
            mock_sns.publish.assert_called_once()

    def test_direct_payload_invocation(self):
        """Direct payload invocation routes to invoke_bedrock_agent."""
        with patch.object(agent_trigger, "BEDROCK_AGENT_ID", ""):
            res = agent_trigger.handler(self.sample_delta, None)
            self.assertEqual(res["statusCode"], 200)
            body = json.loads(res["body"])
            self.assertTrue(body["flood_signal_detected"])


if __name__ == "__main__":
    unittest.main()
