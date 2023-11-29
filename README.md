# thoughtkeeper

This is intended to be a mostly batteries-included blogging software for low-effort content. It's not feature-complete yet.

## Setup

You will need `sqlx` (`cargo install sqlx-cli`) and `sqlite`. Set up the database via

```
sqlx db create --database-url "sqlite://articles.db"
sqlx migrate run
```