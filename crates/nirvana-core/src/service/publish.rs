use chrono::{Local, TimeZone};
use nirvana_entity::publish_entry::Entity as PublishEntry;
use nirvana_entity::{publish_entry, slot, ticket};
use sea_orm::ActiveValue::Set;
use sea_orm::sea_query::Expr;
use sea_orm::{DbErr, EntityTrait, QueryFilter};

use crate::domain::{PublishFailure, PublishResult};
use crate::errors::TrackingError;
use crate::integration::{self, IntegrationError};
use crate::service::Nirvana;

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

impl Nirvana {
    pub async fn publish(
        &self,
        from: i64,
        to: Option<i64>,
    ) -> Result<PublishResult, super::NirvanaError> {
        let connection_id = self
            .config
            .core
            .active_connection
            .ok_or(TrackingError::NoActiveConnection)?;

        let connection = self.get_connection(connection_id).await?;
        let settings = connection.kind.jira_settings();
        if !settings.publish {
            return Err(TrackingError::PublishDisabled(connection.model.name).into());
        }
        let token = self.get_token(connection_id).await?;
        let integ = integration::build_integration(&connection, &token);

        let slots = self.get_unpublished_slots(from, to).await?;
        if slots.is_empty() {
            return Ok(PublishResult {
                published_count: 0,
                failed: vec![],
                timestamp: chrono::Utc::now().timestamp(),
            });
        }

        let worklogs = if settings.squash {
            squash_slots_for_publish(&slots)
        } else {
            slots_for_publish(&slots)
        };
        let now = chrono::Utc::now().timestamp();
        let slot_ids = worklogs
            .iter()
            .flat_map(|w| w.source_slot_ids.iter().copied());
        self.mark_pending(connection_id, slot_ids).await?;

        let mut published_count = 0;
        let mut failed = Vec::new();

        for worklog in &worklogs {
            let result = integ
                .publish_slot(&worklog.ticket_key, worklog.started_at, worklog.seconds)
                .await;
            self.record_publish(connection_id, &worklog.source_slot_ids, &result, now)
                .await?;
            match result {
                Ok(()) => published_count += worklog.source_slot_ids.len(),
                Err(e) => failed.push(PublishFailure {
                    ticket_key: worklog.ticket_key.clone(),
                    error: e.to_string(),
                }),
            }
        }

        Ok(PublishResult {
            published_count,
            failed,
            timestamp: now,
        })
    }

    /// Inserts a pending entry per slot, keeping existing entries of earlier attempts.
    pub(super) async fn mark_pending(
        &self,
        connection_id: i64,
        slot_ids: impl IntoIterator<Item = i64>,
    ) -> Result<(), DbErr> {
        let entries = slot_ids
            .into_iter()
            .map(|slot_id| publish_entry::ActiveModel {
                slot_id: Set(slot_id),
                connection_id: Set(connection_id),
                ..Default::default()
            });
        PublishEntry::insert_many(entries)
            .on_conflict_do_nothing()
            .exec_without_returning(&self.db)
            .await?;
        Ok(())
    }

    pub(super) async fn record_publish(
        &self,
        connection_id: i64,
        slot_ids: &[i64],
        result: &Result<(), IntegrationError>,
        now: i64,
    ) -> Result<(), DbErr> {
        let (published_at, error) = match result {
            Ok(()) => (Some(now), None),
            Err(e) => (None, Some(e.to_string())),
        };
        PublishEntry::update_many()
            .col_expr(
                publish_entry::COLUMN.published_at,
                Expr::value(published_at),
            )
            .col_expr(publish_entry::COLUMN.error, Expr::value(error))
            .filter(publish_entry::COLUMN.connection_id.eq(connection_id))
            .filter(
                publish_entry::COLUMN
                    .slot_id
                    .is_in(slot_ids.iter().copied()),
            )
            .filter(publish_entry::COLUMN.published_at.is_null())
            .exec(&self.db)
            .await?;
        Ok(())
    }
}

fn squash_slots_for_publish(slots: &[(slot::Model, ticket::Model)]) -> Vec<PublishWorklog> {
    let mut slots = slots.iter().collect::<Vec<_>>();
    slots.sort_by_key(|(slot, _)| slot.started_at);
    let mut day_groups: Vec<PublishDayGroup> = Vec::new();

    for (slot, ticket) in slots {
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
            .find(|worklog| worklog.ticket_key == ticket.ticket_key)
        {
            Some(worklog) => {
                worklog.seconds += seconds;
                worklog.source_slot_ids.push(slot.id);
            }
            None => day_group.worklogs.push(PublishWorklog {
                ticket_key: ticket.ticket_key.clone(),
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

fn slots_for_publish(slots: &[(slot::Model, ticket::Model)]) -> Vec<PublishWorklog> {
    slots
        .iter()
        .filter_map(|(slot, ticket)| {
            let stopped_at = slot.stopped_at?;
            let seconds = stopped_at - slot.started_at;
            if seconds <= 0 {
                return None;
            }

            Some(PublishWorklog {
                ticket_key: ticket.ticket_key.clone(),
                started_at: slot.started_at,
                seconds,
                source_slot_ids: vec![slot.id],
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use sea_orm::prelude::Uuid;

    use super::*;

    fn slot(
        id: i64,
        ticket_key: &str,
        started_at: i64,
        stopped_at: i64,
    ) -> (slot::Model, ticket::Model) {
        (
            slot::Model {
                id,
                ticket_id: 1,
                profile_id: 1,
                started_at,
                stopped_at: Some(stopped_at),
                uuid: Uuid::nil(),
            },
            ticket::Model {
                id: 1,
                profile_id: 1,
                connection_id: Some(1),
                ticket_key: ticket_key.to_string(),
                title: None,
                last_worked_at: 0,
                synced_at: 0,
            },
        )
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
