use serde::{Deserialize, Serialize};

use super::query::Queries;
use super::ChContext;

#[derive(Serialize, Deserialize)]
pub struct NodeScore {
    pub score: f64,
    pub details: NodeScoreBreakdown,
}

#[derive(Serialize, Deserialize)]
pub struct NodeScoreBreakdown {
    pub cpu_usage: f64,
    pub loadavg: f64,
    pub memory_ratio: f64,
    pub bandwidth: f64,
}

impl ChContext {
    pub async fn fetch_node_score(
        &self,
        env: &str,
        hostname: &str,
        interface: &str,
        num_cores: usize,
        max_bandwidth_bps: i64,
    ) -> Option<NodeScore> {
        let cpu_usage = match self.fetch_node_cpuusage_max(env, hostname).await {
            Some(m) => m.value,
            None => {
                log::error!("Failed to fetch CPU usage for {}.{}", env, hostname);
                return None;
            }
        };

        let loadavg = match self.fetch_node_cpu_load_avg5(env, hostname).await {
            Some(m) => m.value,
            None => {
                log::error!("Failed to fetch loadavg for {}.{}", env, hostname);
                return None;
            }
        };

        let memory_ratio = match self.fetch_node_memory_usage_ratio(env, hostname).await {
            Some(m) => m.value,
            None => {
                log::error!("Failed to fetch memory ratio for {}.{}", env, hostname);
                return None;
            }
        };

        let tx_bps = match self.fetch_node_bandwidth(env, hostname, interface).await {
            Some(m) => m.value,
            None => {
                log::error!("Failed to fetch tx_bps for {}.{}", env, hostname);
                return None;
            }
        };

        let cpu_score = (cpu_usage / 100.0).clamp(0.0, 1.0);

        let load_score = if num_cores > 0 {
            (loadavg / num_cores as f64).clamp(0.0, 1.0)
        } else {
            1.0
        };

        let memory_score = memory_ratio.clamp(0.0, 1.0);

        let upload_score = (tx_bps / max_bandwidth_bps as f64).clamp(0.0, 1.0);

        let score =
            0.35 * cpu_score + 0.25 * load_score + 0.25 * memory_score + 0.15 * upload_score;

        Some(NodeScore {
            score,
            details: NodeScoreBreakdown {
                cpu_usage,
                loadavg,
                memory_ratio,
                bandwidth: tx_bps,
            },
        })
    }
}
