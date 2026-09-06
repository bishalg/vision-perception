"""Alert Stack for Nepal Flood Disaster Monitoring Architecture.

Defines:
1. Amazon SNS Topic: For broadcasting critical disaster alerts to emergency contacts.
2. Email Subscription: Driven by SNS_ALERT_EMAIL environment variable.
3. Amazon SQS Fallback Queue: Subscribed to the SNS topic for reliable programmatic
   polling and verification during automated demos (bypasses manual email confirmation requirements).
"""

import os
from aws_cdk import (
    CfnOutput,
    Duration,
    Stack,
    aws_sns as sns,
    aws_sns_subscriptions as subscriptions,
    aws_sqs as sqs,
)
from constructs import Construct


class AlertStack(Stack):
    """Provisions SNS alert topic, email subscription, and SQS fallback queue."""

    def __init__(
        self,
        scope: Construct,
        construct_id: str,
        topic_name: str = "flood-disaster-alerts",
        **kwargs,
    ) -> None:
        super().__init__(scope, construct_id, **kwargs)

        # 1. Primary Disaster Alert SNS Topic
        self.topic = sns.Topic(
            self,
            "FloodDisasterAlertTopic",
            topic_name=topic_name,
            display_name="Nepal Flood Disaster Alert System",
        )

        # 2. Email Subscription (Primary human notification path)
        alert_email = os.environ.get("SNS_ALERT_EMAIL", "alerts@example.com")
        self.topic.add_subscription(subscriptions.EmailSubscription(alert_email))

        # 3. SQS Fallback Queue (Programmatic consumer path for demos and integration tests)
        self.alert_queue = sqs.Queue(
            self,
            "FloodAlertFallbackQueue",
            queue_name="flood-disaster-alerts-queue",
            retention_period=Duration.days(14),
            visibility_timeout=Duration.seconds(120),
            encryption=sqs.QueueEncryption.SQS_MANAGED,
        )
        self.topic.add_subscription(subscriptions.SqsSubscription(self.alert_queue))

        # CloudFormation Outputs
        self.topic_arn_output = CfnOutput(
            self,
            "FloodAlertTopicArn",
            value=self.topic.topic_arn,
            description="ARN of the SNS topic for disaster alert dispatches",
            export_name="FloodAlertTopicArn",
        )

        self.topic_name_output = CfnOutput(
            self,
            "FloodAlertTopicName",
            value=self.topic.topic_name,
            description="Name of the SNS topic for disaster alert dispatches",
            export_name="FloodAlertTopicName",
        )

        self.queue_url_output = CfnOutput(
            self,
            "FloodAlertQueueUrl",
            value=self.alert_queue.queue_url,
            description="URL of the SQS fallback queue for programmatic alert verification",
            export_name="FloodAlertQueueUrl",
        )

        self.queue_arn_output = CfnOutput(
            self,
            "FloodAlertQueueArn",
            value=self.alert_queue.queue_arn,
            description="ARN of the SQS fallback queue for programmatic alert verification",
            export_name="FloodAlertQueueArn",
        )
