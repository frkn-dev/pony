App Protocol Description

The application communicates using the ZeroMQ Pub/Sub model, where messages are published to specific topics and subscribed by the application components. Below is the detailed protocol specification for supported actions.
1. CREATE User

Used to create a new user in the system.

Message Structure:

```
{
  "action": "create",
  "user_id": "3747aefe-add3-4bad-badf-621e6585f3d0",
  "trial": false,
  "limit": 6000
}
```

Field Descriptions:

    action: (String, mandatory) - Specifies the action type. Must be "create".
    user_id: (String, mandatory) - A valid UUID representing the user.
    trial: (Boolean, optional, default: true) - Indicates whether the user is a trial user.
    limit: (i64, optional, default: 100) - Data usage limit for the user in megabytes (MB).

2. DELETE User

Used to remove an existing user from the system.

Message Structure:

```
{
  "action": "delete",
  "user_id": "dc79e5c9-4b10-48b3-b7b8-534821ce48c7"
}
```

Field Descriptions:

    action: (String, mandatory) - Specifies the action type. Must be "delete".
    user_id: (String, mandatory) - A valid UUID representing the user.

3. UPDATE User

Used to modify user parameters, such as trial status or data usage limit. Multiple parameters can be updated in a single request. If a user is EXPIRED, the RESTORE action must be called to reactivate them.

Message Structure:

```
{
  "action": "update",
  "user_id": "dc79e5c9-4b10-48b3-b7b8-534821ce48c7",
  "limit": 2000
}

{
  "action": "update",
  "user_id": "dc79e5c9-4b10-48b3-b7b8-534821ce48c7",
  "limit": 2000,
  "trial": false
}

{
  "action": "update",
  "user_id": "dc79e5c9-4b10-48b3-b7b8-534821ce48c7",
  "trial": false
}

```

Field Descriptions:

    action: (String, mandatory) - Specifies the action type. Must be "update".
    user_id: (String, mandatory) - A valid UUID representing the user.
    trial: (Boolean, optional) - New trial status for the user.
    limit: (i64, optional) - Updated data usage limit in megabytes (MB).

4. RESTORE User

Used to reactivate an EXPIRED user.

Message Structure:

```
{
  "action": "restore",
  "user_id": "dc79e5c9-4b10-48b3-b7b8-534821ce48c7"
}
```

Field Descriptions:

    action: (String, mandatory) - Specifies the action type. Must be "restore".
    user_id: (String, mandatory) - A valid UUID representing the user.

General Notes

    UUID Validation: Ensure all user_id values conform to the UUID format.
    Default Values:
        trial defaults to true if not specified.
        limit defaults to 100 MB if not specified.
    Error Handling: Any invalid action or malformed message should result in appropriate logging and error responses.
    Sequence for EXPIRED Users: If a user is marked as EXPIRED, an UPDATE action will not work unless a RESTORE action is successfully performed first.
