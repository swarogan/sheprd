//! Which of this computer's relays is nearest, from the round trips its
//! listeners measure. A session's path is phone to relay to computer, so
//! the phone ranks relays by its own leg plus the one reported here.

use sheprd_relay::Endpoint;
use std::{collections::BTreeMap, time::Duration};

/// `endpoints` as `GET /api/relay` lists them: those with a measured round
/// trip first, fastest first, then the rest in their configured order.
pub(super) fn first(endpoints: &[Endpoint], rtt: &BTreeMap<String, Duration>) -> Vec<String> {
    let mut ranked: Vec<(Option<Duration>, String)> = endpoints
        .iter()
        .map(|endpoint| {
            let name = endpoint.to_string();
            (rtt.get(&name).copied(), name)
        })
        .collect();
    // Stable, so ties and unmeasured relays keep the user's order.
    ranked.sort_by_key(|(rtt, _)| (rtt.is_none(), *rtt));
    ranked.into_iter().map(|(_, name)| name).collect()
}

/// The round trips in whole milliseconds, at least one, as the phone reads
/// them.
pub(super) fn millis(rtt: &BTreeMap<String, Duration>) -> BTreeMap<String, u32> {
    rtt.iter()
        .map(|(name, rtt)| {
            let millis = u32::try_from(rtt.as_millis()).unwrap_or(u32::MAX).max(1);
            (name.clone(), millis)
        })
        .collect()
}
