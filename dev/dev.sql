
CREATE TYPE node_status AS ENUM ('online', 'offline');
CREATE TYPE conn_status AS ENUM ('active', 'expired');
CREATE TYPE proto AS ENUM ('vless_grpc', 'vless_xtls', 'vmess', 'shadowsocks');

CREATE TABLE users (
    id UUID PRIMARY KEY,  
    username TEXT UNIQUE,
    created_at TIMESTAMP DEFAULT NOW(),
    modified_at TIMESTAMP DEFAULT NOW()
);

CREATE TABLE connections (
    id UUID PRIMARY KEY,
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    env TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT NOW(),
    modified_at TIMESTAMP DEFAULT NOW(),
    daily_limit_mb INTEGER DEFAULT 1000,   
    password TEXT NOT NULL,
    is_trial bool NOT NULL,
    online BIGINT NOT NULL DEFAULT 0,
    uplink BIGINT NOT NULL DEFAULT 0,
    downlink BIGINT NOT NULL DEFAULT 0,   
    status conn_status NOT NULL, 
    proto proto NOT NULL  
);


CREATE TABLE nodes (
    id UUID PRIMARY KEY,
    env TEXT NOT NULL,
    hostname TEXT NOT NULL,
    address INET NOT NULL,
    status node_status NOT NULL,
    uuid UUID NOT NULL,
    inbounds JSONB NOT NULL, 
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    modified_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    label TEXT NOT NULL,
    interface TEXT NOT NULL,
    UNIQUE(uuid, env)
);




ALTER TABLE users
    ADD COLUMN telegram_id BIGINT;

ALTER TABLE users
    ADD COLUMN env TEXT NOT NULL DEFAULT 'dev';

ALTER TABLE users
    ADD COLUMN daily_limit_mb INTEGER NOT NULL DEFAULT 1024;

ALTER TABLE users
    ADD COLUMN password TEXT;

ALTER TABLE users
    ADD COLUMN is_deleted BOOL NOT NULL DEFAULT false;

ALTER TABLE connections
    ADD COLUMN is_deleted BOOL NOT NULL DEFAULT false;


CREATE TABLE inbounds (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    node_id UUID NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    tag PROTO NOT NULL,
    port INTEGER NOT NULL,
    stream_settings JSONB,
    uplink BIGINT,
    downlink BIGINT,
    conn_count BIGINT,
    wg_pubkey TEXT,
    wg_privkey TEXT,
    wg_interface TEXT,
    wg_network TEXT, 
    wg_address TEXT
);


ALTER TYPE proto ADD VALUE 'wireguard';

ALTER TABLE connections
ADD COLUMN wg_privkey TEXT,
ADD COLUMN wg_pubkey TEXT
ADD COLUMN wg_address TEXT
ADD COLUMN node_id UUID

ALTER TABLE nodes DROP COLUMN inbounds;

ALTER TABLE connections
ALTER COLUMN password DROP NOT NULL;

ALTER TABLE inbounds ADD COLUMN dns INET[];

ALTER TABLE connections DROP CONSTRAINT connections_user_id_fkey;

ALTER TABLE inbounds ADD COLUMN created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW();
ALTER TABLE inbounds ADD COLUMN modified_at TIMESTAMP WITH TIME ZONE DEFAULT NOW();

CREATE UNIQUE INDEX inbounds_node_id_tag_key
ON inbounds (node_id, tag);

ALTER TABLE users
ALTER COLUMN daily_limit_mb DROP NOT NULL;


ALTER TABLE nodes ADD COLUMN cores INTEGER NOT NULL DEFAULT 1;
ALTER TABLE nodes ADD COLUMN max_bandwidth_bps BIGINT NOT NULL DEFAULT 100000000;

DROP TABLE users;


