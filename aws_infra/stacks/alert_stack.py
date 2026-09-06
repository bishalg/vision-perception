"""Alert Stack for Nepal Flood Disaster Monitoring Architecture.

Defines an Amazon SNS Topic and email subscription for broadcasting
critical disaster alerts triggered by autonomous Bedrock Agent evaluations.
"""

import os
from aws_cdk import (
    CfnOutput,
    Stack,
    aws_sns as sns,
    aws_sns_subscriptions as subscriptions,
)
from constructs import Construct


class AlertStack(Stack):
    """Provisions SNS alert topic with email subscription."""

    def __init__(
        self,
        scope: Construct,
        construct_id: str,
        topic_name: str = "flood-disaster-alerts",
        **kwargs,
    ) -> None:
        super().__init__(scope, construct_id, **kwargs)

        self.topic = sns.Topic(
            self,
            "FloodDisasterAlertTopic",
            topic_name=topic_name,
            display_name="Nepal Flood Disaster Alert System",
        )

        alert_email = os.environ.get("SNS_ALERT_EMAIL", "alerts@example.com")
        self.topic.add_subscription(subscriptions.EmailSubscription(alert_email))

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
