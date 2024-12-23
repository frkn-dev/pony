CREATE TABLE users (
    id UUID PRIMARY KEY,               
    password TEXT NOT NULL,       
    created_at TIMESTAMP DEFAULT NOW(), 
    trial BOOLEAN DEFAULT false,       
    cluster VARCHAR(255)               
);

INSERT INTO users (
    id, password, created_at, trial, cluster
) VALUES (
    'dc79e5c9-4b10-48b3-b7b8-534821ce48c7', 
    'password', 
    NOW(), 
    false,
    'mk3'
);
