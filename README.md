# lockbox-box-service

A simple service providing secure box storage with guardian management. This service allows users to manage boxes and guardians using a simple API managed with AWS Lambda and API Gateway. Boxes contain documents and associated unlock requests, and can be updated by authorized parties.

## Overview

The lockbox-box-service provides endpoints for managing boxes and associated unlock requests. Boxes can be created and updated by the owner, and guardians can assist with unlocking a box via approval or rejection. The service performs proper validations ensuring only authorized users may perform updates.

## API Endpoints

### 1. Get Boxes

**Endpoint:** `GET /boxes`

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

### 2. Update Box (Owner Update)

**Endpoint:** `PATCH /boxes/{id}`

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

### 3. Update Box (Guardian Update)

**Endpoint:** `PATCH /boxes/guardian/{id}`

**Headers:**
- `x-user-id`: Your guardian user identifier

**Description:**
Allows guardians of a box to update its unlock request. The endpoint validates that the user is a guardian (and not rejected) of the box. Based on their role:

- **Lead Guardians:**
  - Must include a `message` field in the JSON payload.
  - Creates or updates an unlock request with the provided message. The unlock request status is set to "pending" and records the initiating lead guardian.

- **Other Guardians:**
  - Can update an existing unlock request by providing either an `approve` or `reject` boolean field. This adds the guardian's approval or rejection to the unlock request.

**Payload Examples:**

_Lead Guardian:_
```json
{
  "message": "Unlock request message"
}
```

_Other Guardian (Approval):_
```json
{
  "approve": true
}
```

_Other Guardian (Rejection):_
```json
{
  "reject": true
}
```

**Response Codes:**
- **200 OK:** Box updated successfully, returning the updated guardian box details.
- **400 Bad Request:** Invalid payload or missing required fields.
- **401 Unauthorized:** The user is not an authorized guardian.
- **404 Not Found:** Box not found.
- **500 Internal Server Error:** An error occurred processing the update.

## Running the Service

This service is designed for AWS Lambda. For local testing, configure an AWS Lambda runtime environment with Rust, Cargo, and the AWS Lambda Rust runtime.

## Additional Notes

- The service uses an in-memory store for demonstration purposes.
- All timestamps are in ISO8601 format.
- Ensure the `x-user-id` header is included in requests for proper authentication and authorization.

## Dependencies

- Rust
- Cargo
- AWS Lambda runtime