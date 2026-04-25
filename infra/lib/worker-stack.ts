import * as cdk from 'aws-cdk-lib';
import * as lambda from 'aws-cdk-lib/aws-lambda';
import * as lambda_event_sources from 'aws-cdk-lib/aws-lambda-event-sources';
import * as iam from 'aws-cdk-lib/aws-iam';
import { Construct } from 'constructs';
import * as path from 'path';

export interface WorkerStackProps extends cdk.StackProps {
  vpc: cdk.aws_ec2.IVpc;
  dbProxy: cdk.aws_rds.IDatabaseProxy;
  dbSecret: cdk.aws_secretsmanager.ISecret;
  uploadsBucket: cdk.aws_s3.IBucket;
  resultsBucket: cdk.aws_s3.IBucket;
  jobQueue: cdk.aws_sqs.IQueue;
  wsConnectionsTable: cdk.aws_dynamodb.ITable;
}

export class WorkerStack extends cdk.Stack {
  constructor(scope: Construct, id: string, props: WorkerStackProps) {
    super(scope, id, props);

    const workerLambda = new lambda.DockerImageFunction(this, 'WorkerLambda', {
        code: lambda.DockerImageCode.fromImageAsset(path.join(__dirname, '../../'), {
            file: 'crates/worker/Dockerfile',
        }),
        vpc: props.vpc,
        vpcSubnets: { subnetType: cdk.aws_ec2.SubnetType.PRIVATE_WITH_EGRESS },
        memorySize: 10240, // 10GB for DepthPro
        timeout: cdk.Duration.minutes(15),
        environment: {
            DATABASE_URL: `postgres://postgres:${props.dbSecret.secretValue.unsafeUnwrap()}@${props.dbProxy.endpoint}:5432/spatial_api`,
            S3_BUCKET: props.uploadsBucket.bucketName,
            DYNAMODB_TABLE: props.wsConnectionsTable.tableName,
            // WEBSOCKET_API_ENDPOINT would be needed here too
        },
    });

    workerLambda.addEventSource(new lambda_event_sources.SqsEventSource(props.jobQueue, {
        batchSize: 1,
    }));

    props.uploadsBucket.grantReadWrite(workerLambda);
    props.resultsBucket.grantReadWrite(workerLambda);
    props.wsConnectionsTable.grantReadWriteData(workerLambda);
    
    // Grant permissions to call API Gateway Management API (for WS push)
    workerLambda.addToRolePolicy(new iam.PolicyStatement({
        actions: ['execute-api:ManageConnections'],
        resources: ['*'], // Ideally scoped to the WsApi ARN
    }));
  }
}
