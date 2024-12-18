App Protocol Description

The application communicates using the ZeroMQ Pub/Sub model, where messages are published to specific topics and subscribed by the application components. If any field provided for existed user_id it would be updated. Below is the detailed protocol specification for supported actions.
1. CREATE User

Used to create a new user in the system.

Message Structure:

```
{
  "action": "create",
  "user_id": "3747aefe-add3-4bad-badf-621e6585f3d0",
  "trial": false,
  "limit": 6000,
  "password": "RANDOM_PASSWORD"
}
```

Field Descriptions:

    action: (String, mandatory) - Specifies the action type. Must be "create".
    user_id: (String, mandatory) - A valid UUID representing the user.
    trial: (Boolean, optional, default: true) - Indicates whether the user is a trial user.
    limit: (i64, optional, default: 100) - Data usage limit for the user in megabytes (MB).
    password: (String, optional) - password for Shadowsocks

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

