-- 011_strat_management.sql
-- Strategy Registry and Configuration Audit Trail

-- 1. Strategy Status Enum
DO $$ 
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'strat_status') THEN
        CREATE TYPE strat_status AS ENUM ('RUNNING', 'PAUSED', 'HALTED', 'ERROR');
    END IF;
END $$;

-- 2. Strategy Mode Enum
DO $$ 
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'strat_mode') THEN
        CREATE TYPE strat_mode AS ENUM ('LIVE', 'PAPER', 'SHADOW');
    END IF;
END $$;

-- 3. Strategy Registry
CREATE TABLE IF NOT EXISTS strat_definitions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    strategy_name VARCHAR(255) NOT NULL UNIQUE,
    version VARCHAR(50) NOT NULL,
    status strat_status NOT NULL DEFAULT 'HALTED',
    mode strat_mode NOT NULL DEFAULT 'PAPER',
    config_json JSONB NOT NULL DEFAULT '{}',
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 4. Strategy Instances (Workers)
CREATE TABLE IF NOT EXISTS strat_instances (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    strategy_id UUID NOT NULL REFERENCES strat_definitions(id),
    instance_id VARCHAR(255) NOT NULL UNIQUE, -- e.g. hostname or pod name
    last_heartbeat TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    metadata JSONB DEFAULT '{}'
);

-- 5. Strategy Config Audit Trail
CREATE TABLE IF NOT EXISTS strat_config_audit (
    id SERIAL PRIMARY KEY,
    strategy_id UUID NOT NULL REFERENCES strat_definitions(id),
    changed_by VARCHAR(255) NOT NULL,
    change_reason TEXT,
    old_config JSONB NOT NULL,
    new_config JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 6. Indices
CREATE INDEX idx_strat_instances_last_heartbeat ON strat_instances(last_heartbeat);
CREATE INDEX idx_strat_config_audit_strategy_id ON strat_config_audit(strategy_id);
CREATE INDEX idx_strat_config_audit_created_at ON strat_config_audit(created_at);

-- 7. Trigger to update updated_at
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE TRIGGER update_strat_definitions_updated_at
    BEFORE UPDATE ON strat_definitions
    FOR EACH ROW
    EXECUTE PROCEDURE update_updated_at_column();
