import * as cdk from 'aws-cdk-lib';
import * as ec2 from 'aws-cdk-lib/aws-ec2';
import * as rds from 'aws-cdk-lib/aws-rds';
import { Construct } from 'constructs';

export class DatabaseStack extends cdk.Stack {
  public readonly vpc: ec2.Vpc;
  public readonly databaseProxy: rds.DatabaseProxy;
  public readonly dbSecret: rds.DatabaseSecret;

  constructor(scope: Construct, id: string, props?: cdk.StackProps) {
    super(scope, id, props);

    this.vpc = new ec2.Vpc(this, 'Vpc', {
      maxAzs: 2,
      natGateways: 1, // Required for worker to download models if not in layer
    });

    this.dbSecret = new rds.DatabaseSecret(this, 'DbSecret', {
      username: 'postgres',
    });

    const cluster = new rds.DatabaseCluster(this, 'Database', {
      engine: rds.DatabaseClusterEngine.auroraPostgres({
        version: rds.AuroraPostgresEngineVersion.VER_15_4,
      }),
      writer: rds.ClusterInstance.serverlessV2('writer'),
      vpc: this.vpc,
      credentials: rds.Credentials.fromSecret(this.dbSecret),
      defaultDatabaseName: 'spatial_api',
    });

    this.databaseProxy = new rds.DatabaseProxy(this, 'DbProxy', {
      proxyTarget: rds.ProxyTarget.fromCluster(cluster),
      secrets: [this.dbSecret],
      vpc: this.vpc,
      iamAuth: false,
    });
  }
}
