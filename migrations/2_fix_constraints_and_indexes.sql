-- Consents PK must be (uid, service_id) so a user can register with multiple services
ALTER TABLE Consents DROP CONSTRAINT consents_pkey;
ALTER TABLE Consents ADD PRIMARY KEY (uid, service_id);

-- Prevent duplicate service rows from concurrent inserts
ALTER TABLE Services ADD CONSTRAINT services_name_type_unique UNIQUE (name, type);

-- Unique constraint closes the register() race window; also acts as the index
-- for WHERE service_id=$1 AND external_id=$2 lookups
ALTER TABLE User_Service_Mappings
    ADD CONSTRAINT usm_service_external_unique UNIQUE (service_id, external_id);

-- Speed up the WHERE external_id=$1 path
CREATE INDEX ON User_Service_Mappings (external_id);
