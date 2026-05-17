#[derive(Debug)]
pub struct Connection {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub base_url: String,
    pub identity: String,
    pub secret_store: String,
    pub created_at: i64,
    pub updated_at: i64,
}
