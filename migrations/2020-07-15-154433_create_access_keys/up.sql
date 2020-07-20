CREATE TYPE status_type AS ENUM ('PENDING', 'FAILED', 'SUCCESS');
CREATE TYPE action_type AS ENUM ('ADD', 'DELETE');
CREATE TYPE permission_type AS ENUM ('NOT_APPLICABLE', 'FULL_ACCESS', 'FUNCTION_CALL');

CREATE TABLE access_keys (
    public_key text NOT NULL,
    account_id text NOT NULL,
    "action" action_type NOT NULL,
    status status_type NOT NULL,
    receipt_hash text NOT NULL,
    block_height numeric(20) NOT NULL, -- numeric(precision) 20 digits should be enought to store u64::MAX
    "permission" permission_type NOT NULL,
    CONSTRAINT access_keys_pk PRIMARY KEY (public_key, account_id, "action", receipt_hash)
);
CREATE INDEX access_keys_public_key_idx ON access_keys (public_key);
CREATE INDEX access_keys_account_id_idx ON access_keys (account_id);
CREATE INDEX access_keys_block_height_idx ON access_keys (block_height);
CREATE INDEX access_keys_receipt_hash_idx ON access_keys (receipt_hash);
