use crate::api::NirvanaApi;
use crate::api::domain::{PublishFailure, PublishResult};
use crate::api::errors::TrackingError;
use crate::credentials;
use crate::integration;
use crate::storage::slot_repo::{self, SlotWithTicket};

#[derive(Debug, PartialEq, Eq)]
struct PublishWorklog {
    ticket_key: String,
    started_at: i64,
    seconds: i64,
    source_slot_ids: Vec<i64>,
}

impl NirvanaApi {
    pub fn publish(
        &self,
        from: i64,
        to: Option<i64>,
    ) -> Result<PublishResult, super::NirvanaError> {
        let connection_id = self
            .config
            .active_connection
            .ok_or(TrackingError::NoActiveConnection)?;

        let connection = self.get_connection(connection_id)?;
        let token = credentials::get_token(&self.db, connection_id)?;
        let integ = integration::build_integration(&connection, &token)?;

        let slots = slot_repo::get_unpublished(&self.db, connection_id, from, to)?;
        if slots.is_empty() {
            return Ok(PublishResult {
                published_count: 0,
                failed: vec![],
                timestamp: chrono::Utc::now().timestamp(),
            });
        }

        let worklogs = if self.config.publish_squashed_worklogs {
            squash_slots_for_publish(&slots)
        } else {
            slots_for_publish(&slots)
        };
        let now = chrono::Utc::now().timestamp();
        let mut published_ids = Vec::new();
        let mut failed = Vec::new();

        for worklog in &worklogs {
            match integ.publish_slot(&worklog.ticket_key, worklog.started_at, worklog.seconds) {
                Ok(()) => published_ids.extend(worklog.source_slot_ids.iter().copied()),
                Err(e) => failed.push(PublishFailure {
                    ticket_key: worklog.ticket_key.clone(),
                    error: e.to_string(),
                }),
            }
        }

        if !published_ids.is_empty() {
            slot_repo::mark_published(&self.db, &published_ids, now)?;
        }

        Ok(PublishResult {
            published_count: published_ids.len(),
            failed,
            timestamp: now,
        })
    }
}

fn squash_slots_for_publish(slots: &[SlotWithTicket]) -> Vec<PublishWorklog> {
    let Some(first_started_at) = slots.iter().map(|slot| slot.started_at).min() else {
        return Vec::new();
    };

    let mut groups: Vec<PublishWorklog> = Vec::new();

    for slot in slots {
        let Some(stopped_at) = slot.stopped_at else {
            continue;
        };

        let seconds = stopped_at - slot.started_at;
        if seconds <= 0 {
            continue;
        }

        match groups
            .iter_mut()
            .find(|worklog| worklog.ticket_key == slot.ticket_key)
        {
            Some(worklog) => {
                worklog.seconds += seconds;
                worklog.source_slot_ids.push(slot.id);
            }
            None => groups.push(PublishWorklog {
                ticket_key: slot.ticket_key.clone(),
                started_at: 0,
                seconds,
                source_slot_ids: vec![slot.id],
            }),
        }
    }

    let mut cursor = first_started_at;
    for worklog in &mut groups {
        worklog.started_at = cursor;
        cursor += worklog.seconds;
    }

    groups
}

fn slots_for_publish(slots: &[SlotWithTicket]) -> Vec<PublishWorklog> {
    slots
        .iter()
        .filter_map(|slot| {
            let stopped_at = slot.stopped_at?;
            let seconds = stopped_at - slot.started_at;
            if seconds <= 0 {
                return None;
            }

            Some(PublishWorklog {
                ticket_key: slot.ticket_key.clone(),
                started_at: slot.started_at,
                seconds,
                source_slot_ids: vec![slot.id],
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(id: i64, ticket_key: &str, started_at: i64, stopped_at: i64) -> SlotWithTicket {
        SlotWithTicket {
            id,
            ticket_key: ticket_key.to_string(),
            summary: None,
            connection_id: 1,
            note: None,
            started_at,
            stopped_at: Some(stopped_at),
            published_at: None,
        }
    }

    #[test]
    fn squashes_non_adjacent_slots_for_same_ticket() {
        let slots = vec![
            slot(1, "DES-1", 100, 160),
            slot(2, "DES-2", 160, 220),
            slot(3, "DES-1", 220, 280),
        ];

        let worklogs = squash_slots_for_publish(&slots);

        assert_eq!(
            worklogs,
            vec![
                PublishWorklog {
                    ticket_key: "DES-1".to_string(),
                    started_at: 100,
                    seconds: 120,
                    source_slot_ids: vec![1, 3],
                },
                PublishWorklog {
                    ticket_key: "DES-2".to_string(),
                    started_at: 220,
                    seconds: 60,
                    source_slot_ids: vec![2],
                },
            ]
        );
    }

    #[test]
    fn preserves_ticket_order_by_first_seen_slot() {
        let slots = vec![
            slot(1, "DES-2", 100, 130),
            slot(2, "DES-1", 130, 170),
            slot(3, "DES-2", 170, 200),
        ];

        let worklogs = squash_slots_for_publish(&slots);

        assert_eq!(worklogs[0].ticket_key, "DES-2");
        assert_eq!(worklogs[1].ticket_key, "DES-1");
        assert_eq!(worklogs[0].source_slot_ids, vec![1, 3]);
        assert_eq!(worklogs[1].source_slot_ids, vec![2]);
    }

    #[test]
    fn places_merged_worklogs_sequentially_from_earliest_start() {
        let slots = vec![
            slot(1, "DES-1", 500, 560),
            slot(2, "DES-2", 560, 590),
            slot(3, "DES-3", 590, 680),
        ];

        let worklogs = squash_slots_for_publish(&slots);

        assert_eq!(worklogs[0].started_at, 500);
        assert_eq!(worklogs[1].started_at, 560);
        assert_eq!(worklogs[2].started_at, 590);
        assert_eq!(
            worklogs[0].started_at + worklogs[0].seconds,
            worklogs[1].started_at
        );
        assert_eq!(
            worklogs[1].started_at + worklogs[1].seconds,
            worklogs[2].started_at
        );
    }

    #[test]
    fn anchors_to_earliest_start_even_if_input_is_not_sorted() {
        let slots = vec![slot(1, "DES-1", 200, 260), slot(2, "DES-2", 100, 130)];

        let worklogs = squash_slots_for_publish(&slots);

        assert_eq!(worklogs[0].ticket_key, "DES-1");
        assert_eq!(worklogs[0].started_at, 100);
        assert_eq!(worklogs[1].started_at, 160);
    }

    #[test]
    fn keeps_individual_slots_when_squashing_is_disabled() {
        let slots = vec![
            slot(1, "DES-1", 100, 160),
            slot(2, "DES-2", 160, 220),
            slot(3, "DES-1", 220, 280),
        ];

        let worklogs = slots_for_publish(&slots);

        assert_eq!(
            worklogs,
            vec![
                PublishWorklog {
                    ticket_key: "DES-1".to_string(),
                    started_at: 100,
                    seconds: 60,
                    source_slot_ids: vec![1],
                },
                PublishWorklog {
                    ticket_key: "DES-2".to_string(),
                    started_at: 160,
                    seconds: 60,
                    source_slot_ids: vec![2],
                },
                PublishWorklog {
                    ticket_key: "DES-1".to_string(),
                    started_at: 220,
                    seconds: 60,
                    source_slot_ids: vec![3],
                },
            ]
        );
    }
}
