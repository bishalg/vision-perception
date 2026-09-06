"""IAM Stack for Nepal Flood Disaster Monitoring Architecture.

Defines least-privilege IAM roles and permission policies for the
Lambda S3 trigger and the Bedrock Agentic Vision reasoning coordinator.
"""

from aws_cdk import (
    CfnOutput,
    Stack,
    aws_iam as iam,
    aws_s3 as s3,
    aws_sns as sns,
)
from constructs import Construct


class IAMStack(Stack):
    """Provisions execution roles for Lambda and Bedrock Agent."""

    def __init__(
        self,
        scope: Construct,
        construct_id: str,
        storage_bucket: s3.IBucket,
        alert_topic: sns.ITopic,
        **kwargs,
    ) -> None:
        super().__init__(scope, construct_id, **kwargs)

        # 1. Lambda Execution Role
        self.lambda_role = iam.Role(
            self,
            "FloodLambdaExecutionRole",
            assumed_by=iam.ServicePrincipal("lambda.amazonaws.com"),
            description="Role used by FloodDeltaAgentTrigger Lambda to fetch S3 deltas and invoke Bedrock Agent",
            managed_policies=[
                iam.ManagedPolicy.from_aws_managed_policy_name(
                    "service-role/AWSLambdaBasicExecutionRole"
                )
            ],
        )

        # Allow reading flood delta JSON and evidence artifacts from S3
        self.lambda_role.add_to_policy(
            iam.PolicyStatement(
                sid="AllowS3ReadFloodData",
                effect=iam.Effect.ALLOW,
                actions=["s3:GetObject", "s3:ListBucket"],
                resources=[
                    storage_bucket.bucket_arn,
                    f"{storage_bucket.bucket_arn}/*",
                ],
            )
        )

        # Allow Lambda to invoke Bedrock Agent and Bedrock foundation models
        self.lambda_role.add_to_policy(
            iam.PolicyStatement(
                sid="AllowInvokeBedrockAgent",
                effect=iam.Effect.ALLOW,
                actions=[
                    "bedrock:InvokeAgent",
                    "bedrock:InvokeModel",
                    "bedrock:GetAgent",
                    "bedrock:GetAgentAlias",
                ],
                resources=["*"],
            )
        )

        # 2. Bedrock Agent Execution Role
        self.bedrock_agent_role = iam.Role(
            self,
            "FloodBedrockAgentRole",
            assumed_by=iam.ServicePrincipal("bedrock.amazonaws.com"),
            description="Role assumed by Amazon Bedrock Agent to reason and trigger alert tools",
        )

        # Allow Claude 3.5 Sonnet foundation model invocation
        self.bedrock_agent_role.add_to_policy(
            iam.PolicyStatement(
                sid="AllowBedrockModelInvocation",
                effect=iam.Effect.ALLOW,
                actions=["bedrock:InvokeModel"],
                resources=[
                    "arn:aws:bedrock:*::foundation-model/anthropic.claude-3-5-sonnet*",
                    "arn:aws:bedrock:*::foundation-model/anthropic.claude-3-5-sonnet-20241022-v2:0",
                ],
            )
        )

        # Allow Bedrock Agent to publish emergency alerts to the SNS Topic
        self.bedrock_agent_role.add_to_policy(
            iam.PolicyStatement(
                sid="AllowPublishToAlertTopic",
                effect=iam.Effect.ALLOW,
                actions=["sns:Publish"],
                resources=[alert_topic.topic_arn],
            )
        )

        # CloudFormation Outputs
        self.lambda_role_arn_output = CfnOutput(
            self,
            "LambdaRoleArn",
            value=self.lambda_role.role_arn,
            description="ARN of the IAM role for FloodDeltaAgentTrigger Lambda",
            export_name="FloodLambdaExecutionRoleArn",
        )

        self.bedrock_role_arn_output = CfnOutput(
            self,
            "BedrockRoleArn",
            value=self.bedrock_agent_role.role_arn,
            description="ARN of the IAM role for Bedrock Disaster Coordinator Agent",
            export_name="FloodBedrockAgentRoleArn",
        )
