"""IAM Stack for Nepal Flood Disaster Monitoring Architecture.

Defines least-privilege IAM roles and permission policies for:
1. Lambda Trigger & Tool Execution: Reading S3 deltas, writing audit receipts,
   managing DynamoDB event states, publishing to SNS, and calling Bedrock runtime.
2. Bedrock Agent Service Principal: Invoking Claude 3.5 Sonnet foundation models.
"""

from aws_cdk import (
    CfnOutput,
    Stack,
    aws_dynamodb as dynamodb,
    aws_iam as iam,
    aws_s3 as s3,
    aws_sns as sns,
    aws_sqs as sqs,
)
from constructs import Construct


class IAMStack(Stack):
    """Provisions execution roles for Lambda and Bedrock Agent with DynamoDB & S3 access."""

    def __init__(
        self,
        scope: Construct,
        construct_id: str,
        storage_bucket: s3.IBucket,
        audit_bucket: s3.IBucket,
        events_table: dynamodb.ITable,
        alert_topic: sns.ITopic,
        alert_queue: sqs.IQueue,
        **kwargs,
    ) -> None:
        super().__init__(scope, construct_id, **kwargs)

        # 1. Lambda Execution Role
        self.lambda_role = iam.Role(
            self,
            "FloodLambdaExecutionRole",
            assumed_by=iam.ServicePrincipal("lambda.amazonaws.com"),
            description="Role used by FloodDeltaAgentTrigger Lambda for S3, DynamoDB, Bedrock, and SNS operations",
            managed_policies=[
                iam.ManagedPolicy.from_aws_managed_policy_name(
                    "service-role/AWSLambdaBasicExecutionRole"
                )
            ],
        )

        # S3 Data Bucket: Read flood deltas and evidence
        storage_bucket.grant_read(self.lambda_role)

        # S3 Audit Bucket: Write action receipts, logs, and agent execution records
        audit_bucket.grant_read_write(self.lambda_role)

        # DynamoDB Events Table: Read/Write for idempotency gate and evidenceToken tracking
        events_table.grant_read_write_data(self.lambda_role)

        # SNS Topic: Publish verified disaster alerts
        alert_topic.grant_publish(self.lambda_role)

        # SQS Fallback Queue: Consume messages for integration checks
        alert_queue.grant_consume_messages(self.lambda_role)

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
            description="Role assumed by Amazon Bedrock Agent to reason and execute Action Group tools",
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
