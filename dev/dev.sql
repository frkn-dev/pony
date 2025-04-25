CREATE TABLE users (
    id UUID PRIMARY KEY,               
    password TEXT NOT NULL,       
    created TIMESTAMP DEFAULT NOW(),
    trial BOOLEAN DEFAULT true,
    data_limit_mb NUMERIC DEFAULT 1000,       
    cluster VARCHAR(255)               
);

INSERT INTO users (
    id, password, created, trial, cluster
) VALUES (
    'dc79e5c9-4b10-48b3-b7b8-534821ce48c7', 
    'password', 
    NOW(), 
    false,
    'dev'
);



CREATE TABLE nodes (
    id SERIAL PRIMARY KEY,
    hostname TEXT NOT NULL,
    ipv4 INET NOT NULL,
    status TEXT NOT NULL,
    env TEXT NOT NULL,
    uuid UUID NOT NULL,
    inbounds JSONB NOT NULL, 
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    modified_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    label TEXT NOT NULL,
    iface TEXT NOT NULL,
    UNIQUE(uuid, env)
);


INSERT INTO nodes (hostname, ipv4, status, env, uuid, inbounds)
VALUES (
    'devmachine',
    '198.18.0.1',
    'Online',
    'dev',
    '9557b391-01cb-4031-a3f5-6cbdd749bcff',
    '{
        "VlessGrpc": {
            "tag": "VlessGrpc",
            "port": 2053,
            "streamSettings": {
                "tcpSettings": null,
                "realitySettings": {
                    "serverNames": ["cdn.discordapp.com", "discordapp.com"],
                    "privateKey": "4AQgu1qeCaGT8nnZTOnKLSOudSp_Z_kS0kzIE6bMcUM",
                    "shortIds": ["", "e5c4d84fb339fb92"],
                    "dest": "discordapp.com:443"
                }
            },
            "uplink": null,
            "downlink": null,
            "user_count": null
        },
        "Vmess": {
            "tag": "Vmess",
            "port": 8081,
            "streamSettings": {
                "tcpSettings": {
                    "header": {
                        "type": "http",
                        "request": {
                            "method": "GET",
                            "path": ["/"],
                            "headers": {
                                "Host": ["google.com"]
                            }
                        }
                    }
                },
                "realitySettings": null
            },
            "uplink": null,
            "downlink": null,
            "user_count": null
        }
    }'::jsonb
);
