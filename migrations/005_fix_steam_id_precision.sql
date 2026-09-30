-- Fix steam IDs corrupted by Kahlua2 double-precision loss.
-- Xaric81: 76561198368622850 (wrong, off by +2) -> 76561198368622848 (correct)
UPDATE character_snapshots
   SET steam_id = '76561198368622848'
 WHERE steam_id = '76561198368622850'
   AND zomboid_username = 'Xaric81';

UPDATE game_events
   SET steam_id = '76561198368622848'
 WHERE steam_id = '76561198368622850';

UPDATE online_days
   SET steam_id = '76561198368622848'
 WHERE steam_id = '76561198368622850';

-- Merge duplicate account: delete the wrong-ID entry, update correct one's display_name
DELETE FROM accounts WHERE steam_id = '76561198368622850';
UPDATE accounts SET display_name = 'Xaric81' WHERE steam_id = '76561198368622848';
