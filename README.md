# lockbox-box-service

A simple service providing secure box storage with guardian management. This service allows users to manage boxes and guardians using a simple API managed with AWS Lambda and API Gateway. Boxes contain documents and associated unlock requests, and can be updated by authorized parties.

## Overview

The lockbox-box-service provides endpoints for managing boxes and associated unlock requests. Boxes can be created and updated by the owner, and guardians can assist with unlocking a box via approval or rejection. The service performs proper validations ensuring only authorized users may perform updates.

## API Endpoints

### Owner Endpoints

#### 1. Get Owned Boxes

**Endpoint:** `GET /boxes/owned`

**Headers:**
- `x-user-id`: Your user identifier

**Description:**
Returns all boxes owned by the user.

**Response Example:**
```json
{
  "boxes": [
    {
      "id": "box_id",
      "name": "Box Name",
      "description": "Description",
      "created_at": "timestamp",
      "updated_at": "timestamp"
    }
  ]
}
```

#### 2. Create Box

**Endpoint:** `POST /boxes/owned`

**Headers:**
- `x-user-id`: Your user identifier

**Description:**
Create a new box with you as the owner.

**Payload Example:**
```json
{
  "name": "New Box",
  "description": "Description of the box"
}
```

#### 3. Update Box (Owner Update)

**Endpoint:** `PATCH /boxes/owned/{id}`

**Headers:**
- `x-user-id`: Your owner user identifier

**Description:**
Allows box owners to update box details such as name and description.

**Payload Example:**
```json
{
  "name": "New Box Name",
  "description": "Updated description"
}
```

**Response Codes:**
- **200 OK:** Box updated successfully.
- **400 Bad Request:** Invalid request payload or missing required fields.
- **401 Unauthorized:** The user is not the owner or the box is not found.

#### 4. Delete Box

**Endpoint:** `DELETE /boxes/owned/{id}`

**Headers:**
- `x-user-id`: Your owner user identifier

**Description:**
Allows box owners to delete a box.

**Response Codes:**
- **200 OK:** Box deleted successfully.
- **401 Unauthorized:** The user is not the owner or the box is not found.
- **404 Not Found:** Box not found.

### Guardian Endpoints

#### 1. Get Guardian Boxes

**Endpoint:** `GET /boxes/guardian`

**Headers:**
- `x-user-id`: Your user identifier

**Description:**
Returns all boxes where the authenticated user is a guardian (excluding rejected entries). It uses the store to filter and convert boxes using the guardian conversion logic.

**Response Example:**
```json
{
  "boxes": [
    {
      "id": "box_id",
      "name": "Box Name",
      "description": "Description",
      "created_at": "timestamp",
      "updated_at": "timestamp",
      "unlock_request": {
         "id": "unlock_request_id",
         "requested_at": "timestamp",
         "status": "pending",
         "message": "Unlock request message",
         "initiated_by": "guardian_id",
         "approved_by": [],
         "rejected_by": []
      }
    }
  ]
}
```

#### 2. Get Guardian Box

**Endpoint:** `GET /boxes/guardian/{id}`

**Headers:**
- `x-user-id`: Your guardian user identifier

**Description:**
Get a specific box where you are a guardian.

**Response Example:**
```json
{
  "box": {
    "id": "box_id",
    "name": "Box Name",
    "description": "Description",
    "created_at": "timestamp",
    "updated_at": "timestamp",
    "unlock_request": { /* unlock request data if present */ }
  }
}
```

#### 3. Request Unlock (Lead Guardian Only)

**Endpoint:** `PATCH /boxes/guardian/{id}/request`

**Headers:**
- `x-user-id`: Your lead guardian user identifier

**Description:**
Allows lead guardians to initiate an unlock request for a box. The endpoint validates that the user is a lead guardian (and not rejected) of the box.

**Payload Example:**
```json
{
  "message": "Unlock request message"
}
```

