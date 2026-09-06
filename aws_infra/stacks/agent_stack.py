"""Agent Stack for Nepal Flood Disaster Monitoring Architecture.

Deploys the Amazon Bedrock Agent (powered by Claude 3.5 Sonnet v2) with:
1. Two-Tool Action Group:
   - getFloodEvidence (Perception): Retrieves pipeline metrics and issues evidenceToken.
   - dispatchFloodAlert (Action): Validates evidenceToken and triggers emergency SNS alert.
2. Lambda Trigger: Driven by S3 flood_delta.json notifications.
3. DynamoDB & S3 Audit integration for idempotency and audit trails.
"""

import json
import os
from aws_cdk import (
    CfnOutput,
    Duration,
    Stack,
    aws_bedrock as bedrock,
    aws_dynamodb as dynamodb,
    aws_iam as iam,
    aws_lambda as _lambda,
    aws_s3 as s3,
    aws_s3_notifications as s3n,
    aws_sns as sns,
    aws_sqs as sqs,
)
from constructs import Construct


class AgentStack(Stack):
    """Provisions Bedrock Agent with two-tool Action Group and S3/DynamoDB Lambda router."""

    def __init__(
        self,
        scope: Construct,
        construct_id: str,
        storage_bucket: s3.IBucket,
        audit_bucket: s3.IBucket,
        events_table: dynamodb.ITable,
        alert_topic: sns.ITopic,
        alert_queue: sqs.IQueue,
        lambda_role: iam.IRole,
        bedrock_agent_role: iam.IRole,
        **kwargs,
    ) -> None:
        super().__init__(scope, construct_id, **kwargs)

        # 1. Load System Prompt
        current_dir = os.path.dirname(os.path.abspath(__file__))
        prompt_path = os.path.join(current_dir, "..", "agent", "system_prompt.txt")
        with open(prompt_path, "r", encoding="utf-8") as f:
            system_prompt = f.read().strip()

        # 2. Define Lambda Trigger & Action Group Executor Function
        agent_dir = os.path.join(current_dir, "..", "agent")
        self.trigger_lambda = _lambda.Function(
            self,
            "FloodDeltaAgentTrigger",
            runtime=_lambda.Runtime.PYTHON_3_12,
            handler="agent_trigger.handler",
            code=_lambda.Code.from_asset(agent_dir),
            role=lambda_role,
            memory_size=256,
            timeout=Duration.seconds(60),
            environment={
                "SNS_TOPIC_ARN": alert_topic.topic_arn,
                "ALERT_QUEUE_URL": alert_queue.queue_url,
                "DATA_BUCKET_NAME": storage_bucket.bucket_name,
                "AUDIT_BUCKET_NAME": audit_bucket.bucket_name,
                "EVENTS_TABLE_NAME": events_table.table_name,
                "BEDROCK_AGENT_ID": "",
                "BEDROCK_AGENT_ALIAS_ID": "TSTALIASID",
            },
        )

        # 3. S3 Event Notification: Trigger on events/ prefix matching flood_delta.json
        storage_bucket.add_event_notification(
            s3.EventType.OBJECT_CREATED,
            s3n.LambdaDestination(self.trigger_lambda),
            s3.NotificationKeyFilter(prefix="events/", suffix="flood_delta.json"),
        )

        # 4. Action Group OpenAPI Schema: Two-Tool Specification
        action_group_openapi = {
            "openapi": "3.0.0",
            "info": {
                "title": "Nepal Flood Disaster Response Action Group API",
                "version": "2.0.0",
                "description": "Two-tool agentic API for autonomous perception retrieval and alert dispatch.",
            },
            "paths": {
                "/getFloodEvidence": {
                    "post": {
                        "summary": "Retrieve flood perception evidence and generate evidence token",
                        "description": "Perception tool: Autonomous retrieval of OpenCV pipeline evidence and generation of an evidenceToken required for alert dispatch.",
                        "operationId": "getFloodEvidence",
                        "requestBody": {
                            "required": True,
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "eventId": {
                                                "type": "string",
                                                "description": "Unique identifier of the disaster event",
                                            }
                                        },
                                        "required": ["eventId"],
                                    }
                                }
                            },
                        },
                        "responses": {
                            "200": {
                                "description": "Evidence retrieved with verification token",
                                "content": {
                                    "application/json": {
                                        "schema": {
                                            "type": "object",
                                            "properties": {
                                                "eventId": {"type": "string"},
                                                "evidenceToken": {"type": "string"},
                                                "floodSignalDetected": {"type": "boolean"},
                                                "channelWidthRatio": {"type": "number"},
                                                "floodFractionDelta": {"type": "number"},
                                                "spatialControlPasses": {"type": "boolean"},
                                                "temporalControlPasses": {"type": "boolean"},
                                                "eventLocation": {"type": "string"},
                                            },
                                            "required": [
                                                "eventId",
                                                "evidenceToken",
                                                "floodSignalDetected",
                                                "channelWidthRatio",
                                                "spatialControlPasses",
                                            ],
                                        }
                                    }
                                },
                            }
                        },
                    }
                },
                "/dispatchFloodAlert": {
                    "post": {
                        "summary": "Dispatch verified emergency flood alert via SNS",
                        "description": "Action tool: Validates evidenceToken against DynamoDB, performs idempotency check, and dispatches emergency broadcast.",
                        "operationId": "dispatchFloodAlert",
                        "requestBody": {
                            "required": True,
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "eventId": {
                                                "type": "string",
                                                "description": "Disaster event identifier",
                                            },
                                            "evidenceToken": {
                                                "type": "string",
                                                "description": "Verification token received from getFloodEvidence",
                                            },
                                            "severity": {
                                                "type": "string",
                                                "description": "Alert severity level (e.g. CRITICAL, HIGH)",
                                            },
                                            "channelWidthRatio": {
                                                "type": "string",
                                                "description": "Widening ratio evaluated by agent (e.g. 2.211x)",
                                            },
                                            "decisionReason": {
                                                "type": "string",
                                                "description": "Agent justification and reasoning for the alert",
                                            },
                                        },
                                        "required": [
                                            "eventId",
                                            "evidenceToken",
                                            "severity",
                                            "channelWidthRatio",
                                            "decisionReason",
                                        ],
                                    }
                                }
                            },
                        },
                        "responses": {
                            "200": {
                                "description": "Alert dispatch confirmation with receipt URI",
                                "content": {
                                    "application/json": {
                                        "schema": {
                                            "type": "object",
                                            "properties": {
                                                "status": {"type": "string"},
                                                "eventId": {"type": "string"},
                                                "snsMessageId": {"type": "string"},
                                                "actionReceiptUri": {"type": "string"},
                                            },
                                            "required": ["status", "eventId"],
                                        }
                                    }
                                },
                            }
                        },
                    }
                },
            },
        }

        # 5. Bedrock Agent Definition
        self.bedrock_agent = bedrock.CfnAgent(
            self,
            "DisasterResponseCoordinatorAgent",
            agent_name="NepalFloodDisasterCoordinator",
            description="Autonomous disaster evaluation and alerting agent with two-tool Perception-Action loop",
            foundation_model="anthropic.claude-3-5-sonnet-20241022-v2:0",
            agent_resource_role_arn=bedrock_agent_role.role_arn,
            instruction=system_prompt,
            action_groups=[
                bedrock.CfnAgent.AgentActionGroupProperty(
                    action_group_name="FloodDisasterActionGroup",
                    description="Autonomous tools for retrieving flood evidence and dispatching verified alerts",
                    action_group_executor=bedrock.CfnAgent.ActionGroupExecutorProperty(
                        lambda_=self.trigger_lambda.function_arn
                    ),
                    api_schema=bedrock.CfnAgent.APISchemaProperty(
                        payload=json.dumps(action_group_openapi)
                    ),
                )
            ],
            auto_prepare=True,
        )

        # Allow Bedrock service to invoke the action group Lambda
        self.trigger_lambda.add_permission(
            "AllowBedrockInvocation",
            principal=iam.ServicePrincipal("bedrock.amazonaws.com"),
            action="lambda:InvokeFunction",
            source_arn=self.bedrock_agent.attr_agent_arn,
        )

        # 6. Bedrock Agent Alias
        self.agent_alias = bedrock.CfnAgentAlias(
            self,
            "DisasterCoordinatorLiveAlias",
            agent_alias_name="live",
            agent_id=self.bedrock_agent.attr_agent_id,
            description="Live production alias for Nepal flood disaster monitoring",
        )

        # Update Lambda environment variables with Agent ID & Alias
        self.trigger_lambda.add_environment(
            "BEDROCK_AGENT_ID", self.bedrock_agent.attr_agent_id
        )
        self.trigger_lambda.add_environment(
            "BEDROCK_AGENT_ALIAS_ID", self.agent_alias.attr_agent_alias_id
        )

        # CloudFormation Outputs
        self.agent_id_output = CfnOutput(
            self,
            "BedrockAgentId",
            value=self.bedrock_agent.attr_agent_id,
            description="Amazon Bedrock Agent ID for Disaster Coordinator",
            export_name="FloodBedrockAgentId",
        )

        self.agent_alias_id_output = CfnOutput(
            self,
            "BedrockAgentAliasId",
            value=self.agent_alias.attr_agent_alias_id,
            description="Amazon Bedrock Agent Alias ID",
            export_name="FloodBedrockAgentAliasId",
        )

        self.lambda_arn_output = CfnOutput(
            self,
            "TriggerLambdaArn",
            value=self.trigger_lambda.function_arn,
            description="ARN of FloodDeltaAgentTrigger Lambda function",
            export_name="FloodLambdaTriggerArn",
        )
