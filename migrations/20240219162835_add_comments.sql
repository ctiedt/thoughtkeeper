CREATE TABLE IF NOT EXISTS comments
(
    id              TEXT PRIMARY KEY NOT NULL,
    article         TEXT NOT NULL,
    author          TEXT NOT NULL,
    content         TEXT NOT NULL,
    published       DATETIME NOT NULL,
    FOREIGN KEY(article) REFERENCES articles(id)
);