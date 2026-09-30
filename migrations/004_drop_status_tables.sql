-- Status system moved to standalone zomboid-status service.
-- These tables are no longer used by the main backend.

DROP TABLE IF EXISTS status_incident_components;
DROP TABLE IF EXISTS status_incident_updates;
DROP TABLE IF EXISTS status_incidents;
DROP TABLE IF EXISTS status_components;
