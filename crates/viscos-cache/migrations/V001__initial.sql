-- V001__initial.sql — viscos-cache initial schema (ADR-0010 §3.1)
--
-- Faz 4.0 Dalga 1: Discord metadata (guild, channel, message, member) + attachment tracking.
-- İndeksler: messages(channel_id, timestamp DESC) sıcak scroll pattern'i için.
-- attachments(expires_at) CDN refresh worker'ın 23h filtresi için (Dalga 2).
--
-- Tüm ID'ler TEXT (Discord snowflake stringified u64) — referans bütünlüğü
-- ileride FK constraint ile sıkılaştırılabilir (Faz 4.0 sonu).

CREATE TABLE messages (
    id TEXT PRIMARY KEY,                  -- Discord snowflake (u64 → string)
    channel_id INTEGER NOT NULL,          -- Discord channel snowflake (FK sonra)
    author_id INTEGER,                    -- author user snowflake
    content TEXT,                         -- raw markdown + mention
    timestamp INTEGER NOT NULL,           -- unix epoch seconds
    edited_timestamp INTEGER,             -- unix epoch seconds (NULL = not edited)
    attachments JSON,                     -- [{id, filename, content_type, size, url}]
    raw JSON NOT NULL                     -- full Discord payload (Faz 2 sonrası)
);

CREATE INDEX idx_messages_channel_timestamp
    ON messages(channel_id, timestamp DESC);

CREATE TABLE guilds (
    id INTEGER PRIMARY KEY,               -- Discord guild snowflake
    name TEXT,
    icon_hash TEXT,                       -- avatar/icon cache key (signed URL değil)
    owner_id INTEGER,
    raw JSON NOT NULL
);

CREATE TABLE channels (
    id INTEGER PRIMARY KEY,               -- Discord channel snowflake
    guild_id INTEGER,                     -- DM için NULL
    name TEXT,
    kind INTEGER,                         -- 0=text, 2=voice, 4=category, ...
    parent_id INTEGER,                    -- category FK
    raw JSON NOT NULL
);

CREATE INDEX idx_channels_guild ON channels(guild_id);
CREATE INDEX idx_channels_parent ON channels(parent_id);

CREATE TABLE members (
    user_id INTEGER NOT NULL,
    guild_id INTEGER NOT NULL,
    nick TEXT,
    joined_at INTEGER,
    raw JSON NOT NULL,
    PRIMARY KEY (user_id, guild_id)
);

CREATE TABLE read_state (
    channel_id INTEGER PRIMARY KEY,
    last_read_message_id INTEGER,
    mention_count INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE attachments (
    id INTEGER PRIMARY KEY,               -- Discord attachment snowflake (signed URL'den BAĞIMSIZ cache key)
    message_id INTEGER,
    filename TEXT,
    content_type TEXT,
    size INTEGER,
    cdn_url TEXT,                         -- şu an geçerli signed URL (refresh worker günceller)
    expires_at INTEGER,                   -- unix epoch seconds (signed URL expire)
    encrypted_path TEXT                   -- foyer hybrid cache blob yolu (encrypted)
);

CREATE INDEX idx_attachments_expires
    ON attachments(expires_at)
    WHERE expires_at IS NOT NULL;         -- partial index: sadece signed URL'li attachment'lar

CREATE INDEX idx_attachments_message ON attachments(message_id);

-- refinery_schema_history tablosu refinery tarafından otomatik oluşturulur
-- (ilk migrate() çağrısında). Bu dosyaya explicit yazmıyoruz.

-- Down migration örneği (refinery destekliyor, V002+ reversibility testleri için):
-- DROP TABLE IF EXISTS attachments;
-- DROP TABLE IF EXISTS read_state;
-- DROP TABLE IF EXISTS members;
-- DROP TABLE IF EXISTS channels;
-- DROP TABLE IF EXISTS guilds;
-- DROP TABLE IF EXISTS messages;