**Response Codes:**
- **200 OK:** Unlock request initiated successfully, returning the updated guardian box details.
- **400 Bad Request:** Invalid payload or missing required fields.
- **401 Unauthorized:** The user is not an authorized lead guardian.
- **404 Not Found:** Box not found.
- **500 Internal Server Error:** An error occurred processing the update.

#### 4. Respond to Unlock Request (Guardian Only)

**Endpoint:** `PATCH /boxes/guardian/{id}/respond`

**Headers:**
- `x-user-id`: Your guardian user identifier

**Description:**
Allows guardians to respond to an existing unlock request. The endpoint validates that:
1. The user is a guardian (and not rejected) of the box
2. There is an active unlock request to respond to
3. The guardian hasn't already approved/rejected

**Payload Examples:**

_Approval:_
```json
{
  "approve": true
}
```

_Rejection:_
```json
{
  "reject": true
}
```

#### 5. Respond to Guardian Invitation

**Endpoint:** `PATCH /boxes/guardian/{id}/invitation`

**Headers:**
- `x-user-id`: Your user identifier

**Description:**
Allows users to accept or reject an invitation to be a guardian for a box. The endpoint validates that:
1. The user has a pending invitation for the box
2. The invitation hasn't already been responded to

**Payload Example:**
```json
{
  "accept": true
}
```

**Response Codes:**
- **200 OK:** Guardian invitation accepted successfully, returning the updated guardian box details.
- **400 Bad Request:** No pending invitation found for this box.
- **404 Not Found:** Box not found.
- **500 Internal Server Error:** An error occurred processing the response.

## Running the Service

This service is designed exclusively for AWS Lambda and cannot be run as a standalone HTTP server.

## Deployment

This service is automatically deployed to AWS Lambda via GitHub Actions when changes are merged into the main branch. The deployment process includes:

1. Running tests to ensure code quality
2. Building the Rust code targeting Amazon Linux 2
3. Packaging the binary for Lambda deployment
4. Updating the Lambda function code

### Prerequisites for Deployment

To enable automatic deployment, you need to configure the following GitHub secrets:

- `AWS_ACCESS_KEY_ID`: AWS access key with permissions to update Lambda
- `AWS_SECRET_ACCESS_KEY`: AWS secret key
- `AWS_REGION`: AWS region where the Lambda function is deployed

### Manual Deployment

If you need to deploy manually, you can use AWS SAM:

```bash
# Install SAM CLI if you haven't already
brew install aws-sam-cli

# Build for Lambda
cargo build --release --target x86_64-unknown-linux-musl
mkdir -p target/lambda
cp target/x86_64-unknown-linux-musl/release/lockbox-box-service target/lambda/bootstrap
cd target/lambda && zip -j bootstrap.zip bootstrap

# Deploy using SAM
sam deploy --guided
```

## Testing

For testing the application, you can use:

```bash
cargo test
```

This will run all unit and integration tests defined in the codebase.

## CI/CD Pipeline

The service uses GitHub Actions for continuous integration and deployment:

- **CI Pipeline**: Runs on all pull requests and pushes to main
  - Code formatting check
  - Linting with Clippy
  - Unit tests

- **Deployment Pipeline**: Runs only on pushes to main branch after tests pass
  - Builds the application for Lambda
  - Packages the binary
  - Deploys to AWS Lambda

## Data Storage

The service uses DynamoDB to store box records. Key features include:
- Tables are defined in the CloudFormation template
- Global Secondary Index (GSI) for querying by owner_id
- Guardian relationships are stored in the box record

See the `GUARDIAN_INDEX_IMPLEMENTATION.md` file for details on future improvements to guardian search functionality.

## Additional Notes

- All timestamps are in ISO8601 format.
- Ensure the `x-user-id` header is included in requests for proper authentication and authorization.
- Box records include both owner information and guardian relationships.

## Dependencies

- Rust
- Cargo
- AWS Lambda runtime
- DynamoDB
- AWS SAM for local testing and deployment