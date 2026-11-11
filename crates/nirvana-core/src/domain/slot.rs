use nirvana_entity::{slot, ticket};

pub enum SlotSort {
    StartedAt,
    TicketId,
}

#[derive(Debug)]
pub struct Slot {
    pub model: slot::Model,
    pub ticket: ticket::Model,
}

#[derive(Debug)]
pub struct StartResult {
    pub started: Slot,
    pub stopped: Option<Slot>,
}
