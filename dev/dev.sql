
CREATE TYPE node_status AS ENUM ('online', 'offline');

CREATE TABLE users (
    id UUID PRIMARY KEY,  
    username TEXT,
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
    is_trial bool NOT NULL     
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




