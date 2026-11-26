use std::collections::HashMap;

use crate::gitlab::{BridgeJob, Job};

#[derive(Debug, Clone)]
pub struct StageJobCard {
    pub job: Job,
    pub retry_count: usize,
    pub is_bridge: bool,
}

#[derive(Debug, Clone)]
pub struct StageColumn {
    pub name: String,
    pub min_job_id: u64,
    pub cards: Vec<StageJobCard>,
}

pub fn build_stage_columns(jobs: &[Job], bridges: &[BridgeJob]) -> Vec<StageColumn> {
    let mut stage_min_id: HashMap<String, u64> = HashMap::new();
    let mut grouped: HashMap<(String, String), (Job, usize)> = HashMap::new();

    for job in jobs {
        stage_min_id
            .entry(job.stage.clone())
            .and_modify(|min| *min = (*min).min(job.id))
            .or_insert(job.id);

        grouped
            .entry((job.stage.clone(), job.name.clone()))
            .and_modify(|(latest, retries)| {
                *retries += 1;
                if job.id > latest.id {
                    *latest = job.clone();
                }
            })
            .or_insert_with(|| (job.clone(), 0));
    }

    let mut stage_cards: HashMap<String, Vec<StageJobCard>> = HashMap::new();
    for ((stage, _), (job, retry_count)) in grouped {
        stage_cards.entry(stage).or_default().push(StageJobCard {
            job,
            retry_count,
            is_bridge: false,
        });
    }

    for bridge in bridges {
        stage_min_id
            .entry(bridge.stage.clone())
            .and_modify(|min| *min = (*min).min(bridge.id))
            .or_insert(bridge.id);
        stage_cards
            .entry(bridge.stage.clone())
            .or_default()
            .push(StageJobCard {
                job: Job {
                    id: bridge.id,
                    name: bridge.name.clone(),
                    status: bridge.status.clone(),
                    stage: bridge.stage.clone(),
                    web_url: bridge.web_url.clone().unwrap_or_default(),
                    duration: None,
                    started_at: None,
                    finished_at: None,
                    allow_failure: bridge.allow_failure,
                },
                retry_count: 0,
                is_bridge: true,
            });
    }

    let mut cols: Vec<StageColumn> = stage_min_id
        .into_iter()
        .map(|(name, min_job_id)| {
            let mut cards = stage_cards.remove(&name).unwrap_or_default();
            cards.sort_by(|a, b| a.job.name.cmp(&b.job.name));
            StageColumn {
                name,
                min_job_id,
                cards,
            }
        })
        .collect();
    cols.sort_by_key(|c| c.min_job_id);
    cols
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_job(id: u64, stage: &str, name: &str, status: &str) -> Job {
        Job {
            id,
            name: name.into(),
            status: status.into(),
            stage: stage.into(),
            web_url: String::new(),
            duration: Some(12.0),
            started_at: None,
            finished_at: None,
            allow_failure: None,
        }
    }

    #[test]
    fn retried_early_stage_job_preserves_stage_order_and_retry_badge() {
        let jobs = vec![
            mk_job(10, "build", "compile", "failed"),
            mk_job(20, "test", "unit", "success"),
            mk_job(30, "deploy", "prod", "manual"),
            mk_job(99, "build", "compile", "success"),
        ];
        let cols = build_stage_columns(&jobs, &[]);
        let names: Vec<&str> = cols.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["build", "test", "deploy"]);
        assert_eq!(cols[0].cards.len(), 1);
        assert_eq!(cols[0].cards[0].job.id, 99);
        assert_eq!(cols[0].cards[0].retry_count, 1);
        assert_eq!(cols[0].cards[0].job.status, "success");
    }

    #[test]
    fn includes_downstream_bridge_jobs_in_stage_column() {
        let jobs = vec![mk_job(10, "build", "compile", "success")];
        let bridges = vec![BridgeJob {
            id: 50,
            name: "trigger-child".into(),
            status: "success".into(),
            stage: "downstream".into(),
            web_url: None,
            allow_failure: None,
        }];
        let cols = build_stage_columns(&jobs, &bridges);
        assert_eq!(cols.len(), 2);
        assert_eq!(cols[1].name, "downstream");
        assert!(cols[1].cards[0].is_bridge);
    }
}
