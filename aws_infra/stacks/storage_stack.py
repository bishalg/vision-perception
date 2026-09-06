"""Storage Stack for Nepal Flood Disaster Monitoring Architecture.

Defines an encrypted, versioned Amazon S3 bucket for multi-temporal
Sentinel-2 scene products, flood deltas, and evidence artifacts.
"""

from aws_cdk import (
    CfnOutput,
    Duration,
    RemovalPolicy,
    Stack,
    aws_s3 as s3,
)
from constructs import Construct


class StorageStack(Stack):
    """Provisions S3 storage with HTTPS enforcement, lifecycle rules, and CORS."""

    def __init__(
        self,
        scope: Construct,
        construct_id: str,
        bucket_name_prefix: str = "vision-perception-flood-data",
        **kwargs,
    ) -> None:
        super().__init__(scope, construct_id, **kwargs)

        # In S3, bucket names must be globally unique across all AWS accounts.
        # If an explicit name is desired, CDK auto-generates a unique suffix if bucket_name is omitted.
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

        # CloudFormation Outputs
        self.bucket_name_output = CfnOutput(
            self,
            "FloodDataBucketName",
            value=self.bucket.bucket_name,
            description="Name of the S3 bucket hosting flood analysis outputs and evidence",
            export_name="FloodDataBucketName",
        )

        self.bucket_arn_output = CfnOutput(
            self,
            "FloodDataBucketArn",
            value=self.bucket.bucket_arn,
            description="ARN of the S3 bucket hosting flood analysis outputs and evidence",
            export_name="FloodDataBucketArn",
        )
