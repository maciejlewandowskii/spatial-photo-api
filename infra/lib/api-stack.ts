import * as cdk from 'aws-cdk-lib';
import * as s3 from 'aws-cdk-lib/aws-s3';
import * as sqs from 'aws-cdk-lib/aws-sqs';
import * as lambda from 'aws-cdk-lib/aws-lambda';
import * as apigatewayv2 from 'aws-cdk-lib/aws-apigatewayv2';
import * as apigatewayv2_integrations from 'aws-cdk-lib/aws-apigatewayv2-integrations';
import * as dynamodb from 'aws-cdk-lib/aws-dynamodb';
import * as iam from 'aws-cdk-lib/aws-iam';
import { Construct } from 'constructs';
import * as path from 'path';

export interface ApiStackProps extends cdk.StackProps {
  vpc: cdk.aws_ec2.IVpc;
  dbProxy: cdk.aws_rds.IDatabaseProxy;
  dbSecret: cdk.aws_secretsmanager.ISecret;
}

export class ApiStack extends cdk.Stack {
  public readonly uploadsBucket: s3.Bucket;
  public readonly resultsBucket: s3.Bucket;
  public readonly jobQueue: sqs.Queue;
  public readonly wsConnectionsTable: dynamodb.Table;
  public readonly apiLambda: lambda.Function;

  constructor(scope: Construct, id: string, props: ApiStackProps) {
    super(scope, id, props);

    this.uploadsBucket = new s3.Bucket(this, 'UploadsBucket', {
        removalPolicy: cdk.RemovalPolicy.DESTROY,
        autoDeleteObjects: true,
        lifecycleRules: [{ expiration: cdk.Duration.hours(2) }],
    });

    this.resultsBucket = new s3.Bucket(this, 'ResultsBucket', {
        removalPolicy: cdk.RemovalPolicy.DESTROY,
        autoDeleteObjects: true,
        lifecycleRules: [{ expiration: cdk.Duration.days(1) }],
    });

    this.jobQueue = new sqs.Queue(this, 'JobQueue.fifo', {
        fifo: true,
        contentBasedDeduplication: true,
        visibilityTimeout: cdk.Duration.minutes(15),
    });

    this.wsConnectionsTable = new dynamodb.Table(this, 'WsConnections', {
        partitionKey: { name: 'connection_id', type: dynamodb.AttributeType.STRING },
        billingMode: dynamodb.BillingMode.PAY_PER_REQUEST,
        removalPolicy: cdk.RemovalPolicy.DESTROY,
        timeToLiveAttribute: 'ttl',
    });
    this.wsConnectionsTable.addGlobalSecondaryIndex({
        indexName: 'job_id-index',
        partitionKey: { name: 'job_id', type: dynamodb.AttributeType.STRING },
        projectionType: dynamodb.ProjectionType.ALL,
    });

    const jwtSecret = new cdk.SecretValue('REPLACE_ME_OR_USE_ENV');

    this.apiLambda = new lambda.Function(this, 'ApiLambda', {
        runtime: lambda.Runtime.PROVIDED_AL2023,
        handler: 'bootstrap',
        code: lambda.Code.fromAsset(path.join(__dirname, '../../target/lambda/api')),
        vpc: props.vpc,
        vpcSubnets: { subnetType: cdk.aws_ec2.SubnetType.PRIVATE_WITH_EGRESS },
        memorySize: 512,
        timeout: cdk.Duration.seconds(30),
        environment: {
            DATABASE_URL: `postgres://postgres:${props.dbSecret.secretValue.unsafeUnwrap()}@${props.dbProxy.endpoint}:5432/spatial_api`,
            JWT_SECRET: 'change-me-in-production',
            S3_BUCKET: this.uploadsBucket.bucketName,
            SQS_QUEUE_URL: this.jobQueue.queueUrl,
            DYNAMODB_TABLE: this.wsConnectionsTable.tableName,
        },
    });

    this.uploadsBucket.grantReadWrite(this.apiLambda);
    this.resultsBucket.grantRead(this.apiLambda);
    this.jobQueue.grantSendMessages(this.apiLambda);
    this.wsConnectionsTable.grantReadWriteData(this.apiLambda);

    const httpApi = new apigatewayv2.HttpApi(this, 'HttpApi');
    httpApi.addRoutes({
        path: '/{proxy+}',
        methods: [apigatewayv2.HttpMethod.ANY],
        integration: new apigatewayv2_integrations.HttpLambdaIntegration('ApiIntegration', this.apiLambda),
    });

    const wsLambda = new lambda.Function(this, 'WsLambda', {
        runtime: lambda.Runtime.PROVIDED_AL2023,
        handler: 'bootstrap',
        code: lambda.Code.fromAsset(path.join(__dirname, '../../target/lambda/websocket')),
        environment: {
            DYNAMODB_TABLE: this.wsConnectionsTable.tableName,
            JWT_SECRET: 'change-me-in-production',
        },
    });
    this.wsConnectionsTable.grantReadWriteData(wsLambda);

    const webSocketApi = new apigatewayv2.WebSocketApi(this, 'WsApi', {
        connectRouteOptions: { integration: new apigatewayv2_integrations.WebSocketLambdaIntegration('ConnectIntegration', wsLambda) },
        disconnectRouteOptions: { integration: new apigatewayv2_integrations.WebSocketLambdaIntegration('DisconnectIntegration', wsLambda) },
        defaultRouteOptions: { integration: new apigatewayv2_integrations.WebSocketLambdaIntegration('DefaultIntegration', wsLambda) },
    });
    new apigatewayv2.WebSocketStage(this, 'ProdStage', {
        webSocketApi,
        stageName: 'prod',
        autoDeploy: true,
    });
  }
}
