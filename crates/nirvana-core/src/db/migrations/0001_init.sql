create table connections (
    id INTEGER primary key,
    name TEXT not null,
    kind TEXT not null,
    base_url TEXT not null,
    identity TEXT not null,
    secret_store TEXT not null default 'keyring',
    created_at INTEGER not null,
    updated_at INTEGER not null
);


create index connections_by_name on connections (name);


create table credentials (
    connection_id INTEGER not null references connections (id) on delete cascade,
    credential TEXT not null,
    primary key (connection_id)
);
