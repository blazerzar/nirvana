use crate::api::NirvanaApi;
use crate::api::domain::{PublishFailure, PublishResult};
use crate::api::errors::TrackingError;
use crate::credentials;
use crate::integration;
use crate::storage::slot_repo::{self, SlotWithTicket};
use chrono::{Local, TimeZone};

#[derive(Debug, PartialEq, Eq)]
struct PublishWorklog {
    ticket_key: String,
    started_at: i64,
    seconds: i64,
    source_slot_ids: Vec<i64>,
}

#[derive(Debug, PartialEq, Eq)]
struct PublishDayGroup {
    day: chrono::NaiveDate,
    first_started_at: i64,
    worklogs: Vec<PublishWorklog>,
}

impl NirvanaApi {
    pub fn publish(
        &self,
        from: i64,
        to: Option<i64>,
    ) -> Result<PublishResult, super::NirvanaError> {
        let connection_id = self
            .config
            .core
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

        let worklogs = if self.config.core.publish_squashed_worklogs {
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
    let mut slots = slots.iter().collect::<Vec<_>>();
    slots.sort_by_key(|slot| slot.started_at);
    let mut day_groups: Vec<PublishDayGroup> = Vec::new();

    for slot in slots {
        let Some(stopped_at) = slot.stopped_at else {
            continue;
        };

        let seconds = stopped_at - slot.started_at;
        if seconds <= 0 {
            continue;
        }

        let Some(day) = local_day(slot.started_at) else {
            continue;
        };
        let day_group = match day_groups.iter_mut().find(|group| group.day == day) {
            Some(group) => group,
            None => {
                day_groups.push(PublishDayGroup {
                    day,
                    first_started_at: slot.started_at,
                    worklogs: Vec::new(),
                });
                day_groups.last_mut().expect("day group just inserted")
            }
        };

        match day_group
            .worklogs
            .iter_mut()
            .find(|worklog| worklog.ticket_key == slot.ticket_key)
        {
            Some(worklog) => {
                worklog.seconds += seconds;
                worklog.source_slot_ids.push(slot.id);
            }
            None => day_group.worklogs.push(PublishWorklog {
                ticket_key: slot.ticket_key.clone(),
                started_at: 0,
                seconds,
                source_slot_ids: vec![slot.id],
            }),
        }
    }

    day_groups
        .into_iter()
        .flat_map(|mut group| {
            let mut cursor = group.first_started_at;
            for worklog in &mut group.worklogs {
                worklog.started_at = cursor;
                cursor += worklog.seconds;
            }
            group.worklogs
        })
        .collect()
}

fn local_day(timestamp: i64) -> Option<chrono::NaiveDate> {
    match Local.timestamp_opt(timestamp, 0) {
        chrono::LocalResult::Single(date_time) => Some(date_time.date_naive()),
        chrono::LocalResult::Ambiguous(earliest, _) => Some(earliest.date_naive()),
        chrono::LocalResult::None => None,
    }
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

    fn local_ts(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> i64 {
        Local
            .with_ymd_and_hms(year, month, day, hour, minute, 0)
            .single()
            .expect("test timestamp should be valid in local timezone")
            .timestamp()
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
    fn orders_worklogs_by_slot_start_even_if_input_is_not_sorted() {
        let slots = vec![slot(1, "DES-1", 200, 260), slot(2, "DES-2", 100, 130)];

        let worklogs = squash_slots_for_publish(&slots);

        assert_eq!(worklogs[0].ticket_key, "DES-2");
        assert_eq!(worklogs[0].started_at, 100);
        assert_eq!(worklogs[1].ticket_key, "DES-1");
        assert_eq!(worklogs[1].started_at, 130);
    }

    #[test]
    fn squashes_same_ticket_within_each_local_day_only() {
        let monday_9 = local_ts(2026, 5, 18, 9, 0);
        let monday_10 = local_ts(2026, 5, 18, 10, 0);
        let tuesday_9 = local_ts(2026, 5, 19, 9, 0);
        let slots = vec![
            slot(1, "DES-1", monday_9, monday_9 + 30 * 60),
            slot(2, "DES-1", monday_10, monday_10 + 15 * 60),
            slot(3, "DES-1", tuesday_9, tuesday_9 + 45 * 60),
        ];

        let worklogs = squash_slots_for_publish(&slots);

        assert_eq!(
            worklogs,
            vec![
                PublishWorklog {
                    ticket_key: "DES-1".to_string(),
                    started_at: monday_9,
                    seconds: 45 * 60,
                    source_slot_ids: vec![1, 2],
                },
                PublishWorklog {
                    ticket_key: "DES-1".to_string(),
                    started_at: tuesday_9,
                    seconds: 45 * 60,
                    source_slot_ids: vec![3],
                },
            ]
        );
    }

    #[test]
    fn sequences_squashed_worklogs_independently_per_local_day() {
        let monday_9 = local_ts(2026, 5, 18, 9, 0);
        let monday_10 = local_ts(2026, 5, 18, 10, 0);
        let tuesday_9 = local_ts(2026, 5, 19, 9, 0);
        let tuesday_10 = local_ts(2026, 5, 19, 10, 0);
        let slots = vec![
            slot(1, "DES-1", monday_9, monday_9 + 30 * 60),
            slot(2, "DES-2", monday_10, monday_10 + 15 * 60),
            slot(3, "DES-1", monday_10 + 15 * 60, monday_10 + 30 * 60),
            slot(4, "DES-2", tuesday_9, tuesday_9 + 20 * 60),
            slot(5, "DES-1", tuesday_10, tuesday_10 + 40 * 60),
            slot(6, "DES-2", tuesday_10 + 40 * 60, tuesday_10 + 50 * 60),
        ];

        let worklogs = squash_slots_for_publish(&slots);

        assert_eq!(worklogs[0].ticket_key, "DES-1");
        assert_eq!(worklogs[0].started_at, monday_9);
        assert_eq!(worklogs[0].seconds, 45 * 60);
        assert_eq!(worklogs[1].ticket_key, "DES-2");
        assert_eq!(worklogs[1].started_at, monday_9 + 45 * 60);
        assert_eq!(worklogs[1].seconds, 15 * 60);
        assert_eq!(worklogs[2].ticket_key, "DES-2");
        assert_eq!(worklogs[2].started_at, tuesday_9);
        assert_eq!(worklogs[2].seconds, 30 * 60);
        assert_eq!(worklogs[3].ticket_key, "DES-1");
        assert_eq!(worklogs[3].started_at, tuesday_9 + 30 * 60);
        assert_eq!(worklogs[3].seconds, 40 * 60);
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
