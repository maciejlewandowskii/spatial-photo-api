#!/usr/bin/env node
import 'source-map-support/register';
import * as cdk from 'aws-cdk-lib';
import { DatabaseStack } from '../lib/database-stack';
import { ApiStack } from '../lib/api-stack';
import { WorkerStack } from '../lib/worker-stack';

const app = new cdk.App();

const env = { 
    account: process.env.CDK_DEFAULT_ACCOUNT, 
    region: process.env.CDK_DEFAULT_REGION || 'us-east-1' 
};

const dbStack = new DatabaseStack(app, 'SpatialApi-Database', { env });

const apiStack = new ApiStack(app, 'SpatialApi-Api', {
    env,
    vpc: dbStack.vpc,
    dbProxy: dbStack.databaseProxy,
    dbSecret: dbStack.dbSecret,
});

new WorkerStack(app, 'SpatialApi-Worker', {
    env,
    vpc: dbStack.vpc,
    dbProxy: dbStack.databaseProxy,
    dbSecret: dbStack.dbSecret,
    uploadsBucket: apiStack.uploadsBucket,
    resultsBucket: apiStack.resultsBucket,
    jobQueue: apiStack.jobQueue,
    wsConnectionsTable: apiStack.wsConnectionsTable,
});
