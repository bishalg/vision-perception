"""Storage Stack for Nepal Flood Disaster Monitoring Architecture.

Defines:
1. Data S3 Bucket: Multi-temporal Sentinel-2 scenes, flood deltas, and evidence.
2. Audit S3 Bucket: Dedicated bucket for immutable action receipts and agent traces (prevents recursive S3 event triggers).
3. Events DynamoDB Table: Server-side event registry for idempotency, evidenceToken binding, and audit trail state.
"""

from aws_cdk import (
    CfnOutput,
    Duration,
    RemovalPolicy,
    Stack,
    aws_dynamodb as dynamodb,
    aws_s3 as s3,
)
from constructs import Construct


class StorageStack(Stack):
    """Provisions S3 data bucket, dedicated audit bucket, and DynamoDB event store."""

    def __init__(
        self,
        scope: Construct,
        construct_id: str,
        stage: str = "dev",
        **kwargs,
    ) -> None:
        super().__init__(scope, construct_id, **kwargs)

        # 1. Primary Flood Data S3 Bucket
        self.bucket = s3.Bucket(
            self,
            "FloodDataBucket",
            versioned=True,
            encryption=s3.BucketEncryption.S3_MANAGED,
            enforce_ssl=True,
            block_public_access=s3.BlockPublicAccess.BLOCK_ALL,
            removal_policy=RemovalPolicy.RETAIN,
            auto_delete_objects=False,
            lifecycle_rules=[
                s3.LifecycleRule(
                    id="TransitionToInfrequentAccessAfter90Days",
                    transitions=[
                        s3.Transition(
                            storage_class=s3.StorageClass.INFREQUENT_ACCESS,
                            transition_after=Duration.days(90),
                        )
                    ],
                )
            ],
            cors=[
                s3.CorsRule(
                    allowed_methods=[s3.HttpMethods.GET, s3.HttpMethods.HEAD],
                    allowed_origins=["*"],
                    allowed_headers=["*"],
                    max_age=3600,
                )
            ],
        )

        # 2. Dedicated Audit S3 Bucket (Action receipts, decision logs, agent traces)
        # Keeps audit writes decoupled from the data bucket to eliminate recursive S3 trigger loops
        self.audit_bucket = s3.Bucket(
            self,
            "FloodAuditBucket",
            versioned=True,
            encryption=s3.BucketEncryption.S3_MANAGED,
            enforce_ssl=True,
            block_public_access=s3.BlockPublicAccess.BLOCK_ALL,
            removal_policy=RemovalPolicy.RETAIN,
            auto_delete_objects=False,
            lifecycle_rules=[
                s3.LifecycleRule(
                    id="TransitionAuditAfter90Days",
                    transitions=[
                        s3.Transition(
                            storage_class=s3.StorageClass.INFREQUENT_ACCESS,
                            transition_after=Duration.days(90),
                        )
                    ],
                )
            ],
        )

        # 3. Events DynamoDB Table (Idempotency, Token binding, Alert state)
        self.events_table = dynamodb.Table(
            self,
            "FloodEventsTable",
            table_name=f"vision-perception-events-{stage}",
            partition_key=dynamodb.Attribute(
                name="eventId",
                type=dynamodb.AttributeType.STRING,
            ),
            billing_mode=dynamodb.BillingMode.PAY_PER_REQUEST,
            removal_policy=RemovalPolicy.DESTROY,
            point_in_time_recovery=False,
            time_to_live_attribute="ttl",
        )

        # CloudFormation Outputs
        self.bucket_name_output = CfnOutput(
            self,
            "FloodDataBucketName",
            value=self.bucket.bucket_name,
            description="Name of S3 bucket hosting flood analysis imagery and deltas",
            export_name="FloodDataBucketName",
        )

        self.bucket_arn_output = CfnOutput(
            self,
            "FloodDataBucketArn",
            value=self.bucket.bucket_arn,
            description="ARN of S3 bucket hosting flood analysis imagery and deltas",
            export_name="FloodDataBucketArn",
        )

        self.audit_bucket_name_output = CfnOutput(
            self,
            "FloodAuditBucketName",
            value=self.audit_bucket.bucket_name,
            description="Name of dedicated S3 bucket hosting immutable action receipts and traces",
            export_name="FloodAuditBucketName",
        )

        self.audit_bucket_arn_output = CfnOutput(
            self,
            "FloodAuditBucketArn",
            value=self.audit_bucket.bucket_arn,
            description="ARN of dedicated S3 bucket hosting immutable action receipts and traces",
            export_name="FloodAuditBucketArn",
        )

        self.events_table_name_output = CfnOutput(
            self,
            "FloodEventsTableName",
            value=self.events_table.table_name,
            description="Name of DynamoDB table for event tracking and idempotency",
            export_name="FloodEventsTableName",
        )

        self.events_table_arn_output = CfnOutput(
            self,
            "FloodEventsTableArn",
            value=self.events_table.table_arn,
            description="ARN of DynamoDB table for event tracking and idempotency",
            export_name="FloodEventsTableArn",
        )
