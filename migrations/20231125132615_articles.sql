CREATE TABLE IF NOT EXISTS articles
(
    id          TEXT PRIMARY KEY NOT NULL,
    title       TEXT NOT NULL,
    content     TEXT NOT NULL,
    published   DATETIME NOT NULL
);