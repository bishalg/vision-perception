-- owner: WP-L.0
-- Locale per user and per organisation (docs/spec/modules/asd-localisation/technical.md
-- I18N-10; ADR-0013 owner question 11). Typed, CHECK-constrained columns instead of the
-- JSONB `organizations.settings.locale`: the negotiation (I18N-12) reads them without JSON,
-- and `settings` keeps the free-form keys. WP-L.1e adds the reads and writes.
--
-- 0027 stays reserved for WP-B.21 (identity, blocked on ADR-0011 Q3). This file may be
-- applied while 0027 does not exist; if B.21 lands after it, B.21 takes 0029.

-- `users.locale`: the user's override; NULL = follow the organisation.
ALTER TABLE users ADD COLUMN locale text NULL CHECK (locale IN ('en', 'ne'));

-- `organizations.default_locale`: the organisation's default, `en` unless set.
ALTER TABLE organizations
    ADD COLUMN default_locale text NOT NULL DEFAULT 'en' CHECK (default_locale IN ('en', 'ne'));

-- Carry an existing `settings.locale` (none expected) into the column, then drop the key.
UPDATE organizations SET default_locale = settings->>'locale'
 WHERE settings->>'locale' IN ('en', 'ne');
UPDATE organizations SET settings = settings - 'locale' WHERE settings ? 'locale';
