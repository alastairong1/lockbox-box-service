# Guardian Index Implementation Plan

## Current Implementation

Currently, searching for boxes by guardian ID requires a full table scan because:

1. Guardian information is stored as an array of objects in each box record
2. DynamoDB doesn't support direct indexing of nested array elements
3. Each box can have multiple guardians with different statuses

## Proposed Solutions

### Option 1: Separate Guardian-Box Mapping Table

Create a separate DynamoDB table to track guardian-to-box relationships:

```
GuardianBoxMappingTable:
  Type: AWS::DynamoDB::Table
  Properties:
    TableName: lockbox-guardian-box-mappings
    BillingMode: PAY_PER_REQUEST
    AttributeDefinitions:
      - AttributeName: guardian_id
        AttributeType: S
      - AttributeName: box_id
        AttributeType: S
    KeySchema:
      - AttributeName: guardian_id
        KeyType: HASH
      - AttributeName: box_id
        KeyType: RANGE
    GlobalSecondaryIndexes:
      - IndexName: box-id-index
        KeySchema:
          - AttributeName: box_id
            KeyType: HASH
        Projection:
          ProjectionType: ALL
```

This approach requires:

1. Updating the service to maintain both tables when boxes/guardians are updated
2. Creating a data migration plan for existing records
3. Additional consistency checks

### Option 2: Use DynamoDB Streams for Denormalization

1. Use DynamoDB Streams to capture changes to the boxes table
2. Process stream events with a Lambda function
3. Maintain a denormalized view of guardian relationships

This approach provides eventual consistency but requires additional infrastructure.

### Option 3: Sparse GSI with Status Field

For a lighter approach, we could modify how guardian status is stored:

1. Add a flattened field to box records: `guardian_ids_accepted: Set<String>`
2. Create a sparse GSI on this field
3. Update this field whenever guardian status changes

```yaml
AttributeDefinitions:
  - AttributeName: id
    AttributeType: S
  - AttributeName: owner_id
    AttributeType: S
  - AttributeName: guardian_id_accepted
    AttributeType: S

GlobalSecondaryIndexes:
  - IndexName: guardian-id-index
    KeySchema:
      - AttributeName: guardian_id_accepted
        KeyType: HASH
    Projection:
      ProjectionType: ALL
```

## Implementation Steps

1. Choose the most appropriate approach (Option 1 recommended for production)
2. Update the data model to support the new structure
3. Create the migration strategy for existing data
4. Update the `get_boxes_by_guardian_id` method to use the new index
5. Add comprehensive testing for different guardian scenarios
6. Update CloudFormation template with the new resources

## Estimated Effort

- Development: 3-5 days
- Testing: 2-3 days
- Migration: 1-2 days (depending on data volume)
