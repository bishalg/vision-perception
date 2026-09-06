"""Agent Stack for Nepal Flood Disaster Monitoring Architecture.

Deploys the Amazon Bedrock Agent (powered by Claude 3.5 Sonnet v2),
wires the FloodAlertActionGroup tool, and configures the Lambda trigger
driven by S3 flood_delta.json notifications.
"""

import json
import os
from aws_cdk import (
    CfnOutput,
    Duration,
    Stack,
    aws_bedrock as bedrock,
    aws_iam as iam,
    aws_lambda as _lambda,
    aws_s3 as s3,
    aws_s3_notifications as s3n,
    aws_sns as sns,
)
from constructs import Construct


class AgentStack(Stack):
    """Provisions Bedrock Agent, Lambda trigger, and S3 event routing."""

    def __init__(
        self,
        scope: Construct,
        construct_id: str,
        storage_bucket: s3.IBucket,
        alert_topic: sns.ITopic,
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

        # 2. Define Lambda Trigger Function
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
                "BEDROCK_AGENT_ID": "",  # Populated after Agent resource creation
                "BEDROCK_AGENT_ALIAS_ID": "TSTALIASID",
            },
        )

        # 3. S3 Event Notification: Trigger Lambda when flood_delta.json is uploaded
        storage_bucket.add_event_notification(
            s3.EventType.OBJECT_CREATED,
            s3n.LambdaDestination(self.trigger_lambda),
            s3.NotificationKeyFilter(suffix="flood_delta.json"),
        )

        # 4. Action Group OpenAPI Schema
        action_group_openapi = {
            "openapi": "3.0.0",
            "info": {
                "title": "Nepal Flood Alert API",
                "version": "1.0.0",
                "description": "Emergency alerting tool for Nepal flood monitoring",
            },
            "paths": {
                "/publish-flood-alert": {
                    "post": {
                        "summary": "Publish disaster flood alert via SNS",
                        "description": "Dispatches critical alert notification with channel widening and flood signature metrics to NEOC and stakeholders.",
                        "operationId": "publish_flood_alert",
                        "requestBody": {
                            "required": True,
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "location": {
                                                "type": "string",
                                                "description": "Geographic location of the event",
                                            },
                                            "severity": {
                                                "type": "string",
                                                "description": "Severity level (e.g. CRITICAL, HIGH)",
                                            },
                                            "channel_width_ratio": {
                                                "type": "string",
                                                "description": "Observed channel width expansion ratio",
                                            },
                                            "message": {
                                                "type": "string",
                                                "description": "Detailed disaster summary with physical interpretation",
                                            },
                                        },
                                        "required": [
                                            "location",
                                            "severity",
                                            "channel_width_ratio",
                                            "message",
                                        ],
                                    }
                                }
                            },
                        },
                        "responses": {
                            "200": {
                                "description": "Alert dispatch confirmation",
                                "content": {
                                    "application/json": {
                                        "schema": {
                                            "type": "object",
                                            "properties": {
                                                "status": {"type": "string"},
                                                "sns_message_id": {"type": "string"},
                                            },
                                        }
                                    }
                                },
                            }
                        },
                    }
                }
            },
        }

        # 5. Bedrock Agent Definition
        self.bedrock_agent = bedrock.CfnAgent(
            self,
            "DisasterResponseCoordinatorAgent",
            agent_name="NepalFloodDisasterCoordinator",
            description="Autonomous disaster evaluation and alerting agent for glacier-collapse floods in Nepal",
            foundation_model="anthropic.claude-3-5-sonnet-20241022-v2:0",
            agent_resource_role_arn=bedrock_agent_role.role_arn,
            instruction=system_prompt,
            action_groups=[
                bedrock.CfnAgent.AgentActionGroupProperty(
                    action_group_name="FloodAlertActionGroup",
                    description="Publishes verified emergency alerts to SNS when disaster thresholds are met",
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

        # Update Lambda environment variable with Agent ID & Alias
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
            description="ARN of the FloodDeltaAgentTrigger Lambda function",
            export_name="FloodLambdaTriggerArn",
        )
